//! Official AWS SDK adapter. Explicit-bucket operations never call ListBuckets.
use aws_sdk_s3::{
    Client,
    config::{
        BehaviorVersion, Credentials as AwsCredentials, Region, retry::RetryConfig,
        timeout::TimeoutConfig,
    },
    presigning::PresigningConfig,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};
use voids3_core::{Credentials, Entry, Error, Page, Profile, Result};

#[derive(Clone)]
pub struct S3 {
    pub client: Client,
    pub profile: Profile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectInfo {
    pub size: u64,
    pub etag: String,
    pub content_type: Option<String>,
    pub cache_control: Option<String>,
    pub metadata: HashMap<String, String>,
    pub version_id: Option<String>,
}

impl S3 {
    pub fn new(profile: Profile, credentials: Credentials, retries: u32) -> Result<Self> {
        profile.validate()?;
        credentials.validate()?;
        let credentials = AwsCredentials::new(
            credentials.access_key,
            credentials.secret_key,
            credentials.session_token,
            None,
            "VoidS3 OS keychain",
        );
        let config = aws_sdk_s3::config::Builder::new()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new(profile.region.clone()))
            .endpoint_url(&profile.endpoint)
            .credentials_provider(credentials)
            .force_path_style(profile.path_style)
            .retry_config(RetryConfig::standard().with_max_attempts(retries + 1))
            .timeout_config(
                TimeoutConfig::builder()
                    .connect_timeout(Duration::from_secs(15))
                    .operation_attempt_timeout(Duration::from_secs(300))
                    .build(),
            )
            .request_checksum_calculation(
                aws_sdk_s3::config::RequestChecksumCalculation::WhenRequired,
            )
            .response_checksum_validation(
                aws_sdk_s3::config::ResponseChecksumValidation::WhenRequired,
            )
            .build();
        Ok(Self {
            client: Client::from_conf(config),
            profile,
        })
    }
    pub async fn test(&self) -> Result<()> {
        self.list(&self.profile.prefix, None, false)
            .await
            .map(|_| ())
    }
    pub async fn buckets(&self) -> Result<Vec<String>> {
        let output = self.client.list_buckets().send().await.map_err(|e| {
            api_error(
                "List buckets",
                e.as_service_error().and_then(|s| s.meta().code()),
            )
        })?;
        Ok(output
            .buckets()
            .iter()
            .filter_map(|b| b.name().map(str::to_owned))
            .collect())
    }
    pub async fn list(&self, prefix: &str, token: Option<String>, recursive: bool) -> Result<Page> {
        let output = self
            .client
            .list_objects_v2()
            .bucket(&self.profile.bucket)
            .prefix(prefix)
            .set_delimiter(if recursive { None } else { Some("/".into()) })
            .set_continuation_token(token)
            .max_keys(1000)
            .send()
            .await
            .map_err(|e| {
                api_error(
                    "List objects",
                    e.as_service_error().and_then(|s| s.meta().code()),
                )
            })?;
        let mut entries = Vec::new();
        for p in output.common_prefixes() {
            if let Some(key) = p.prefix() {
                entries.push(Entry {
                    name: key
                        .trim_end_matches('/')
                        .rsplit('/')
                        .next()
                        .unwrap_or(key)
                        .into(),
                    path: key.into(),
                    is_dir: true,
                    size: 0,
                    modified: None,
                    etag: None,
                });
            }
        }
        for object in output.contents() {
            if let Some(key) = object.key() {
                if key == prefix && key.ends_with('/') {
                    continue;
                }
                entries.push(Entry {
                    name: if recursive {
                        key.strip_prefix(prefix).unwrap_or(key).into()
                    } else {
                        key.rsplit('/').next().unwrap_or(key).into()
                    },
                    path: key.into(),
                    is_dir: false,
                    size: object.size().unwrap_or(0).max(0) as u64,
                    modified: object.last_modified().map(|t| t.secs()),
                    etag: object.e_tag().map(str::to_owned),
                });
            }
        }
        Ok(Page {
            entries,
            next_token: output.next_continuation_token().map(str::to_owned),
        })
    }
    pub async fn scan(&self, prefix: &str) -> Result<Vec<Entry>> {
        let mut entries = Vec::new();
        let mut token = None;
        loop {
            let page = self.list(prefix, token, true).await?;
            entries.extend(page.entries);
            if entries.len() > 100_000 {
                return Err(Error::Invalid(
                    "Sync scan is limited to 100,000 objects; choose a narrower prefix".into(),
                ));
            }
            token = page.next_token;
            if token.is_none() {
                return Ok(entries);
            }
        }
    }
    pub async fn head(&self, key: &str) -> Result<ObjectInfo> {
        let output = self
            .client
            .head_object()
            .bucket(&self.profile.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| {
                api_error(
                    "Inspect object",
                    e.as_service_error().and_then(|s| s.meta().code()),
                )
            })?;
        Ok(ObjectInfo {
            size: output.content_length().unwrap_or(0).max(0) as u64,
            etag: output.e_tag().unwrap_or_default().into(),
            content_type: output.content_type().map(str::to_owned),
            cache_control: output.cache_control().map(str::to_owned),
            metadata: output.metadata().cloned().unwrap_or_default(),
            version_id: output.version_id().map(str::to_owned),
        })
    }
    pub async fn folder(&self, key: &str) -> Result<()> {
        self.profile.writable()?;
        if !key.ends_with('/') || key == "/" {
            return Err(Error::Invalid("Folder key must end in /".into()));
        }
        self.client
            .put_object()
            .bucket(&self.profile.bucket)
            .key(key)
            .if_none_match("*")
            .body(aws_sdk_s3::primitives::ByteStream::from_static(b""))
            .send()
            .await
            .map_err(|e| {
                api_error(
                    "Create folder marker",
                    e.as_service_error().and_then(|s| s.meta().code()),
                )
            })?;
        Ok(())
    }
    pub async fn delete(&self, key: &str, etag: Option<&str>) -> Result<()> {
        self.profile.writable()?;
        self.client
            .delete_object()
            .bucket(&self.profile.bucket)
            .key(key)
            .set_if_match(etag.map(str::to_owned))
            .send()
            .await
            .map_err(|e| {
                api_error(
                    "Delete object",
                    e.as_service_error().and_then(|s| s.meta().code()),
                )
            })?;
        Ok(())
    }
    /// Copy only: callers must separately authorize deletion. Destination copy is not atomic.
    pub async fn copy(&self, source: &str, destination: &str, etag: &str) -> Result<()> {
        self.profile.writable()?;
        if source == destination {
            return Err(Error::Invalid(
                "Source and destination are identical".into(),
            ));
        }
        match self.head(destination).await {
            Ok(_) => return Err(Error::Conflict("Copy destination exists".into())),
            Err(Error::Provider(message))
                if message.contains("NotFound") || message.contains("NoSuchKey") => {}
            Err(e) => return Err(e),
        }
        let encoded = percent_encoding::utf8_percent_encode(
            &format!("{}/{}", self.profile.bucket, source),
            percent_encoding::NON_ALPHANUMERIC,
        )
        .to_string();
        self.client
            .copy_object()
            .bucket(&self.profile.bucket)
            .key(destination)
            .copy_source(encoded)
            .copy_source_if_match(etag)
            .send()
            .await
            .map_err(|e| {
                api_error(
                    "Copy object",
                    e.as_service_error().and_then(|s| s.meta().code()),
                )
            })?;
        Ok(())
    }
    pub async fn presign(&self, key: &str, upload: bool, seconds: u64) -> Result<String> {
        if !(1..=604800).contains(&seconds) {
            return Err(Error::Invalid("Expiry must be 1–604800 seconds".into()));
        }
        let config = PresigningConfig::expires_in(Duration::from_secs(seconds))
            .map_err(|_| Error::Invalid("Invalid URL expiry".into()))?;
        if upload {
            self.profile.writable()?;
            Ok(self
                .client
                .put_object()
                .bucket(&self.profile.bucket)
                .key(key)
                .presigned(config)
                .await
                .map_err(|_| Error::Provider("Cannot sign upload URL".into()))?
                .uri()
                .into())
        } else {
            Ok(self
                .client
                .get_object()
                .bucket(&self.profile.bucket)
                .key(key)
                .presigned(config)
                .await
                .map_err(|_| Error::Provider("Cannot sign download URL".into()))?
                .uri()
                .into())
        }
    }
    pub async fn preview_text(&self, key: &str) -> Result<String> {
        let info = self.head(key).await?;
        if info.size > 512 * 1024 {
            return Err(Error::Invalid("Text preview is limited to 512 KiB".into()));
        }
        let output = self
            .client
            .get_object()
            .bucket(&self.profile.bucket)
            .key(key)
            .if_match(&info.etag)
            .range("bytes=0-524287")
            .send()
            .await
            .map_err(|e| {
                api_error(
                    "Preview object",
                    e.as_service_error().and_then(|s| s.meta().code()),
                )
            })?;
        let bytes = output
            .body
            .collect()
            .await
            .map_err(|_| Error::Provider("Preview stream interrupted".into()))?
            .into_bytes();
        String::from_utf8(bytes.to_vec())
            .map_err(|_| Error::Invalid("This object is not UTF-8 text".into()))
    }
}

/// Do not include raw SDK errors: debug output can contain signed request headers.
pub fn api_error(operation: &str, code: Option<&str>) -> Error {
    let code = code.unwrap_or("NetworkOrTimeout");
    let advice = match code {
        "AccessDenied" | "Forbidden" | "403" => {
            "Check bucket-scoped object permissions and the endpoint jurisdiction. Account-wide bucket listing is optional."
        }
        "InvalidAccessKeyId" => "Save a valid access key for this provider.",
        "SignatureDoesNotMatch" | "AuthorizationHeaderMalformed" => {
            "Check the signing region, secret key, endpoint, and system clock. R2 uses region auto."
        }
        "NoSuchBucket" => "Check the explicit bucket name and account endpoint.",
        "NoSuchKey" | "NotFound" | "404" => "The object no longer exists; refresh the listing.",
        "PreconditionFailed" | "ConditionalRequestConflict" => {
            "The object changed or the destination exists. Refresh and resolve the conflict."
        }
        "SlowDown" | "Throttling" => {
            "The provider is throttling requests. Reduce concurrency and retry."
        }
        "NoSuchUpload" => {
            "The multipart session no longer exists. Cancel and restart the transfer."
        }
        _ => {
            "Check network connectivity, TLS certificates, endpoint, and provider capabilities; retry when available."
        }
    };
    Error::Provider(format!("{operation} [{code}]. {advice}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn errors_are_actionable_and_redacted() {
        let error = api_error("List objects", Some("AccessDenied")).to_string();
        assert!(error.contains("bucket-scoped"));
        assert!(!error.contains("Authorization:"));
    }
}
