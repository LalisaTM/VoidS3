//! Desktop shutdown admission barrier and durable worker drain.
use super::*;
use std::sync::atomic::Ordering;

/// Blocks admissions while the native close dialog is open and cleanup runs.
/// Dropping it after "Keep open" or an error permits new work again.
pub struct CloseGuard(Arc<Engine>);
impl Drop for CloseGuard {
    fn drop(&mut self) {
        self.0.closing.store(false, Ordering::Release);
    }
}

pub(super) struct BackgroundWork(Arc<Engine>);
impl Drop for BackgroundWork {
    fn drop(&mut self) {
        self.0.background.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Includes remote download discovery and credential lookup before jobs exist.
pub struct Preparation {
    _background: BackgroundWork,
    token: CancellationToken,
    engine: Arc<Engine>,
}
impl Drop for Preparation {
    fn drop(&mut self) {
        self.engine.preparing.fetch_sub(1, Ordering::AcqRel);
    }
}
impl Preparation {
    pub async fn run<T>(&self, work: impl std::future::Future<Output = Result<T>>) -> Result<T> {
        tokio::select! { biased;
            _ = self.token.cancelled() => Err(Error::Cancelled),
            result = work => result,
        }
    }
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct CloseSummary {
    pub transfers: usize,
    pub batches: usize,
    pub background_tasks: usize,
}
impl CloseSummary {
    pub fn needs_confirmation(&self) -> bool {
        self.transfers != 0 || self.batches != 0 || self.background_tasks != 0
    }
}

impl Engine {
    pub fn begin_preparation(self: &Arc<Self>) -> Result<Preparation> {
        let _controls = self.controls.lock().map_err(lock)?;
        self.ensure_open()?;
        if self.maintenance.load(Ordering::Acquire) {
            return Err(Error::Conflict(
                "An application update is in progress".into(),
            ));
        }
        let token = self.preparation.lock().map_err(lock)?.clone();
        self.preparing.fetch_add(1, Ordering::AcqRel);
        Ok(Preparation {
            _background: self.background_work(),
            token,
            engine: self.clone(),
        })
    }
    pub(super) fn ensure_open(&self) -> Result<()> {
        if self.closing.load(Ordering::Acquire) {
            return Err(Error::Conflict(
                "VoidS3 is closing. Keep the app open before starting more work.".into(),
            ));
        }
        Ok(())
    }
    // Call while holding controls, making registration atomic with begin_close.
    pub(super) fn background_work(self: &Arc<Self>) -> shutdown::BackgroundWork {
        self.background.fetch_add(1, Ordering::AcqRel);
        shutdown::BackgroundWork(self.clone())
    }
    pub fn begin_close(self: &Arc<Self>) -> Result<(CloseGuard, CloseSummary)> {
        let controls = self.controls.lock().map_err(lock)?;
        if self.maintenance.load(Ordering::Acquire) {
            return Err(Error::Conflict("An application update is in progress. Wait for it to finish before closing VoidS3.".into()));
        }
        self.ensure_open()?;
        let jobs = self.jobs.lock().map_err(lock)?;
        let transfers = jobs
            .values()
            .filter(|job| {
                !job.status.terminal()
                    || job.upload_id.is_some()
                    || controls.contains_key(&job.id)
                    || (job.status == Status::Cancelled && job.error.is_some())
            })
            .count();
        drop(jobs);
        let batches = self
            .feeds
            .lock()
            .map_err(lock)?
            .values()
            .filter(|feed| {
                matches!(
                    *feed.state.borrow(),
                    batch::FeedState::Running | batch::FeedState::Paused
                )
            })
            .count();
        self.closing.store(true, Ordering::Release);
        Ok((
            CloseGuard(self.clone()),
            CloseSummary {
                transfers,
                batches,
                background_tasks: self.background.load(Ordering::Acquire),
            },
        ))
    }
    /// Do not equate a cancelled status with completed provider/filesystem cleanup.
    pub fn is_drained(&self) -> Result<bool> {
        let controls = self.controls.lock().map_err(lock)?;
        Ok(controls.is_empty() && self.background.load(Ordering::Acquire) == 0)
    }
    /// Called off the async executor after draining; retain failed cleanup for retry.
    pub fn check_closed(&self) -> Result<()> {
        if !self.is_drained()? {
            return Err(Error::Conflict("Transfer cleanup is still running".into()));
        }
        let jobs = self.jobs.lock().map_err(lock)?;
        let pending = jobs
            .values()
            .filter(|job| {
                !job.status.terminal()
                    || job.upload_id.is_some()
                    || (job.status == Status::Cancelled && job.error.is_some())
            })
            .count();
        if pending != 0 {
            return Err(Error::Conflict(format!(
                "Cleanup could not finish for {pending} transfer(s). Their saved records are retained so cancellation can be retried."
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn close_does_not_cancel_preparation_until_confirmed_then_drains_it() {
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(Store::open(&root.path().join("test.db")).unwrap());
        let engine = Engine::new(store, TransferSettings::default()).unwrap();
        let preparation = engine.begin_preparation().unwrap();
        assert!(engine.begin_maintenance().is_err());
        let (guard, summary) = engine.begin_close().unwrap();
        assert!(summary.needs_confirmation());
        assert!(engine.begin_preparation().is_err());
        assert!(!preparation.token.is_cancelled());
        drop(guard);
        assert!(!preparation.token.is_cancelled());
        let (_guard, _) = engine.begin_close().unwrap();
        engine.interrupt_all(true).unwrap();
        let result: Result<()> = preparation.run(std::future::pending()).await;
        assert!(matches!(result, Err(Error::Cancelled)));
        assert!(!engine.is_drained().unwrap());
        drop(preparation);
        assert!(engine.is_drained().unwrap());
        engine.check_closed().unwrap();
    }
    #[test]
    fn close_gate_reopens_without_cancelling_and_excludes_updater() {
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(Store::open(&root.path().join("test.db")).unwrap());
        let engine = Engine::new(store, TransferSettings::default()).unwrap();
        let (guard, summary) = engine.begin_close().unwrap();
        assert!(!summary.needs_confirmation());
        assert!(engine.ensure_open().is_err());
        assert!(engine.begin_close().is_err());
        assert!(engine.begin_maintenance().is_err());
        drop(guard);
        engine.ensure_open().unwrap();
        let maintenance = engine.begin_maintenance().unwrap();
        assert!(engine.begin_close().is_err());
        drop(maintenance);
        assert!(engine.begin_close().is_ok());
    }
    #[test]
    fn drain_includes_background_discovery_and_conflict_tasks() {
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(Store::open(&root.path().join("test.db")).unwrap());
        let engine = Engine::new(store, TransferSettings::default()).unwrap();
        let task = engine.background_work();
        let (_guard, summary) = engine.begin_close().unwrap();
        assert!(summary.needs_confirmation());
        assert!(!engine.is_drained().unwrap());
        assert!(engine.check_closed().is_err());
        drop(task);
        assert!(engine.is_drained().unwrap());
        engine.check_closed().unwrap();
    }
}
