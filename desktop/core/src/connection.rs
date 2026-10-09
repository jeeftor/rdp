use crate::{normalize_fingerprint, Profile};
use serde::Serialize;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub(crate) fn validate_password(password: &str) -> Result<(), String> {
    if password.is_empty() || password.len() > 4096 || password.chars().any(char::is_control) {
        return Err("Enter a password without control characters (up to 4096 bytes).".into());
    }
    Ok(())
}

fn copyable_command(command: &Command, arguments: &[String]) -> String {
    let quote = |value: &str| format!("'{}'", value.replace('\'', "'\\''"));
    let mut copyable = Vec::new();
    for (name, value) in command.get_envs() {
        if let Some(value) = value {
            if copyable.is_empty() {
                copyable.push("env".into());
            }
            copyable.push(format!(
                "{}={}",
                name.to_string_lossy(),
                quote(&value.to_string_lossy())
            ));
        }
    }
    copyable.push(quote(&command.get_program().to_string_lossy()));
    copyable.extend(arguments.iter().map(|value| quote(value)));
    copyable.join(" ")
}

pub(crate) fn spawn(
    command: &mut Command,
    mut arguments: Vec<String>,
    password: Option<&str>,
) -> Result<(Child, String), String> {
    if let Some(password) = password {
        validate_password(password)?;
        arguments.push(format!("/p:{password}"));
        // FreeRDP reads one argument per line. The password never appears in argv.
        command.arg("/args-from:stdin").stdin(Stdio::piped());
    } else {
        command.args(&arguments);
    }
    let copyable = copyable_command(command, &arguments);
    let purpose = if arguments.iter().any(|argument| argument == "+auth-only") {
        "authentication test (+auth-only does not open a desktop)"
    } else {
        "desktop connection"
    };
    // The air-gap operator explicitly wants a full command, including credentials.
    eprintln!(
        "FreeRDP {purpose} command (includes password; copyable equivalent to stdin arguments): {}",
        copyable
    );
    let mut child = command
        .spawn()
        .map_err(|error| format!("Cannot launch FreeRDP: {error}"))?;
    if password.is_some() {
        let input = arguments.join("\n") + "\n";
        let result = child
            .stdin
            .take()
            .ok_or("Cannot open FreeRDP input.")?
            .write_all(input.as_bytes());
        if let Err(error) = result {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("Cannot send credentials to FreeRDP: {error}"));
        }
    }
    Ok((child, copyable))
}

/// The outcome of one NLA authentication attempt, with password-redacted details.
#[derive(Debug, Serialize)]
pub struct TestResult {
    pub success: bool,
    pub kind: String,
    pub summary: String,
    pub details: String,
    pub command: String,
    pub fingerprint: Option<String>,
    pub runtime_warning: Option<String>,
}

/// Check server reachability, certificate policy and credentials without a desktop session.
pub fn test_connection(
    profile: Profile,
    client: &Path,
    password: &str,
) -> Result<TestResult, String> {
    test_with_timeout(profile, client, password, Duration::from_secs(20))
}

fn test_with_timeout(
    profile: Profile,
    client: &Path,
    password: &str,
    timeout: Duration,
) -> Result<TestResult, String> {
    let mut profile = profile.validate()?;
    // A password test must use NLA, not a TLS-only connection that never checks credentials.
    profile.fullscreen = false;
    profile.multi_monitor = false;
    profile.monitors.clear();
    let mut arguments = profile.arguments();
    arguments.extend(
        [
            "+auth-only",
            "/sec:nla",
            "/timeout:10000",
            "/log-level:INFO",
        ]
        .map(str::to_owned),
    );
    // Capture into an unnamed private file so output cannot block the child on a full pipe.
    let mut log = tempfile::tempfile().map_err(|error| error.to_string())?;
    let mut command = Command::new(client);
    command
        .stdout(Stdio::from(
            log.try_clone().map_err(|error| error.to_string())?,
        ))
        .stderr(Stdio::from(
            log.try_clone().map_err(|error| error.to_string())?,
        ));
    let (mut child, copyable) = spawn(&mut command, arguments, Some(password))?;
    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            break status;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            let _ = child.kill();
            break child.wait().map_err(|error| error.to_string())?;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let details = read_log(&mut log, password)?;
    let mut result = classify(status.success() && !timed_out, timed_out, details);
    result.command = copyable;
    Ok(result)
}

fn read_log(log: &mut File, password: &str) -> Result<String, String> {
    let length = log.metadata().map_err(|error| error.to_string())?.len();
    log.seek(SeekFrom::Start(length.saturating_sub(65536)))
        .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    log.take(65536)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    Ok(String::from_utf8_lossy(&bytes)
        .replace(password, "[password redacted]")
        .chars()
        .filter(|character| !character.is_control() || *character == '\n' || *character == '\t')
        .collect())
}

fn classify(success: bool, timed_out: bool, details: String) -> TestResult {
    let lower = details.to_ascii_lowercase();
    let runtime_warning = (lower.contains("legacy provider failed") || lower.contains("no md4 support")
        || lower.contains("failed to initialize digest md4"))
        .then(|| "The OpenSSL legacy provider is unavailable; NTLM authentication cannot work with this runtime.".into());
    let changed_certificate = lower.contains("remote host identification has changed")
        || lower.contains("certificate has changed")
        || lower.contains("host key has changed");
    let (kind, summary) = if timed_out {
        (
            "timeout",
            "The test timed out after 20 seconds. Check the host, network and server response.",
        )
    } else if success {
        ("success", "NLA authentication succeeded. The server accepted your credentials; a full desktop session was not tested.")
    } else if changed_certificate {
        ("certificate_changed", "The server certificate differs from the remembered certificate. Verify the new fingerprint before replacing trust.")
    } else if lower.contains("certificate")
        && (lower.contains("not trusted")
            || lower.contains("verification failure")
            || lower.contains("fingerprint")
            || lower.contains("changed"))
    {
        ("certificate", "Certificate trust failed. Verify the server fingerprint, then choose a certificate policy below.")
    } else if runtime_warning.is_some() {
        (
            "runtime",
            "Authentication is blocked by a missing OpenSSL legacy provider.",
        )
    } else if [
        "logon_failure",
        "authentication_failed",
        "wrong_password",
        "access_denied",
        "account_locked",
        "password_expired",
        "logon_denied",
    ]
    .iter()
    .any(|value| lower.contains(value))
    {
        ("credentials", "The server rejected authentication. Check the username/domain, password and account permissions. Details may identify a locked or expired account.")
    } else if [
        "dns_name_not_found",
        "connect_failed",
        "connection refused",
        "host unreachable",
        "connection timed out",
    ]
    .iter()
    .any(|value| lower.contains(value))
    {
        (
            "network",
            "The server could not be reached. Check its address, port, firewall and RDP service.",
        )
    } else {
        ("protocol", "The authentication test failed. Review the details for TLS, NLA or server configuration errors.")
    };
    let fingerprint = details
        .lines()
        .filter(|line| {
            line.to_ascii_lowercase().contains("fingerprint")
                && !line.to_ascii_lowercase().contains("old fingerprint")
        })
        .flat_map(str::split_whitespace)
        .find_map(|word| normalize_fingerprint(word.trim_matches([',', ';', '"', '\''])).ok());
    TestResult {
        success,
        kind: kind.into(),
        summary: summary.into(),
        details,
        command: String::new(),
        fingerprint,
        runtime_warning,
    }
}

pub(crate) fn certificate_filename(authority: &str) -> Result<String, String> {
    let (host, port) = if let Some(rest) = authority.strip_prefix('[') {
        let (host, suffix) = rest.split_once(']').ok_or("Invalid bracketed IPv6 host.")?;
        host.parse::<std::net::Ipv6Addr>()
            .map_err(|_| "Invalid IPv6 host.")?;
        let port = if suffix.is_empty() {
            3389
        } else {
            suffix
                .strip_prefix(':')
                .ok_or("Invalid host port.")?
                .parse::<u16>()
                .map_err(|_| "Invalid host port.")?
        };
        (host, port)
    } else if authority.matches(':').count() > 1 {
        authority
            .parse::<std::net::Ipv6Addr>()
            .map_err(|_| "Use [IPv6]:port for an explicit port.")?;
        (authority, 3389)
    } else if let Some((host, port)) = authority.split_once(':') {
        (host, port.parse::<u16>().map_err(|_| "Invalid host port.")?)
    } else {
        (authority, 3389)
    };
    if host.is_empty()
        || port == 0
        || !host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b".-_:".contains(&byte))
    {
        return Err(
            "Use a hostname, IP address, or host:port to manage its saved certificate.".into(),
        );
    }
    Ok(format!(
        "{}_{port}.pem",
        host.to_ascii_lowercase().replace(':', ".")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CertificatePolicy;
    use std::fs;

    #[test]
    fn distinguishes_certificate_and_password_failures() {
        let fingerprint = "ab:".repeat(31) + "ab";
        let result = classify(false, false, format!("certificate not trusted\nThe fingerprint for the host key is {fingerprint}\nlegacy provider failed"));
        assert_eq!(result.kind, "certificate");
        assert_eq!(result.fingerprint, Some("ab".repeat(32)));
        assert!(result.runtime_warning.is_some());
        assert_eq!(
            classify(false, false, "ERRCONNECT_LOGON_FAILURE".into()).kind,
            "credentials"
        );
        assert_eq!(
            classify(false, false, "ERRCONNECT_CONNECT_FAILED".into()).kind,
            "network"
        );
    }

    #[test]
    fn identifies_changed_certificates_runtime_failures_and_success() {
        assert_eq!(
            classify(
                false,
                false,
                "WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!".into()
            )
            .kind,
            "certificate_changed"
        );
        assert_eq!(
            classify(false, false, "Failed to initialize digest md4".into()).kind,
            "runtime"
        );
        assert_eq!(classify(true, false, String::new()).kind, "success");
    }

    #[test]
    fn matches_freerdp_certificate_filenames_without_accepting_paths() {
        assert_eq!(
            certificate_filename("WINDOWS:3390").unwrap(),
            "windows_3390.pem"
        );
        assert_eq!(
            certificate_filename("[2001:db8::1]:3390").unwrap(),
            "2001.db8..1_3390.pem"
        );
        assert!(certificate_filename("../../other").is_err());
        assert!(certificate_filename("host:0").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn logged_command_can_be_run_with_spaces_and_quoted_passwords() {
        let _guard = crate::PROCESS_TEST_LOCK.lock().unwrap();
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let client = directory.path().join("client's wrapper");
        fs::write(&client, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n").unwrap();
        fs::set_permissions(&client, fs::Permissions::from_mode(0o700)).unwrap();
        let arguments = vec![
            "/v:host".into(),
            "/u:DOMAIN\\user".into(),
            "/p:a 'quoted' password".into(),
        ];
        let line = copyable_command(&Command::new(client), &arguments);
        let output = Command::new("sh").args(["-c", &line]).output().unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            arguments.join("\n") + "\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn tests_credentials_through_stdin_and_redacts_output_with_a_deadline() {
        let _guard = crate::PROCESS_TEST_LOCK.lock().unwrap();
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let client = directory.path().join("client");
        let args = directory.path().join("args");
        fs::write(&client, format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\ncat > '{}.stdin'\nprintf 'ERRCONNECT_LOGON_FAILURE secret-password\\n'\nexit 1\n", args.display(), args.display())).unwrap();
        fs::set_permissions(&client, fs::Permissions::from_mode(0o700)).unwrap();
        let profile = Profile {
            id: "test".into(),
            name: "Test".into(),
            host: "example.invalid".into(),
            user: "user".into(),
            fullscreen: true,
            software_rendering: false,
            multi_monitor: true,
            monitors: "1".into(),
            certificate: CertificatePolicy::Ignore,
            fingerprint: String::new(),
        };
        let result = test_connection(profile.clone(), &client, "secret-password").unwrap();
        assert_eq!(result.kind, "credentials");
        assert!(!result.details.contains("secret-password"));
        assert!(result.command.contains("'/p:secret-password'"));
        assert!(result.command.contains("'+auth-only'"));
        assert_eq!(fs::read_to_string(&args).unwrap(), "/args-from:stdin\n");
        let input = fs::read_to_string(args.with_extension("stdin")).unwrap();
        assert!(input.contains("+auth-only\n/sec:nla\n"));
        assert!(input.contains("/cert:ignore\n"));
        assert!(!input.contains("/multimon"));
        assert!(test_connection(profile.clone(), &client, "bad\n/p:injection").is_err());
        fs::write(&client, "#!/bin/sh\ncat >/dev/null\nexec sleep 10\n").unwrap();
        let start = Instant::now();
        let result =
            test_with_timeout(profile, &client, "secret", Duration::from_millis(100)).unwrap();
        assert_eq!(result.kind, "timeout");
        assert!(start.elapsed() < Duration::from_secs(2));
    }
}
