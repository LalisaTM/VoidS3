//! Small queue DTOs: never send credentials, part maps or 10,000 rows to the UI.
use super::*;
#[derive(Debug, Clone, Serialize)]
pub struct TransferView {
    pub id: String,
    pub profile_id: String,
    pub key: String,
    pub local_path: PathBuf,
    pub direction: Direction,
    pub size: u64,
    pub bytes_done: u64,
    pub status: Status,
    pub error: Option<String>,
    pub created: u64,
    pub upload_id: Option<String>,
    pub batch_id: Option<String>,
    pub conflict: Option<ConflictView>,
    pub source_modified: Option<i64>,
}
#[derive(Debug, Clone, Serialize)]
pub struct ConflictView {
    pub size: u64,
    pub modified: Option<i64>,
    pub can_replace: bool,
}
impl From<&Transfer> for TransferView {
    fn from(job: &Transfer) -> Self {
        Self {
            id: job.id.clone(),
            profile_id: job.profile_id.clone(),
            key: job.key.clone(),
            local_path: job.local_path.clone(),
            direction: job.direction,
            size: job.size,
            bytes_done: job.bytes_done,
            status: job.status,
            error: job.error.clone(),
            created: job.created,
            upload_id: job.upload_id.clone(),
            batch_id: job.batch_id.clone(),
            source_modified: job
                .source
                .as_ref()
                .map(|source| (source.modified_nanos / 1_000_000_000) as i64),
            conflict: job.conflict.as_ref().map(|info| ConflictView {
                size: info.size,
                modified: info.modified,
                can_replace: !info.etag.is_empty(),
            }),
        }
    }
}
#[derive(Debug, Default, Serialize)]
pub struct QueueSummary {
    pub total: usize,
    pub active: usize,
    pub counts: HashMap<String, usize>,
    pub bytes_done: u64,
    pub cancellable: usize,
    pub clearable: usize,
    pub recent_errors: Vec<TransferView>,
    pub batches: Vec<batch::BatchProgress>,
}
#[derive(Serialize)]
pub struct QueuePage {
    pub total: usize,
    pub offset: usize,
    pub items: Vec<TransferView>,
}
impl Engine {
    pub fn summary(&self) -> Result<QueueSummary> {
        let mut summary = QueueSummary::default();
        {
            let jobs = self.jobs.lock().map_err(lock)?;
            summary.total = jobs.len();
            for job in jobs.values() {
                let status = match job.status {
                    Status::Queued => "queued",
                    Status::Preparing => "preparing",
                    Status::Uploading => "uploading",
                    Status::Downloading => "downloading",
                    Status::Paused => "paused",
                    Status::Verifying => "verifying",
                    Status::Completed => "completed",
                    Status::Failed => "failed",
                    Status::Cancelled => "cancelled",
                    Status::Conflict => "conflict",
                    Status::Skipped => "skipped",
                };
                *summary.counts.entry(status.into()).or_default() += 1;
                if job.status.active() || job.status == Status::Queued {
                    summary.active += 1;
                }
                summary.bytes_done = summary.bytes_done.saturating_add(job.bytes_done);
                if !job.status.terminal() || job.upload_id.is_some() || job.error.is_some() {
                    summary.cancellable += 1;
                }
                if job.status.terminal() && job.upload_id.is_none() && job.error.is_none() {
                    summary.clearable += 1;
                }
            }
            let mut errors: Vec<_> = jobs
                .values()
                .filter(|job| job.status == Status::Failed)
                .collect();
            errors.sort_by_key(|job| std::cmp::Reverse(job.created));
            summary.recent_errors = errors
                .into_iter()
                .take(10)
                .map(TransferView::from)
                .collect();
        }
        let feeds = self.feeds.lock().map_err(lock)?;
        summary.batches = feeds
            .values()
            .map(|feed| {
                let mut record = feed.progress.clone();
                record.state = *feed.state.borrow();
                record
            })
            .collect();
        summary.batches.sort_by_key(|record| {
            (
                !matches!(
                    record.state,
                    batch::FeedState::Running | batch::FeedState::Paused
                ),
                record.id.clone(),
            )
        });
        summary.batches.truncate(8);
        Ok(summary)
    }
    pub fn page(&self, filter: &str, offset: usize, limit: usize) -> Result<QueuePage> {
        if limit == 0 || limit > 100 || offset > 100_000 {
            return Err(Error::Invalid("Queue pages contain 1–100 rows".into()));
        }
        if ![
            "all",
            "active",
            "completed",
            "failed",
            "paused",
            "conflict",
            "cancelled",
            "skipped",
        ]
        .contains(&filter)
        {
            return Err(Error::Invalid("Unknown transfer filter".into()));
        }
        let jobs = self.jobs.lock().map_err(lock)?;
        let mut rows: Vec<_> = jobs
            .values()
            .filter(|job| match filter {
                "all" => true,
                "active" => job.status.active() || job.status == Status::Queued,
                "completed" => job.status == Status::Completed,
                "failed" => job.status == Status::Failed,
                "paused" => job.status == Status::Paused,
                "conflict" => job.status == Status::Conflict,
                "cancelled" => job.status == Status::Cancelled,
                "skipped" => job.status == Status::Skipped,
                _ => false,
            })
            .collect();
        rows.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then(a.created.cmp(&b.created))
                .then(a.id.cmp(&b.id))
        });
        Ok(QueuePage {
            total: rows.len(),
            offset,
            items: rows
                .into_iter()
                .skip(offset)
                .take(limit)
                .map(TransferView::from)
                .collect(),
        })
    }
    pub fn transfer(&self, id: &str) -> Result<TransferView> {
        Ok(TransferView::from(&self.get(id)?))
    }
}
