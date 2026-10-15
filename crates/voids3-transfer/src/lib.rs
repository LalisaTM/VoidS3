//! Persistent transfer engine shared by desktop and CLI. Files never cross IPC.
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
}
impl Status {
    pub fn terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled)
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
    size: u64,
    modified_nanos: u128,
}
impl Fingerprint {
    pub async fn read(path: &Path) -> Result<Self> {
        let meta = tokio::fs::metadata(path).await?;
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
    pub priority: i32,
    pub settings: TransferSettings,
}
#[derive(Clone)]
struct Control {
    token: CancellationToken,
    cancel: Arc<std::sync::atomic::AtomicBool>,
}

pub struct Engine {
    store: Arc<Store>,
    jobs: Mutex<HashMap<String, Transfer>>,
    controls: Mutex<HashMap<String, Control>>,
    files: Arc<Semaphore>,
    requests: Arc<Semaphore>,
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
        Ok(Arc::new(Self {
            store,
            jobs: Mutex::new(jobs),
            controls: Mutex::new(HashMap::new()),
            files: Arc::new(Semaphore::new(settings.concurrent_files)),
            requests: Arc::new(Semaphore::new(settings.max_requests)),
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
    pub async fn enqueue(
        self: &Arc<Self>,
        s3: S3,
        direction: Direction,
        local: PathBuf,
        key: String,
        settings: TransferSettings,
    ) -> Result<String> {
        settings.validate()?;
        if key.is_empty() || key.ends_with('/') {
            return Err(Error::Invalid("Select a file object, not a folder".into()));
        }
        let (size, source, etag) = match direction {
            Direction::Upload => {
                s3.profile.writable()?;
                let source = Fingerprint::read(&local).await?;
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
            let parent = local.parent().ok_or_else(|| {
                Error::Invalid("Download destination requires a parent directory".into())
            })?;
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
            priority: 0,
            settings,
        };
        self.save(&job)?;
        self.start(s3, job)?;
        Ok(id)
    }
    fn start(self: &Arc<Self>, s3: S3, job: Transfer) -> Result<()> {
        let control = Control {
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
        {
            let mut controls = self.controls.lock().map_err(lock)?;
            if controls.contains_key(&job.id) {
                return Err(Error::Invalid("Transfer is already running".into()));
            }
            self.save(&job)?;
            controls.insert(job.id.clone(), control.clone());
        }
        let engine = Arc::clone(self);
        tokio::spawn(async move {
            let mut job = job;
            let result = engine.run(&s3, &mut job, &control).await;
            if let Err(error) = result {
                let cancelled = control.cancel.load(std::sync::atomic::Ordering::Relaxed);
                job.status = if cancelled {
                    Status::Cancelled
                } else if matches!(error, Error::Paused) {
                    Status::Paused
                } else {
                    Status::Failed
                };
                job.error = if matches!(error, Error::Paused) {
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
                            job.error = Some(
                                api_error(
                                    "Cancelled, but multipart cleanup failed",
                                    e.as_service_error().and_then(|s| s.meta().code()),
                                )
                                .to_string(),
                            );
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
            match engine.controls.lock() {
                Ok(mut controls) => {
                    if let Err(e) = engine.save(&job) {
                        tracing::error!(transfer_id = %job.id, error = %e, "Cannot persist transfer outcome");
                    }
                    controls.remove(&job.id);
                }
                Err(_) => tracing::error!("Transfer control lock poisoned"),
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
        {
            let controls = self.controls.lock().map_err(lock)?;
            if let Some(control) = controls.get(id) {
                control
                    .cancel
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                control.token.cancel();
                return Ok(());
            }
        }
        let mut job = self.get(id)?;
        if job.status.terminal() {
            return Ok(());
        }
        job.status = Status::Queued;
        let control = Control {
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
                j.status.terminal() && !controls.contains_key(&j.id) && j.upload_id.is_none()
            })
            .map(|j| j.id.clone())
            .collect();
        for id in ids {
            self.store.delete("transfers", &id)?;
            jobs.remove(&id);
        }
        Ok(())
    }
    async fn request(&self, control: &Control) -> Result<OwnedSemaphorePermit> {
        tokio::select! { biased; _ = control.token.cancelled() => Err(Error::Paused), permit = self.requests.clone().acquire_owned() => permit.map_err(|_| Error::Cancelled) }
    }
    async fn run(&self, s3: &S3, job: &mut Transfer, control: &Control) -> Result<()> {
        let _file = tokio::select! { biased; _ = control.token.cancelled() => return Err(Error::Paused), permit = self.files.clone().acquire_owned() => permit.map_err(|_| Error::Cancelled)? };
        job.status = Status::Preparing;
        self.save(job)?;
        match job.direction {
            Direction::Upload => self.upload(s3, job, control).await?,
            Direction::Download => self.download(s3, job, control).await?,
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
        s3.profile.writable()?;
        self.validate_source(job).await?;
        job.status = Status::Uploading;
        self.save(job)?;
        if job.size < job.settings.multipart_threshold {
            let _permit = self.request(control).await?;
            let body = ByteStream::from_path(&job.local_path)
                .await
                .map_err(|_| Error::Invalid("Cannot open upload source".into()))?;
            tokio::select! { biased;
                _ = control.token.cancelled() => return Err(Error::Paused),
                result = s3.client.put_object().bucket(&s3.profile.bucket).key(&job.key).if_none_match("*")
                    .content_type(mime_guess::from_path(&job.local_path).first_or_octet_stream().as_ref()).body(body).send() => {
                    result.map_err(|e| api_error("Upload", e.as_service_error().and_then(|s| s.meta().code())))?;
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
            self.save(job)?;
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
            self.save(job)?;
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
            self.save(job)?;
        }
        drop(pending);
        self.validate_source(job).await?;
        job.status = Status::Verifying;
        self.save(job)?;
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
        // Atomic no-clobber completion. Never overwrite an object that appeared during upload.
        s3.client
            .complete_multipart_upload()
            .bucket(&s3.profile.bucket)
            .key(&job.key)
            .upload_id(&upload_id)
            .if_none_match("*")
            .multipart_upload(
                CompletedMultipartUpload::builder()
                    .set_parts(Some(parts))
                    .build(),
            )
            .send()
            .await
            .map_err(|e| {
                api_error(
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
        self.save(job)?;
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
        self.save(job)?;
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

#[cfg(test)]
mod tests {
    use super::*;
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
