//! Bounded local discovery feeds workers without a batch-wide remote preflight.
use super::*;
use std::collections::VecDeque;
use tokio::sync::watch;
use upload::ConflictAction;

const CHUNK: usize = 32;
pub const BACKLOG: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedState {
    Running,
    Paused,
    Cancelled,
    Finished,
    Interrupted,
    Failed,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchProgress {
    pub id: String,
    pub profile_id: String,
    pub discovered: usize,
    pub state: FeedState,
    pub error: Option<String>,
    pub policy: Option<ConflictAction>,
}
pub(super) struct Feed {
    pub progress: BatchProgress,
    pub state: watch::Sender<FeedState>,
}
pub(super) fn recover(store: &Store) -> Result<HashMap<String, Feed>> {
    let mut feeds = HashMap::new();
    for mut progress in store.list::<BatchProgress>("upload_batches")? {
        if matches!(progress.state, FeedState::Running | FeedState::Paused) {
            progress.state = FeedState::Interrupted;
            progress.error = Some("Discovery was interrupted. Queued files are retained; select the folder again to add remaining files. Existing objects still require a conflict decision.".into());
            store.put("upload_batches", &progress.id, &progress)?;
        }
        if let Some(policy) = store.get("upload_policies", &progress.id)? {
            progress.policy = Some(policy);
        }
        let (state, _) = watch::channel(progress.state);
        feeds.insert(progress.id.clone(), Feed { progress, state });
    }
    Ok(feeds)
}

/// Traversal state moves to a blocking thread for one bounded chunk at a time.
/// It holds no executor or blocking-pool thread while the batch is paused.
struct Scanner {
    roots: VecDeque<PathBuf>,
    walk: Option<(PathBuf, String, walkdir::IntoIter)>,
    prefix: String,
    seen: HashMap<String, PathBuf>,
}
type Source = (PathBuf, String, Fingerprint);
impl Scanner {
    fn new(paths: Vec<PathBuf>, prefix: String) -> Result<Self> {
        if paths.is_empty() || paths.len() > 100_000 {
            return Err(Error::Invalid(
                "Select between 1 and 100,000 upload items".into(),
            ));
        }
        Ok(Self {
            roots: paths.into(),
            walk: None,
            prefix,
            seen: HashMap::new(),
        })
    }
    fn chunk(&mut self) -> Result<(Vec<Source>, bool)> {
        let mut files = Vec::new();
        // Bound traversal work too, so empty directories do not create an
        // uninterruptible walk. No network requests occur in discovery.
        for _ in 0..512 {
            let (path, key) = if let Some((root, target, walk)) = &mut self.walk {
                match walk.next() {
                    Some(item) => {
                        let item = item.map_err(lock)?;
                        if !item.file_type().is_file() {
                            continue;
                        }
                        let relative = item
                            .path()
                            .strip_prefix(root)
                            .map_err(lock)?
                            .to_str()
                            .ok_or_else(|| {
                                Error::Invalid("Upload requires UTF-8 filenames".into())
                            })?
                            .replace('\\', "/");
                        voids3_core::safe_relative_key(&relative)?;
                        (item.path().to_owned(), format!("{target}{relative}"))
                    }
                    None => {
                        self.walk = None;
                        continue;
                    }
                }
            } else if let Some(path) = self.roots.pop_front() {
                let metadata = std::fs::symlink_metadata(&path)?;
                if metadata.file_type().is_symlink() {
                    return Err(Error::Invalid(
                        "Choose the actual path instead of a symbolic link".into(),
                    ));
                }
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| {
                        Error::Invalid(
                            "Upload requires a UTF-8 filename, not a filesystem root".into(),
                        )
                    })?;
                let key = format!("{}{name}", self.prefix);
                if metadata.is_dir() {
                    self.walk = Some((
                        path.clone(),
                        format!("{key}/"),
                        walkdir::WalkDir::new(path).follow_links(false).into_iter(),
                    ));
                    continue;
                }
                (path, key)
            } else {
                return Ok((files, true));
            };
            if key.len() > 1024 || key.ends_with('/') {
                return Err(Error::Invalid(
                    "Upload key exceeds the S3 filename limit".into(),
                ));
            }
            let path = path.canonicalize()?;
            if let Some(previous) = self.seen.get(&key) {
                if previous == &path {
                    continue;
                }
                return Err(Error::Conflict(format!(
                    "Multiple selected files target {key}; rename them or upload separately. Earlier discovered files remain in the queue."
                )));
            }
            if self.seen.len() >= 100_000 {
                return Err(Error::Invalid("A batch is limited to 100,000 files".into()));
            }
            self.seen.insert(key.clone(), path.clone());
            let source = Fingerprint::read_blocking(&path)?;
            files.push((path, key, source));
            if files.len() == CHUNK {
                break;
            }
        }
        Ok((files, false))
    }
}

impl Engine {
    pub fn start_batch(
        self: &Arc<Self>,
        s3: S3,
        paths: Vec<PathBuf>,
        prefix: String,
        settings: TransferSettings,
    ) -> Result<String> {
        s3.profile.writable()?;
        settings.validate()?;
        let scanner = Scanner::new(paths, prefix)?;
        let controls = self.controls.lock().map_err(lock)?;
        if self.maintenance.load(std::sync::atomic::Ordering::Acquire) {
            return Err(Error::Conflict(
                "An application update is in progress".into(),
            ));
        }
        let mut feeds = self.feeds.lock().map_err(lock)?;
        if feeds
            .values()
            .filter(|feed| matches!(*feed.state.borrow(), FeedState::Running | FeedState::Paused))
            .count()
            >= 4
        {
            return Err(Error::Invalid(
                "At most four upload batches can discover files at once".into(),
            ));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let progress = BatchProgress {
            id: id.clone(),
            profile_id: s3.profile.id.clone(),
            discovered: 0,
            state: FeedState::Running,
            error: None,
            policy: None,
        };
        self.store.put("upload_batches", &id, &progress)?;
        let (state, receiver) = watch::channel(FeedState::Running);
        feeds.insert(id.clone(), Feed { progress, state });
        drop(feeds);
        drop(controls);
        let engine = self.clone();
        let batch_id = id.clone();
        tokio::spawn(async move {
            let result = engine
                .feed_batch(&s3, &batch_id, scanner, settings, receiver)
                .await;
            let outcome = match result {
                Ok(()) => (FeedState::Finished, None),
                Err(Error::Cancelled) => (FeedState::Cancelled, None),
                Err(error) => (FeedState::Failed, Some(error.to_string())),
            };
            let _ = engine.update_feed(&batch_id, 0, Some(outcome)).await;
        });
        Ok(id)
    }
    async fn feed_batch(
        self: &Arc<Self>,
        s3: &S3,
        id: &str,
        mut scanner: Scanner,
        settings: TransferSettings,
        mut state: watch::Receiver<FeedState>,
    ) -> Result<()> {
        loop {
            self.feed_ready(&mut state).await?;
            let (next, chunk) = tokio::task::spawn_blocking(move || {
                let chunk = scanner.chunk();
                (scanner, chunk)
            })
            .await
            .map_err(lock)?;
            scanner = next;
            let (files, done) = chunk?;
            for (path, key, source) in files {
                loop {
                    self.feed_ready(&mut state).await?;
                    let intent = UploadIntent {
                        source: source.clone(),
                        condition: UploadCondition::Create,
                        batch_id: Some(id.to_owned()),
                    };
                    match self
                        .enqueue_upload(
                            s3.clone(),
                            path.clone(),
                            key.clone(),
                            settings.clone(),
                            intent,
                        )
                        .await
                    {
                        Ok(_) => break,
                        Err(Error::Paused) => continue,
                        Err(error) => return Err(error),
                    }
                }
                self.update_feed(id, 1, None).await?;
            }
            if done {
                return Ok(());
            }
        }
    }
    async fn feed_ready(&self, state: &mut watch::Receiver<FeedState>) -> Result<()> {
        loop {
            match *state.borrow_and_update() {
                FeedState::Cancelled => return Err(Error::Cancelled),
                FeedState::Failed => return Err(Error::Provider("Discovery stopped after a connection or permission failure. Fix the connection, then start the remaining files again.".into())),
                FeedState::Running if self.controls.lock().map_err(lock)?.len() < BACKLOG => {
                    return Ok(());
                }
                _ => {}
            }
            tokio::select! {
                changed = state.changed() => changed.map_err(|_| Error::Cancelled)?,
                _ = tokio::time::sleep(std::time::Duration::from_millis(25)) => {}
            }
        }
    }
    async fn update_feed(
        &self,
        id: &str,
        added: usize,
        outcome: Option<(FeedState, Option<String>)>,
    ) -> Result<()> {
        let snapshot = {
            let mut feeds = self.feeds.lock().map_err(lock)?;
            let feed = feeds.get_mut(id).ok_or(Error::Cancelled)?;
            feed.progress.discovered += added;
            let finished = outcome.is_some();
            if let Some((state, error)) = outcome {
                feed.state.send_replace(state);
                feed.progress.error = error;
            }
            feed.progress.state = *feed.state.borrow();
            // Discovery counters need not fsync for each file. Transfer records
            // themselves are durable before workers start.
            if !finished && feed.progress.discovered % CHUNK != 0 {
                return Ok(());
            }
            feed.progress.clone()
        };
        let store = self.store.clone();
        tokio::task::spawn_blocking(move || store.put("upload_batches", &snapshot.id, &snapshot))
            .await
            .map_err(lock)?
    }
    pub fn resume_batches(&self) -> Result<()> {
        let _controls = self.controls.lock().map_err(lock)?;
        if self.maintenance.load(std::sync::atomic::Ordering::Acquire) {
            return Err(Error::Conflict(
                "An application update is in progress".into(),
            ));
        }
        for feed in self.feeds.lock().map_err(lock)?.values() {
            let current = *feed.state.borrow();
            if current == FeedState::Paused {
                feed.state.send_replace(FeedState::Running);
            }
        }
        Ok(())
    }
    pub fn batch_policy(&self, id: Option<&str>) -> Result<Option<ConflictAction>> {
        Ok(id.and_then(|id| self.feeds.lock().ok()?.get(id)?.progress.policy))
    }
    pub(super) fn fail_batch(&self, id: &str, error: &str) {
        if let Ok(mut feeds) = self.feeds.lock()
            && let Some(feed) = feeds.get_mut(id)
        {
            let current = *feed.state.borrow();
            if matches!(current, FeedState::Running | FeedState::Paused) {
                feed.progress.error = Some(error.into());
                feed.state.send_replace(FeedState::Failed);
            }
        }
    }
    pub fn set_batch_policy(&self, id: &str, policy: ConflictAction) -> Result<()> {
        let mut feeds = self.feeds.lock().map_err(lock)?;
        let feed = feeds
            .get_mut(id)
            .ok_or_else(|| Error::Invalid("Upload batch not found".into()))?;
        feed.progress.policy = Some(policy);
        feed.progress.state = *feed.state.borrow();
        self.store.put("upload_policies", id, &policy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restart_marks_unfinished_discovery_and_keeps_its_saved_policy() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::open(&root.path().join("test.db")).unwrap();
        let progress = BatchProgress {
            id: "batch".into(),
            profile_id: "profile".into(),
            discovered: 64,
            state: FeedState::Paused,
            error: None,
            policy: None,
        };
        store.put("upload_batches", "batch", &progress).unwrap();
        store
            .put("upload_policies", "batch", &ConflictAction::Skip)
            .unwrap();
        let recovered = recover(&store).unwrap();
        assert_eq!(recovered["batch"].progress.state, FeedState::Interrupted);
        assert_eq!(
            recovered["batch"].progress.policy,
            Some(ConflictAction::Skip)
        );
        assert!(recovered["batch"].progress.error.is_some());
    }
    #[test]
    fn discovery_returns_a_bounded_chunk_without_walking_the_entire_tree() {
        let root = tempfile::tempdir().unwrap();
        for index in 0..100 {
            std::fs::write(root.path().join(format!("{index}.jpg")), b"x").unwrap();
        }
        let mut scanner = Scanner::new(vec![root.path().to_owned()], "images/".into()).unwrap();
        let (chunk, done) = scanner.chunk().unwrap();
        assert_eq!(chunk.len(), CHUNK);
        assert!(!done);
        assert_eq!(scanner.seen.len(), CHUNK);
        assert!(chunk.iter().all(|(_, key, _)| key.starts_with("images/")));
    }
}
