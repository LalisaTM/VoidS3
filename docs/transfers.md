# Transfer engine

Defaults: 2 active files, 3 parts per multipart file, 6 transfer requests, 16 MiB parts, 32 MiB multipart threshold, and 3 SDK retries per request. File/request concurrency is fixed at application startup; other settings are copied into each new queue record. Settings validate bounded limits before persistence.

Small uploads use `PutObject` and streaming `ByteStream::from_path`. Large uploads use `CreateMultipartUpload`, offset/length file streams for `UploadPart`, ordered ETags for completion, and conditional `CompleteMultipartUpload`. No production path buffers an entire file or part in JavaScript or Rust. Part sizes are increased when needed to stay within 10,000 parts, with a conservative 5 TiB maximum object size.

Desktop upload preflight expands the selected folders, deduplicates overlapping selections, fingerprints local files, and inspects every remote destination with bounded concurrency. Existing objects prompt for Replace, Skip, or Keep both, with optional application to all remaining conflicts in that batch. Cancel discards the entire pending batch. No worker starts before all choices are available. Permission/network failures stop preflight; they are not interpreted as absent objects. Distinct selected sources targeting the same key are rejected before enqueueing; rename them or upload them separately.

Replace captures the destination ETag and sends `If-Match` on `PutObject` or `CompleteMultipartUpload`. New and Keep both destinations use `If-None-Match: *`. Keep both inserts a numbered suffix before the final extension, checks existing objects and reserves names within the batch and queue, trying at most 1,000 candidates. Workers validate the source and destination again before streaming; atomic write conditions protect against subsequent races. These conditions follow [S3 conditional-write semantics](https://docs.aws.amazon.com/AmazonS3/latest/userguide/conditional-writes.html). Unsupported provider operations fail visibly; there is no unconditional-write fallback. A raced destination requires a fresh upload to review choices; cancel any failed multipart entry to clean up its old session. Retry/resume preserves the original permission and ETag. CLI upload and additions-only sync continue to prohibit overwrites.

Confirmation plans are held only in backend memory, single-use, limited to four pending batches, and expire after 15 minutes. Connection changes and local source changes require a fresh preflight. A cancellation during the checking phase discards the result once outstanding inspection requests settle. Name allocation/source validation completes before workers start; a later enqueue failure reports the queued and failed siblings separately. Existing persisted jobs default to the original no-overwrite policy.

Each multipart record persists the source absolute path, size, modification time in nanoseconds, upload ID, part size, provider ETags, connection UUID, and status. Recovery starts paused. Resume validates source identity and lists all provider parts with pagination, checking their numbers and sizes. It uploads missing parts and refuses a source that changed. The fingerprint is not a cryptographic content hash; a same-size edit with a deliberately restored modification time is outside the current detection guarantee.

Upload progress means **provider-confirmed bytes**, so small-file progress may jump on completion and multipart progress advances per completed part. It does not invent smooth intermediate percentages. Throughput derives from observed byte deltas; it includes the gaps between unit completions and is not a network bandwidth measurement.

Pause cancels in-flight request futures. Multipart parts already accepted by S3 remain reusable after reconciliation. Small uploads restart as a whole request and can report a destination conflict if the prior request completed server-side just before cancellation. Cancel attempts `AbortMultipartUpload`, preserves cleanup errors in the transfer, and does not silently discard an outstanding upload ID. Clear-finished retains records with unresolved multipart or partial-file cleanup. Global pause/cancel signals every active or queued worker before looking up any credentials. Cancel all also cancels idle paused/failed records, while Resume all resumes only paused records. The controls apply across all profiles and ignore the visible queue filter. Completed files remain; cancellation does not undo a provider request that already finished.

Downloads inspect ETag/size, stream to a transfer-specific partial in the destination directory, and resume from that file's actual length using `Range` and `If-Match`. The server must honor the requested range; otherwise the operation fails without appending a full response. Final length must equal the original expected length. Flush/sync completes before a same-directory no-clobber hard link creates the destination. Filesystems without hard-link support fail visibly and preserve the partial. Cancellation deletes partials when possible. ETag identifies the remote version for conditional access; it is not interpreted as a content checksum.

Dragging between panes uses pointer events so Windows native Explorer drops can remain enabled. Drops onto folder rows use that folder as the destination. Remote folder downloads recursively scan that prefix, retain its folder name and descendants, validate every relative key, reject target collisions, and enqueue regular file downloads. Existing local files remain protected. Empty folders create local directories. Folder downloads and queue size are bounded to 100,000 files.

Crash windows remain: the provider can accept a final upload before local completion state is committed. A subsequent retry can report a conflict or `NoSuchUpload`; verify the remote object, then cancel the stale queue entry. Automatic resolution of ambiguous completion, checksum verification, automatic retry of an interrupted download body, parallel ranged downloads, and bandwidth shaping remain unfinished. An OS file lock prevents concurrent instances from operating the shared queue; distributed scheduling is not supported.

## Integration tests

Run an isolated Moto S3 endpoint on loopback:

```sh
docker run --rm -p 127.0.0.1:5000:5000 motoserver/moto:5.1.14
```

Set `VOIDS3_TEST_ENDPOINT=http://127.0.0.1:5000`, `VOIDS3_TEST_ACCESS_KEY=local-test-access`, and `VOIDS3_TEST_SECRET_KEY=local-test-secret`, then run:

```sh
cargo test -p voids3-transfer --test s3 -- --ignored --nocapture
```

The suite exercises real HTTP requests against a locally hosted S3 API emulator: single/multipart uploads, concurrency, listing, no-clobber writes, downloads, persisted multipart/partial-download recovery, changed-source refusal, multipart abort, global pause/cancel of queued workers, copy/delete, folder markers, and URL signing. Moto is not a real cloud provider; success is not evidence that all providers implement identical conditions. The same suite can target MinIO or a disposable cloud bucket.

For live R2, supply an existing disposable bucket via `VOIDS3_TEST_BUCKET`, signing region `auto`, and its correct jurisdiction endpoint. The suite does not call `ListBuckets`. It creates objects only under a random `voids3-integration/<uuid>/` prefix and attempts cleanup, including multipart sessions. Requests/storage may incur charges. Cleanup failures are test failures. Bucket-scoped credentials need listing, read/write/delete, multipart-list, and abort permissions. Optional GitHub live tests require the `r2-test` environment secrets documented in the release guide. Do not run against a bucket without authorization.
