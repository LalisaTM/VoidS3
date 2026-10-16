//! Upload preflight and explicit destination decisions. No workers start until
//! every conflict has a choice; the worker also enforces the captured ETag.
use crate::{Engine, Fingerprint, UploadCondition, UploadIntent};
use futures::{StreamExt, TryStreamExt, stream};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};
use voids3_core::{Error, Result, TransferSettings};
use voids3_s3::{ObjectInfo, S3};

pub async fn sources(paths: Vec<PathBuf>, prefix: &str) -> Result<Vec<(PathBuf, String)>> {
    if paths.is_empty() || paths.len() > 100_000 {
        return Err(Error::Invalid(
            "Select between 1 and 100,000 upload items".into(),
        ));
    }
    let mut pending = Vec::new();
    let mut keys = HashSet::new();
    let mut seen = HashSet::new();
    for path in paths {
        let meta = tokio::fs::symlink_metadata(&path).await?;
        if meta.file_type().is_symlink() {
            return Err(Error::Invalid(
                "Symbolic link uploads must be selected by their actual path".into(),
            ));
        }
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                Error::Invalid("Upload requires a UTF-8 filename, not a filesystem root".into())
            })?;
        let entries = if meta.is_dir() {
            let root = path.clone();
            tokio::task::spawn_blocking(move || voids3_platform::recursive_files(&root))
                .await
                .map_err(|e| Error::Storage(e.to_string()))??
                .into_iter()
                .map(|entry| {
                    (
                        PathBuf::from(entry.path),
                        format!("{prefix}{filename}/{}", entry.name),
                    )
                })
                .collect()
        } else {
            vec![(path.clone(), format!("{prefix}{filename}"))]
        };
        for (path, key) in entries {
            let path = tokio::fs::canonicalize(path).await?;
            if !seen.insert((path.clone(), key.clone())) {
                continue;
            }
            if !keys.insert(key.clone()) {
                return Err(Error::Conflict(format!(
                    "Multiple selected files target {key}. Rename them or upload them separately to choose how to resolve the conflict."
                )));
            }
            if key.len() > 1024 || key.ends_with('/') {
                return Err(Error::Invalid(
                    "Upload destination must be a file key of at most 1024 UTF-8 bytes".into(),
                ));
            }
            pending.push((path, key));
            if pending.len() > 100_000 {
                return Err(Error::Invalid(
                    "Upload batch is limited to 100,000 files".into(),
                ));
            }
        }
    }
    Ok(pending)
}

#[derive(Debug, Serialize)]
pub struct FileDetails {
    pub size: u64,
    pub modified: Option<i64>,
}
#[derive(Debug, Serialize)]
pub struct UploadConflict {
    pub index: usize,
    pub path: PathBuf,
    pub key: String,
    pub source: FileDetails,
    pub destination: FileDetails,
    pub can_replace: bool,
}
#[derive(Debug, Serialize)]
pub struct UploadPreview {
    pub total: usize,
    pub conflicts: Vec<UploadConflict>,
}
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictAction {
    Replace,
    Skip,
    KeepBoth,
}
#[derive(Debug, Deserialize)]
pub struct UploadChoice {
    pub index: usize,
    pub action: ConflictAction,
}
#[derive(Debug, Default, Serialize)]
pub struct UploadResult {
    pub queued: Vec<String>,
    pub skipped: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}
struct Item {
    path: PathBuf,
    key: String,
    source: Fingerprint,
    existing: Option<ObjectInfo>,
}
pub struct UploadBatch {
    s3: S3,
    settings: TransferSettings,
    items: Vec<Item>,
}
impl UploadBatch {
    pub fn profile(&self) -> &voids3_core::Profile {
        &self.s3.profile
    }
    pub async fn prepare(
        s3: S3,
        paths: Vec<PathBuf>,
        prefix: &str,
        settings: TransferSettings,
    ) -> Result<Self> {
        s3.profile.writable()?;
        settings.validate()?;
        let pending = sources(paths, prefix).await?;
        let items = stream::iter(pending)
            .map(|(path, key)| {
                let s3 = &s3;
                async move {
                    let source = Fingerprint::read(&path).await?;
                    let existing = s3.head_optional(&key).await?;
                    Ok::<_, Error>(Item {
                        path,
                        key,
                        source,
                        existing,
                    })
                }
            })
            .buffered(settings.max_requests.min(16))
            .try_collect()
            .await?;
        Ok(Self {
            s3,
            settings,
            items,
        })
    }
    pub fn preview(&self) -> UploadPreview {
        UploadPreview {
            total: self.items.len(),
            conflicts: self
                .items
                .iter()
                .enumerate()
                .filter_map(|(index, item)| {
                    item.existing.as_ref().map(|existing| UploadConflict {
                        index,
                        path: item.path.clone(),
                        key: item.key.clone(),
                        source: FileDetails {
                            size: item.source.size,
                            modified: Some((item.source.modified_nanos / 1_000_000_000) as i64),
                        },
                        destination: FileDetails {
                            size: existing.size,
                            modified: existing.modified,
                        },
                        can_replace: !existing.etag.is_empty(),
                    })
                })
                .collect(),
        }
    }
    pub async fn enqueue(
        self,
        engine: &Arc<Engine>,
        choices: Vec<UploadChoice>,
    ) -> Result<UploadResult> {
        let mut decisions = HashMap::new();
        for choice in choices {
            if !self
                .items
                .get(choice.index)
                .is_some_and(|item| item.existing.is_some())
                || decisions.insert(choice.index, choice.action).is_some()
            {
                return Err(Error::Invalid(
                    "Invalid or duplicate upload conflict choice".into(),
                ));
            }
        }
        if decisions.len()
            != self
                .items
                .iter()
                .filter(|item| item.existing.is_some())
                .count()
        {
            return Err(Error::Invalid(
                "Choose Replace, Skip or Keep both for every upload conflict".into(),
            ));
        }
        let jobs = engine.list()?;
        let mut reserved: HashSet<_> = self.items.iter().map(|item| item.key.clone()).collect();
        reserved.extend(
            jobs.iter()
                .filter(|job| job.profile_id == self.s3.profile.id)
                .map(|job| job.key.clone()),
        );
        let mut ready = Vec::new();
        let mut result = UploadResult::default();
        for (index, item) in self.items.into_iter().enumerate() {
            let action = decisions.get(&index);
            if matches!(action, Some(ConflictAction::Skip)) {
                result.skipped += 1;
                continue;
            }
            if Fingerprint::read(&item.path).await? != item.source {
                return Err(Error::Conflict(format!(
                    "{} changed since preflight. Upload again to review it.",
                    item.path.display()
                )));
            }
            let (key, condition) = match action {
                Some(ConflictAction::Replace) => {
                    let etag = item
                        .existing
                        .as_ref()
                        .expect("validated conflict")
                        .etag
                        .clone();
                    if etag.is_empty() {
                        return Err(Error::Invalid(
                            "Provider returned no ETag; choose Skip or Keep both".into(),
                        ));
                    }
                    (item.key, UploadCondition::Replace { etag })
                }
                Some(ConflictAction::KeepBoth) => {
                    let mut available = None;
                    for number in 1..=1000 {
                        let key = numbered_key(&item.key, number)?;
                        if !reserved.contains(&key) && self.s3.head_optional(&key).await?.is_none()
                        {
                            reserved.insert(key.clone());
                            available = Some(key);
                            break;
                        }
                    }
                    (
                        available.ok_or_else(|| {
                            Error::Conflict(
                                "Cannot find a free Keep both name after 1,000 attempts".into(),
                            )
                        })?,
                        UploadCondition::Create,
                    )
                }
                _ => (item.key, UploadCondition::Create),
            };
            ready.push((
                item.path,
                key,
                UploadIntent {
                    source: item.source,
                    condition,
                },
            ));
        }
        if ready.len() + jobs.len() > 100_000 {
            return Err(Error::Invalid("Queue limit is 100,000 files. Clear completed history or choose a smaller selection".into()));
        }
        // Complete validation/name allocation before starting any workers. Report
        // partial enqueue failures explicitly rather than hiding queued siblings.
        for (path, key, intent) in ready {
            match engine
                .enqueue_upload(
                    self.s3.clone(),
                    path,
                    key.clone(),
                    self.settings.clone(),
                    intent,
                )
                .await
            {
                Ok(id) => result.queued.push(id),
                Err(error) => {
                    result.failed += 1;
                    if result.errors.len() < 20 {
                        result.errors.push(format!("{key}: {error}"));
                    }
                }
            }
        }
        Ok(result)
    }
}
fn numbered_key(key: &str, number: usize) -> Result<String> {
    let (prefix, name) = key
        .rsplit_once('/')
        .map_or(("", key), |(prefix, name)| (&key[..prefix.len() + 1], name));
    let (stem, extension) = name
        .rsplit_once('.')
        .filter(|(stem, _)| !stem.is_empty())
        .map_or((name, String::new()), |(stem, extension)| {
            (stem, format!(".{extension}"))
        });
    let key = format!("{prefix}{stem} ({number}){extension}");
    if key.len() > 1024 {
        return Err(Error::Invalid(
            "Keep both name exceeds the S3 key limit; rename the local file".into(),
        ));
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keeps_extensions_folders_unicode_and_dotfiles() {
        assert_eq!(
            numbered_key("docs/report.tar.gz", 2).unwrap(),
            "docs/report.tar (2).gz"
        );
        assert_eq!(numbered_key(".env", 1).unwrap(), ".env (1)");
        assert_eq!(numbered_key("dír/readme", 3).unwrap(), "dír/readme (3)");
        assert!(numbered_key(&"x".repeat(1024), 1).is_err());
    }
    #[tokio::test]
    async fn expands_folders_deduplicates_selection_and_rejects_colliding_sources() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        tokio::fs::create_dir_all(a.join("nested")).await.unwrap();
        tokio::fs::create_dir(&b).await.unwrap();
        let file = a.join("nested/same.txt");
        tokio::fs::write(&file, "one").await.unwrap();
        tokio::fs::write(b.join("same.txt"), "two").await.unwrap();
        let expanded = sources(vec![a.clone(), a.clone()], "target/")
            .await
            .unwrap();
        assert_eq!(expanded.len(), 1);
        assert_eq!(expanded[0].1, "target/a/nested/same.txt");
        assert!(sources(vec![file, b.join("same.txt")], "").await.is_err());
    }
}
