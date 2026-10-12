#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use rdpctl_core::{DiagnosticHistory, Profile, Store, VideoMode};
use serde::Serialize;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Child, ExitStatus};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};
use std::time::Duration;
use tauri::Emitter;

fn client_path() -> Result<PathBuf, String> {
    let client = std::env::var_os("RDPCTL_FREERDP")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("APPDIR")
                .filter(|value| !value.is_empty())
                .map(|directory| PathBuf::from(directory).join("usr/share/rdpctl/freerdp/freerdp"))
                .unwrap_or_else(|| PathBuf::from("/opt/freerdp-portable/freerdp"))
        });
    if !client.is_absolute() {
        return Err("RDPCTL_FREERDP must be an absolute path to the FreeRDP wrapper.".into());
    }
    Ok(client)
}

#[tauri::command]
async fn list_monitors(video_mode: Option<VideoMode>) -> Result<Vec<rdpctl_core::Monitor>, String> {
    let client = client_path()?;
    tauri::async_runtime::spawn_blocking(move || {
        rdpctl_core::list_monitors(&client, video_mode.unwrap_or_default())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
fn list_profiles() -> Result<Vec<Profile>, String> {
    Store::from_environment()?.list()
}

#[tauri::command]
fn create_profile(profile: Profile) -> Result<Profile, String> {
    Store::from_environment()?.create(profile)
}

#[tauri::command]
fn update_profile(profile: Profile) -> Result<Profile, String> {
    Store::from_environment()?.update(profile)
}

#[tauri::command]
fn delete_profile(id: String) -> Result<(), String> {
    Store::from_environment()?.delete(&id)
}

#[tauri::command]
fn saved_password(id: String) -> Result<Option<String>, String> {
    Store::from_environment()?.password(&id)
}

#[tauri::command]
fn save_password(id: String, password: String) -> Result<(), String> {
    Store::from_environment()?.save_password(&id, &password)
}

#[tauri::command]
fn forget_password(id: String) -> Result<(), String> {
    Store::from_environment()?.forget_password(&id)
}

#[tauri::command]
fn forget_certificate(id: String) -> Result<bool, String> {
    Store::from_environment()?.forget_certificate(&id)
}

#[tauri::command]
async fn test_reachability(
    id: String,
    history: tauri::State<'_, DiagnosticHistory>,
) -> Result<rdpctl_core::ReachabilityResult, String> {
    let profile = Store::from_environment()?.load(&id)?;
    let result =
        tauri::async_runtime::spawn_blocking(move || rdpctl_core::test_server(&profile.host))
            .await
            .map_err(|error| error.to_string())??;
    history.start(
        &id,
        "server reachability",
        String::new(),
        format!("{}\n{}", result.summary, result.details),
    );
    Ok(result)
}

#[tauri::command]
async fn test_profile(
    id: String,
    password: Option<String>,
    history: tauri::State<'_, DiagnosticHistory>,
) -> Result<rdpctl_core::TestResult, String> {
    let store = Store::from_environment()?;
    let profile = store.load(&id)?;
    let password = match password {
        Some(password) => Some(password),
        None => store.password(&id)?,
    }
    .ok_or("Enter a password or save one before testing.")?;
    let client = client_path()?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        rdpctl_core::test_connection(profile, &client, &password)
    })
    .await
    .map_err(|error| error.to_string())??;
    history.authentication(&id, &result);
    Ok(result)
}

#[tauri::command]
fn video_availability() -> Vec<rdpctl_core::VideoAvailability> {
    rdpctl_core::video_availability()
}

#[tauri::command]
fn export_diagnostics(
    id: String,
    history: tauri::State<'_, DiagnosticHistory>,
) -> Result<String, String> {
    let profile = Store::from_environment()?.load(&id)?;
    history
        .export(&profile)
        .map(|path| path.to_string_lossy().into_owned())
}

#[tauri::command]
async fn compare_authentication(
    id: String,
    password: Option<String>,
    history: tauri::State<'_, DiagnosticHistory>,
) -> Result<Vec<rdpctl_core::TestResult>, String> {
    let store = Store::from_environment()?;
    let profile = store.load(&id)?;
    let password = match password {
        Some(password) => Some(password),
        None => store.password(&id)?,
    }
    .ok_or("Enter a password or save one before testing.")?;
    let client = client_path()?;
    let results = tauri::async_runtime::spawn_blocking(move || {
        let stdin = rdpctl_core::test_connection(profile.clone(), &client, &password)?;
        let argv = rdpctl_core::test_connection_argv(profile, &client, &password)?;
        Ok::<_, String>(vec![stdin, argv])
    })
    .await
    .map_err(|error| error.to_string())??;
    for result in &results {
        history.authentication(&id, result);
    }
    Ok(results)
}

#[tauri::command]
async fn run_video_tests(
    app: tauri::AppHandle,
    id: String,
    password: Option<String>,
    history: tauri::State<'_, DiagnosticHistory>,
) -> Result<rdpctl_core::VideoTestBatch, String> {
    let store = Store::from_environment()?;
    let profile = store.load(&id)?;
    let password = password
        .or(store.password(&id)?)
        .ok_or("Enter a password or save one before testing.")?;
    let client = client_path()?;
    let history = history.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        rdpctl_core::run_video_tests(profile, &client, &password, &history, |mode| {
            let _ = app.emit(
                "video-test-progress",
                VideoTestProgress {
                    id: id.clone(),
                    mode,
                    phase: "starting",
                },
            );
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

#[derive(Clone, Serialize)]
struct VideoTestProgress {
    id: String,
    mode: VideoMode,
    phase: &'static str,
}

#[derive(Serialize)]
struct LaunchResult {
    pid: u32,
    command: String,
    graphics: String,
}

#[derive(Clone, Serialize)]
struct SessionLog {
    id: String,
    line: String,
}

fn relay_log(
    app: tauri::AppHandle,
    id: String,
    history: DiagnosticHistory,
    stamp: u128,
    output: impl Read + Send + 'static,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        for line in BufReader::new(output).lines() {
            let Ok(line) = line else { break };
            eprintln!("{line}");
            history.append(stamp, &line);
            let _ = app.emit(
                "session-log",
                SessionLog {
                    id: id.clone(),
                    line,
                },
            );
        }
    })
}

#[derive(Clone, Serialize)]
struct SessionEnded {
    id: String,
    video_test: bool,
    disconnected: bool,
    name: String,
    pid: u32,
    success: bool,
    code: Option<i32>,
    outcome: rdpctl_core::SessionOutcome,
}

#[derive(Clone, Default)]
struct SessionRegistry(Arc<SessionState>);

#[derive(Default)]
struct SessionState {
    next: AtomicU64,
    sessions: Mutex<HashMap<u64, OwnedSession>>,
}

#[derive(Clone)]
struct OwnedSession {
    id: String,
    child: Arc<Mutex<Child>>,
    disconnected: Arc<AtomicBool>,
}

#[derive(Serialize)]
struct ActiveSession {
    id: String,
    pid: u32,
}

impl SessionRegistry {
    fn register(&self, id: String, child: Child) -> (u64, OwnedSession) {
        let token = self.0.next.fetch_add(1, Ordering::Relaxed);
        let session = OwnedSession {
            id,
            child: Arc::new(Mutex::new(child)),
            disconnected: Arc::new(AtomicBool::new(false)),
        };
        self.0
            .sessions
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(token, session.clone());
        (token, session)
    }

    fn snapshot(&self) -> Vec<(u64, OwnedSession)> {
        self.0
            .sessions
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .iter()
            .map(|(token, session)| (*token, session.clone()))
            .collect()
    }

    fn remove(&self, token: u64) {
        self.0
            .sessions
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&token);
    }

    fn active(&self) -> Result<Vec<ActiveSession>, String> {
        let mut active = Vec::new();
        for (token, session) in self.snapshot() {
            let mut child = session
                .child
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if child
                .try_wait()
                .map_err(|error| format!("Check desktop process: {error}"))?
                .is_some()
            {
                self.remove(token);
            } else {
                active.push(ActiveSession {
                    id: session.id,
                    pid: child.id(),
                });
            }
        }
        active.sort_by(|left, right| (&left.id, left.pid).cmp(&(&right.id, right.pid)));
        Ok(active)
    }

    fn disconnect(&self, id: &str) -> Result<usize, String> {
        let mut stopped = 0;
        let mut failure = None;
        for (token, session) in self.snapshot() {
            if session.id != id {
                continue;
            }
            let mut child = session
                .child
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            // Reap before signaling: an exited child's PID may already belong to another process.
            match child.try_wait() {
                Ok(Some(_)) => self.remove(token),
                Ok(None) => match child.kill() {
                    Ok(()) => {
                        session.disconnected.store(true, Ordering::Release);
                        stopped += 1;
                    }
                    Err(error) => failure = Some(format!("Disconnect desktop: {error}")),
                },
                Err(error) => failure = Some(format!("Check desktop process: {error}")),
            }
        }
        match failure {
            Some(error) => Err(error),
            None => Ok(stopped),
        }
    }

    fn wait(&self, token: u64, child: &Mutex<Child>) -> std::io::Result<ExitStatus> {
        let result = loop {
            let status = child
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .try_wait();
            match status {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => std::thread::sleep(Duration::from_millis(100)),
                Err(error) => break Err(error),
            }
        };
        self.remove(token);
        result
    }
}

#[tauri::command]
fn list_sessions(
    sessions: tauri::State<'_, SessionRegistry>,
) -> Result<Vec<ActiveSession>, String> {
    sessions.active()
}

#[tauri::command]
fn disconnect_profile(
    id: String,
    sessions: tauri::State<'_, SessionRegistry>,
) -> Result<usize, String> {
    sessions.disconnect(&id)
}

#[tauri::command]
async fn launch_profile(
    app: tauri::AppHandle,
    id: String,
    password: Option<String>,
    video_mode: Option<VideoMode>,
    history: tauri::State<'_, DiagnosticHistory>,
    sessions: tauri::State<'_, SessionRegistry>,
) -> Result<LaunchResult, String> {
    let store = Store::from_environment()?;
    let profile = store.load(&id)?;
    let password = match password {
        Some(password) => Some(password),
        None => store.password(&id)?,
    };
    // Only the local environment can override the client; the webview supplies an ID.
    let client = client_path()?;
    let name = profile.name.clone();
    let history = history.inner().clone();
    let launched = tauri::async_runtime::spawn_blocking(move || {
        rdpctl_core::launch(profile, &client, password.as_deref(), video_mode)
    })
    .await
    .map_err(|error| error.to_string())?;
    let (mut child, command, graphics) = match launched {
        Ok(launched) => launched,
        Err(error) => {
            let stamp = history.start(&id, "desktop launch failure", String::new(), error.clone());
            history.finish(stamp, false, None);
            return Err(error);
        }
    };
    let graphics_summary = rdpctl_core::session_outcome(true, &graphics).graphics;
    let stamp = history.start(
        &id,
        if video_mode.is_some() {
            "video trial"
        } else {
            "desktop"
        },
        command.clone(),
        graphics,
    );
    let mut readers = Vec::new();
    if let Some(output) = child.stdout.take() {
        readers.push(relay_log(
            app.clone(),
            id.clone(),
            history.clone(),
            stamp,
            output,
        ));
    }
    if let Some(output) = child.stderr.take() {
        readers.push(relay_log(
            app.clone(),
            id.clone(),
            history.clone(),
            stamp,
            output,
        ));
    }
    let pid = child.id();
    let sessions = sessions.inner().clone();
    let (token, session) = sessions.register(id.clone(), child);
    std::thread::spawn(move || {
        let status = sessions.wait(token, &session.child);
        let disconnected = session.disconnected.load(Ordering::Acquire);
        for reader in readers {
            let _ = reader.join();
        }
        let code = status.as_ref().ok().and_then(|status| status.code());
        let outcome = if disconnected {
            history.finish_disconnected(stamp, code)
        } else {
            history.finish(
                stamp,
                status.as_ref().is_ok_and(|status| status.success()),
                code,
            )
        };
        let event = SessionEnded {
            id,
            outcome,
            video_test: video_mode.is_some(),
            disconnected,
            name,
            pid,
            success: status.as_ref().is_ok_and(|status| status.success()),
            code: status.ok().and_then(|status| status.code()),
        };
        let _ = app.emit("session-ended", event);
    });
    Ok(LaunchResult {
        pid,
        command,
        graphics: graphics_summary,
    })
}

fn main() {
    #[cfg(target_os = "linux")]
    if let Some(directory) = std::env::var_os("APPDIR") {
        let fonts = PathBuf::from(directory).join("usr/share/rdpctl/fonts.conf");
        if fonts.is_file() {
            // Set this before GTK starts its worker threads, including in AppImages.
            std::env::set_var("FONTCONFIG_FILE", fonts);
        }
    }
    tauri::Builder::default()
        .manage(DiagnosticHistory::default())
        .manage(SessionRegistry::default())
        .invoke_handler(tauri::generate_handler![
            list_profiles,
            create_profile,
            update_profile,
            delete_profile,
            test_reachability,
            saved_password,
            save_password,
            forget_password,
            forget_certificate,
            test_profile,
            launch_profile,
            list_sessions,
            disconnect_profile,
            list_monitors,
            video_availability,
            compare_authentication,
            run_video_tests,
            export_diagnostics
        ])
        .run(tauri::generate_context!())
        .expect("Cannot start the rdpctl desktop application");
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use glib::variant::ToVariant;

    #[test]
    fn glib_string_variant_iteration_survives_optimization() {
        let values = ["first", "second", "third"];
        let variant = values.to_variant();
        assert_eq!(
            variant.array_iter_str().unwrap().collect::<Vec<_>>(),
            values
        );
        assert_eq!(
            variant.array_iter_str().unwrap().rev().collect::<Vec<_>>(),
            ["third", "second", "first"]
        );
    }
}

#[cfg(all(test, unix))]
mod session_tests {
    use super::SessionRegistry;
    use std::process::{Command, Stdio};

    fn sleeper() -> std::process::Child {
        Command::new("sh")
            .args(["-c", "exec sleep 30"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap()
    }

    #[test]
    fn disconnect_stops_all_owned_sessions_for_only_the_requested_profile() {
        let sessions = SessionRegistry::default();
        let (first_token, first) = sessions.register("selected".into(), sleeper());
        let (second_token, second) = sessions.register("selected".into(), sleeper());
        let (other_token, other) = sessions.register("other".into(), sleeper());
        let mut outside = sleeper();
        assert_eq!(sessions.active().unwrap().len(), 3);
        assert_eq!(sessions.disconnect("selected").unwrap(), 2);
        assert!(first.disconnected.load(super::Ordering::Acquire));
        assert!(second.disconnected.load(super::Ordering::Acquire));
        assert!(!other.disconnected.load(super::Ordering::Acquire));
        assert!(!sessions.wait(first_token, &first.child).unwrap().success());
        assert!(!sessions
            .wait(second_token, &second.child)
            .unwrap()
            .success());
        assert_eq!(sessions.disconnect("selected").unwrap(), 0);
        let remaining = sessions.active().unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, "other");
        assert!(other.child.lock().unwrap().try_wait().unwrap().is_none());
        assert!(outside.try_wait().unwrap().is_none());
        other.child.lock().unwrap().kill().unwrap();
        sessions.wait(other_token, &other.child).unwrap();
        outside.kill().unwrap();
        outside.wait().unwrap();
    }

    #[test]
    fn naturally_exited_session_is_reaped_without_signaling_its_stale_pid() {
        let sessions = SessionRegistry::default();
        let child = Command::new("sh").args(["-c", "exit 0"]).spawn().unwrap();
        let (token, child) = sessions.register("finished".into(), child);
        child.child.lock().unwrap().wait().unwrap();
        assert_eq!(sessions.disconnect("finished").unwrap(), 0);
        assert!(!child.disconnected.load(super::Ordering::Acquire));
        assert!(sessions.active().unwrap().is_empty());
        assert!(sessions.wait(token, &child.child).unwrap().success());
        assert_eq!(sessions.disconnect("finished").unwrap(), 0);
    }
}
