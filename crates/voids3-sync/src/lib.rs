//! Conservative sync preview: existing pairs require explicit conflict resolution.
//! S3 LastModified is never treated as local mtime and ETags are never assumed to be MD5.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use voids3_core::{Entry, Result, safe_relative_key};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Upload,
    Download,
    Bidirectional,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Upload,
    Download,
    Conflict,
    Skip,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub key: String,
    pub action: Action,
    pub size: u64,
    pub reason: String,
}
pub fn preview(local: &[Entry], remote: &[Entry], mode: Mode) -> Result<Vec<Change>> {
    let local: BTreeMap<_, _> = local
        .iter()
        .filter(|e| !e.is_dir)
        .map(|e| (e.name.as_str(), e))
        .collect();
    let remote: BTreeMap<_, _> = remote
        .iter()
        .filter(|e| !e.is_dir && !e.name.ends_with('/'))
        .map(|e| (e.name.as_str(), e))
        .collect();
    let mut keys: Vec<_> = local.keys().chain(remote.keys()).copied().collect();
    keys.sort_unstable();
    keys.dedup();
    keys.into_iter().filter(|key| !key.split('/').any(|part| part == ".git" || part.starts_with(".voids3-"))).map(|key| {
        safe_relative_key(key)?;
        let (action,size,reason) = match (local.get(key),remote.get(key)) {
            (Some(l),None) if mode != Mode::Download => (Action::Upload,l.size,"Only local; create remote object"),
            (None,Some(r)) if mode != Mode::Upload => (Action::Download,r.size,"Only remote; create local file"),
            (Some(l),Some(_)) => (Action::Conflict,l.size,"Both exist. Size, S3 ETag, and LastModified alone cannot prove equality; existing files are preserved"),
            (Some(l),None) => (Action::Skip,l.size,"Outside the selected sync direction"),
            (None,Some(r)) => (Action::Skip,r.size,"Outside the selected sync direction"),
            (None,None) => unreachable!("key came from the union"),
        };
        Ok(Change { key: key.into(), action, size, reason: reason.into() })
    }).collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    fn entry(name: &str) -> Entry {
        Entry {
            name: name.into(),
            path: name.into(),
            is_dir: false,
            size: 1,
            modified: None,
            etag: Some("not-md5".into()),
            storage_class: None,
        }
    }
    #[test]
    fn bidirectional_never_deletes_or_assumes_equal_sizes_mean_equal_content() {
        let result = preview(
            &[entry("both"), entry("local")],
            &[entry("both"), entry("remote")],
            Mode::Bidirectional,
        )
        .unwrap();
        assert_eq!(
            result.iter().map(|c| c.action).collect::<Vec<_>>(),
            vec![Action::Conflict, Action::Upload, Action::Download]
        );
    }
    #[test]
    fn traversal_and_git_are_handled() {
        assert!(preview(&[], &[entry("../escape")], Mode::Download).is_err());
        assert!(
            preview(&[entry(".git/config")], &[], Mode::Upload)
                .unwrap()
                .is_empty()
        );
    }
}
