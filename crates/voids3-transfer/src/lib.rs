//! Persistent transfer engine shared by desktop and CLI. Files never cross IPC.
pub mod batch;
mod conflict;
pub mod shutdown;
pub mod upload;
pub mod view;
use aws_sdk_s3::{
    primitives::ByteStream,
    types::{CompletedMultipartUpload, CompletedPart},
};
use futures::{StreamExt, stream};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::AsyncWriteExt,
    sync::{OwnedSemaphorePermit, Semaphore},
};
use tokio_util::sync::CancellationToken;
use voids3_core::{Error, Result, TransferSettings, part_size};
use voids3_s3::{S3, api_error};
use voids3_storage::Store;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Upload,
    Download,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Queued,
    Preparing,
    Uploading,
    Downloading,
    Paused,
    Verifying,
    Completed,
    Failed,
    Cancelled,
    Conflict,
    Skipped,
}
impl Status {
    pub fn terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled | Self::Skipped)
    }
    fn active(self) -> bool {
        matches!(
            self,
            Self::Preparing | Self::Uploading | Self::Downloading | Self::Verifying
        )
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fingerprint {
    pub size: u64,
    pub modified_nanos: u128,
}
impl Fingerprint {
    pub fn read_blocking(path: &Path) -> Result<Self> {
        Self::from_metadata(std::fs::metadata(path)?)
    }
    pub async fn read(path: &Path) -> Result<Self> {
        let meta = tokio::fs::metadata(path).await?;
        Self::from_metadata(meta)
    }
    fn from_metadata(meta: std::fs::Metadata) -> Result<Self> {
        if !meta.is_file() {
            return Err(Error::Invalid(
                "Transfer source must be a regular file".into(),
            ));
        }
        Ok(Self {
            size: meta.len(),
            modified_nanos: meta
                .modified()?
                .duration_since(UNIX_EPOCH)
                .map_err(|_| {
                    Error::Invalid("Source modification time predates the Unix epoch".into())
                })?
                .as_nanos(),
        })
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum UploadCondition {
    #[default]
    Create,
    Replace {
        etag: String,
    },
}
#[derive(Clone)]
pub struct UploadIntent {
    pub source: Fingerprint,
    pub condition: UploadCondition,
    pub batch_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transfer {
    pub id: String,
    pub profile_id: String,
    pub target: voids3_core::Profile,
    pub direction: Direction,
    pub local_path: PathBuf,
    pub key: String,
    pub size: u64,
    pub bytes_done: u64,
    pub status: Status,
    pub error: Option<String>,
    pub created: u64,
    pub part_size: u64,
    pub upload_id: Option<String>,
    pub parts: BTreeMap<i32, String>,
    pub source: Option<Fingerprint>,
    pub remote_etag: Option<String>,
    #[serde(default)]
    pub upload_condition: UploadCondition,
    #[serde(default)]
    pub batch_id: Option<String>,
    #[serde(default)]
    pub conflict: Option<voids3_s3::ObjectInfo>,
    pub priority: i32,
    pub settings: TransferSettings,
}
#[derive(Clone)]
struct Control {
    token: CancellationToken,
    cancel: Arc<std::sync::atomic::AtomicBool>,
    epoch: Option<u64>,
}

pub struct Engine {
    store: Arc<Store>,
    jobs: Mutex<HashMap<String, Transfer>>,
    controls: Mutex<HashMap<String, Control>>,
    files: Arc<Semaphore>,
    requests: Arc<Semaphore>,
    maintenance: std::sync::atomic::AtomicBool,
    feeds: Mutex<HashMap<String, batch::Feed>>,
    stop_epoch: std::sync::atomic::AtomicU64,
    closing: std::sync::atomic::AtomicBool,
    background: std::sync::atomic::AtomicUsize,
    preparation: Mutex<CancellationToken>,
    preparing: std::sync::atomic::AtomicUsize,
}
/// Holds the queue idle while a desktop update is downloaded and installed.
/// Dropping the guard after a failed update allows transfers again.
pub struct MaintenanceGuard(Arc<Engine>);
impl Drop for MaintenanceGuard {
    fn drop(&mut self) {
        self.0
            .maintenance
            .store(false, std::sync::atomic::Ordering::Release);
    }
}
impl Engine {
    pub fn new(store: Arc<Store>, settings: TransferSettings) -> Result<Arc<Self>> {
        settings.validate()?;
        let mut jobs: HashMap<String, Transfer> = HashMap::new();
        for mut job in store.list::<Transfer>("transfers")? {
            if job.status.active() || job.status == Status::Queued {
                job.status = Status::Paused;
                job.error = Some("Recovered after restart. Resume explicitly to validate source and remote state.".into());
                store.put("transfers", &job.id, &job)?;
            }
            jobs.insert(job.id.clone(), job);
        }
        let feeds = batch::recover(&store)?;
        Ok(Arc::new(Self {
            store,
            jobs: Mutex::new(jobs),
            controls: Mutex::new(HashMap::new()),
            files: Arc::new(Semaphore::new(settings.concurrent_files)),
            requests: Arc::new(Semaphore::new(settings.max_requests)),
            maintenance: std::sync::atomic::AtomicBool::new(false),
            feeds: Mutex::new(feeds),
            stop_epoch: std::sync::atomic::AtomicU64::new(0),
            closing: std::sync::atomic::AtomicBool::new(false),
            background: std::sync::atomic::AtomicUsize::new(0),
            preparation: Mutex::new(CancellationToken::new()),
            preparing: std::sync::atomic::AtomicUsize::new(0),
        }))
    }
    pub fn list(&self) -> Result<Vec<Transfer>> {
        let mut jobs: Vec<_> = self.jobs.lock().map_err(lock)?.values().cloned().collect();
        jobs.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then(a.created.cmp(&b.created))
                .then(a.id.cmp(&b.id))
        });
        Ok(jobs)
    }
    pub fn len(&self) -> Result<usize> {
        Ok(self.jobs.lock().map_err(lock)?.len())
    }
    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }
    fn save(&self, job: &Transfer) -> Result<()> {
        self.store.put("transfers", &job.id, job)?;
        self.jobs
            .lock()
            .map_err(lock)?
            .insert(job.id.clone(), job.clone());
        Ok(())
    }
    fn get(&self, id: &str) -> Result<Transfer> {
        self.jobs
            .lock()
            .map_err(lock)?
            .get(id)
            .cloned()
            .ok_or_else(|| Error::Invalid("Transfer not found".into()))
    }
    async fn persist(&self, job: &Transfer) -> Result<()> {
        let store = self.store.clone();
        let snapshot = job.clone();
        tokio::task::spawn_blocking(move || store.put("transfers", &snapshot.id, &snapshot))
            .await
            .map_err(lock)??;
        self.jobs
            .lock()
            .map_err(lock)?
            .insert(job.id.clone(), job.clone());
        Ok(())
    }
    pub async fn enqueue(
        self: &Arc<Self>,
        s3: S3,
        direction: Direction,
        local: PathBuf,
        key: String,
        settings: TransferSettings,
    ) -> Result<String> {
        self.enqueue_with_intent(s3, direction, local, key, settings, None)
            .await
    }
    pub async fn enqueue_upload(
        self: &Arc<Self>,
        s3: S3,
        local: PathBuf,
        key: String,
        settings: TransferSettings,
        intent: UploadIntent,
    ) -> Result<String> {
        self.enqueue_with_intent(s3, Direction::Upload, local, key, settings, Some(intent))
            .await
    }
    async fn enqueue_with_intent(
        self: &Arc<Self>,
        s3: S3,
        direction: Direction,
        local: PathBuf,
        key: String,
        settings: TransferSettings,
        intent: Option<UploadIntent>,
    ) -> Result<String> {
        settings.validate()?;
        if key.is_empty() || key.ends_with('/') {
            return Err(Error::Invalid("Select a file object, not a folder".into()));
        }
        let (size, source, etag) = match direction {
            Direction::Upload => {
                s3.profile.writable()?;
                let source = Fingerprint::read(&local).await?;
                if let Some(intent) = &intent {
                    if intent.source != source {
                        return Err(Error::Conflict(
                            "Source changed since upload confirmation. Upload again to review it."
                                .into(),
                        ));
                    }
                    if matches!(&intent.condition, UploadCondition::Replace { etag } if etag.is_empty())
                    {
                        return Err(Error::Invalid(
                            "Replacement requires the confirmed destination ETag".into(),
                        ));
                    }
                }
                (source.size, Some(source), None)
            }
            Direction::Download => {
                let info = s3.head(&key).await?;
                if info.etag.is_empty() {
                    return Err(Error::Invalid(
                        "Provider did not return an ETag; safe resumable download is unavailable"
                            .into(),
                    ));
                }
                if tokio::fs::try_exists(&local).await? {
                    return Err(Error::Conflict(
                        "Local destination exists; choose another name".into(),
                    ));
                }
                (info.size, None, Some(info.etag))
            }
        };
        let local = if direction == Direction::Upload {
            tokio::fs::canonicalize(local).await?
        } else {
            let parent = local
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            let name = local
                .file_name()
                .ok_or_else(|| Error::Invalid("Download destination requires a filename".into()))?;
            tokio::fs::canonicalize(parent).await?.join(name)
        };
        let id = uuid::Uuid::new_v4().to_string();
        let job = Transfer {
            id: id.clone(),
            profile_id: s3.profile.id.clone(),
            target: s3.profile.clone(),
            direction,
            local_path: local,
            key,
            size,
            bytes_done: 0,
            status: Status::Queued,
            error: None,
            created: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            part_size: part_size(size, settings.part_size)?,
            upload_id: None,
            parts: BTreeMap::new(),
            source,
            remote_etag: etag,
            batch_id: intent.as_ref().and_then(|intent| intent.batch_id.clone()),
            conflict: None,
            upload_condition: intent.map(|intent| intent.condition).unwrap_or_default(),
            priority: 0,
            settings,
        };
        let engine = self.clone();
        tokio::task::spawn_blocking(move || engine.start(s3, job))
            .await
            .map_err(lock)??;
        Ok(id)
    }
    fn start(self: &Arc<Self>, s3: S3, job: Transfer) -> Result<()> {
        let control = Control {
            epoch: None,
            token: CancellationToken::new(),
            cancel: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        self.start_controlled(s3, job, control)
    }
    fn start_controlled(self: &Arc<Self>, s3: S3, job: Transfer, control: Control) -> Result<()> {
        if job.target.endpoint != s3.profile.endpoint
            || job.target.bucket != s3.profile.bucket
            || job.target.region != s3.profile.region
            || job.target.path_style != s3.profile.path_style
        {
            return Err(Error::Conflict("Connection target changed since this transfer was queued. Restore its original endpoint, bucket, region, and addressing mode before resuming or cancelling.".into()));
        }
        let worker_epoch;
        let background;
        {
            let mut controls = self.controls.lock().map_err(lock)?;
            // Cancellation-only workers may still start to abort an idle MPU.
            if !control.cancel.load(std::sync::atomic::Ordering::Acquire) {
                self.ensure_open()?;
            }
            if control.epoch.is_some_and(|epoch| {
                epoch != self.stop_epoch.load(std::sync::atomic::Ordering::Acquire)
            }) {
                return Err(Error::Paused);
            }
            if control.epoch.is_some() && self.get(&job.id)?.status != Status::Conflict {
                return Err(Error::Cancelled);
            }
            if self.maintenance.load(std::sync::atomic::Ordering::Acquire) {
                return Err(Error::Conflict(
                    "An application update is in progress. Try the transfer again after updating."
                        .into(),
                ));
            }
            if controls.contains_key(&job.id) {
                return Err(Error::Invalid("Transfer is already running".into()));
            }
            let is_new = !self.jobs.lock().map_err(lock)?.contains_key(&job.id);
            if is_new && self.jobs.lock().map_err(lock)?.len() >= 100_000 {
                return Err(Error::Invalid(
                    "Queue limit is 100,000 files; clear finished history".into(),
                ));
            }
            if is_new && job.batch_id.is_some() && controls.len() >= batch::BACKLOG {
                return Err(Error::Paused);
            }
            if is_new
                && let Some(id) = &job.batch_id
                && let Some(feed) = self.feeds.lock().map_err(lock)?.get(id)
            {
                match *feed.state.borrow() {
                    batch::FeedState::Paused => return Err(Error::Paused),
                    batch::FeedState::Cancelled => return Err(Error::Cancelled),
                    _ => {}
                }
            }
            self.save(&job)?;
            worker_epoch = self.stop_epoch.load(std::sync::atomic::Ordering::Acquire);
            controls.insert(job.id.clone(), control.clone());
            background = self.background_work();
        }
        let engine = Arc::clone(self);
        tokio::spawn(async move {
            let _background = background;
            let mut job = job;
            let result = engine.run(&s3, &mut job, &control).await;
            if let Err(error) = result {
                if let (Some(batch), Error::Provider(message)) = (&job.batch_id, &error)
                    && [
                        "NoSuchBucket",
                        "InvalidAccessKeyId",
                        "SignatureDoesNotMatch",
                        "AccessDenied",
                        "Forbidden",
                        "ExpiredToken",
                    ]
                    .iter()
                    .any(|code| message.contains(code))
                {
                    engine.fail_batch(batch, message);
                }
                let cancelled = control.cancel.load(std::sync::atomic::Ordering::Relaxed);
                job.status = if cancelled {
                    Status::Cancelled
                } else if job.conflict.is_some() {
                    Status::Conflict
                } else if matches!(error, Error::Paused) {
                    Status::Paused
                } else {
                    Status::Failed
                };
                job.error = if cancelled || matches!(error, Error::Paused) {
                    None
                } else {
                    Some(error.to_string())
                };
                if cancelled {
                    if let Some(upload_id) = &job.upload_id {
                        if let Err(e) = s3
                            .client
                            .abort_multipart_upload()
                            .bucket(&s3.profile.bucket)
                            .key(&job.key)
                            .upload_id(upload_id)
                            .send()
                            .await
                        {
                            if e.as_service_error()
                                .is_some_and(|s| s.meta().code() == Some("NoSuchUpload"))
                            {
                                job.upload_id = None;
                            } else {
                                job.error = Some(
                                    api_error(
                                        "Cancelled, but multipart cleanup failed",
                                        e.as_service_error().and_then(|s| s.meta().code()),
                                    )
                                    .to_string(),
                                );
                            }
                        } else {
                            job.upload_id = None;
                        }
                    }
                    if job.direction == Direction::Download
                        && let Err(e) = tokio::fs::remove_file(partial_path(&job)).await
                        && e.kind() != std::io::ErrorKind::NotFound
                    {
                        job.error = Some(format!("Partial file cleanup failed: {e}"));
                    }
                }
            }
            // Hold the control map until the final state is committed, preventing a resume race.
            let policy_batch = if job.status == Status::Conflict
                && job.upload_condition == UploadCondition::Create
            {
                job.batch_id.clone()
            } else {
                None
            };
            let transfer_id = job.id.clone();
            let final_engine = engine.clone();
            let _ = tokio::task::spawn_blocking(move || match final_engine.controls.lock() {
                Ok(mut controls) => {
                    if let Err(e) = final_engine.save(&job) {
                        tracing::error!(transfer_id = %job.id, error = %e, "Cannot persist transfer outcome");
                    }
                    controls.remove(&job.id);
                }
                Err(_) => tracing::error!("Transfer control lock poisoned"),
            }).await;
            if let Some(batch) = policy_batch
                && let Ok(Some(action)) = engine.batch_policy(Some(&batch))
            {
                // A close prompt freezes admission, but Keep open must not lose
                // a batch policy decision that arrived while the prompt was up.
                while worker_epoch == engine.stop_epoch.load(std::sync::atomic::Ordering::Acquire) {
                    if engine.closing.load(std::sync::atomic::Ordering::Acquire) {
                        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                        continue;
                    }
                    let resolver = engine.clone();
                    let target = s3.clone();
                    let id = transfer_id.clone();
                    let result = tokio::task::spawn_blocking(move || {
                        resolver.resolve_conflicts_at_epoch(
                            target,
                            &id,
                            action,
                            false,
                            worker_epoch,
                        )
                    })
                    .await;
                    if matches!(result, Ok(Err(_)))
                        && engine.closing.load(std::sync::atomic::Ordering::Acquire)
                    {
                        continue;
                    }
                    break;
                }
            }
        });
        Ok(())
    }
    pub fn pause(&self, id: &str) -> Result<()> {
        let controls = self.controls.lock().map_err(lock)?;
        let control = controls
            .get(id)
            .ok_or_else(|| Error::Invalid("Transfer is not running".into()))?;
        control.token.cancel();
        Ok(())
    }
    pub fn begin_maintenance(self: &Arc<Self>) -> Result<MaintenanceGuard> {
        let controls = self.controls.lock().map_err(lock)?;
        self.ensure_open()?;
        if !controls.is_empty()
            || self.preparing.load(std::sync::atomic::Ordering::Acquire) != 0
            || self
                .feeds
                .lock()
                .map_err(lock)?
                .values()
                .any(|feed| *feed.state.borrow() == batch::FeedState::Running)
            || self
                .jobs
                .lock()
                .map_err(lock)?
                .values()
                .any(|job| job.status.active() || job.status == Status::Queued)
        {
            return Err(Error::Conflict("Pause or cancel all running and queued transfers, wait until they stop, then install the update.".into()));
        }
        if self
            .maintenance
            .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            return Err(Error::Conflict(
                "An application update is already in progress".into(),
            ));
        }
        Ok(MaintenanceGuard(self.clone()))
    }
    /// Signal every worker, including workers waiting for a file/request permit,
    /// before any network or credential work can delay a global stop.
    pub fn interrupt_all(&self, cancel: bool) -> Result<Vec<String>> {
        let controls = self.controls.lock().map_err(lock)?;
        let mut preparation = self.preparation.lock().map_err(lock)?;
        preparation.cancel();
        *preparation = CancellationToken::new();
        drop(preparation);
        self.stop_epoch
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        for feed in self.feeds.lock().map_err(lock)?.values() {
            let current = *feed.state.borrow();
            if matches!(
                current,
                batch::FeedState::Running | batch::FeedState::Paused
            ) {
                feed.state.send_replace(if cancel {
                    batch::FeedState::Cancelled
                } else {
                    batch::FeedState::Paused
                });
            }
        }
        for control in controls.values() {
            if cancel {
                control
                    .cancel
                    .store(true, std::sync::atomic::Ordering::Relaxed);
            }
            control.token.cancel();
        }
        Ok(controls.keys().cloned().collect())
    }
    /// Returns false only when an idle multipart upload needs its provider client
    /// for remote cleanup. Running workers already own that client.
    pub fn request_cancel(&self, id: &str) -> Result<bool> {
        let controls = self.controls.lock().map_err(lock)?;
        if let Some(control) = controls.get(id) {
            control
                .cancel
                .store(true, std::sync::atomic::Ordering::Relaxed);
            control.token.cancel();
            return Ok(true);
        }
        let mut job = self.get(id)?;
        if matches!(job.status, Status::Completed | Status::Skipped) {
            return Ok(true);
        }
        if job.upload_id.is_some() {
            return Ok(false);
        }
        if job.direction == Direction::Download
            && let Err(error) = std::fs::remove_file(partial_path(&job))
            && error.kind() != std::io::ErrorKind::NotFound
        {
            return Err(error.into());
        }
        job.status = Status::Cancelled;
        job.error = None;
        self.save(&job)?;
        Ok(true)
    }
    pub fn cancel_idle(&self) -> Result<Vec<String>> {
        let controls = self.controls.lock().map_err(lock)?;
        let mut jobs = self.jobs.lock().map_err(lock)?;
        let mut updates = Vec::new();
        for job in jobs.values() {
            if controls.contains_key(&job.id) || job.upload_id.is_some() || job.status.terminal() {
                continue;
            }
            let mut job = job.clone();
            job.error = None;
            if job.direction == Direction::Download
                && let Err(error) = std::fs::remove_file(partial_path(&job))
                && error.kind() != std::io::ErrorKind::NotFound
            {
                // A failed cleanup must not stop cancellation of other jobs.
                job.error = Some(format!("Partial file cleanup failed: {error}"));
            }
            job.status = Status::Cancelled;
            job.conflict = None;
            updates.push((job.id.clone(), job));
        }
        self.store.put_many("transfers", &updates)?;
        let ids = updates.iter().map(|(id, _)| id.clone()).collect();
        jobs.extend(updates);
        Ok(ids)
    }
    pub fn resume(self: &Arc<Self>, s3: S3, id: &str) -> Result<()> {
        let mut job = self.get(id)?;
        if !matches!(job.status, Status::Paused | Status::Failed) {
            return Err(Error::Invalid(
                "Only paused or failed transfers can resume".into(),
            ));
        }
        job.status = Status::Queued;
        job.error = None;
        self.start(s3, job.clone())?;
        // The task persists Preparing; do not overwrite a state already advanced by the task.
        Ok(())
    }
    pub fn cancel(self: &Arc<Self>, s3: S3, id: &str) -> Result<()> {
        if self.request_cancel(id)? {
            return Ok(());
        }
        let mut job = self.get(id)?;
        if job.status == Status::Completed
            || (job.status == Status::Cancelled && job.upload_id.is_none())
        {
            return Ok(());
        }
        job.status = Status::Queued;
        let control = Control {
            epoch: None,
            token: CancellationToken::new(),
            cancel: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        };
        control.token.cancel();
        self.start_controlled(s3, job, control)
    }
    pub fn clear_finished(&self) -> Result<()> {
        let controls = self.controls.lock().map_err(lock)?;
        let mut jobs = self.jobs.lock().map_err(lock)?;
        let ids: Vec<_> = jobs
            .values()
            .filter(|j| {
                j.status.terminal()
                    && !controls.contains_key(&j.id)
                    && j.upload_id.is_none()
                    && j.error.is_none()
            })
            .map(|j| j.id.clone())
            .collect();
        self.store.delete_many("transfers", &ids)?;
        for id in ids {
            jobs.remove(&id);
        }
        let remaining_batches: std::collections::HashSet<_> = jobs
            .values()
            .filter_map(|job| job.batch_id.clone())
            .collect();
        let mut feeds = self.feeds.lock().map_err(lock)?;
        let finished: Vec<_> = feeds
            .iter()
            .filter(|(id, feed)| {
                !remaining_batches.contains(*id)
                    && !matches!(
                        *feed.state.borrow(),
                        batch::FeedState::Running | batch::FeedState::Paused
                    )
            })
            .map(|(id, _)| id.clone())
            .collect();
        self.store.delete_many("upload_batches", &finished)?;
        self.store.delete_many("upload_policies", &finished)?;
        for id in finished {
            feeds.remove(&id);
        }
        Ok(())
    }
    async fn request(&self, control: &Control) -> Result<OwnedSemaphorePermit> {
        tokio::select! { biased; _ = control.token.cancelled() => Err(Error::Paused), permit = self.requests.clone().acquire_owned() => permit.map_err(|_| Error::Cancelled) }
    }
    async fn run(&self, s3: &S3, job: &mut Transfer, control: &Control) -> Result<()> {
        let _file = tokio::select! { biased; _ = control.token.cancelled() => return Err(Error::Paused), permit = self.files.clone().acquire_owned() => permit.map_err(|_| Error::Cancelled)? };
        job.status = Status::Preparing;
        self.persist(job).await?;
        match job.direction {
            Direction::Upload => self.upload(s3, job, control).await?,
            Direction::Download => self.download(s3, job, control).await?,
        }
        if job.status == Status::Skipped {
            return Ok(());
        }
        job.status = Status::Completed;
        job.bytes_done = job.size;
        job.error = None;
        Ok(())
    }
    async fn validate_source(&self, job: &Transfer) -> Result<()> {
        if job.source.as_ref() != Some(&Fingerprint::read(&job.local_path).await?) {
            return Err(Error::Conflict("Source file changed. Cancel this transfer and enqueue the new file; completed parts will not be reused.".into()));
        }
        Ok(())
    }
    async fn upload(&self, s3: &S3, job: &mut Transfer, control: &Control) -> Result<()> {
        loop {
            let fast_create = job.batch_id.is_some()
                && job.upload_condition == UploadCondition::Create
                && self.batch_policy(job.batch_id.as_deref())?.is_none()
                && job.size < job.settings.multipart_threshold;
            if !fast_create {
                let _permit = self.request(control).await?;
                let existing = tokio::select! { biased;
                    _ = control.token.cancelled() => return Err(Error::Paused),
                    result = s3.head_optional(&job.key) => result?
                };
                match (&job.upload_condition, existing) {
                    (UploadCondition::Create, Some(info)) => {
                        self.destination_conflict(s3, job, info, true).await?
                    }
                    (UploadCondition::Replace { etag }, Some(info)) if *etag != info.etag => {
                        self.destination_conflict(s3, job, info, false).await?
                    }
                    (UploadCondition::Replace { .. }, None) => return Err(Error::Conflict(
                        "Destination disappeared after confirmation. Upload again to review it."
                            .into(),
                    )),
                    _ => {}
                }
            }
            if job.status == Status::Skipped {
                return Ok(());
            }
            match self.upload_body(s3, job, control).await {
                Err(Error::DestinationChanged) if job.batch_id.is_some() => {
                    let _permit = self.request(control).await?;
                    let info = s3.head_optional(&job.key).await?.ok_or_else(|| {
                        Error::Conflict(
                            "Destination changed during upload; retry with a fresh confirmation"
                                .into(),
                        )
                    })?;
                    let allow_policy = job.upload_condition == UploadCondition::Create;
                    self.destination_conflict(s3, job, info, allow_policy)
                        .await?;
                    if job.status == Status::Skipped {
                        return Ok(());
                    }
                }
                result => return result,
            }
        }
    }
    async fn destination_conflict(
        &self,
        s3: &S3,
        job: &mut Transfer,
        info: voids3_s3::ObjectInfo,
        allow_policy: bool,
    ) -> Result<()> {
        if job.batch_id.is_none() {
            return Err(Error::Conflict(
                "Upload destination exists or changed; it was not overwritten".into(),
            ));
        }
        job.conflict = Some(info);
        if allow_policy && let Some(action) = self.batch_policy(job.batch_id.as_deref())? {
            self.apply_conflict(s3, job, action).await?;
            return Ok(());
        }
        Err(Error::Conflict("Destination exists or changed. Choose Replace, Skip or Keep both in the queue; other files continue.".into()))
    }
    async fn upload_body(&self, s3: &S3, job: &mut Transfer, control: &Control) -> Result<()> {
        s3.profile.writable()?;
        self.validate_source(job).await?;
        let (if_match, if_none_match) = match &job.upload_condition {
            UploadCondition::Create => (None, Some("*".to_owned())),
            UploadCondition::Replace { etag } => (Some(etag.clone()), None),
        };
        job.status = Status::Uploading;
        self.persist(job).await?;
        if job.size < job.settings.multipart_threshold {
            let _permit = self.request(control).await?;
            let body = ByteStream::from_path(&job.local_path)
                .await
                .map_err(|_| Error::Invalid("Cannot open upload source".into()))?;
            tokio::select! { biased;
                _ = control.token.cancelled() => return Err(Error::Paused),
                result = s3.client.put_object().bucket(&s3.profile.bucket).key(&job.key)
                    .set_if_match(if_match).set_if_none_match(if_none_match)
                    .content_type(mime_guess::from_path(&job.local_path).first_or_octet_stream().as_ref()).body(body).send() => {
                    result.map_err(|e| upload_error("Upload", e.as_service_error().and_then(|s| s.meta().code())))?;
                }
            }
            self.validate_source(job).await?;
            return Ok(());
        }
        if job.upload_id.is_none() {
            let _permit = self.request(control).await?;
            let output = s3
                .client
                .create_multipart_upload()
                .bucket(&s3.profile.bucket)
                .key(&job.key)
                .content_type(
                    mime_guess::from_path(&job.local_path)
                        .first_or_octet_stream()
                        .as_ref(),
                )
                .send()
                .await
                .map_err(|e| {
                    api_error(
                        "Start multipart upload",
                        e.as_service_error().and_then(|s| s.meta().code()),
                    )
                })?;
            job.upload_id = Some(
                output
                    .upload_id()
                    .ok_or_else(|| {
                        Error::Provider("Provider returned no multipart upload ID".into())
                    })?
                    .into(),
            );
            self.persist(job).await?;
        } else {
            // Provider state is authoritative: a cancelled request may have completed server-side.
            let mut parts = BTreeMap::new();
            let mut marker = None;
            loop {
                let _permit = self.request(control).await?;
                let output = s3
                    .client
                    .list_parts()
                    .bucket(&s3.profile.bucket)
                    .key(&job.key)
                    .set_upload_id(job.upload_id.clone())
                    .set_part_number_marker(marker)
                    .send()
                    .await
                    .map_err(|e| {
                        api_error(
                            "Reconcile multipart upload",
                            e.as_service_error().and_then(|s| s.meta().code()),
                        )
                    })?;
                for part in output.parts() {
                    let number = part.part_number().ok_or_else(|| {
                        Error::Provider("Provider returned part without number".into())
                    })?;
                    let expected = job
                        .size
                        .saturating_sub((number.saturating_sub(1) as u64) * job.part_size)
                        .min(job.part_size);
                    if number < 1
                        || number as u64 > job.size.div_ceil(job.part_size)
                        || part.size().unwrap_or(-1) != expected as i64
                    {
                        return Err(Error::Conflict(
                            "Provider multipart state does not match this source".into(),
                        ));
                    }
                    parts.insert(
                        number,
                        part.e_tag()
                            .ok_or_else(|| {
                                Error::Provider("Provider returned part without ETag".into())
                            })?
                            .into(),
                    );
                }
                if !output.is_truncated().unwrap_or(false) {
                    break;
                }
                marker = output.next_part_number_marker().map(str::to_owned);
                if marker.is_none() {
                    return Err(Error::Provider(
                        "Provider omitted multipart pagination marker".into(),
                    ));
                }
            }
            job.parts = parts;
            job.bytes_done = job
                .parts
                .keys()
                .map(|n| (job.size - (*n as u64 - 1) * job.part_size).min(job.part_size))
                .sum();
            self.persist(job).await?;
        }
        let missing: Vec<i32> = (1..=job.size.div_ceil(job.part_size) as i32)
            .filter(|n| !job.parts.contains_key(n))
            .collect();
        let upload_id = job
            .upload_id
            .clone()
            .ok_or_else(|| Error::Invalid("Missing multipart state".into()))?;
        let path = job.local_path.clone();
        let size = job.size;
        let chunk = job.part_size;
        let key = job.key.clone();
        let mut pending = stream::iter(missing).map(|number| {
            let upload_id = &upload_id; let path = &path; let key = &key;
            async move {
                let _permit = self.request(control).await?;
                let offset = (number as u64 - 1) * chunk;
                let length = (size - offset).min(chunk);
                let body = ByteStream::read_from().path(path).offset(offset).length(aws_sdk_s3::primitives::Length::Exact(length)).build().await
                    .map_err(|_| Error::Invalid("Cannot open source part".into()))?;
                let output = tokio::select! { biased;
                    _ = control.token.cancelled() => return Err(Error::Paused),
                    result = s3.client.upload_part().bucket(&s3.profile.bucket).key(key).upload_id(upload_id).part_number(number).content_length(length as i64).body(body).send() => result.map_err(|e| api_error("Upload part",e.as_service_error().and_then(|s| s.meta().code())))?
                };
                let etag = output.e_tag().ok_or_else(|| Error::Provider("Provider returned no part ETag".into()))?.to_owned();
                Ok::<_,Error>((number,etag,length))
            }
        }).buffer_unordered(job.settings.concurrent_parts);
        while let Some(result) = pending.next().await {
            let (number, etag, length) = result?;
            job.parts.insert(number, etag);
            job.bytes_done += length;
            self.persist(job).await?;
        }
        drop(pending);
        self.validate_source(job).await?;
        job.status = Status::Verifying;
        self.persist(job).await?;
        let _permit = self.request(control).await?;
        let parts = job
            .parts
            .iter()
            .map(|(number, etag)| {
                CompletedPart::builder()
                    .part_number(*number)
                    .e_tag(etag)
                    .build()
            })
            .collect();
        // Evaluate the confirmed destination version atomically at completion.
        s3.client
            .complete_multipart_upload()
            .bucket(&s3.profile.bucket)
            .key(&job.key)
            .upload_id(&upload_id)
            .set_if_match(if_match)
            .set_if_none_match(if_none_match)
            .multipart_upload(
                CompletedMultipartUpload::builder()
                    .set_parts(Some(parts))
                    .build(),
            )
            .send()
            .await
            .map_err(|e| {
                upload_error(
                    "Complete multipart upload",
                    e.as_service_error().and_then(|s| s.meta().code()),
                )
            })?;
        job.upload_id = None;
        Ok(())
    }
    async fn download(&self, s3: &S3, job: &mut Transfer, control: &Control) -> Result<()> {
        let _permit = self.request(control).await?;
        let info = s3.head(&job.key).await?;
        if Some(&info.etag) != job.remote_etag.as_ref() || info.size != job.size {
            return Err(Error::Conflict(
                "Remote object changed; cancel and restart the download".into(),
            ));
        }
        let partial = partial_path(job);
        // Reject links and special files. create_new avoids accidentally truncating another file.
        let mut file = match tokio::fs::symlink_metadata(&partial).await {
            Ok(meta) => {
                if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > job.size {
                    return Err(Error::Conflict(
                        "Partial file is unsafe or too large".into(),
                    ));
                }
                tokio::fs::OpenOptions::new()
                    .append(true)
                    .open(&partial)
                    .await?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tokio::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&partial)
                    .await?
            }
            Err(e) => return Err(e.into()),
        };
        let offset = file.metadata().await?.len();
        job.bytes_done = offset;
        job.status = Status::Downloading;
        self.persist(job).await?;
        if offset < job.size {
            let output = s3
                .client
                .get_object()
                .bucket(&s3.profile.bucket)
                .key(&job.key)
                .if_match(&info.etag)
                .set_range(if offset > 0 {
                    Some(format!("bytes={offset}-"))
                } else {
                    None
                })
                .send()
                .await
                .map_err(|e| {
                    api_error(
                        "Download",
                        e.as_service_error().and_then(|s| s.meta().code()),
                    )
                })?;
            if offset > 0
                && !output
                    .content_range()
                    .is_some_and(|range| range.starts_with(&format!("bytes {offset}-")))
            {
                return Err(Error::Provider(
                    "Provider did not honor the resume Range request; partial file was preserved"
                        .into(),
                ));
            }
            let mut body = output.body;
            loop {
                let bytes = tokio::select! { biased; _ = control.token.cancelled() => { file.sync_all().await?; return Err(Error::Paused); }, bytes = body.next() => bytes };
                let Some(bytes) = bytes else {
                    break;
                };
                let bytes = bytes.map_err(|_| {
                    Error::Provider(
                        "Download stream interrupted; resume to request the remaining range".into(),
                    )
                })?;
                if job.bytes_done + bytes.len() as u64 > job.size {
                    return Err(Error::Conflict(
                        "Provider sent more data than the expected object size".into(),
                    ));
                }
                file.write_all(&bytes).await?;
                job.bytes_done += bytes.len() as u64;
                // Transient progress stays in memory. On restart, actual partial-file size is authoritative.
                self.jobs
                    .lock()
                    .map_err(lock)?
                    .insert(job.id.clone(), job.clone());
            }
        }
        if job.bytes_done != job.size {
            return Err(Error::Provider(
                "Download ended before the expected size; resume to continue".into(),
            ));
        }
        job.status = Status::Verifying;
        self.persist(job).await?;
        file.sync_all().await?;
        drop(file);
        // Same-directory hard link gives atomic no-replace finalization on supported filesystems.
        // Failure leaves the partial intact; never fall back to a potentially destructive rename.
        tokio::fs::hard_link(&partial, &job.local_path).await?;
        tokio::fs::remove_file(&partial).await?;
        Ok(())
    }
}
fn partial_path(job: &Transfer) -> PathBuf {
    job.local_path
        .with_file_name(format!(".voids3-{}.part", job.id))
}
fn lock(e: impl std::fmt::Display) -> Error {
    Error::Storage(e.to_string())
}
fn upload_error(operation: &str, code: Option<&str>) -> Error {
    if matches!(
        code,
        Some("PreconditionFailed" | "ConditionalRequestConflict" | "412" | "409")
    ) {
        Error::DestinationChanged
    } else {
        api_error(operation, code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ten_thousand_jobs_have_bounded_pages_and_correct_aggregate_counts() {
        let (_dir, engine, template) = fixture();
        {
            let mut jobs = engine.jobs.lock().unwrap();
            jobs.clear();
            for index in 0..10_000 {
                let mut job = template.clone();
                job.id = format!("{index:05}");
                job.status = if index % 2 == 0 {
                    Status::Completed
                } else {
                    Status::Conflict
                };
                jobs.insert(job.id.clone(), job);
            }
        }
        let started = std::time::Instant::now();
        let summary = engine.summary().unwrap();
        let page = engine.page("conflict", 4900, 100).unwrap();
        println!(
            "10,000-job aggregate + final 100-row filtered page: {:?}",
            started.elapsed()
        );
        assert_eq!(summary.total, 10_000);
        assert_eq!(summary.counts.get("conflict"), Some(&5000));
        assert_eq!(page.total, 5000);
        assert_eq!(page.items.len(), 100);
        assert!(page.items.iter().all(|job| job.status == Status::Conflict));
        assert!(engine.page("all", 0, 101).is_err());
        assert!(engine.page("all", 10_000, 100).unwrap().items.is_empty());
        assert_eq!(engine.cancel_idle().unwrap().len(), 5000);
        assert_eq!(
            engine.summary().unwrap().counts.get("cancelled"),
            Some(&5000)
        );
    }
    fn fixture() -> (tempfile::TempDir, Arc<Engine>, Transfer) {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(Store::open(&dir.path().join("queue.db")).unwrap());
        let engine = Engine::new(store, TransferSettings::default()).unwrap();
        let target = voids3_core::Profile {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Isolated test".into(),
            provider: voids3_core::Provider::Custom,
            endpoint: "https://example.invalid".into(),
            region: "test".into(),
            bucket: "test".into(),
            prefix: String::new(),
            path_style: true,
            read_only: false,
            allow_http: false,
        };
        let job = Transfer {
            id: uuid::Uuid::new_v4().to_string(),
            profile_id: target.id.clone(),
            target,
            direction: Direction::Download,
            local_path: dir.path().join("finished.bin"),
            key: "file.bin".into(),
            size: 20,
            bytes_done: 10,
            status: Status::Paused,
            error: None,
            created: 0,
            part_size: 16 * voids3_core::MIB,
            upload_id: None,
            parts: BTreeMap::new(),
            source: None,
            remote_etag: Some("test-etag".into()),
            batch_id: None,
            conflict: None,
            upload_condition: UploadCondition::default(),
            priority: 0,
            settings: TransferSettings::default(),
        };
        engine.save(&job).unwrap();
        (dir, engine, job)
    }
    #[test]
    fn global_interrupt_signals_queued_and_running_workers_without_credentials() {
        let (_dir, engine, _) = fixture();
        let controls: Vec<_> = ["waiting-for-permit", "uploading"]
            .into_iter()
            .map(|id| {
                let control = Control {
                    epoch: None,
                    token: CancellationToken::new(),
                    cancel: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                };
                engine
                    .controls
                    .lock()
                    .unwrap()
                    .insert(id.into(), control.clone());
                control
            })
            .collect();
        assert_eq!(engine.interrupt_all(false).unwrap().len(), 2);
        for control in &controls {
            assert!(control.token.is_cancelled());
            assert!(!control.cancel.load(std::sync::atomic::Ordering::Relaxed));
        }
        assert_eq!(engine.interrupt_all(true).unwrap().len(), 2);
        for control in &controls {
            assert!(control.cancel.load(std::sync::atomic::Ordering::Relaxed));
        }
    }
    #[test]
    fn updater_guard_requires_idle_workers_and_blocks_resume_until_released() {
        let (_dir, engine, mut job) = fixture();
        let guard = engine.begin_maintenance().unwrap();
        assert!(engine.begin_maintenance().is_err());
        let client = S3::new(
            job.target.clone(),
            voids3_core::Credentials {
                access_key: "isolated-test".into(),
                secret_key: "isolated-test".into(),
                session_token: None,
            },
            0,
        )
        .unwrap();
        assert!(engine.resume(client, &job.id).is_err());
        assert_eq!(engine.get(&job.id).unwrap().status, Status::Paused);
        assert!(engine.controls.lock().unwrap().is_empty());
        drop(guard);
        assert!(engine.begin_maintenance().is_ok());
        job.status = Status::Queued;
        engine.save(&job).unwrap();
        assert!(engine.begin_maintenance().is_err());
        job.status = Status::Paused;
        engine.save(&job).unwrap();
        engine.controls.lock().unwrap().insert(
            job.id.clone(),
            Control {
                epoch: None,
                token: CancellationToken::new(),
                cancel: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            },
        );
        assert!(engine.begin_maintenance().is_err());
    }
    #[test]
    fn idle_cancel_removes_only_owned_partial_and_preserves_completed_files() {
        let (_dir, engine, mut job) = fixture();
        std::fs::write(partial_path(&job), "partial").unwrap();
        std::fs::write(&job.local_path, "completed separately").unwrap();
        assert!(engine.request_cancel(&job.id).unwrap());
        assert!(!partial_path(&job).exists());
        assert_eq!(
            std::fs::read_to_string(&job.local_path).unwrap(),
            "completed separately"
        );
        assert_eq!(engine.get(&job.id).unwrap().status, Status::Cancelled);
        job.status = Status::Completed;
        engine.save(&job).unwrap();
        assert!(engine.request_cancel(&job.id).unwrap());
        assert_eq!(engine.get(&job.id).unwrap().status, Status::Completed);
    }
    #[test]
    fn idle_multipart_cancel_requires_cleanup_and_finished_clear_keeps_failures() {
        let (_dir, engine, mut job) = fixture();
        job.direction = Direction::Upload;
        job.upload_id = Some("unfinished-session".into());
        engine.save(&job).unwrap();
        assert!(!engine.request_cancel(&job.id).unwrap());
        assert_eq!(engine.get(&job.id).unwrap().status, Status::Paused);
        job.status = Status::Cancelled;
        engine.save(&job).unwrap();
        engine.clear_finished().unwrap();
        assert!(engine.get(&job.id).is_ok());
        job.upload_id = None;
        job.error = Some("Partial cleanup failed".into());
        engine.save(&job).unwrap();
        engine.clear_finished().unwrap();
        assert!(engine.get(&job.id).is_ok());
        assert!(engine.request_cancel(&job.id).unwrap());
        engine.clear_finished().unwrap();
        assert!(engine.get(&job.id).is_err());
        assert!(
            engine
                .store
                .list::<Transfer>("transfers")
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn close_summary_includes_paused_conflicts_and_retained_cleanup_failures() {
        let (_dir, engine, mut job) = fixture();
        let (guard, summary) = engine.begin_close().unwrap();
        assert_eq!(summary.transfers, 1);
        assert!(summary.needs_confirmation());
        assert!(engine.resume_batches().is_err());
        drop(guard);
        job.status = Status::Conflict;
        engine.save(&job).unwrap();
        let (guard, summary) = engine.begin_close().unwrap();
        assert_eq!(summary.transfers, 1);
        assert!(engine.check_closed().is_err());
        drop(guard);
        job.status = Status::Cancelled;
        job.error = Some("Partial file cleanup failed".into());
        engine.save(&job).unwrap();
        let (_guard, summary) = engine.begin_close().unwrap();
        assert!(summary.needs_confirmation());
        assert!(engine.check_closed().is_err());
        engine.request_cancel(&job.id).unwrap();
        engine.check_closed().unwrap();
        assert_eq!(
            engine.store.list::<Transfer>("transfers").unwrap()[0].status,
            Status::Cancelled
        );
    }
    #[test]
    fn one_partial_cleanup_failure_does_not_prevent_other_idle_cancellations() {
        let (_dir, engine, job) = fixture();
        let partial = partial_path(&job);
        std::fs::create_dir(&partial).unwrap(); // remove_file must fail on this fixture.
        let mut other = job.clone();
        other.id = "another-pending-transfer".into();
        other.direction = Direction::Upload;
        engine.save(&other).unwrap();
        assert_eq!(engine.cancel_idle().unwrap().len(), 2);
        let failed = engine.get(&job.id).unwrap();
        assert_eq!(failed.status, Status::Cancelled);
        assert!(
            failed
                .error
                .unwrap()
                .contains("Partial file cleanup failed")
        );
        assert_eq!(engine.get(&other.id).unwrap().status, Status::Cancelled);
        assert!(engine.check_closed().is_err());
        engine.clear_finished().unwrap();
        assert!(engine.get(&job.id).is_ok());
        std::fs::remove_dir(partial).unwrap();
        engine.request_cancel(&job.id).unwrap();
        engine.check_closed().unwrap();
    }
    #[tokio::test]
    async fn fingerprint_detects_changed_source() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        tokio::fs::write(&path, b"before").await.unwrap();
        let before = Fingerprint::read(&path).await.unwrap();
        tokio::fs::write(&path, b"after change").await.unwrap();
        assert_ne!(before, Fingerprint::read(&path).await.unwrap());
    }
    #[test]
    fn statuses_distinguish_recovery_from_completion() {
        assert!(Status::Completed.terminal());
        assert!(!Status::Failed.terminal());
        assert!(Status::Downloading.active());
    }
}
