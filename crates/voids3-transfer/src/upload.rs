//! Upload path helpers and shared conflict choices.
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::PathBuf};
use voids3_core::{Error, Result};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictAction {
    Replace,
    Skip,
    KeepBoth,
}
pub(crate) fn numbered_key(key: &str, number: usize) -> Result<String> {
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
