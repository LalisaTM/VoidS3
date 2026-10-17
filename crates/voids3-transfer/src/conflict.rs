use super::*;
use upload::{ConflictAction, numbered_key};
impl Engine {
    pub(super) async fn apply_conflict(
        &self,
        s3: &S3,
        job: &mut Transfer,
        action: ConflictAction,
    ) -> Result<()> {
        if !matches!(action, ConflictAction::Skip) {
            self.validate_source(job).await?;
        }
        if let Some(id) = &job.upload_id {
            if let Err(error) = s3
                .client
                .abort_multipart_upload()
                .bucket(&s3.profile.bucket)
                .key(&job.key)
                .upload_id(id)
                .send()
                .await
                && !error
                    .as_service_error()
                    .is_some_and(|error| error.meta().code() == Some("NoSuchUpload"))
            {
                return Err(api_error(
                    "Clean up conflicting multipart upload",
                    error
                        .as_service_error()
                        .and_then(|error| error.meta().code()),
                ));
            }
            job.upload_id = None;
            job.parts.clear();
            job.bytes_done = 0;
        }
        match action {
            ConflictAction::Skip => {
                job.status = Status::Skipped;
            }
            ConflictAction::Replace => {
                let etag = job
                    .conflict
                    .as_ref()
                    .map(|info| info.etag.clone())
                    .filter(|etag| !etag.is_empty())
                    .ok_or_else(|| {
                        Error::Invalid("Provider returned no ETag; choose Skip or Keep both".into())
                    })?;
                job.upload_condition = UploadCondition::Replace { etag };
            }
            ConflictAction::KeepBoth => {
                let reserved: std::collections::HashSet<_> = self
                    .jobs
                    .lock()
                    .map_err(lock)?
                    .values()
                    .filter(|other| other.profile_id == job.profile_id)
                    .map(|other| other.key.clone())
                    .collect();
                let mut available = None;
                for number in 1..=1000 {
                    let key = numbered_key(&job.key, number)?;
                    if !reserved.contains(&key) && s3.head_optional(&key).await?.is_none() {
                        available = Some(key);
                        break;
                    }
                }
                job.key = available.ok_or_else(|| {
                    Error::Conflict("No free numbered name after 1,000 attempts".into())
                })?;
                job.upload_condition = UploadCondition::Create;
            }
        }
        job.conflict = None;
        job.error = None;
        Ok(())
    }
    /// Return immediately; resolving a large existing-conflict set is bounded
    /// by the same worker backlog as discovery and stops at a global stop.
    pub fn resolve_conflicts(
        self: &Arc<Self>,
        s3: S3,
        id: &str,
        action: ConflictAction,
        all: bool,
    ) -> Result<()> {
        self.resolve_conflicts_at_epoch(
            s3,
            id,
            action,
            all,
            self.stop_epoch.load(std::sync::atomic::Ordering::Acquire),
        )
    }
    pub(super) fn resolve_conflicts_at_epoch(
        self: &Arc<Self>,
        s3: S3,
        id: &str,
        action: ConflictAction,
        all: bool,
        epoch: u64,
    ) -> Result<()> {
        let controls = self.controls.lock().map_err(lock)?;
        self.ensure_open()?;
        if self.stop_epoch.load(std::sync::atomic::Ordering::Acquire) != epoch {
            return Err(Error::Cancelled);
        }
        let selected = self.get(id)?;
        if selected.status != Status::Conflict {
            return Err(Error::Invalid(
                "This transfer is no longer waiting for a decision".into(),
            ));
        }
        s3.profile.writable()?;
        if selected.profile_id != s3.profile.id {
            return Err(Error::Invalid(
                "Wrong connection for upload conflict".into(),
            ));
        }
        if selected.target.endpoint != s3.profile.endpoint
            || selected.target.bucket != s3.profile.bucket
            || selected.target.region != s3.profile.region
            || selected.target.path_style != s3.profile.path_style
        {
            return Err(Error::Conflict("Connection target changed. Restore the original connection or cancel this transfer.".into()));
        }
        if all && let Some(batch) = &selected.batch_id {
            self.set_batch_policy(batch, action)?;
        }
        let ids: Vec<_> = self
            .jobs
            .lock()
            .map_err(lock)?
            .values()
            .filter(|job| {
                job.status == Status::Conflict
                    && (job.id == id
                        || (all
                            && selected.batch_id.is_some()
                            && job.batch_id == selected.batch_id))
            })
            .map(|job| job.id.clone())
            .collect();
        let engine = self.clone();
        let background = self.background_work();
        let stop = self.preparation.lock().map_err(lock)?.clone();
        drop(controls);
        tokio::spawn(async move {
            let _background = background;
            for id in ids {
                while engine.controls.lock().is_ok_and(|controls| {
                    controls.len() >= batch::BACKLOG
                        || engine.closing.load(std::sync::atomic::Ordering::Acquire)
                }) {
                    if engine.stop_epoch.load(std::sync::atomic::Ordering::Acquire) != epoch {
                        return;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                }
                if engine.stop_epoch.load(std::sync::atomic::Ordering::Acquire) != epoch {
                    return;
                }
                let Ok(mut job) = engine.get(&id) else {
                    continue;
                };
                if job.status != Status::Conflict {
                    continue;
                }
                let result = tokio::select! { biased;
                    _ = stop.cancelled() => return,
                    result = engine.apply_conflict(&s3, &mut job, action) => result,
                };
                if let Err(error) = result {
                    job.error = Some(error.to_string());
                    // Leave a resolvable conflict visible, including cleanup failures.
                } else if job.status != Status::Skipped {
                    job.status = Status::Queued;
                }
                let target = s3.clone();
                let worker_engine = engine.clone();
                let _ = tokio::task::spawn_blocking(move || -> Result<()> {
                    if job.status == Status::Queued {
                        let control = Control {
                            token: CancellationToken::new(),
                            cancel: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                            epoch: Some(epoch),
                        };
                        worker_engine.start_controlled(target, job, control)
                    } else {
                        let _controls = worker_engine.controls.lock().map_err(lock)?;
                        if worker_engine
                            .stop_epoch
                            .load(std::sync::atomic::Ordering::Acquire)
                            == epoch
                            && worker_engine.get(&id)?.status == Status::Conflict
                        {
                            worker_engine.save(&job)?;
                        }
                        Ok(())
                    }
                })
                .await;
            }
        });
        Ok(())
    }
}
