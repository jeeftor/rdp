//! Bounded session evidence and explicit, private diagnostic exports.
use crate::{Profile, TestResult, VideoMode};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Relevant display settings inherited by the GUI and its child processes.
pub fn display_environment() -> BTreeMap<String, String> {
    [
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XDG_RUNTIME_DIR",
        "XDG_SESSION_TYPE",
        "XDG_CURRENT_DESKTOP",
        "XAUTHORITY",
        "SDL_VIDEO_DRIVER",
        "SDL_VIDEODRIVER",
        "SDL_RENDER_DRIVER",
        "SDL_FRAMEBUFFER_ACCELERATION",
        "LIBGL_ALWAYS_SOFTWARE",
        "LD_LIBRARY_PATH",
        "OPENSSL_CONF",
        "OPENSSL_MODULES",
        "RDPCTL_FREERDP",
    ]
    .into_iter()
    .filter_map(|name| std::env::var(name).ok().map(|value| (name.into(), value)))
    .collect()
}

/// A choice that has a display endpoint; its renderer still needs a real trial.
#[derive(Serialize)]
pub struct VideoAvailability {
    pub mode: VideoMode,
    pub available: bool,
    pub reason: String,
}

/// Report endpoint availability without promising that a graphics driver works.
pub fn video_availability() -> Vec<VideoAvailability> {
    [
        VideoMode::Auto,
        VideoMode::X11Opengl,
        VideoMode::WaylandOpengl,
        VideoMode::X11Opengles2,
        VideoMode::WaylandOpengles2,
        VideoMode::X11Software,
    ]
    .into_iter()
    .map(|mode| {
        let variable = mode.settings().map(|(backend, _)| {
            if backend == "x11" {
                "DISPLAY"
            } else {
                "WAYLAND_DISPLAY"
            }
        });
        let available = variable
            .is_none_or(|name| std::env::var_os(name).is_some_and(|value| !value.is_empty()));
        VideoAvailability {
            mode,
            available,
            reason: if available {
                "Display endpoint present; test to check the renderer.".into()
            } else {
                format!(
                    "Unavailable: {} is not set in this desktop session.",
                    variable.unwrap()
                )
            },
        }
    })
    .collect()
}

/// Evidence from a desktop process, which cannot prove that its window was usable.
#[derive(Clone, Serialize)]
pub struct SessionOutcome {
    pub kind: String,
    pub summary: String,
    pub graphics: String,
}

/// Separate hard renderer/authentication errors from initialization and harmless warnings.
pub fn session_outcome(success: bool, details: &str) -> SessionOutcome {
    let lower = details.to_ascii_lowercase();
    let (kind, summary) = if lower.contains("window framebuffer support not available")
        || lower.contains("desktop graphics detection failed")
    {
        (
            "renderer",
            "Renderer failed. Try another available video mode; see the graphics details.",
        )
    } else if lower.contains("errconnect_logon_failure")
        || lower.contains("errconnect_authentication_failed")
    {
        (
            "credentials",
            "Server rejected sign-in. Review authentication details.",
        )
    } else if lower.contains("certificate not trusted")
        || lower.contains("remote host identification has changed")
    {
        (
            "certificate",
            "Certificate trust failed. Review the fingerprint and certificate policy.",
        )
    } else if lower.contains("local framebuffer format")
        && lower.contains("remote framebuffer format")
    {
        (
            "initialized",
            "Session initialized. Confirm you saw a usable desktop.",
        )
    } else if success {
        ("closed", "Process closed successfully; no desktop initialization was captured. Confirm visually before marking Works.")
    } else {
        (
            "failed",
            "Desktop process failed. Review the connection log and test authentication separately.",
        )
    };
    let graphics = details
        .lines()
        .filter(|line| line.contains("FreeRDP graphics:"))
        .collect::<Vec<_>>()
        .join("\n");
    let cpu = graphics.contains("/software passed")
        || [
            "llvmpipe",
            "softpipe",
            "swrast",
            "software rasterizer",
            "swiftshader",
        ]
        .iter()
        .any(|name| graphics.to_ascii_lowercase().contains(name));
    SessionOutcome {
        kind: kind.into(),
        summary: summary.into(),
        graphics: if graphics.is_empty() {
            "GPU/driver identity unavailable; OpenGL does not prove hardware acceleration.".into()
        } else if cpu {
            format!("CPU rendering detected: {graphics}")
        } else {
            format!("{graphics}\nRenderer reported by the bundled SDL frame probe; not a performance benchmark.")
        },
    }
}

/// One attempt, including credentials in its copyable command at the operator's request.
#[derive(Clone, Serialize)]
pub struct DiagnosticAttempt {
    pub id: String,
    pub started_ns: u128,
    pub purpose: String,
    pub command: String,
    pub environment: BTreeMap<String, String>,
    pub details: String,
    pub truncated: bool,
    pub exit_code: Option<i32>,
    pub outcome: Option<SessionOutcome>,
    pub authentication: Option<TestResult>,
    pub screenshot: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_mode: Option<VideoMode>,
}

/// Retain the latest 32 attempts in memory, each with at most 1 MiB of output.
#[derive(Clone, Default)]
pub struct DiagnosticHistory(Arc<Mutex<Vec<DiagnosticAttempt>>>);
impl DiagnosticHistory {
    /// Start an attempt before its output reader threads run.
    pub fn start(&self, id: &str, purpose: &str, command: String, details: String) -> u128 {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let mut records = self.0.lock().unwrap();
        if records.len() == 32 {
            records.remove(0);
        }
        records.push(DiagnosticAttempt {
            id: id.into(),
            started_ns: stamp,
            purpose: purpose.into(),
            command,
            environment: display_environment(),
            details,
            truncated: false,
            exit_code: None,
            outcome: None,
            authentication: None,
            screenshot: None,
            batch_id: None,
            video_mode: None,
        });
        stamp
    }
    /// Append complete lines, preserving the most recent evidence when the limit is reached.
    pub fn append(&self, stamp: u128, line: &str) {
        if let Some(record) = self
            .0
            .lock()
            .unwrap()
            .iter_mut()
            .find(|record| record.started_ns == stamp)
        {
            record.details.push_str(line);
            record.details.push('\n');
            if record.details.len() > 1_048_576 {
                let mut start = record.details.len() - 1_048_576;
                while !record.details.is_char_boundary(start) {
                    start += 1;
                }
                record.details.drain(..start);
                record.truncated = true;
            }
        }
    }
    /// Finish after both output readers have drained, ensuring reliable classification.
    pub fn finish(&self, stamp: u128, success: bool, code: Option<i32>) -> SessionOutcome {
        let mut records = self.0.lock().unwrap();
        let details = records
            .iter()
            .find(|record| record.started_ns == stamp)
            .map(|record| record.details.as_str())
            .unwrap_or("");
        let outcome = session_outcome(success, details);
        if let Some(record) = records.iter_mut().find(|record| record.started_ns == stamp) {
            record.exit_code = code;
            record.outcome = Some(outcome.clone());
        }
        outcome
    }
    /// Retain private screenshot metadata alongside a captured desktop attempt.
    pub(crate) fn screenshot(&self, stamp: u128, evidence: serde_json::Value) {
        if let Some(record) = self
            .0
            .lock()
            .unwrap()
            .iter_mut()
            .find(|record| record.started_ns == stamp)
        {
            record.screenshot = Some(evidence);
        }
    }

    /// Keep the exported outcome consistent with deadline and screenshot evidence.
    pub(crate) fn video_outcome(&self, stamp: u128, outcome: SessionOutcome) {
        if let Some(record) = self
            .0
            .lock()
            .unwrap()
            .iter_mut()
            .find(|record| record.started_ns == stamp)
        {
            record.outcome = Some(outcome);
        }
    }

    /// Associate only batch-test attempts with their run and explicit renderer.
    pub(crate) fn batch(&self, stamp: u128, id: &str, mode: Option<VideoMode>) {
        if let Some(record) = self
            .0
            .lock()
            .unwrap()
            .iter_mut()
            .find(|record| record.started_ns == stamp)
        {
            record.batch_id = Some(id.into());
            record.video_mode = mode;
        }
    }

    /// Store an authentication result with its actual transport and exit status.
    pub fn authentication(&self, id: &str, result: &TestResult) -> u128 {
        let stamp = self.start(
            id,
            &format!("authentication ({})", result.transport),
            result.command.clone(),
            result.details.clone(),
        );
        if let Some(record) = self
            .0
            .lock()
            .unwrap()
            .iter_mut()
            .find(|record| record.started_ns == stamp)
        {
            record.exit_code = result.exit_code;
            record.authentication = Some(result.clone());
        }
        stamp
    }
    /// Export only the requested connection to a new private JSON file in your state directory.
    pub fn export(&self, profile: &Profile) -> Result<PathBuf, String> {
        let base = std::env::var_os("XDG_STATE_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
            })
            .ok_or("Cannot locate your state directory.")?;
        if !base.is_absolute() {
            return Err("Your state directory must be absolute.".into());
        }
        let directory = base.join("rdpctl/diagnostics");
        fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
                .map_err(|error| error.to_string())?;
        }
        let records = self.0.lock().unwrap();
        let attempts: Vec<_> = records
            .iter()
            .filter(|record| record.id == profile.id)
            .collect();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let path = directory.join(format!("rdp-{stamp}.json"));
        let data = serde_json::to_vec_pretty(&serde_json::json!({ "version": env!("CARGO_PKG_VERSION"), "profile": profile, "environment": display_environment(), "retention": "Latest 32 attempts in this GUI process; each retains its last 1 MiB. Authentication output retains its last 64 KiB.", "attempts": attempts })).map_err(|error| error.to_string())?;
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&path).map_err(|error| error.to_string())?;
        file.write_all(&data)
            .and_then(|()| file.sync_all())
            .map_err(|error| error.to_string())?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn renderer_failure_beats_framebuffer_initialization_and_gpu_identity_is_honest() {
        let log = "Local framebuffer format BGRA32\nRemote framebuffer format BGRA32\nWindow framebuffer support not available\nFreeRDP graphics: x11/opengl passed frame presentation (llvmpipe LLVM)";
        let outcome = session_outcome(true, log);
        assert_eq!(outcome.kind, "renderer");
        assert!(outcome.graphics.contains("CPU rendering detected"));
        assert_eq!(
            session_outcome(false, "ERRCONNECT_LOGON_FAILURE").kind,
            "credentials"
        );
        assert_eq!(session_outcome(true, "").kind, "closed");
        assert_eq!(
            session_outcome(
                false,
                "Local framebuffer format BGRA32\nRemote framebuffer format BGRA32"
            )
            .kind,
            "initialized"
        );
    }
    #[cfg(unix)]
    #[test]
    fn export_preserves_passwords_and_filters_other_profiles_with_private_permissions() {
        let _guard = crate::PROCESS_TEST_LOCK.lock().unwrap();
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let previous = std::env::var_os("XDG_STATE_HOME");
        std::env::set_var("XDG_STATE_HOME", directory.path());
        let history = DiagnosticHistory::default();
        let profile = Profile {
            id: "test".into(),
            name: "Test".into(),
            host: "example.invalid".into(),
            user: "user".into(),
            fullscreen: false,
            software_rendering: false,
            video_mode: VideoMode::Auto,
            working_video_modes: vec![],
            multi_monitor: false,
            monitors: String::new(),
            certificate: crate::CertificatePolicy::Ignore,
            fingerprint: String::new(),
        };
        let stamp = history.start(
            "test",
            "desktop",
            "client '/p:a visible password'".into(),
            "FreeRDP graphics: x11/opengl passed frame presentation (AMD)".into(),
        );
        history.append(stamp, "Local framebuffer format BGRA32");
        history.append(stamp, "Remote framebuffer format BGRA32");
        history.finish(stamp, true, Some(0));
        history.start("other", "desktop", "unrelated secret".into(), String::new());
        let path = history.export(&profile).unwrap();
        let data = fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&data).unwrap();
        assert_eq!(parsed["attempts"].as_array().unwrap().len(), 1);
        assert!(data.contains("a visible password"));
        assert!(!data.contains("unrelated secret"));
        assert_eq!(parsed["attempts"][0]["exit_code"], 0);
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(!video_availability().is_empty());
        if let Some(previous) = previous {
            std::env::set_var("XDG_STATE_HOME", previous);
        } else {
            std::env::remove_var("XDG_STATE_HOME");
        }
    }

    #[test]
    fn bounded_history_retains_utf8_and_exit_evidence() {
        let history = DiagnosticHistory::default();
        let stamp = history.start("a", "desktop", "command with /p:visible".into(), "".into());
        history.append(stamp, &"é".repeat(600_000));
        history.append(stamp, "ERRCONNECT_LOGON_FAILURE");
        assert_eq!(history.finish(stamp, false, Some(1)).kind, "credentials");
        let records = history.0.lock().unwrap();
        assert!(records[0].truncated);
        assert!(records[0].details.len() <= 1_048_576);
        assert_eq!(records[0].exit_code, Some(1));
    }
}
