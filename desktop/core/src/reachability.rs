//! A bounded DNS/TCP check, independent of certificates, credentials and graphics.
use crate::connection::authority_host_port;
use serde::Serialize;
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Server address and port reachability, which does not establish an RDP session.
#[derive(Serialize)]
pub struct ReachabilityResult {
    pub success: bool,
    pub kind: String,
    pub summary: String,
    pub details: String,
}

/// Resolve the server and try its TCP port within a shared five-second deadline.
pub fn test_server(authority: &str) -> Result<ReachabilityResult, String> {
    test_with_deadline(authority, Duration::from_secs(5))
}

fn result(success: bool, kind: &str, summary: &str, details: String) -> ReachabilityResult {
    ReachabilityResult {
        success,
        kind: kind.into(),
        summary: summary.into(),
        details,
    }
}

fn test_with_deadline(authority: &str, timeout: Duration) -> Result<ReachabilityResult, String> {
    let (host, port) = authority_host_port(authority)?;
    let deadline = Instant::now() + timeout;
    let addresses = if let Ok(ip) = host.parse::<IpAddr>() {
        vec![SocketAddr::new(ip, port)]
    } else {
        // The system resolver has no portable cancellation API. Its worker cannot hold up the GUI.
        let host = host.to_owned();
        let (send, receive) = mpsc::channel();
        std::thread::spawn(move || {
            let resolved = (host.as_str(), port)
                .to_socket_addrs()
                .map(|addresses| addresses.take(16).collect::<Vec<_>>())
                .map_err(|error| error.to_string());
            let _ = send.send(resolved);
        });
        match receive.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Ok(addresses)) => addresses,
            Ok(Err(error)) => {
                return Ok(result(
                    false,
                    "dns",
                    "Could not resolve your server hostname.",
                    error,
                ))
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Ok(result(
                    false,
                    "timeout",
                    "Server name lookup timed out.",
                    format!("No DNS result within {} seconds.", timeout.as_secs_f64()),
                ))
            }
            Err(error) => {
                return Ok(result(
                    false,
                    "dns",
                    "Server name lookup did not finish.",
                    error.to_string(),
                ))
            }
        }
    };
    if addresses.is_empty() {
        return Ok(result(
            false,
            "dns",
            "Your server hostname resolved to no addresses.",
            String::new(),
        ));
    }
    let mut details = Vec::new();
    for address in addresses {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match TcpStream::connect_timeout(&address, remaining.min(Duration::from_secs(1))) {
            Ok(_) => return Ok(result(true, "reachable", "Server TCP port is reachable. Certificate and sign-in checks come next.", format!("Connected to {address}. This checks TCP only, not the RDP protocol or authentication."))),
            Err(error) => details.push(format!("{address}: {error}")),
        }
    }
    let timed_out = Instant::now() >= deadline;
    Ok(result(
        false,
        if timed_out { "timeout" } else { "network" },
        if timed_out {
            "Server connection timed out. Check the address, network and RDP service."
        } else {
            "Could not connect to your server TCP port. Check the address and RDP service."
        },
        details.join("\n"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn checks_tcp_without_authentication_and_reports_refused_ports() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let reached = test_server(&address).unwrap();
        assert!(reached.success);
        let named = test_server(&format!(
            "localhost:{}",
            listener.local_addr().unwrap().port()
        ))
        .unwrap();
        assert!(named.success);
        assert_eq!(reached.kind, "reachable");
        assert!(reached.details.contains("TCP only"));
        drop(listener);
        let refused = test_server(&address).unwrap();
        assert!(!refused.success);
        assert_eq!(refused.kind, "network");
        assert!(refused.details.contains(&address));
    }

    #[test]
    fn handles_ipv6_default_ports_and_a_shared_deadline() {
        assert_eq!(
            authority_host_port("example.invalid").unwrap(),
            ("example.invalid", 3389)
        );
        assert_eq!(authority_host_port("[::1]:3390").unwrap(), ("::1", 3390));
        assert_eq!(authority_host_port("::1").unwrap(), ("::1", 3389));
        assert!(test_server("../profile").is_err());
        assert!(test_server("host:0").is_err());
        let started = Instant::now();
        let expired = test_with_deadline("127.0.0.1:3389", Duration::ZERO).unwrap();
        assert_eq!(expired.kind, "timeout");
        assert!(started.elapsed() < Duration::from_millis(100));
        if let Ok(listener) = TcpListener::bind("[::1]:0") {
            assert!(
                test_server(&listener.local_addr().unwrap().to_string())
                    .unwrap()
                    .success
            );
        }
    }
}
