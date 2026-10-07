#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use rdpctl_core::{Profile, Store};
use serde::Serialize;
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
async fn list_monitors() -> Result<Vec<rdpctl_core::Monitor>, String> {
    let client = client_path()?;
    tauri::async_runtime::spawn_blocking(move || rdpctl_core::list_monitors(&client))
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

#[derive(Clone, Serialize)]
struct SessionEnded {
    name: String,
    pid: u32,
    success: bool,
    code: Option<i32>,
}

#[tauri::command]
fn launch_profile(app: tauri::AppHandle, id: String) -> Result<u32, String> {
    let profile = Store::from_environment()?.load(&id)?;
    // Only the local environment can override the client; the webview supplies an ID.
    let client = client_path()?;
    let name = profile.name.clone();
    let mut child = rdpctl_core::launch(profile, &client)?;
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
    Ok(pid)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            list_profiles,
            create_profile,
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
