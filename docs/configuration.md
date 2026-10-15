# Connections and troubleshooting

Create a connection from the sidebar. Each profile stores a UUID, name, provider, endpoint origin, signing region, bucket, initial prefix, addressing mode, read-only flag, and explicit HTTP opt-in. Endpoints may not contain embedded credentials, paths, queries, or fragments.

Cloudflare R2 uses signing region `auto`. Use `https://<account-id>.r2.cloudflarestorage.com`, or `https://<account-id>.eu.r2.cloudflarestorage.com` for an EU-jurisdiction bucket. FedRAMP uses `.fedramp.r2.cloudflarestorage.com` and requires the corresponding Cloudflare access. Endpoint helpers are editable. See [Cloudflare data location](https://developers.cloudflare.com/r2/reference/data-location/) and [API tokens](https://developers.cloudflare.com/r2/api/tokens/).

Bucket-scoped credentials can browse their configured bucket without account-wide bucket listing. The test action uses `ListObjectsV2` with the configured prefix. Read-only mode blocks write/delete APIs in Rust in addition to UI restrictions. Choose read/write credentials only when uploads or deletion are needed. Multipart recovery and cleanup may require additional bucket permissions.

AWS profiles need the bucket region, its S3 endpoint, access key/secret, and a session token for temporary credentials. Use virtual-hosted addressing unless your compatible provider needs path style. No AWS default credential chain or application-wide cloud credentials are embedded.

Other presets are starting conventions, not discovery. B2 account regions vary; replace the example region/endpoint with the one provided by Backblaze. Wasabi and Spaces similarly require matching region and endpoint. MinIO defaults to loopback HTTP and path style. Never allow HTTP for an untrusted network. Custom CA bundles and per-profile proxy/timeout settings are not implemented; system TLS configuration is used.

| Error                        | Check                                                                                          |
| ---------------------------- | ---------------------------------------------------------------------------------------------- |
| AccessDenied / Forbidden     | Bucket name, bucket object permissions, prefix restrictions, jurisdiction                      |
| InvalidAccessKeyId           | Correct provider/account key and credential lifetime                                           |
| SignatureDoesNotMatch        | Region, endpoint, secret, session token, local clock                                           |
| NoSuchBucket                 | Explicit bucket name and endpoint account/jurisdiction                                         |
| PreconditionFailed           | Destination already exists or object changed; refresh, inspect, choose a new destination       |
| NoSuchUpload                 | Remote session expired/was aborted; verify remote object, cancel stale transfer, then restart  |
| NetworkOrTimeout             | Endpoint/DNS, TLS trust, connectivity, proxy requirements                                      |
| Keychain unavailable         | Unlock Windows/macOS keychain or Linux Secret Service; no insecure fallback is used            |
| Download finalization failed | Existing destination, unsupported hard links, permissions, or disk space; partial is preserved |

Application data is under the OS local application-data directory in `VoidS3/state.db`. On Windows this is normally `%LOCALAPPDATA%\VoidS3\state.db`. Back up this non-secret state with the application closed. Copying it to another machine does not copy OS keychain entries. Re-enter credentials on that machine.

Profile export copies only the non-secret JSON to the clipboard. Secrets remain in the OS keychain. Exported endpoints/bucket names can still be sensitive. Duplicate profiles require re-entering credentials. Delete removes the profile and its keychain entry, not bucket contents; finish or cancel its transfers first.
