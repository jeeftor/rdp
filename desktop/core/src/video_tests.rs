//! Sequential, bounded desktop trials; visual acceptance remains an operator decision.
use crate::{connection, desktop_command, DiagnosticHistory, Profile, TestResult, VideoMode};
use base64::Engine;
use serde::Serialize;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

/// Captured evidence from one explicit renderer, without assuming visual usability.
#[derive(Serialize)]
pub struct VideoTrial {
    pub mode: VideoMode,
    pub kind: String,
    pub summary: String,
    pub details: String,
    pub command: String,
    pub graphics: String,
    pub exit_code: Option<i32>,
    pub desktop_initialized: bool,
    pub skipped: bool,
    pub timed_out: bool,
    pub screenshot: serde_json::Value,
}

/// Authentication is attempted once before any desktop sign-in attempts.
#[derive(Serialize)]
pub struct VideoTestBatch {
    pub batch_id: String,
    pub authentication: TestResult,
    pub trials: Vec<VideoTrial>,
}

/// Test each available renderer with a small NLA desktop and a 15-second deadline.
pub fn run_video_tests(
    profile: Profile,
    client: &Path,
    password: &str,
    history: &DiagnosticHistory,
    on_start: impl FnMut(VideoMode),
) -> Result<VideoTestBatch, String> {
    run_with_deadline(
        profile,
        client,
        password,
        history,
        Duration::from_secs(15),
        on_start,
    )
}

fn run_with_deadline(
    profile: Profile,
    client: &Path,
    password: &str,
    history: &DiagnosticHistory,
    duration: Duration,
    mut on_start: impl FnMut(VideoMode),
) -> Result<VideoTestBatch, String> {
    let start = history.start(
        &profile.id,
        "batch video start",
        String::new(),
        String::new(),
    );
    let batch_id = start.to_string();
    history.batch(start, &batch_id, None);
    let authentication = crate::test_connection(profile.clone(), client, password)?;
    let stamp = history.authentication(&profile.id, &authentication);
    history.batch(stamp, &batch_id, None);
    let mut trials = Vec::new();
    let mut stop = !authentication.success;
    for available in crate::video_availability()
        .into_iter()
        .filter(|choice| choice.mode != VideoMode::Auto)
    {
        let mode = available.mode;
        let skip_reason = if !available.available {
            Some(available.reason)
        } else if stop {
            Some(
                "Skipped: authentication was rejected or unresolved; no further sign-in attempts."
                    .into(),
            )
        } else {
            None
        };
        if let Some(summary) = skip_reason {
            trials.push(VideoTrial {
                mode,
                kind: "skipped".into(),
                summary,
                details: String::new(),
                command: String::new(),
                graphics: String::new(),
                exit_code: None,
                desktop_initialized: false,
                skipped: true,
                timed_out: false,
                screenshot: unavailable_capture("No desktop was tested."),
            });
            continue;
        }
        on_start(mode);
        let prepared = desktop_command(profile.clone(), client, Some(mode));
        let (mut command, arguments, graphics) = match prepared {
            Ok(value) => value,
            Err(details) => {
                let stamp = history.start(
                    &profile.id,
                    "batch video probe",
                    String::new(),
                    details.clone(),
                );
                history.batch(stamp, &batch_id, Some(mode));
                let outcome = history.finish(stamp, false, None);
                trials.push(VideoTrial {
                    mode,
                    kind: "renderer".into(),
                    summary: "Renderer unavailable; see details.".into(),
                    details,
                    command: String::new(),
                    graphics: outcome.graphics,
                    exit_code: None,
                    desktop_initialized: false,
                    skipped: false,
                    timed_out: false,
                    screenshot: unavailable_capture("Renderer probe failed before desktop launch."),
                });
                continue;
            }
        };
        // A private file cannot fill a pipe and block the client or leave reader threads alive.
        let mut log = tempfile::tempfile().map_err(|error| error.to_string())?;
        command
            .stdout(Stdio::from(
                log.try_clone().map_err(|error| error.to_string())?,
            ))
            .stderr(Stdio::from(
                log.try_clone().map_err(|error| error.to_string())?,
            ));
        let (mut child, copyable) = connection::spawn(&mut command, arguments, Some(password))?;
        let stamp = history.start(
            &profile.id,
            "batch video trial",
            copyable.clone(),
            graphics.clone(),
        );
        history.batch(stamp, &batch_id, Some(mode));
        let deadline = Instant::now() + duration;
        let mut timed_out = false;
        let started = Instant::now();
        let mut capture_attempt = 0;
        let mut screenshot = unavailable_capture(
            "Desktop closed before capture, or native Wayland capture is unavailable.",
        );
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {}
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(error.to_string());
                }
            }
            if mode.settings().is_some_and(|(backend, _)| backend == "x11")
                && capture_attempt < 2
                && started.elapsed() >= Duration::from_secs(5 * (capture_attempt + 1))
                && screenshot["status"] != "content"
            {
                screenshot = capture_window(client, child.id(), stamp, capture_attempt);
                capture_attempt += 1;
            }
            if Instant::now() >= deadline {
                timed_out = true;
                let _ = child.kill();
                break child.wait().map_err(|error| error.to_string())?;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let captured = connection::read_log(&mut log)?;
        let details = format!("{graphics}{captured}");
        history.append(stamp, &captured);
        if timed_out {
            history.append(stamp, "Trial automatically closed at its deadline.");
        }
        history.screenshot(stamp, screenshot.clone());
        let mut outcome = history.finish(stamp, status.success(), status.code());
        let desktop_initialized = details
            .to_ascii_lowercase()
            .contains("local framebuffer format")
            && details
                .to_ascii_lowercase()
                .contains("remote framebuffer format");
        if timed_out
            && outcome.kind != "renderer"
            && outcome.kind != "credentials"
            && outcome.kind != "certificate"
        {
            outcome.kind = if desktop_initialized {
                "initialized"
            } else {
                "timeout"
            }
            .into();
            outcome.summary = if desktop_initialized {
                "Session initialized; automatically closed. Confirm you saw the desktop."
            } else {
                "Timed out before a desktop was detected."
            }
            .into();
        }
        if outcome.kind == "initialized" {
            match screenshot["status"].as_str() {
                Some("blank") => {
                    outcome.kind = "blank".into();
                    outcome.summary =
                        "Window looks blank or uniform; check the captured image.".into();
                }
                Some("content") => {
                    outcome.summary =
                        "Window content captured; confirm you saw a usable desktop.".into()
                }
                _ => {}
            }
        }
        history.video_outcome(stamp, outcome.clone());
        // Stop if the server starts rejecting authentication during later trials.
        stop = outcome.kind == "credentials" || outcome.kind == "certificate";
        trials.push(VideoTrial {
            mode,
            kind: outcome.kind,
            summary: outcome.summary,
            details,
            command: copyable,
            graphics: outcome.graphics,
            exit_code: status.code(),
            desktop_initialized,
            skipped: false,
            timed_out,
            screenshot,
        });
    }
    Ok(VideoTestBatch {
        batch_id,
        authentication,
        trials,
    })
}

fn unavailable_capture(reason: &str) -> serde_json::Value {
    serde_json::json!({"status": "unavailable", "reason": reason})
}

fn capture_window(client: &Path, pid: u32, stamp: u128, attempt: u64) -> serde_json::Value {
    let capture = || -> Result<serde_json::Value, String> {
        let base = client
            .parent()
            .ok_or("Cannot locate bundled window capture.")?;
        let helper = base.join("bin/rdp-capture-window");
        if !helper.is_file() {
            return Err("Bundled X11 window capture is unavailable.".into());
        }
        let state = std::env::var_os("XDG_STATE_HOME")
            .filter(|value| !value.is_empty())
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|home| std::path::PathBuf::from(home).join(".local/state"))
            })
            .ok_or("Cannot locate your state directory.")?;
        if !state.is_absolute() {
            return Err("Your state directory must be absolute.".into());
        }
        let directory = state.join("rdpctl/diagnostics");
        std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
                .map_err(|error| error.to_string())?;
        }
        let path = directory.join(format!("window-{stamp}-{attempt}.png"));
        let mut paths = vec![base.join("lib")];
        if let Some(existing) = std::env::var_os("LD_LIBRARY_PATH") {
            paths.extend(std::env::split_paths(&existing));
        }
        let output = std::process::Command::new(helper)
            .args(["--pid", &pid.to_string(), "--output"])
            .arg(&path)
            .env(
                "LD_LIBRARY_PATH",
                std::env::join_paths(paths).map_err(|error| error.to_string())?,
            )
            .stdin(Stdio::null())
            .output()
            .map_err(|error| error.to_string())?;
        let mut evidence: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|_| "Window capture returned invalid evidence.".to_string())?;
        if !["content", "blank", "unavailable"].contains(&evidence["status"].as_str().unwrap_or(""))
        {
            return Err("Window capture returned unsupported evidence.".into());
        }
        if evidence["file"].is_string() {
            evidence["path"] = serde_json::json!(path);
            // Read only the fresh child-window capture, with a strict allocation cap.
            let width = evidence["width"].as_u64().unwrap_or(0);
            let height = evidence["height"].as_u64().unwrap_or(0);
            if width > 0 && height > 0 && width <= 4096 && height <= 4096 {
                use std::io::Read;
                if let Ok(file) = std::fs::File::open(&path) {
                    let mut png = Vec::new();
                    if file.take(4_194_305).read_to_end(&mut png).is_ok()
                        && png.len() <= 4_194_304
                        && png.starts_with(b"\x89PNG\r\n\x1a\n")
                    {
                        evidence["data_url"] = serde_json::json!(format!(
                            "data:image/png;base64,{}",
                            base64::engine::general_purpose::STANDARD.encode(png)
                        ));
                    }
                }
            }
        }
        Ok(evidence)
    };
    capture().unwrap_or_else(|reason| unavailable_capture(&reason))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn profile() -> Profile {
        Profile {
            id: "batch".into(),
            name: "Batch".into(),
            host: "example.invalid".into(),
            user: "user".into(),
            fullscreen: true,
            multi_monitor: true,
            monitors: "3,4".into(),
            software_rendering: false,
            video_mode: VideoMode::Auto,
            working_video_modes: vec![],
            certificate: crate::CertificatePolicy::Ignore,
            fingerprint: String::new(),
        }
    }

    #[test]
    fn authentication_gates_desktops_and_trials_capture_serial_results_with_deadlines() {
        let _guard = crate::PROCESS_TEST_LOCK.lock().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let client = directory.path().join("client");
        let trace = directory.path().join("trace");
        let previous = ["DISPLAY", "WAYLAND_DISPLAY", "XDG_STATE_HOME"]
            .map(|name| (name, std::env::var_os(name)));
        std::env::set_var("DISPLAY", ":99");
        std::env::set_var("XDG_STATE_HOME", directory.path());
        std::env::remove_var("WAYLAND_DISPLAY");
        fs::write(
            &client,
            "#!/bin/sh\ncat >/dev/null\nprintf 'ERRCONNECT_LOGON_FAILURE\\n'\nexit 134\n",
        )
        .unwrap();
        fs::set_permissions(&client, fs::Permissions::from_mode(0o700)).unwrap();
        let history = DiagnosticHistory::default();
        let rejected = run_with_deadline(
            profile(),
            &client,
            "visible password",
            &history,
            Duration::from_millis(100),
            |_| {},
        )
        .unwrap();
        assert!(!rejected.authentication.success);
        assert_eq!(rejected.authentication.exit_code, Some(134));
        assert!(rejected.trials.iter().all(|trial| trial.skipped));
        assert_eq!(rejected.trials.len(), 5);
        fs::write(&client, format!("#!/bin/sh\ninput=$(cat)\ncase \"$input\" in *+auth-only*) exit 0;; esac\nprintf '%s start\\n' \"$SDL_RENDER_DRIVER\" >> '{}'\nprintf '%s\\n' \"$input\"\nprintf 'Local framebuffer format BGRA32\\nRemote framebuffer format BGRA32\\n' >&2\nprintf '%s end\\n' \"$SDL_RENDER_DRIVER\" >> '{}'\nexit 0\n", trace.display(), trace.display())).unwrap();
        let mut active_modes = Vec::new();
        let passed = run_with_deadline(
            profile(),
            &client,
            "visible password",
            &history,
            Duration::from_millis(100),
            |mode| active_modes.push(mode),
        )
        .unwrap();
        assert!(passed.authentication.success);
        assert_eq!(
            active_modes,
            vec![
                VideoMode::X11Opengl,
                VideoMode::X11Opengles2,
                VideoMode::X11Software
            ]
        );
        assert_eq!(fs::read_to_string(&trace).unwrap(), "opengl start\nopengl end\nopengles2 start\nopengles2 end\nsoftware start\nsoftware end\n");
        for trial in passed.trials.iter().filter(|trial| !trial.skipped) {
            assert!(trial.desktop_initialized);
            assert_eq!(trial.kind, "initialized");
            assert!(trial.command.contains("/p:visible password"));
            assert!(trial.details.contains("/cert:ignore"));
            assert!(trial.details.contains("/size:1280x720"));
            assert!(trial.details.contains("/sec:nla"));
            assert!(!trial.details.contains("/monitors:3,4"));
            assert!(!trial.details.contains("/multimon"));
            assert!(!trial.details.contains("/f\n"));
            assert!(trial.details.contains("Remote framebuffer format"));
        }
        fs::write(&client, "#!/bin/sh\ninput=$(cat)\ncase \"$input\" in *+auth-only*) exit 0;; esac\nprintf 'Local framebuffer format BGRA32\\nRemote framebuffer format BGRA32\\n'\nexec sleep 10\n").unwrap();
        let start = Instant::now();
        let bounded = run_with_deadline(
            profile(),
            &client,
            "visible password",
            &history,
            Duration::from_millis(100),
            |_| {},
        )
        .unwrap();
        assert!(start.elapsed() < Duration::from_secs(2));
        assert!(bounded
            .trials
            .iter()
            .filter(|trial| !trial.skipped)
            .all(|trial| trial.timed_out
                && trial.desktop_initialized
                && trial.kind == "initialized"
                && trial.exit_code.is_none()));
        fs::write(&client, "#!/bin/sh\ninput=$(cat)\ncase \"$input\" in *+auth-only*) exit 0;; esac\nprintf 'ERRCONNECT_LOGON_FAILURE\\n'\nexit 134\n").unwrap();
        let later_rejection = run_with_deadline(
            profile(),
            &client,
            "visible password",
            &history,
            Duration::from_millis(100),
            |_| {},
        )
        .unwrap();
        assert_eq!(later_rejection.trials[0].kind, "credentials");
        assert_eq!(later_rejection.trials[0].exit_code, Some(134));
        assert!(later_rejection.trials[1..]
            .iter()
            .all(|trial| trial.skipped));
        let exported = history.export(&profile()).unwrap();
        let exported: serde_json::Value =
            serde_json::from_slice(&fs::read(exported).unwrap()).unwrap();
        let latest: Vec<_> = exported["attempts"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|attempt| attempt["batch_id"] == later_rejection.batch_id)
            .collect();
        assert_eq!(latest.len(), 3);
        assert_eq!(latest[0]["purpose"], "batch video start");
        assert!(latest[1]["authentication"]["success"].as_bool().unwrap());
        assert_eq!(latest[2]["video_mode"], "x11-opengl");
        assert_eq!(latest[2]["outcome"]["kind"], "credentials");
        assert_ne!(passed.batch_id, later_rejection.batch_id);
        for (name, value) in previous {
            if let Some(value) = value {
                std::env::set_var(name, value);
            } else {
                std::env::remove_var(name);
            }
        }
    }
    #[test]
    fn window_capture_retains_private_evidence_and_missing_helper_is_unavailable() {
        let _guard = crate::PROCESS_TEST_LOCK.lock().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let previous = std::env::var_os("XDG_STATE_HOME");
        std::env::set_var("XDG_STATE_HOME", directory.path());
        let client = directory.path().join("client");
        assert_eq!(
            capture_window(&client, 123, 123, 0)["status"],
            "unavailable"
        );
        fs::create_dir(directory.path().join("bin")).unwrap();
        let helper = directory.path().join("bin/rdp-capture-window");
        fs::write(&helper, "#!/bin/sh\nprintf 'png fixture' > \"$4\"\nprintf '{\"status\":\"content\",\"reason\":\"Captured nonuniform window\",\"file\":\"%s\",\"width\":10,\"height\":10,\"nonblack_ratio\":0.8,\"variance\":12}\\n' \"$4\"\n").unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)).unwrap();
        let evidence = capture_window(&client, 123, 123, 0);
        assert_eq!(evidence["status"], "content");
        let path = std::path::PathBuf::from(evidence["path"].as_str().unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), "png fixture");
        assert_eq!(
            fs::metadata(path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert!(evidence.get("data_url").is_none());
        let script = fs::read_to_string(&helper)
            .unwrap()
            .replace("printf 'png fixture'", "printf '\\211PNG\\r\\n\\032\\n'");
        fs::write(&helper, &script).unwrap();
        let preview = capture_window(&client, 123, 123, 1);
        assert_eq!(preview["data_url"], "data:image/png;base64,iVBORw0KGgo=");
        fs::write(&helper, script.replace("\"width\":10", "\"width\":5000")).unwrap();
        assert!(capture_window(&client, 123, 123, 2)
            .get("data_url")
            .is_none());
        let history = DiagnosticHistory::default();
        let stamp = history.start(
            "batch",
            "batch video trial",
            "command".into(),
            String::new(),
        );
        history.screenshot(stamp, evidence);
        let exported = history.export(&profile()).unwrap();
        let exported: serde_json::Value =
            serde_json::from_slice(&fs::read(exported).unwrap()).unwrap();
        assert_eq!(exported["attempts"][0]["screenshot"]["status"], "content");
        assert_eq!(
            exported["attempts"][0]["screenshot"]["path"],
            path.to_string_lossy().as_ref()
        );
        if let Some(value) = previous {
            std::env::set_var("XDG_STATE_HOME", value);
        } else {
            std::env::remove_var("XDG_STATE_HOME");
        }
    }
}
