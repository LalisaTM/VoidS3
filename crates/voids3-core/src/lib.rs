//! Shared types and validation. No GUI, filesystem, or network dependencies.
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

pub const MIB: u64 = 1024 * 1024;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid configuration: {0}")]
    Invalid(String),
    #[error("Local filesystem: {0}")]
    Io(#[from] std::io::Error),
    #[error("State storage: {0}")]
    Storage(String),
    #[error("Credential store: {0}. Unlock your OS keychain and try again.")]
    Credentials(String),
    #[error("{0}")]
    Provider(String),
    #[error("Conflict: {0}")]
    Conflict(String),
    #[error("Transfer paused")]
    Paused,
    #[error("Transfer cancelled")]
    Cancelled,
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    #[default]
    R2,
    Aws,
    B2,
    Wasabi,
    Minio,
    Spaces,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub provider: Provider,
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    #[serde(default)]
    pub prefix: String,
    #[serde(default = "yes")]
    pub path_style: bool,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub allow_http: bool,
}
fn yes() -> bool {
    true
}
impl Profile {
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() || self.region.trim().is_empty() {
            return Err(Error::Invalid(
                "Name and signing region are required".into(),
            ));
        }
        if uuid::Uuid::parse_str(&self.id).is_err() {
            return Err(Error::Invalid("Profile ID must be a UUID".into()));
        }
        let endpoint =
            url::Url::parse(&self.endpoint).map_err(|e| Error::Invalid(e.to_string()))?;
        if endpoint.host_str().is_none()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
            || endpoint.path() != "/"
        {
            return Err(Error::Invalid("Endpoint must contain only scheme, host, and optional port; never credentials or a path".into()));
        }
        if endpoint.scheme() != "https" && !(endpoint.scheme() == "http" && self.allow_http) {
            return Err(Error::Invalid(
                "HTTPS is required. Explicitly allow HTTP only for a trusted local test service"
                    .into(),
            ));
        }
        if self.bucket.is_empty() || self.bucket.contains(['/', '\\', ':']) {
            return Err(Error::Invalid("An explicit bucket name is required".into()));
        }
        Ok(())
    }
    pub fn writable(&self) -> Result<()> {
        if self.read_only {
            Err(Error::Invalid("This connection is read-only".into()))
        } else {
            Ok(())
        }
    }
}

/// Intentionally does not implement Debug or Serialize: cannot accidentally log/export secrets.
#[derive(Clone, Deserialize)]
pub struct Credentials {
    pub access_key: String,
    pub secret_key: String,
    pub session_token: Option<String>,
}
impl Credentials {
    pub fn validate(&self) -> Result<()> {
        if self.access_key.trim().is_empty() || self.secret_key.is_empty() {
            Err(Error::Invalid(
                "Access key and secret key are required".into(),
            ))
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<i64>,
    pub etag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_class: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    pub entries: Vec<Entry>,
    pub next_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferSettings {
    pub concurrent_files: usize,
    pub concurrent_parts: usize,
    pub max_requests: usize,
    pub part_size: u64,
    pub multipart_threshold: u64,
    pub retries: u32,
}
impl Default for TransferSettings {
    fn default() -> Self {
        Self {
            concurrent_files: 2,
            concurrent_parts: 3,
            max_requests: 6,
            part_size: 16 * MIB,
            multipart_threshold: 32 * MIB,
            retries: 3,
        }
    }
}
impl TransferSettings {
    pub fn validate(&self) -> Result<()> {
        if !(1..=16).contains(&self.concurrent_files)
            || !(1..=16).contains(&self.concurrent_parts)
            || !(1..=64).contains(&self.max_requests)
            || !(5 * MIB..=512 * MIB).contains(&self.part_size)
            || self.multipart_threshold < 5 * MIB
            || self.retries > 10
        {
            return Err(Error::Invalid("Files/parts: 1–16, requests: 1–64, part size: 5–512 MiB, threshold ≥5 MiB, retries ≤10".into()));
        }
        Ok(())
    }
}

pub fn part_size(size: u64, requested: u64) -> Result<u64> {
    if size > 5 * 1024 * 1024 * 1024 * 1024_u64 {
        return Err(Error::Invalid(
            "Objects larger than the conservative 5 TiB provider limit are not supported".into(),
        ));
    }
    Ok(requested
        .max(5 * MIB)
        .max(size.div_ceil(10_000).div_ceil(MIB) * MIB))
}

/// Reject ambiguous names on every platform so downloads are portable and cannot escape a root.
pub fn safe_relative_key(key: &str) -> Result<PathBuf> {
    if key.is_empty() || key.starts_with('/') || key.contains(['\\', ':', '\0']) {
        return Err(Error::Invalid(format!("Unsafe object key: {key:?}")));
    }
    for name in key.split('/') {
        let stem = name
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        if name.is_empty()
            || name == "."
            || name == ".."
            || name.ends_with(['.', ' '])
            || name
                .chars()
                .any(|c| c.is_control() || "<>\"|?*".contains(c))
            || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || (stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        {
            return Err(Error::Invalid(format!(
                "Object key cannot be safely mapped to a local path: {key:?}"
            )));
        }
    }
    let path = Path::new(key);
    if path
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(Error::Invalid("Path traversal rejected".into()));
    }
    Ok(path.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn multipart_limits() {
        assert_eq!(part_size(0, 1).unwrap(), 5 * MIB);
        let large = 500 * 1024 * MIB;
        assert!(large.div_ceil(part_size(large, 5 * MIB).unwrap()) <= 10_000);
        assert!(part_size(6 * 1024 * 1024 * MIB, MIB).is_err());
    }
    #[test]
    fn portable_paths() {
        for key in [
            "../escape",
            "/root",
            "a/../b",
            "a//b",
            "C:/x",
            "a\\b",
            "CON.txt",
            "a/NUL",
            "x.",
            "x ",
            "a/./b",
            "COM1",
            "a\0",
        ] {
            assert!(safe_relative_key(key).is_err(), "{key}");
        }
        assert_eq!(
            safe_relative_key("assets/日本語.txt").unwrap(),
            PathBuf::from("assets/日本語.txt")
        );
    }
    #[test]
    fn settings_are_bounded() {
        assert!(TransferSettings::default().validate().is_ok());
        assert!(
            TransferSettings {
                concurrent_parts: 100,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
}
