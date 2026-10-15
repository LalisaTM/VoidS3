#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use serde::Deserialize;
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};
use voids3_app::{App, BatchControl, DeleteEntry};
use voids3_core::{Credentials, Error, Profile, Result, TransferSettings};

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum Request {
    Profiles,
    SaveProfile {
        profile: Profile,
        credentials: Option<Credentials>,
    },
    DeleteProfile {
        id: String,
    },
    TestConnection {
        id: String,
    },
    Home,
    LocalList {
        path: PathBuf,
        hidden: bool,
    },
    LocalMkdir {
        path: PathBuf,
        name: String,
    },
    LocalRename {
        path: PathBuf,
        name: String,
    },
    LocalDelete {
        path: PathBuf,
    },
    DeleteEntries {
        id: Option<String>,
        entries: Vec<DeleteEntry>,
    },
    RemoteList {
        id: String,
        prefix: String,
        token: Option<String>,
    },
    RemoteMkdir {
        id: String,
        key: String,
    },
    RemoteDelete {
        id: String,
        key: String,
        etag: Option<String>,
    },
    RemoteCopy {
        id: String,
        source: String,
        destination: String,
        etag: String,
    },
    Properties {
        id: String,
        key: String,
    },
    Preview {
        id: String,
        key: String,
    },
    Presign {
        id: String,
        key: String,
        seconds: u64,
    },
    Upload {
        id: String,
        paths: Vec<PathBuf>,
        prefix: String,
    },
    Download {
        id: String,
        keys: Vec<String>,
        directory: PathBuf,
    },
    Transfers,
    TransferControl {
        id: String,
        operation: Control,
    },
    TransferControlAll {
        operation: BatchControl,
    },
    ClearFinished,
    Settings,
    SaveSettings {
        settings: TransferSettings,
    },
    SyncPreview {
        id: String,
        local: PathBuf,
        prefix: String,
        mode: voids3_sync::Mode,
    },
    SyncApply {
        id: String,
        local: PathBuf,
        prefix: String,
        mode: voids3_sync::Mode,
    },
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Control {
    Pause,
    Resume,
    Cancel,
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| Error::Storage(e.to_string()))?
}
async fn execute(app: Arc<App>, request: Request) -> Result<Value> {
    match request {
        Request::Profiles => Ok(json!(blocking(move || app.store.profiles()).await?)),
        Request::SaveProfile {
            profile,
            credentials,
        } => {
            blocking(move || app.store.save_profile(&profile, credentials.as_ref())).await?;
            Ok(Value::Null)
        }
        Request::DeleteProfile { id } => {
            blocking(move || app.delete_profile(&id)).await?;
            Ok(Value::Null)
        }
        Request::TestConnection { id } => {
            app.client(&id).await?.test().await?;
            Ok(Value::Null)
        }
        Request::Home => Ok(json!(voids3_platform::home_dir()?)),
        Request::LocalList { path, hidden } => Ok(json!(
            blocking(move || voids3_platform::list_dir(&path, hidden)).await?
        )),
        Request::LocalMkdir { path, name } => {
            blocking(move || voids3_platform::create_dir(&path, &name)).await?;
            Ok(Value::Null)
        }
        Request::LocalRename { path, name } => {
            blocking(move || voids3_platform::rename(&path, &name)).await?;
            Ok(Value::Null)
        }
        Request::LocalDelete { path } => {
            blocking(move || voids3_platform::recycle(&path)).await?;
            Ok(Value::Null)
        }
        Request::DeleteEntries { id, entries } => {
            Ok(json!(app.delete_entries(id.as_deref(), entries).await?))
        }
        Request::RemoteList { id, prefix, token } => Ok(json!(
            app.client(&id).await?.list(&prefix, token, false).await?
        )),
        Request::RemoteMkdir { id, key } => {
            app.client(&id).await?.folder(&key).await?;
            Ok(Value::Null)
        }
        Request::RemoteDelete { id, key, etag } => {
            app.client(&id).await?.delete(&key, etag.as_deref()).await?;
            Ok(Value::Null)
        }
        Request::RemoteCopy {
            id,
            source,
            destination,
            etag,
        } => {
            app.client(&id)
                .await?
                .copy(&source, &destination, &etag)
                .await?;
            Ok(Value::Null)
        }
        Request::Properties { id, key } => Ok(json!(app.client(&id).await?.head(&key).await?)),
        Request::Preview { id, key } => Ok(json!(app.client(&id).await?.preview_text(&key).await?)),
        Request::Presign { id, key, seconds } => Ok(json!(
            app.client(&id).await?.presign(&key, false, seconds).await?
        )),
        Request::Upload { id, paths, prefix } => Ok(json!(app.upload(&id, paths, &prefix).await?)),
        Request::Download {
            id,
            keys,
            directory,
        } => Ok(json!(app.download(&id, keys, directory).await?)),
        Request::Transfers => Ok(json!(app.engine.list()?)),
        Request::TransferControl { id, operation } => {
            if matches!(operation, Control::Cancel) && app.engine.request_cancel(&id)? {
                return Ok(Value::Null);
            }
            match operation {
                Control::Pause => app.engine.pause(&id)?,
                Control::Resume | Control::Cancel => {
                    let job = app
                        .engine
                        .list()?
                        .into_iter()
                        .find(|j| j.id == id)
                        .ok_or_else(|| Error::Invalid("Transfer not found".into()))?;
                    let client = app.client(&job.profile_id).await?;
                    match operation {
                        Control::Resume => app.engine.resume(client, &id)?,
                        Control::Cancel => app.engine.cancel(client, &id)?,
                        Control::Pause => {}
                    }
                }
            };
            Ok(Value::Null)
        }
        Request::TransferControlAll { operation } => Ok(json!(app.control_all(operation).await?)),
        Request::ClearFinished => {
            app.engine.clear_finished()?;
            Ok(Value::Null)
        }
        Request::Settings => Ok(json!(app.settings()?)),
        Request::SaveSettings { settings } => {
            app.save_settings(settings)?;
            Ok(Value::Null)
        }
        Request::SyncPreview {
            id,
            local,
            prefix,
            mode,
        } => Ok(json!(app.sync_preview(&id, local, &prefix, mode).await?)),
        Request::SyncApply {
            id,
            local,
            prefix,
            mode,
        } => Ok(json!(app.sync_additions(&id, local, &prefix, mode).await?)),
    }
}
#[tauri::command]
async fn dispatch(
    state: tauri::State<'_, Arc<App>>,
    request: Request,
) -> std::result::Result<Value, String> {
    execute(state.inner().clone(), request)
        .await
        .map_err(|e| e.to_string())
}
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Never enable SDK request tracing: signed headers can contain sensitive data.
    tracing_subscriber::fmt()
        .with_env_filter("voids3=info")
        .with_target(false)
        .init();
    let app = Arc::new(App::open()?);
    tauri::Builder::default()
        .manage(app)
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![dispatch])
        .run(tauri::generate_context!())?;
    Ok(())
}
