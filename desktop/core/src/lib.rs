//! Shared profiles and process launching for the desktop connection manager.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

mod connection;
pub use connection::{test_connection, TestResult};

/// How FreeRDP checks a server certificate for this connection.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CertificatePolicy {
    #[default]
    Verify,
    Tofu,
    Fingerprint,
    Ignore,
}

/// A connection using the same password-free format as the Go terminal app.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Profile {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub host: String,
    pub user: String,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default)]
    pub software_rendering: bool,
    #[serde(default)]
    pub multi_monitor: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub monitors: String,
    #[serde(default)]
    pub certificate: CertificatePolicy,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub fingerprint: String,
}

/// Generate the stable filename used by the terminal app.
pub fn slug(value: &str) -> String {
    let mut result = String::new();
    for character in value.to_lowercase().chars() {
        if character.is_alphabetic() || character.is_numeric() {
            result.push(character);
        } else if !result.is_empty() && !result.ends_with('-') {
            result.push('-');
        }
    }
    result.trim_end_matches('-').to_owned()
}

impl Profile {
    /// Normalize user input and reject invalid connection settings.
    pub fn validate(mut self) -> Result<Self, String> {
        self.name = self.name.trim().to_owned();
        self.host = self.host.trim().to_owned();
        self.user = self.user.trim().to_owned();
        self.monitors = self.monitors.trim().to_owned();
        if self.certificate == CertificatePolicy::Fingerprint {
            self.fingerprint = normalize_fingerprint(&self.fingerprint)?;
        } else {
            self.fingerprint.clear();
        }
        if [&self.name, &self.host, &self.user]
            .iter()
            .any(|value| value.is_empty() || value.chars().any(char::is_control))
        {
            return Err(
                "Name, host, and user are required and must not contain control characters.".into(),
            );
        }
        if self.name.len() > 160 || self.host.len() > 255 || self.user.len() > 255 {
            return Err("Name must be at most 160 bytes; host and user at most 255 bytes.".into());
        }
        if !self.monitors.is_empty()
            && !self.monitors.split(',').all(|part| {
                !part.is_empty() && part.chars().all(|character| character.is_ascii_digit())
            })
        {
            return Err("Monitor IDs must be comma-separated numbers, such as 0,1.".into());
        }
        if self.id.is_empty() {
            self.id = slug(&self.name);
        }
        if self.id.is_empty() || slug(&self.id) != self.id {
            return Err(
                "The connection ID must contain letters or numbers separated by dashes.".into(),
            );
        }
        Ok(self)
    }

    /// Build separate FreeRDP arguments without invoking a shell.
    pub fn arguments(&self) -> Vec<String> {
        let mut arguments = vec![format!("/v:{}", self.host), format!("/u:{}", self.user)];
        if self.multi_monitor {
            arguments.push("/multimon".into());
        }
        if !self.monitors.is_empty() {
            arguments.push(format!("/monitors:{}", self.monitors));
        }
        if self.fullscreen {
            arguments.push("/f".into());
        }
        arguments.push(match self.certificate {
            CertificatePolicy::Verify => "/cert:deny".into(),
            CertificatePolicy::Tofu => "/cert:tofu".into(),
            CertificatePolicy::Ignore => "/cert:ignore".into(),
            CertificatePolicy::Fingerprint => {
                format!("/cert:fingerprint:sha256:{}", self.fingerprint)
            }
        });
        arguments
    }
}

/// A profile store rooted in the user's XDG configuration directory.
pub struct Store {
    directory: PathBuf,
}

impl Store {
    /// Open a store without creating or changing files.
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    /// Resolve the existing terminal app's configuration directory.
    pub fn from_environment() -> Result<Self, String> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .ok_or("Cannot locate your home or configuration directory.")?;
        if !base.is_absolute() {
            return Err("Your configuration directory must be an absolute path.".into());
        }
        Ok(Self::new(base.join("rdpctl/connections")))
    }

    /// Read existing profiles in display-name order.
    pub fn list(&self) -> Result<Vec<Profile>, String> {
        let entries = match fs::read_dir(&self.directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(error) => return Err(format!("Cannot read connection profiles: {error}")),
        };
        let mut profiles = Vec::new();
        for entry in entries {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
                && path.is_file()
            {
                let profile = self.read(&path)?;
                if path.file_stem().and_then(|stem| stem.to_str()) != Some(&profile.id) {
                    return Err(format!(
                        "Profile ID does not match its filename: {}",
                        path.display()
                    ));
                }
                profiles.push(profile);
            }
        }
        profiles.sort_by_key(|profile| profile.name.to_lowercase());
        Ok(profiles)
    }

    fn read(&self, path: &Path) -> Result<Profile, String> {
        let contents = fs::read(path).map_err(|error| format!("Cannot read profile: {error}"))?;
        let profile: Profile = serde_json::from_slice(&contents)
            .map_err(|error| format!("Cannot parse profile {}: {error}", path.display()))?;
        profile.validate()
    }

    /// Load a validated profile without accepting paths from the frontend.
    pub fn load(&self, id: &str) -> Result<Profile, String> {
        if id.is_empty() || slug(id) != id {
            return Err("Invalid connection ID.".into());
        }
        let profile = self.read(&self.directory.join(format!("{id}.json")))?;
        if profile.id != id {
            return Err("Profile ID does not match its filename.".into());
        }
        Ok(profile)
    }

    /// Atomically create a private profile, preserving any existing connection.
    pub fn create(&self, profile: Profile) -> Result<Profile, String> {
        let profile = profile.validate()?;
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(&self.directory)
            .map_err(|error| format!("Cannot create profile directory: {error}"))?;
        let mut temporary = tempfile::NamedTempFile::new_in(&self.directory)
            .map_err(|error| format!("Cannot create profile: {error}"))?;
        serde_json::to_writer_pretty(&mut temporary, &profile)
            .map_err(|error| error.to_string())?;
        temporary
            .write_all(b"\n")
            .map_err(|error| error.to_string())?;
        temporary
            .persist_noclobber(self.directory.join(format!("{}.json", profile.id)))
            .map_err(|error| {
                format!(
                    "Cannot save connection; an existing profile will not be overwritten: {error}"
                )
            })?;
        Ok(profile)
    }

    /// Update an existing profile while retaining its stable ID.
    pub fn update(&self, profile: Profile) -> Result<Profile, String> {
        let profile = profile.validate()?;
        self.load(&profile.id)?;
        let mut temporary = tempfile::NamedTempFile::new_in(&self.directory)
            .map_err(|error| format!("Cannot create profile: {error}"))?;
        serde_json::to_writer_pretty(&mut temporary, &profile)
            .map_err(|error| error.to_string())?;
        temporary
            .write_all(b"\n")
            .map_err(|error| error.to_string())?;
        temporary
            .persist(self.directory.join(format!("{}.json", profile.id)))
            .map_err(|error| format!("Cannot update connection: {error}"))?;
        Ok(profile)
    }

    fn password_path(&self, id: &str) -> Result<PathBuf, String> {
        self.load(id)?;
        Ok(self.directory.join("passwords").join(id))
    }

    /// Return a locally saved password without including it in profile JSON.
    pub fn password(&self, id: &str) -> Result<Option<String>, String> {
        match fs::read_to_string(self.password_path(id)?) {
            Ok(password) => Ok(Some(password)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(format!("Cannot read saved password: {error}")),
        }
    }

    /// Report saved-password presence without returning the secret to the webview.
    pub fn has_password(&self, id: &str) -> Result<bool, String> {
        Ok(self.password_path(id)?.is_file())
    }

    /// Save a plaintext password with private directory and file permissions.
    pub fn save_password(&self, id: &str, password: &str) -> Result<(), String> {
        connection::validate_password(password)?;
        let path = self.password_path(id)?;
        let directory = path.parent().ok_or("Cannot locate password directory.")?;
        fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
                .map_err(|error| error.to_string())?;
        }
        let mut temporary =
            tempfile::NamedTempFile::new_in(directory).map_err(|error| error.to_string())?;
        temporary
            .write_all(password.as_bytes())
            .map_err(|error| error.to_string())?;
        temporary.persist(path).map_err(|error| error.to_string())?;
        Ok(())
    }

    /// Remove only the password belonging to an existing connection.
    pub fn forget_password(&self, id: &str) -> Result<(), String> {
        match fs::remove_file(self.password_path(id)?) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("Cannot forget password: {error}")),
        }
    }

    /// Back up this host's remembered certificate so TOFU can accept its replacement.
    pub fn forget_certificate(&self, id: &str) -> Result<bool, String> {
        let profile = self.load(id)?;
        let config = self
            .directory
            .parent()
            .and_then(Path::parent)
            .ok_or("Cannot locate certificate directory.")?;
        let filename = connection::certificate_filename(&profile.host)?;
        let path = config.join("freerdp/server").join(filename);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.to_string()),
        };
        if !metadata.is_file() {
            return Err("The saved certificate is not a regular file.".into());
        }
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        fs::rename(&path, path.with_extension(format!("pem.previous-{stamp}")))
            .map_err(|error| format!("Cannot back up saved certificate: {error}"))?;
        Ok(true)
    }
}

/// Start the selected profile through an explicit FreeRDP wrapper path.
pub fn launch(
    profile: Profile,
    client: &Path,
    password: Option<&str>,
) -> Result<(Child, String), String> {
    let profile = profile.validate()?;
    let mut command = Command::new(client);
    if profile.software_rendering {
        if std::env::var_os("DISPLAY").is_none_or(|value| value.is_empty()) {
            return Err("X11 software rendering requires DISPLAY. Launch from an X11 desktop or a Wayland desktop with XWayland enabled.".into());
        }
        command
            .env("SDL_VIDEODRIVER", "x11")
            .env("SDL_RENDER_DRIVER", "software")
            .env("SDL_FRAMEBUFFER_ACCELERATION", "0");
    }
    command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    connection::spawn(&mut command, profile.arguments(), password)
}

/// Accept a SHA-256 fingerprint in compact or colon-separated form.
pub fn normalize_fingerprint(value: &str) -> Result<String, String> {
    let value = value.trim().replace(':', "").to_ascii_lowercase();
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("A SHA-256 fingerprint must contain 64 hexadecimal digits.".into());
    }
    Ok(value)
}

/// A display ID reported by the actual SDL FreeRDP client.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Monitor {
    pub id: u32,
    pub label: String,
}

/// Parse display IDs without assuming that SDL starts numbering at zero.
pub fn parse_monitors(output: &str) -> Result<Vec<Monitor>, String> {
    let mut monitors = Vec::new();
    for line in output.lines() {
        let line = line.trim().trim_start_matches('*').trim();
        let Some(rest) = line.strip_prefix('[') else {
            continue;
        };
        let Some((id, description)) = rest.split_once(']') else {
            continue;
        };
        let Ok(id) = id.parse::<u32>() else { continue };
        if description.trim().starts_with('[') {
            monitors.push(Monitor {
                id,
                label: format!("{id} — {}", description.trim()),
            });
        }
    }
    if monitors.is_empty() {
        return Err(
            "FreeRDP did not report any monitors. Check that your desktop display is available."
                .into(),
        );
    }
    Ok(monitors)
}

/// Ask the installed client for its current Wayland or X11 displays.
pub fn list_monitors(client: &Path) -> Result<Vec<Monitor>, String> {
    let output = Command::new(client)
        .arg("/list:monitor")
        .output()
        .map_err(|error| format!("Cannot enumerate monitors at {}: {error}", client.display()))?;
    // FreeRDP 3.30 returns 255 after successful informational commands.
    if !output.status.success() && output.status.code() != Some(255) {
        return Err(format!(
            "FreeRDP monitor enumeration failed ({}).",
            output.status
        ));
    }
    parse_monitors(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> Profile {
        Profile {
            id: String::new(),
            name: "Windows lab".into(),
            host: "windows.example.invalid".into(),
            user: "DOMAIN\\user".into(),
            fullscreen: true,
            software_rendering: false,
            multi_monitor: true,
            monitors: "0,1".into(),
            certificate: CertificatePolicy::Verify,
            fingerprint: String::new(),
        }
    }

    #[test]
    fn uses_actual_sdl_monitor_ids_and_rejects_empty_output() {
        let monitors = parse_monitors("listing 2 monitors:\n     * [1] [screen] 1280x1024\t+0+0\n       [3] [Display two] 1920x1080\t+1280+0\n").unwrap();
        assert_eq!(
            monitors
                .iter()
                .map(|monitor| monitor.id)
                .collect::<Vec<_>>(),
            vec![1, 3]
        );
        assert!(parse_monitors("[ERROR] cannot open display").is_err());
    }

    #[test]
    fn shares_go_json_and_never_overwrites_a_connection() {
        let temporary = tempfile::tempdir().unwrap();
        let store = Store::new(temporary.path().join("connections"));
        let saved = store.create(profile()).unwrap();
        assert_eq!(saved.id, "windows-lab");
        assert_eq!(store.list().unwrap(), vec![saved.clone()]);
        assert_eq!(store.load(&saved.id).unwrap(), saved);
        assert!(store.create(profile()).is_err());
        let contents =
            fs::read_to_string(temporary.path().join("connections/windows-lab.json")).unwrap();
        assert!(contents.contains("\"multi_monitor\": true"));
        assert!(!contents.contains("password"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(temporary.path().join("connections/windows-lab.json"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn saves_replaces_and_forgets_private_plaintext_passwords() {
        let temporary = tempfile::tempdir().unwrap();
        let store = Store::new(temporary.path().join("rdpctl/connections"));
        let saved = store.create(profile()).unwrap();
        assert!(!store.has_password(&saved.id).unwrap());
        store.save_password(&saved.id, "one ' secret").unwrap();
        assert_eq!(
            store.password(&saved.id).unwrap().as_deref(),
            Some("one ' secret")
        );
        let path = temporary
            .path()
            .join("rdpctl/connections/passwords/windows-lab");
        assert_eq!(fs::read_to_string(&path).unwrap(), "one ' secret");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(path.parent().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
        store.save_password(&saved.id, "replacement").unwrap();
        assert_eq!(
            store.password(&saved.id).unwrap().as_deref(),
            Some("replacement")
        );
        assert!(store.save_password("unknown", "secret").is_err());
        assert!(store
            .save_password(&saved.id, "secret\n/p:injected")
            .is_err());
        assert!(
            !fs::read_to_string(temporary.path().join("rdpctl/connections/windows-lab.json"))
                .unwrap()
                .contains("replacement")
        );
        store.forget_password(&saved.id).unwrap();
        assert_eq!(store.password(&saved.id).unwrap(), None);
        store.forget_password(&saved.id).unwrap();
    }

    #[test]
    fn persists_certificate_policy_and_backs_up_only_the_selected_host() {
        let temporary = tempfile::tempdir().unwrap();
        let store = Store::new(temporary.path().join("rdpctl/connections"));
        let mut saved = store.create(profile()).unwrap();
        saved.certificate = CertificatePolicy::Fingerprint;
        saved.fingerprint = "AB:".repeat(31) + "AB";
        store.update(saved.clone()).unwrap();
        assert_eq!(
            store.load(&saved.id).unwrap().arguments().last().unwrap(),
            &format!("/cert:fingerprint:sha256:{}", "ab".repeat(32))
        );
        let certificates = temporary.path().join("freerdp/server");
        fs::create_dir_all(&certificates).unwrap();
        let selected = certificates.join("windows.example.invalid_3389.pem");
        let unrelated = certificates.join("other_3389.pem");
        fs::write(&selected, "original certificate").unwrap();
        fs::write(&unrelated, "unrelated certificate").unwrap();
        assert!(store.forget_certificate(&saved.id).unwrap());
        assert!(!selected.exists());
        assert_eq!(
            fs::read_to_string(&unrelated).unwrap(),
            "unrelated certificate"
        );
        let backup = fs::read_dir(&certificates)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| path.to_string_lossy().contains(".pem.previous-"))
            .unwrap();
        assert_eq!(fs::read_to_string(backup).unwrap(), "original certificate");
        assert!(!store.forget_certificate(&saved.id).unwrap());
    }

    #[test]
    fn accepts_existing_go_profile_without_monitors() {
        let temporary = tempfile::tempdir().unwrap();
        fs::write(temporary.path().join("existing.json"), r#"{"id":"existing","name":"Existing","host":"host.example.invalid","user":"user","fullscreen":false,"multi_monitor":false}"#).unwrap();
        let store = Store::new(temporary.path().to_owned());
        assert_eq!(store.load("existing").unwrap().monitors, "");
        assert_eq!(store.list().unwrap().len(), 1);
    }

    #[test]
    fn rejects_paths_and_malformed_monitor_ids() {
        let store = Store::new(PathBuf::from("/unused"));
        assert!(store.load("../../outside").is_err());
        let mut invalid = profile();
        invalid.monitors = "0,/p:secret".into();
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn keeps_user_values_in_separate_arguments() {
        let mut connection = profile();
        connection.host = "host; touch /tmp/unwanted".into();
        assert_eq!(
            connection.arguments(),
            vec![
                "/v:host; touch /tmp/unwanted",
                "/u:DOMAIN\\user",
                "/multimon",
                "/monitors:0,1",
                "/f",
                "/cert:deny"
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn launches_a_real_process_with_the_saved_arguments() {
        use std::os::unix::fs::PermissionsExt;
        let temporary = tempfile::tempdir().unwrap();
        let client = temporary.path().join("client");
        let output = temporary.path().join("arguments");
        fs::write(
            &client,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\n",
                output.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&client, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(launch(profile(), &client, None)
            .unwrap()
            .0
            .wait()
            .unwrap()
            .success());
        assert_eq!(
            fs::read_to_string(output).unwrap(),
            "/v:windows.example.invalid\n/u:DOMAIN\\user\n/multimon\n/monitors:0,1\n/f\n/cert:deny\n"
        );
    }
}
