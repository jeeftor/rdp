#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use rdpctl_core::{Profile, Store};
use serde::Serialize;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
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
async fn list_monitors(
    software_rendering: Option<bool>,
) -> Result<Vec<rdpctl_core::Monitor>, String> {
    let client = client_path()?;
    tauri::async_runtime::spawn_blocking(move || {
        rdpctl_core::list_monitors(&client, software_rendering.unwrap_or(false))
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
fn password_saved(id: String) -> Result<bool, String> {
    Store::from_environment()?.has_password(&id)
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
async fn test_profile(
    id: String,
    password: Option<String>,
) -> Result<rdpctl_core::TestResult, String> {
    let store = Store::from_environment()?;
    let profile = store.load(&id)?;
    let password = match password {
        Some(password) => Some(password),
        None => store.password(&id)?,
    }
    .ok_or("Enter a password or save one before testing.")?;
    let client = client_path()?;
    tauri::async_runtime::spawn_blocking(move || {
        rdpctl_core::test_connection(profile, &client, &password)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[derive(Serialize)]
struct LaunchResult {
    pid: u32,
    command: String,
}

#[derive(Clone, Serialize)]
struct SessionLog {
    id: String,
    line: String,
}

fn relay_log(app: tauri::AppHandle, id: String, output: impl Read + Send + 'static) {
    std::thread::spawn(move || {
        for line in BufReader::new(output).lines() {
            let Ok(line) = line else { break };
            eprintln!("{line}");
            let _ = app.emit(
                "session-log",
                SessionLog {
                    id: id.clone(),
                    line,
                },
            );
        }
    });
}

#[derive(Clone, Serialize)]
struct SessionEnded {
    name: String,
    pid: u32,
    success: bool,
    code: Option<i32>,
}

#[tauri::command]
fn launch_profile(
    app: tauri::AppHandle,
    id: String,
    password: Option<String>,
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
    let (mut child, command) = rdpctl_core::launch(profile, &client, password.as_deref())?;
    if let Some(output) = child.stdout.take() {
        relay_log(app.clone(), id.clone(), output);
    }
    if let Some(output) = child.stderr.take() {
        relay_log(app.clone(), id, output);
    }
    let pid = child.id();
    std::thread::spawn(move || {
        let status = child.wait();
        let event = SessionEnded {
            name,
            pid,
            success: status.as_ref().is_ok_and(|status| status.success()),
            code: status.ok().and_then(|status| status.code()),
        };
        let _ = app.emit("session-ended", event);
    });
    Ok(LaunchResult { pid, command })
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
        .invoke_handler(tauri::generate_handler![
            list_profiles,
            create_profile,
            update_profile,
            password_saved,
            save_password,
            forget_password,
            forget_certificate,
            test_profile,
            launch_profile,
            list_monitors
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
