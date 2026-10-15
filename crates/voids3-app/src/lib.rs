//! Application use cases shared across Tauri IPC and CLI.
use std::{path::PathBuf, sync::Arc};
use voids3_core::{Error, Result, TransferSettings};
use voids3_s3::S3;
use voids3_storage::Store;
use voids3_transfer::{Direction, Engine};

pub struct App {
    pub store: Arc<Store>,
    pub engine: Arc<Engine>,
    _instance_lock: std::fs::File,
}
impl App {
    pub fn open() -> Result<Self> {
        let path = voids3_platform::data_dir()?.join("instance.lock");
        let instance_lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        instance_lock.try_lock().map_err(|_| Error::Conflict("Another VoidS3 desktop or CLI instance is using this state. Close it before continuing.".into()))?;
        let store = Arc::new(Store::default_location()?);
        let settings = store.get("settings", "transfers")?.unwrap_or_default();
        let engine = Engine::new(store.clone(), settings)?;
        Ok(Self {
            store,
            engine,
            _instance_lock: instance_lock,
        })
    }
    pub fn settings(&self) -> Result<TransferSettings> {
        Ok(self.store.get("settings", "transfers")?.unwrap_or_default())
    }
    pub fn delete_profile(&self, id: &str) -> Result<()> {
        if self
            .engine
            .list()?
            .iter()
            .any(|j| j.profile_id == id && (!j.status.terminal() || j.upload_id.is_some()))
        {
            return Err(Error::Conflict("Finish or cancel this connection's transfers, including multipart cleanup, before deleting its credentials.".into()));
        }
        self.store.delete_profile(id)
    }
    pub fn save_settings(&self, settings: TransferSettings) -> Result<()> {
        settings.validate()?;
        self.store.put("settings", "transfers", &settings)
    }
    pub async fn client(&self, id: &str) -> Result<S3> {
        let store = self.store.clone();
        let id = id.to_owned();
        let (profile, credentials, settings) = tokio::task::spawn_blocking(move || {
            Ok::<_, Error>((
                store.profile(&id)?,
                store.credentials(&id)?,
                store
                    .get::<TransferSettings>("settings", "transfers")?
                    .unwrap_or_default(),
            ))
        })
        .await
        .map_err(|e| Error::Storage(e.to_string()))??;
        S3::new(profile, credentials, settings.retries)
    }
    pub async fn upload(&self, id: &str, paths: Vec<PathBuf>, prefix: &str) -> Result<Vec<String>> {
        let client = self.client(id).await?;
        client.profile.writable()?;
        let settings = self.settings()?;
        let mut pending = Vec::new();
        for path in paths {
            let meta = tokio::fs::symlink_metadata(&path).await?;
            if meta.file_type().is_symlink() {
                return Err(Error::Invalid(
                    "Symbolic link uploads must be selected by their actual path".into(),
                ));
            }
            let filename = path
                .file_name()
                .ok_or_else(|| Error::Invalid("Cannot upload a filesystem root".into()))?
                .to_string_lossy()
                .to_string();
            if meta.is_dir() {
                let root = path.clone();
                let entries =
                    tokio::task::spawn_blocking(move || voids3_platform::recursive_files(&root))
                        .await
                        .map_err(|e| Error::Storage(e.to_string()))??;
                for entry in entries {
                    pending.push((
                        PathBuf::from(entry.path),
                        format!("{prefix}{filename}/{}", entry.name),
                    ));
                }
            } else {
                pending.push((path, format!("{prefix}{filename}")));
            }
        }
        if pending.len() + self.engine.list()?.len() > 100_000 {
            return Err(Error::Invalid("Queue limit is 100,000 files. Clear completed history or choose a smaller selection".into()));
        }
        let mut ids = Vec::new();
        for (path, key) in pending {
            ids.push(
                self.engine
                    .enqueue(
                        client.clone(),
                        Direction::Upload,
                        path,
                        key,
                        settings.clone(),
                    )
                    .await?,
            );
        }
        Ok(ids)
    }
    pub async fn download(
        &self,
        id: &str,
        keys: Vec<String>,
        directory: PathBuf,
    ) -> Result<Vec<String>> {
        let client = self.client(id).await?;
        let settings = self.settings()?;
        let mut pending = Vec::new();
        for key in keys {
            if key.ends_with('/') {
                return Err(Error::Invalid(
                    "Select files. Use sync preview for recursive directory downloads".into(),
                ));
            }
            let name = key.rsplit('/').next().unwrap_or_default();
            pending.push((
                key.clone(),
                voids3_platform::download_target(&directory, name)?,
            ));
        }
        let mut ids = Vec::new();
        for (key, path) in pending {
            ids.push(
                self.engine
                    .enqueue(
                        client.clone(),
                        Direction::Download,
                        path,
                        key,
                        settings.clone(),
                    )
                    .await?,
            );
        }
        Ok(ids)
    }
    pub async fn sync_preview(
        &self,
        id: &str,
        local: PathBuf,
        prefix: &str,
        mode: voids3_sync::Mode,
    ) -> Result<Vec<voids3_sync::Change>> {
        let client = self.client(id).await?;
        let local = tokio::task::spawn_blocking(move || voids3_platform::recursive_files(&local));
        let remote = client.scan(prefix).await?;
        let local = local.await.map_err(|e| Error::Storage(e.to_string()))??;
        voids3_sync::preview(&local, &remote, mode)
    }
    pub async fn sync_additions(
        &self,
        id: &str,
        local: PathBuf,
        prefix: &str,
        mode: voids3_sync::Mode,
    ) -> Result<Vec<String>> {
        // Always re-scan at apply time. Engine uses conditional remote writes and no-clobber local writes.
        let changes = self.sync_preview(id, local.clone(), prefix, mode).await?;
        let client = self.client(id).await?;
        let settings = self.settings()?;
        let mut ids = Vec::new();
        for change in changes {
            let direction = match change.action {
                voids3_sync::Action::Upload => Direction::Upload,
                voids3_sync::Action::Download => Direction::Download,
                _ => continue,
            };
            let path = voids3_platform::download_target(&local, &change.key)?;
            if direction == Direction::Download
                && let Some(parent) = path.parent()
            {
                tokio::fs::create_dir_all(parent).await?;
            }
            ids.push(
                self.engine
                    .enqueue(
                        client.clone(),
                        direction,
                        path,
                        format!("{prefix}{}", change.key),
                        settings.clone(),
                    )
                    .await?,
            );
        }
        Ok(ids)
    }
}
