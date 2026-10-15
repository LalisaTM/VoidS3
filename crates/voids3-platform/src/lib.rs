//! Blocking local operations; callers in async applications must use spawn_blocking.
use std::{
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};
use voids3_core::{Entry, Error, Result, safe_relative_key};

pub fn data_dir() -> Result<PathBuf> {
    let path = dirs::data_local_dir()
        .ok_or_else(|| Error::Invalid("Cannot locate application data directory".into()))?
        .join("VoidS3");
    fs::create_dir_all(&path)?;
    Ok(path)
}
pub fn home_dir() -> Result<PathBuf> {
    dirs::home_dir().ok_or_else(|| Error::Invalid("Cannot locate home directory".into()))
}
pub fn list_dir(path: &Path, hidden: bool) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for item in fs::read_dir(path)? {
        let item = item?;
        let name = item.file_name().to_string_lossy().into_owned();
        if !hidden && name.starts_with('.') {
            continue;
        }
        let metadata = item.metadata()?;
        // Do not allow directory symlink navigation implicitly through listings.
        if item.file_type()?.is_symlink() {
            continue;
        }
        entries.push(Entry {
            name,
            path: item.path().to_string_lossy().into_owned(),
            is_dir: metadata.is_dir(),
            size: metadata.len(),
            modified: metadata
                .modified()
                .ok()
                .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                .map(|m| m.as_secs() as i64),
            etag: None,
        });
    }
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}
pub fn create_dir(parent: &Path, name: &str) -> Result<()> {
    let relative = safe_relative_key(name)?;
    if relative.components().count() != 1 {
        return Err(Error::Invalid("Enter a single folder name".into()));
    }
    fs::create_dir(parent.join(relative))?;
    Ok(())
}
pub fn rename(path: &Path, name: &str) -> Result<()> {
    let relative = safe_relative_key(name)?;
    if relative.components().count() != 1 {
        return Err(Error::Invalid("Enter a single file name".into()));
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error::Invalid("Cannot rename a filesystem root".into()))?;
    let target = parent.join(relative);
    if target.exists() {
        return Err(Error::Conflict("Destination already exists".into()));
    }
    fs::rename(path, target)?;
    Ok(())
}
pub fn recycle(path: &Path) -> Result<()> {
    if path.parent().is_none() {
        return Err(Error::Invalid("Cannot delete a filesystem root".into()));
    }
    trash::delete(path).map_err(|e| Error::Storage(e.to_string()))
}
pub fn recursive_files(root: &Path) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for item in walkdir::WalkDir::new(root).follow_links(false) {
        let item = item.map_err(|e| Error::Storage(e.to_string()))?;
        if !item.file_type().is_file() {
            continue;
        }
        let metadata = item.metadata().map_err(|e| Error::Storage(e.to_string()))?;
        let name = item
            .path()
            .strip_prefix(root)
            .map_err(|e| Error::Invalid(e.to_string()))?
            .to_string_lossy()
            .replace('\\', "/");
        safe_relative_key(&name)?;
        entries.push(Entry {
            name,
            path: item.path().to_string_lossy().into_owned(),
            is_dir: false,
            size: metadata.len(),
            modified: metadata
                .modified()
                .ok()
                .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                .map(|m| m.as_secs() as i64),
            etag: None,
        });
        if entries.len() > 100_000 {
            return Err(Error::Invalid(
                "This operation is limited to 100,000 files; choose a narrower folder".into(),
            ));
        }
    }
    Ok(entries)
}

/// Refuse symlinks in any existing ancestor. A remote key must never redirect a download.
pub fn download_target(root: &Path, key: &str) -> Result<PathBuf> {
    let relative = safe_relative_key(key)?;
    let root = root.canonicalize()?;
    let mut target = root;
    for part in relative.components() {
        target.push(part);
        match fs::symlink_metadata(&target) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(Error::Invalid(
                    "Download path contains a symbolic link".into(),
                ));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_overwrite_on_rename() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a"), "a").unwrap();
        fs::write(dir.path().join("b"), "b").unwrap();
        assert!(rename(&dir.path().join("a"), "b").is_err());
        assert!(create_dir(dir.path(), "../bad").is_err());
        assert!(download_target(dir.path(), "../bad").is_err());
    }
}
