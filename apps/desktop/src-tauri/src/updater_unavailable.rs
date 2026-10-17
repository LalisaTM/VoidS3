//! Only Windows x64 installers currently have a published updater channel.
#[derive(Default)]
pub struct Updates {}

#[tauri::command]
pub async fn check_for_updates() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({ "supported": false, "update": null }))
}

#[tauri::command]
pub async fn install_update() -> Result<(), String> {
    Err("Self-updating is currently available for Windows x64 installers only".into())
}
