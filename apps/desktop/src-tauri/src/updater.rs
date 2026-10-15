//! Only Rust can select an update or launch its verified installer.
use serde::Serialize;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tauri::{AppHandle, State, ipc::Channel};
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio::sync::Mutex;
use voids3_app::App;

#[derive(Default)]
pub struct Updates(Mutex<Option<Update>>);

#[derive(Serialize)]
pub struct UpdateInfo {
    version: String,
    notes: String,
}
#[derive(Serialize)]
pub struct CheckResult {
    supported: bool,
    update: Option<UpdateInfo>,
}
#[derive(Clone, Serialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub enum Progress {
    Downloading { downloaded: u64, total: Option<u64> },
    Verifying,
    Installing,
}

fn supported() -> bool {
    cfg!(all(target_os = "windows", target_arch = "x86_64"))
}
fn allowed_download(update: &Update) -> bool {
    let url = &update.download_url;
    url.scheme() == "https"
        && url.host_str() == Some("github.com")
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && url
            .path()
            .to_ascii_lowercase()
            .starts_with("/lalisatm/voids3/releases/download/")
        && url.path().ends_with("-setup.exe")
}

#[tauri::command]
pub async fn check_for_updates(
    handle: AppHandle,
    updates: State<'_, Updates>,
) -> Result<CheckResult, String> {
    if !supported() {
        return Ok(CheckResult {
            supported: false,
            update: None,
        });
    }
    let mut pending = updates
        .0
        .try_lock()
        .map_err(|_| "An update operation is already in progress")?;
    let mut candidate = handle
        .updater_builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|error| error.to_string())?
        .check()
        .await
        .map_err(|error| format!("Cannot check GitHub Releases: {error}"))?;
    if candidate
        .as_ref()
        .is_some_and(|update| !allowed_download(update))
    {
        return Err("Release contains an unsupported installer URL".into());
    }
    if let Some(update) = &mut candidate {
        update.timeout = Some(Duration::from_secs(300));
    }
    let update = candidate.as_ref().map(|update| UpdateInfo {
        version: update.version.clone(),
        notes: update.body.clone().unwrap_or_default(),
    });
    *pending = candidate;
    Ok(CheckResult {
        supported: true,
        update,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn updater() -> tauri_plugin_updater::Updater {
        updater_with_key(include_str!("../tests/fixtures/test.pub").trim())
    }
    fn updater_with_key(pubkey: &str) -> tauri_plugin_updater::Updater {
        let mut context = tauri::test::mock_context(tauri::test::noop_assets());
        context.config_mut().plugins.0.insert(
            "updater".into(),
            json!({
                "pubkey": pubkey,
                "endpoints": ["https://example.invalid/latest.json"],
                "requireSignedVersion": true,
            }),
        );
        let app = tauri::test::mock_builder()
            .plugin(tauri_plugin_updater::Builder::new().build())
            .build(context)
            .unwrap();
        app.updater_builder()
            .target("windows-x86_64")
            .build()
            .unwrap()
    }
    fn manifest(version: &str) -> serde_json::Value {
        json!({"version": version, "platforms": {"windows-x86_64": {
            "url": "https://github.com/LalisaTM/VoidS3/releases/download/v0.2.0/VoidS3_0.2.0_x64-setup.exe",
            "signature": include_str!("../tests/fixtures/update.txt.sig").trim(),
        }}})
    }
    #[test]
    fn verifies_package_and_signed_version_and_refuses_tampering() {
        let updater = updater();
        let update = updater.restore_update(manifest("0.2.0")).unwrap().unwrap();
        let bytes = include_bytes!("../tests/fixtures/update.txt");
        update.verify(bytes).unwrap();
        assert!(update.verify(b"tampered package").is_err());
        let inflated = updater.restore_update(manifest("99.0.0")).unwrap().unwrap();
        assert!(inflated.verify(bytes).is_err());
        assert!(updater.restore_update(manifest("0.0.1")).unwrap().is_none());
        assert!(updater.restore_update(manifest("0.1.0")).unwrap().is_none());
    }
    #[test]
    fn limits_installers_to_this_repositorys_https_release_assets() {
        let mut update = updater()
            .restore_update(manifest("0.2.0"))
            .unwrap()
            .unwrap();
        assert!(allowed_download(&update));
        for url in [
            "http://github.com/LalisaTM/VoidS3/releases/download/v0.2.0/app-setup.exe",
            "https://github.com/other/repo/releases/download/v0.2.0/app-setup.exe",
            "https://example.invalid/app-setup.exe",
            "https://github.com/LalisaTM/VoidS3/releases/download/v0.2.0/app.zip",
            "https://github.com:444/LalisaTM/VoidS3/releases/download/v0.2.0/app-setup.exe",
        ] {
            update.download_url = url.parse().unwrap();
            assert!(!allowed_download(&update), "{url}");
        }
    }
    #[test]
    #[ignore = "Requires explicitly supplied signed release files; never installs or launches anything"]
    fn published_package_matches_embedded_key() {
        let manifest_path =
            std::env::var("VOIDS3_UPDATE_MANIFEST").expect("Set VOIDS3_UPDATE_MANIFEST");
        let package_path =
            std::env::var("VOIDS3_UPDATE_PACKAGE").expect("Set VOIDS3_UPDATE_PACKAGE");
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let updater = updater_with_key(config["plugins"]["updater"]["pubkey"].as_str().unwrap());
        let manifest = serde_json::from_slice(&std::fs::read(manifest_path).unwrap()).unwrap();
        let update = updater
            .restore_update(manifest)
            .unwrap()
            .expect("Release must be newer than the isolated 0.1.0 test context");
        assert!(allowed_download(&update));
        let bytes = std::fs::read(package_path).unwrap();
        update.verify(&bytes).unwrap();
        let mut corrupted = bytes.clone();
        corrupted[0] ^= 1;
        assert!(update.verify(&corrupted).is_err());
        let mut changed_version = update.raw_json.clone();
        changed_version["version"] = json!("99.0.0");
        assert!(
            updater
                .restore_update(changed_version)
                .unwrap()
                .unwrap()
                .verify(&bytes)
                .is_err()
        );
    }
}

#[tauri::command]
pub async fn install_update(
    version: String,
    on_event: Channel<Progress>,
    updates: State<'_, Updates>,
    app: State<'_, Arc<App>>,
) -> Result<(), String> {
    if !supported() {
        return Err("Self-updating is currently available for Windows x64 installers only".into());
    }
    let pending = updates
        .0
        .try_lock()
        .map_err(|_| "An update operation is already in progress")?;
    let update = pending
        .as_ref()
        .filter(|update| update.version == version)
        .ok_or("Check for this update again before installing")?
        .clone();
    let maintenance = app
        .engine
        .begin_maintenance()
        .map_err(|error| error.to_string())?;
    let mut downloaded = 0_u64;
    let mut last_event = Instant::now();
    let finished = on_event.clone();
    let _ = on_event.send(Progress::Downloading {
        downloaded,
        total: None,
    });
    let bytes = update
        .download(
            |chunk, total| {
                downloaded = downloaded.saturating_add(chunk as u64);
                if last_event.elapsed() >= Duration::from_millis(100) || total == Some(downloaded) {
                    let _ = on_event.send(Progress::Downloading { downloaded, total });
                    last_event = Instant::now();
                }
            },
            move || {
                let _ = finished.send(Progress::Verifying);
            },
        )
        .await
        .map_err(|error| format!("Update download or signature verification failed: {error}"))?;
    let _ = on_event.send(Progress::Installing);
    // Windows exits after launching the installer; the OS releases the SQLite
    // instance lock before the installer relaunches the new application.
    tokio::task::spawn_blocking(move || {
        let _maintenance = maintenance;
        update
            .install(bytes)
            .map_err(|error| format!("Cannot install update: {error}"))
    })
    .await
    .map_err(|error| format!("Update worker failed: {error}"))?
}
