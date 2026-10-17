//! Application use cases shared across Tauri IPC and CLI.
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};
use voids3_core::{Error, Result, TransferSettings};
use voids3_s3::S3;
use voids3_storage::Store;
use voids3_transfer::{Direction, Engine, Status};

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchControl {
    Pause,
    Resume,
    Cancel,
}

#[derive(Default, Serialize)]
pub struct BatchControlResult {
    pub requested: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

#[derive(Deserialize)]
pub struct DeleteEntry {
    pub path: String,
    pub etag: Option<String>,
}

#[derive(Default, Serialize)]
pub struct DeleteResult {
    pub deleted: Vec<String>,
    pub failed: usize,
    pub errors: Vec<String>,
}
impl DeleteResult {
    fn record(&mut self, path: String, outcome: Result<()>) {
        match outcome {
            Ok(()) => self.deleted.push(path),
            Err(error) => {
                self.failed += 1;
                if self.errors.len() < 20 {
                    self.errors.push(format!("{path}: {error}"));
                }
            }
        }
    }
}

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
        if self.engine.summary()?.batches.iter().any(|batch| {
            batch.profile_id == id
                && matches!(
                    batch.state,
                    voids3_transfer::batch::FeedState::Running
                        | voids3_transfer::batch::FeedState::Paused
                )
        }) {
            return Err(Error::Conflict(
                "Finish or cancel this connection's upload discovery first".into(),
            ));
        }
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
    pub async fn control_all(&self, operation: BatchControl) -> Result<BatchControlResult> {
        let engine = self.engine.clone();
        let mut interrupted: HashSet<_> = background(move || match operation {
            BatchControl::Pause => engine.interrupt_all(false),
            BatchControl::Cancel => engine.interrupt_all(true),
            BatchControl::Resume => {
                engine.resume_batches()?;
                Ok(Vec::new())
            }
        })
        .await?
        .into_iter()
        .collect();
        if matches!(operation, BatchControl::Cancel) {
            let engine = self.engine.clone();
            interrupted.extend(
                tokio::task::spawn_blocking(move || engine.cancel_idle())
                    .await
                    .map_err(|e| Error::Storage(e.to_string()))??,
            );
        }
        let mut result = BatchControlResult {
            requested: interrupted.len(),
            ..Default::default()
        };
        if matches!(operation, BatchControl::Pause) {
            return Ok(result);
        }
        let mut clients: HashMap<String, std::result::Result<S3, String>> = HashMap::new();
        let engine = self.engine.clone();
        let jobs = background(move || engine.list()).await?;
        for job in jobs {
            if interrupted.contains(&job.id)
                || match operation {
                    BatchControl::Resume => job.status != Status::Paused,
                    BatchControl::Cancel => {
                        job.status.terminal() && job.upload_id.is_none() && job.error.is_none()
                    }
                    BatchControl::Pause => true,
                }
            {
                continue;
            }
            let outcome = async {
                if matches!(operation, BatchControl::Cancel) {
                    let engine = self.engine.clone();
                    let id = job.id.clone();
                    if background(move || engine.request_cancel(&id)).await? {
                        return Ok(());
                    }
                }
                if !clients.contains_key(&job.profile_id) {
                    clients.insert(
                        job.profile_id.clone(),
                        self.client(&job.profile_id)
                            .await
                            .map_err(|e| e.to_string()),
                    );
                }
                let client = clients
                    .get(&job.profile_id)
                    .expect("client inserted")
                    .clone()
                    .map_err(Error::Provider)?;
                match operation {
                    BatchControl::Cancel => {
                        let engine = self.engine.clone();
                        let id = job.id.clone();
                        background(move || engine.cancel(client, &id)).await
                    }
                    BatchControl::Resume => {
                        let engine = self.engine.clone();
                        let id = job.id.clone();
                        tokio::task::spawn_blocking(move || engine.resume(client, &id))
                            .await
                            .map_err(|e| Error::Storage(e.to_string()))?
                    }
                    BatchControl::Pause => Ok(()),
                }
            }
            .await;
            match outcome {
                Ok(()) => result.requested += 1,
                Err(error) => {
                    result.failed += 1;
                    if result.errors.len() < 20 {
                        result.errors.push(format!("{}: {error}", job.key));
                    }
                }
            }
        }
        Ok(result)
    }
    /// The caller holds CloseGuard until exit or an explicit decision to stay.
    pub async fn cancel_and_drain(&self) -> Result<()> {
        let cancelled = self.control_all(BatchControl::Cancel).await?;
        loop {
            let engine = self.engine.clone();
            if background(move || engine.is_drained()).await? {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        if cancelled.failed != 0 {
            return Err(Error::Conflict(format!(
                "Could not cancel {} transfer(s): {}",
                cancelled.failed,
                cancelled.errors.join("; ")
            )));
        }
        let engine = self.engine.clone();
        background(move || engine.check_closed()).await
    }
    async fn load_settings(&self) -> Result<TransferSettings> {
        let store = self.store.clone();
        background(move || Ok(store.get("settings", "transfers")?.unwrap_or_default())).await
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
        let settings = self.load_settings().await?;
        let pending = voids3_transfer::upload::sources(paths, prefix).await?;
        if pending.len() + self.engine.len()? > 100_000 {
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
    pub async fn start_upload(
        &self,
        id: &str,
        paths: Vec<PathBuf>,
        prefix: String,
    ) -> Result<String> {
        let client = self.client(id).await?;
        let settings = self.load_settings().await?;
        let engine = self.engine.clone();
        tokio::task::spawn_blocking(move || engine.start_batch(client, paths, prefix, settings))
            .await
            .map_err(|e| Error::Storage(e.to_string()))?
    }
    pub async fn resolve_upload(
        &self,
        id: &str,
        action: voids3_transfer::upload::ConflictAction,
        all: bool,
    ) -> Result<()> {
        let job = self.engine.transfer(id)?;
        let client = self.client(&job.profile_id).await?;
        let engine = self.engine.clone();
        let id = id.to_owned();
        tokio::task::spawn_blocking(move || engine.resolve_conflicts(client, &id, action, all))
            .await
            .map_err(|e| Error::Storage(e.to_string()))?
    }
    pub async fn delete_entries(
        &self,
        id: Option<&str>,
        entries: Vec<DeleteEntry>,
    ) -> Result<DeleteResult> {
        if entries.is_empty()
            || entries.len() > 100_000
            || entries.iter().any(|entry| entry.path.is_empty())
        {
            return Err(Error::Invalid(
                "Select between 1 and 100,000 items to delete".into(),
            ));
        }
        if let Some(id) = id {
            if entries.iter().any(|entry| entry.path.ends_with('/')) {
                return Err(Error::Invalid(
                    "Select remote files, not folder prefixes, to delete".into(),
                ));
            }
            let client = self.client(id).await?;
            client.profile.writable()?;
            let mut result = DeleteResult::default();
            for entry in entries {
                let outcome = match entry.etag.as_deref().filter(|etag| !etag.is_empty()) {
                    Some(etag) => client.delete(&entry.path, Some(etag)).await,
                    None => Err(Error::Invalid(
                        "Object has no listed ETag; refresh before deleting".into(),
                    )),
                };
                result.record(entry.path, outcome);
            }
            Ok(result)
        } else {
            tokio::task::spawn_blocking(move || {
                let mut result = DeleteResult::default();
                for entry in entries {
                    let outcome = voids3_platform::recycle(std::path::Path::new(&entry.path));
                    result.record(entry.path, outcome);
                }
                result
            })
            .await
            .map_err(|e| Error::Storage(e.to_string()))
        }
    }
    pub async fn download(
        &self,
        id: &str,
        keys: Vec<String>,
        directory: PathBuf,
    ) -> Result<Vec<String>> {
        let client = self.client(id).await?;
        let settings = self.load_settings().await?;
        let mut pending = Vec::new();
        let mut directories = Vec::new();
        for key in keys {
            if key.ends_with('/') {
                let folder = key
                    .trim_end_matches('/')
                    .rsplit('/')
                    .next()
                    .unwrap_or_default()
                    .to_owned();
                if folder.is_empty() {
                    return Err(Error::Invalid("Choose a named remote folder".into()));
                }
                let entries = client.scan(&key).await?;
                let root = directory.clone();
                let (folder, files) = background(move || {
                    let folder = voids3_platform::download_target(&root, &folder)?;
                    let mut files = Vec::new();
                    for entry in entries
                        .into_iter()
                        .filter(|entry| !entry.path.ends_with('/'))
                    {
                        let path = folder_download_target(&root, &key, &entry.path)?;
                        files.push((entry.path, path));
                    }
                    Ok((folder, files))
                })
                .await?;
                directories.push(folder);
                pending.extend(files);
            } else {
                let root = directory.clone();
                let name = key.rsplit('/').next().unwrap_or_default().to_owned();
                let path =
                    background(move || voids3_platform::download_target(&root, &name)).await?;
                pending.push((key, path));
            }
            if pending.len() > 100_000 {
                return Err(Error::Invalid(
                    "A download selection is limited to 100,000 files".into(),
                ));
            }
        }
        if pending.len() + self.engine.len()? > 100_000 {
            return Err(Error::Invalid("Queue limit is 100,000 files".into()));
        }
        let pending = background(move || {
            let mut targets = HashSet::new();
            for (_, path) in &pending {
                if !targets.insert(path.clone()) {
                    return Err(Error::Conflict(
                        "Selected objects map to the same local filename; download them separately"
                            .into(),
                    ));
                }
            }
            Ok(pending)
        })
        .await?;
        for path in directories {
            tokio::fs::create_dir_all(path).await?;
        }
        for (_, path) in &pending {
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
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
        background(move || voids3_sync::preview(&local, &remote, mode)).await
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
        let settings = self.load_settings().await?;
        let mut ids = Vec::new();
        for change in changes {
            let direction = match change.action {
                voids3_sync::Action::Upload => Direction::Upload,
                voids3_sync::Action::Download => Direction::Download,
                _ => continue,
            };
            let root = local.clone();
            let key = change.key.clone();
            let path = background(move || voids3_platform::download_target(&root, &key)).await?;
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

async fn background<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| Error::Storage(error.to_string()))?
}

fn folder_download_target(directory: &std::path::Path, prefix: &str, key: &str) -> Result<PathBuf> {
    let folder = prefix
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default();
    let relative = key.strip_prefix(prefix).ok_or_else(|| {
        Error::Invalid("Provider returned a key outside the selected folder".into())
    })?;
    voids3_platform::download_target(directory, &format!("{folder}/{relative}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn shutdown_waiting_for_storage_does_not_block_the_async_executor() {
        // Hold SQLite's actual writer mutex while shutdown tries to persist
        // cancellation. A timer on this single-thread executor must still fire.
        struct HeldWrite {
            started: std::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
            release: std::sync::Mutex<std::sync::mpsc::Receiver<()>>,
        }
        impl Serialize for HeldWrite {
            fn serialize<S: serde::Serializer>(
                &self,
                serializer: S,
            ) -> std::result::Result<S::Ok, S::Error> {
                self.started
                    .lock()
                    .unwrap()
                    .take()
                    .unwrap()
                    .send(())
                    .unwrap();
                let _ = self
                    .release
                    .lock()
                    .unwrap()
                    .recv_timeout(std::time::Duration::from_secs(2));
                serializer.serialize_str("isolated held writer")
            }
        }
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(Store::open(&root.path().join("queue.db")).unwrap());
        let engine = Engine::new(store.clone(), TransferSettings::default()).unwrap();
        let app = App {
            store: store.clone(),
            engine,
            _instance_lock: std::fs::File::create(root.path().join("test.lock")).unwrap(),
        };
        let (_guard, _) = app.engine.begin_close().unwrap();
        let (started, ready) = tokio::sync::oneshot::channel();
        let (release, held) = std::sync::mpsc::channel();
        let writer = tokio::task::spawn_blocking(move || {
            store.put_many(
                "isolated-test",
                &[(
                    "held".into(),
                    HeldWrite {
                        started: std::sync::Mutex::new(Some(started)),
                        release: std::sync::Mutex::new(held),
                    },
                )],
            )
        });
        ready.await.unwrap();
        let cleanup = app.cancel_and_drain();
        tokio::pin!(cleanup);
        tokio::select! { biased;
            result = &mut cleanup => panic!("Cleanup blocked the executor until the held writer timed out: {result:?}"),
            _ = tokio::time::sleep(std::time::Duration::from_millis(20)) => {},
        }
        release.send(()).unwrap();
        cleanup.await.unwrap();
        writer.await.unwrap().unwrap();
    }

    #[test]
    fn folder_downloads_preserve_hierarchy_and_reject_unsafe_provider_keys() {
        let root = std::env::current_dir().unwrap().canonicalize().unwrap();
        assert_eq!(
            folder_download_target(&root, "releases/assets/", "releases/assets/nested/file.txt")
                .unwrap(),
            root.join("assets").join("nested").join("file.txt")
        );
        for key in [
            "other/file.txt",
            "releases/assets/../escape.txt",
            "releases/assets/C:/escape.txt",
            "releases/assets/nested\\escape.txt",
        ] {
            assert!(
                folder_download_target(&root, "releases/assets/", key).is_err(),
                "{key}"
            );
        }
    }

    #[test]
    fn batch_delete_reports_partial_success_and_bounds_error_details() {
        let mut result = DeleteResult::default();
        result.record("deleted.txt".into(), Ok(()));
        for index in 0..25 {
            result.record(
                format!("failed-{index}.txt"),
                Err(Error::Conflict("Object changed".into())),
            );
        }
        assert_eq!(result.deleted, ["deleted.txt"]);
        assert_eq!(result.failed, 25);
        assert_eq!(result.errors.len(), 20);
        assert!(result.errors[0].contains("failed-0.txt"));
    }
}
