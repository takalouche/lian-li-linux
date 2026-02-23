//! Tauri command handlers — all delegate to the daemon via IPC.

use crate::ipc_client;
use lianli_shared::config::{AppConfig, LcdConfig};
use lianli_shared::fan::FanConfig;
use lianli_shared::ipc::{DeviceInfo, IpcRequest, TelemetrySnapshot};

#[tauri::command]
pub fn connect_daemon() -> Result<bool, String> {
    Ok(ipc_client::is_daemon_running())
}

#[tauri::command]
pub fn get_socket_path() -> String {
    ipc_client::SOCKET_PATH.clone()
}

#[tauri::command]
pub fn list_devices() -> Result<Vec<DeviceInfo>, String> {
    let resp = ipc_client::send_request(&IpcRequest::ListDevices)?;
    ipc_client::unwrap_response(resp)
}

#[tauri::command]
pub fn get_telemetry() -> Result<TelemetrySnapshot, String> {
    let resp = ipc_client::send_request(&IpcRequest::GetTelemetry)?;
    ipc_client::unwrap_response(resp)
}

#[tauri::command]
pub fn get_config() -> Result<AppConfig, String> {
    let resp = ipc_client::send_request(&IpcRequest::GetConfig)?;
    ipc_client::unwrap_response(resp)
}

#[tauri::command]
pub fn set_config(config: AppConfig) -> Result<(), String> {
    let resp = ipc_client::send_request(&IpcRequest::SetConfig { config })?;
    ipc_client::unwrap_response::<serde_json::Value>(resp)?;
    Ok(())
}

#[tauri::command]
pub fn set_lcd_media(device_id: String, config: LcdConfig) -> Result<(), String> {
    let resp = ipc_client::send_request(&IpcRequest::SetLcdMedia { device_id, config })?;
    ipc_client::unwrap_response::<serde_json::Value>(resp)?;
    Ok(())
}

#[tauri::command]
pub fn set_fan_config(config: FanConfig) -> Result<(), String> {
    let resp = ipc_client::send_request(&IpcRequest::SetFanConfig { config })?;
    ipc_client::unwrap_response::<serde_json::Value>(resp)?;
    Ok(())
}

#[tauri::command]
pub async fn pick_media_file(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let file = app
        .dialog()
        .file()
        .add_filter(
            "Media Files",
            &["jpg", "jpeg", "png", "bmp", "gif", "mp4", "avi", "mkv", "webm"],
        )
        .add_filter("Images", &["jpg", "jpeg", "png", "bmp"])
        .add_filter("Videos", &["mp4", "avi", "mkv", "webm", "gif"])
        .blocking_pick_file();

    Ok(file.and_then(|f| f.as_path().map(|p| p.to_string_lossy().to_string())))
}
