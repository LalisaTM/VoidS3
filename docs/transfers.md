# Transfer engine

Defaults: 2 active files, 3 parts per multipart file, 6 transfer requests, 16 MiB parts, 32 MiB multipart threshold, and 3 SDK retries per request. File/request concurrency is fixed at application startup; other settings are copied into each new queue record. Settings validate bounded limits before persistence.

Small uploads use `PutObject` and streaming `ByteStream::from_path`. Large uploads use `CreateMultipartUpload`, offset/length file streams for `UploadPart`, ordered ETags for completion, and conditional `CompleteMultipartUpload`. No production path buffers an entire file or part in JavaScript or Rust. Part sizes are increased when needed to stay within 10,000 parts, with a conservative 5 TiB maximum object size.

Desktop uploads register a background batch immediately. Discovery walks local selections in chunks of at most 32 files / 512 traversal steps on blocking workers, without any remote scan. At most four discovery batches run or pause at once. They share a backlog capped at 64 queued/running worker tasks, and stop producing when that backlog is full. Previously discovered files can upload before the remaining tree is traversed. Duplicate path/key selections are deduplicated; distinct sources mapping to the same key stop further discovery with a visible batch error. Earlier queued files remain.

For a new small file with no batch conflict policy, a worker sends conditional `PutObject` directly and fetches destination metadata only after a conditional-write conflict. Large files, replacements and batches with an explicit conflict policy inspect destinations when a worker reaches that file. This avoids a batch-wide HEAD barrier and unnecessary multipart traffic for existing objects. A rejected small request may already have transmitted body data; zero duplicate network traffic is not guaranteed.

An existing destination moves that file to `conflict`, frees its worker slot and lets other files continue. Queue Resolve offers Replace, Skip, Keep both and Decide later. Apply to all persists a policy scoped to that batch, including later conflicts. Skip can discard a transfer even if its local source changed. Replace requires the unchanged source and the destination ETag shown for that file. Later destination changes require a fresh decision; an earlier Replace policy cannot silently refresh an already-confirmed ETag.

Replace sends `If-Match` on `PutObject` or `CompleteMultipartUpload`; new/numbered destinations use `If-None-Match: *`, following [S3 conditional-write semantics](https://docs.aws.amazon.com/AmazonS3/latest/userguide/conditional-writes.html). Keep both finds an unused numbered filename, checking queue reservations and remote candidates up to 1,000 times. Old multipart sessions are aborted before applying a choice. Cleanup failures remain visible and retryable. Unsupported operations fail visibly without an unconditional-write fallback. CLI upload and additions-only sync retain no-overwrite behavior.

Global pause/cancel also stops discovery. Resume all resumes paused discovery and jobs. Admission checks the stop state again before starting a worker; a stop epoch prevents earlier asynchronous conflict resolutions from starting after a global stop. Completed files are never undone. After restart, admitted jobs recover paused with their original write conditions. Unfinished discovery is marked interrupted with a visible explanation: select the folder again to discover unqueued files. The traversal cursor is not persisted yet.

The frontend polls a global aggregate and one filtered page of at most 100 lightweight rows at one-second intervals. It receives no per-file profiles/settings or multipart maps. Collapsed queues stop row polling. Filtering/pagination happens in Rust; global controls use the full queue. Admission and durable progress writes run on blocking workers, and idle cancellation/history cleanup use SQLite transactions.

Settings provides five backend-defined presets: Balanced (the defaults above), Many small files (12 files / 2 parts / 16 requests), Large files (2 / 8 / 16), Low impact (1 / 1 / 2) and Unreliable connection (2 / 2 / 4, six retries). Advanced settings show editable part sizes/thresholds. These are validated starting points, not an adaptive optimizer or benchmark claim. File/request concurrency changes still apply after restart; new jobs capture part settings and retry configuration.

Each multipart record persists the source absolute path, size, modification time in nanoseconds, upload ID, part size, provider ETags, connection UUID, and status. Recovery starts paused. Resume validates source identity and lists all provider parts with pagination, checking their numbers and sizes. It uploads missing parts and refuses a source that changed. The fingerprint is not a cryptographic content hash; a same-size edit with a deliberately restored modification time is outside the current detection guarantee.

Upload progress means **provider-confirmed bytes**, so small-file progress may jump on completion and multipart progress advances per completed part. It does not invent smooth intermediate percentages. Throughput derives from observed byte deltas; it includes the gaps between unit completions and is not a network bandwidth measurement.

Pause cancels in-flight request futures. Multipart parts already accepted by S3 remain reusable after reconciliation. Small uploads restart as a whole request and can report a destination conflict if the prior request completed server-side just before cancellation. Cancel attempts `AbortMultipartUpload`, preserves cleanup errors in the transfer, and does not silently discard an outstanding upload ID. Clear-finished retains records with unresolved multipart or partial-file cleanup. Global pause/cancel signals every active or queued worker before looking up any credentials. Cancel all also cancels idle paused/failed records, while Resume all resumes only paused records. The controls apply across all profiles and ignore the visible queue filter. Completed files remain; cancellation does not undo a provider request that already finished.

Downloads inspect ETag/size, stream to a transfer-specific partial in the destination directory, and resume from that file's actual length using `Range` and `If-Match`. The server must honor the requested range; otherwise the operation fails without appending a full response. Final length must equal the original expected length. Flush/sync completes before a same-directory no-clobber hard link creates the destination. Filesystems without hard-link support fail visibly and preserve the partial. Cancellation deletes partials when possible. ETag identifies the remote version for conditional access; it is not interpreted as a content checksum.

Dragging between panes uses pointer events so Windows native Explorer drops can remain enabled. Drops onto folder rows use that folder as the destination. Remote folder downloads recursively scan that prefix, retain its folder name and descendants, validate every relative key, reject target collisions, and enqueue regular file downloads. Existing local files remain protected. Empty folders create local directories. Folder downloads and queue size are bounded to 100,000 files.

Crash windows remain: the provider can accept a final upload before local completion state is committed. A subsequent retry can report a conflict or `NoSuchUpload`; verify the remote object, then cancel the stale queue entry. Automatic resolution of ambiguous completion, checksum verification, automatic retry of an interrupted download body, parallel ranged downloads, and bandwidth shaping remain unfinished. An OS file lock prevents concurrent instances from operating the shared queue; distributed scheduling is not supported.

## Closing the desktop app

The titlebar close button, Alt+F4 and application exit requests use a native close check. If transfers, preparation, discovery or cleanup remain, VoidS3 reports the pending counts and asks whether to cancel and close. Keep open is the default; Escape or dismissing the dialog leaves work running. New admissions wait or fail while the close check is open so a new batch cannot slip past an empty-queue check.

Confirming cancels preparation, discovery, deferred conflict decisions and every pending transfer across connections, including paused and conflicting records. Cancellation-only workers may still start to abort idle multipart uploads. The app waits for workers, discovery and conflict tasks to settle and persist their outcomes. It removes only its own partial downloads and attempts to abort unfinished multipart sessions; completed files are preserved. The native window title reports cancellation/cleanup. No storage, traversal, transfer or cleanup work runs in the native close callback.

If cleanup fails or takes over 30 seconds, the app stays open for a second decision: Keep open to inspect/retry, or Close anyway. The latter can leave remote multipart sessions or local partial files; saved records remain recoverable. A forced process kill, crash or OS termination cannot provide this orderly shutdown guarantee. An update installation reserves the queue separately: ordinary closing is blocked during update download/install, and the verified installer restart retains paused work as before.

## Integration test setup

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
