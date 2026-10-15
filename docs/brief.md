# Project: VoidS3 — High-Performance S3 Desktop Client

## 1. Project Overview

Build **VoidS3**, a modern, high-performance, cross-platform desktop application for managing S3-compatible object storage.

VoidS3 should be a powerful alternative to WinSCP, Cyberduck, S3 Browser, and similar applications, combining professional file management, advanced transfer capabilities, modern UI/UX, and excellent performance.

The application must be fully functional, production-oriented, and designed for long-term maintainability.

**Primary target:** Windows 10/11 (x64).

**Secondary targets:** Linux and macOS, with cross-platform compatibility considered throughout development.

**Primary storage provider:** Cloudflare R2.

**Additional providers:** Amazon S3, Backblaze B2 S3 API, Wasabi, MinIO, DigitalOcean Spaces, and other S3-compatible providers.

### Core philosophy

- Native performance through Rust.
- Modern, responsive desktop interface.
- Excellent performance with both large files and thousands of small files.
- Efficient parallel and multipart uploads.
- Reliable, resumable transfers.
- Professional dual-pane file management.
- Secure credential storage.
- Advanced configuration for experienced users.
- Simple defaults for beginners.
- Never sacrifice data integrity for speed.

Do not build a prototype or visual-only application. Implement a real, working desktop application.

---

## 2. Technology Stack

### Backend

- Rust (stable edition, latest compatible toolchain).
- Tauri 2.
- Tokio for asynchronous execution.
- Official AWS SDK for Rust (`aws-sdk-s3`).
- `serde` / `serde_json` for serialization.
- `thiserror` for typed errors.
- `tracing` and `tracing-subscriber` for structured logging.
- `futures` for concurrent operations.
- `rusqlite` or SQLx with SQLite for persistent application state.
- An established Rust keychain integration for secure credentials.
- Platform-specific native filesystem APIs where necessary.

Use additional dependencies when justified.

### Frontend

- React 19.
- TypeScript with strict mode.
- Vite.
- Tailwind CSS 4.
- shadcn/ui.
- Radix UI.
- Lucide icons.
- TanStack Query for asynchronous state.
- Zustand for lightweight client state.
- TanStack Virtual for large file listings.
- React Router if multiple application views justify it.

### Architecture

Use a clear separation between:

1. Rust storage/provider layer.
2. Rust transfer engine.
3. Rust filesystem operations.
4. Rust application services.
5. Tauri IPC commands and events.
6. React application frontend.

Avoid placing business logic inside React components or Tauri command handlers.

All large file transfers must be performed in Rust. Never route file contents through JavaScript or Tauri IPC.

---

# 3. Application UI / UX

Create a polished, professional desktop interface inspired by modern developer tools.

The design should feel closer to a premium desktop application than a traditional file-transfer utility.

### Visual design

- Dark theme by default.
- Optional light theme.
- Minimalistic appearance.
- Neutral charcoal backgrounds.
- Subtle borders and shadows.
- Clean typography.
- Consistent spacing.
- Smooth, subtle transitions.
- Resizable panels.
- Native-feeling desktop interactions.
- Proper high-DPI scaling.
- Keyboard accessibility.

Avoid excessive gradients, glass effects, unnecessary animations, or oversized controls.

### Main window

Implement the following layout:

**Top toolbar**
- Connection selector.
- Current provider and bucket.
- Connection status.
- Breadcrumb navigation.
- Search.
- Upload button.
- Download button.
- Refresh.
- Transfer activity indicator.
- Settings.

**Left sidebar**
- Saved connections.
- Favorite buckets.
- Bookmarks.
- Recent locations.
- Transfer queue shortcut.
- Activity history.
- Settings shortcut.

**Main content**

A configurable dual-pane file manager.

Left pane: Local filesystem.

Right pane: Remote S3 bucket.

Allow users to switch between:
- Dual-pane mode.
- Remote-only mode.
- Local-only mode.

Both panes should have independent navigation histories and resizable widths.

**Bottom transfer panel**
- Active transfers.
- Queued transfers.
- Completed transfers.
- Failed transfers.
- Overall speed.
- Total progress.
- Pause/resume controls.
- Detailed transfer information.

The bottom panel must be collapsible and resizable.

### File browser

Provide:

- Details view.
- Compact list view.
- Grid view.
- Name, size, type, and modified date columns.
- Sortable columns.
- Multi-selection.
- Shift/Ctrl selection.
- Right-click context menus.
- Double-click navigation.
- Breadcrumb navigation.
- Back/forward navigation.
- Keyboard shortcuts.
- Drag and drop.
- Copy object key/path.
- Copy object URL.
- File properties.
- Quick preview.
- Filename filtering.
- Hidden file visibility options for local files.
- Human-readable file sizes.
- Context-sensitive toolbar actions.

Persist user interface preferences between sessions.

---

# 4. Connection Manager

Implement a complete S3 connection manager.

Users must be able to create, edit, duplicate, test, export, and delete connection profiles.

Each connection should support:

- Connection name.
- Provider selection.
- Endpoint URL.
- AWS region.
- Access Key ID.
- Secret Access Key.
- Session token where applicable.
- Bucket name.
- Initial remote directory/prefix.
- Path-style or virtual-hosted-style addressing.
- HTTPS configuration.
- Custom TLS certificate authorities where supported.
- Connection timeout.
- Request timeout.
- Proxy configuration where supported.
- Custom retry settings.
- Optional read-only mode.

### Provider presets

Create presets for:

- Cloudflare R2.
- Amazon S3.
- Backblaze B2.
- Wasabi.
- MinIO.
- DigitalOcean Spaces.
- Custom S3-compatible provider.

Each preset should preconfigure appropriate known endpoint conventions, addressing defaults, and signing regions.

Allow all relevant values to be overridden.

### Cloudflare R2

Cloudflare R2 is the primary development target.

Support:
- Standard R2 endpoints.
- EU jurisdiction endpoints.
- Other documented jurisdiction endpoints.
- Region `auto` for signing.
- Account ID configuration.
- Bucket-specific credentials.
- Tokens with limited permissions.
- Direct connection to an explicitly named bucket without requiring account-wide bucket-list permissions.

Do not assume that credentials allowing object access also permit listing every bucket.

Provide detailed, helpful errors for AccessDenied, InvalidAccessKeyId, SignatureDoesNotMatch, endpoint mismatches, missing permissions, and bucket-not-found errors.

### Security

Never store S3 secrets in plaintext configuration files or application logs.

Use the operating system's credential store where possible:
- Windows Credential Manager / DPAPI-backed protection.
- macOS Keychain.
- Linux Secret Service or an explicitly configured secure alternative.

Never expose secret keys unnecessarily to the React frontend.

Connection profiles should reference stored credentials rather than contain secrets.

Provide a secure export workflow that excludes secrets by default.

---

# 5. Professional File Manager

Implement WinSCP-style file management.

### Local filesystem

Support:
- Directory navigation.
- Directory creation.
- Rename.
- Move.
- Copy.
- Delete.
- Recycle Bin integration where practical.
- File properties.
- Opening files with their associated applications.
- Revealing files in Windows Explorer.
- File and folder search.
- Drag and drop.
- Local path input.

### Remote S3 filesystem

Support:
- Listing buckets when permissions allow.
- Listing objects and prefixes.
- Pagination.
- Prefix navigation.
- Object upload.
- Object download.
- Object deletion.
- Batch deletion.
- Object rename.
- Object copy.
- Object move.
- Creating folder markers.
- Removing folder markers.
- Object metadata.
- Content-Type handling.
- User-defined metadata.
- Tags where supported.
- Storage class display where supported.
- Version browsing where supported.
- Presigned download URLs.
- Presigned upload URLs.
- Object URL copying.
- Bucket properties.
- Bucket versioning controls where supported.

### Important S3 behavior

S3 is object storage, not a traditional filesystem.

Design the UI to feel like a filesystem while correctly handling S3 semantics.

For example:
- Folders usually represent key prefixes.
- Renaming an object generally involves copy and delete.
- Moving a directory may require copying and deleting many objects.
- Modification timestamps are not equivalent to filesystem timestamps.
- There may be no real empty directories without folder markers.
- Permissions and provider capabilities differ.
- Presigned URLs may require provider-specific endpoint handling.

Never assume every S3 provider supports every feature.

Implement capability detection where possible and disable unsupported operations with clear explanations.

---

# 6. High-Performance Transfer Engine

The transfer engine is one of the most important parts of VoidS3.

Design it as a standalone Rust subsystem, independent of the GUI.

It must support high-throughput transfers, controlled concurrency, cancellation, retries, persistence, and detailed progress reporting.

### Upload modes

Implement:

1. Single-request upload for small objects.
2. Multipart upload for large objects.
3. Concurrent uploads of multiple files.
4. Recursive directory upload.
5. Background upload queue.

Use configurable thresholds to determine when multipart uploads should be used.

### Multipart uploads

Implement the complete S3 multipart upload lifecycle:

1. CreateMultipartUpload.
2. Split the source file into parts.
3. UploadPart for individual parts.
4. Track ETags and part numbers.
5. CompleteMultipartUpload.
6. AbortMultipartUpload when appropriate.

Support configurable part sizes, with appropriate defaults.

Respect provider-specific multipart limits, including minimum part size, maximum part count, and maximum object size.

Use bounded concurrency to prevent excessive memory usage.

Avoid loading entire large files into memory.

Use streaming file reads or carefully bounded part buffers.

### Concurrency

Support separate controls for:

- Maximum simultaneous files.
- Maximum concurrent multipart parts per file.
- Global maximum in-flight requests.
- Memory budget.
- Connection pool behavior.

Default values must be conservative and stable.

Users should be able to tune these settings through the GUI.

### Resumable uploads

Implement persistent resumable multipart uploads.

Store enough information to recover an interrupted upload:

- Connection/profile identifier.
- Bucket.
- Object key.
- Local source path.
- Source size.
- Source modification information.
- Multipart upload ID.
- Part size.
- Completed part numbers.
- Returned ETags.
- Upload metadata.
- Current transfer status.

On application restart:
- Restore incomplete transfers.
- Validate the local source file.
- Reconcile state with the provider when supported.
- Resume missing parts.
- Restart safely if the existing multipart state cannot be reused.

Never silently resume with a modified source file.

### Downloads

Implement:

- Normal streaming downloads.
- Parallel ranged downloads where supported.
- Resume of interrupted downloads.
- Temporary partial files.
- Validation of remote object identity before resuming.
- Atomic finalization where possible.
- Configurable download concurrency.

Use HTTP range requests for eligible objects.

Do not assume every object or endpoint supports range requests.

### Reliability

Implement:

- Exponential backoff with jitter.
- Provider-aware retry handling.
- Rate-limit handling.
- Network timeout recovery.
- Graceful cancellation.
- Persistent queue state.
- Transfer conflict handling.
- Checks for source-file changes.
- Cleanup of abandoned multipart sessions.
- Retry limits.
- Reliable completion-state tracking.

Distinguish retryable failures from permanent errors.

For example, never endlessly retry invalid credentials.

### Transfer queue

Each transfer should support:

- Queued.
- Preparing.
- Uploading/downloading.
- Pausing.
- Paused.
- Retrying.
- Verifying.
- Completed.
- Failed.
- Cancelled.

Users must be able to:

- Pause a transfer.
- Resume a transfer where technically possible.
- Cancel a transfer.
- Retry a failed transfer.
- Change queue priority.
- Remove completed transfers.
- Clear history.
- View errors.
- View per-file details.

Make pause and resume semantics explicit. For uploads that cannot resume at the byte level, preserve completed multipart parts or restart the applicable unit safely.

---

# 7. Transfer Monitoring and Statistics

Create an advanced transfer dashboard.

For each transfer display:

- Filename.
- Direction.
- Source.
- Destination.
- File size.
- Bytes transferred.
- Percentage.
- Current speed.
- Average speed.
- Estimated remaining time.
- Elapsed time.
- Current phase.
- Retry count.
- Error details.

Global statistics:

- Total upload speed.
- Total download speed.
- Active transfers.
- Queued transfers.
- Total bytes transferred.
- Successful transfers.
- Failed transfers.
- Session statistics.

### Performance graphs

Implement a real-time graph displaying upload/download throughput.

Support:
- Current throughput.
- Rolling average.
- Peak throughput.
- Per-transfer or total throughput.
- Optional historical view.

Avoid unnecessary high-frequency IPC traffic.

Aggregate progress events in Rust and publish them at a reasonable rate.

Keep the React interface responsive during heavy transfers.

---

# 8. Smart Upload Optimization

Add an optional automatic upload optimizer.

The optimizer should test different transfer configurations using controlled benchmarks.

It can vary:

- Multipart part size.
- Concurrent requests.
- Concurrent files.
- Connection settings.
- Request batch behavior where applicable.

### Benchmark tool

Build a dedicated benchmark screen.

Allow users to select:
- Target connection.
- Bucket.
- Test file size.
- Concurrency range.
- Part sizes.
- Number of trials.

Display:
- Throughput.
- Upload duration.
- Average latency.
- Failed requests.
- Retry count.
- Throughput over time.
- Configuration used.

Benchmark data must be based on real transfers, not simulated values.

Warn the user that benchmark uploads create temporary objects and may incur storage or request charges.

Use a dedicated temporary object prefix.

Attempt to clean up benchmark objects and unfinished multipart uploads, and clearly report any cleanup failure.

### Adaptive concurrency

Optionally implement adaptive concurrency.

Increase or decrease parallelism based on:
- Observed throughput.
- Error rates.
- Retry rates.
- Request latency.
- Resource consumption.

Avoid unnecessary aggressive request bursts.

Keep this feature optional and disabled until validated.

### Performance diagnostics

Create a diagnostics view that helps distinguish:

- Client-side transfer limits.
- Low network throughput.
- High request latency.
- Multipart configuration issues.
- Throttling.
- Connection failures.
- Local disk bottlenecks.
- Excessively small files.

Do not claim a specific bottleneck without supporting measurements.

---

# 9. Folder Synchronization

Implement professional synchronization features.

Support:

- Local to remote.
- Remote to local.
- Bidirectional synchronization.
- One-way mirror.
- Backup-style synchronization.
- Manual synchronization.
- Scheduled synchronization.

### Sync comparison

Compare files based on configurable signals:

- Key/path.
- Object size.
- Modification information where meaningful.
- Checksums when available and comparable.

Do not assume S3 ETags are always MD5 hashes.

Avoid treating S3 LastModified timestamps as preserved local file modification times.

Provide explicit policies for conflict resolution.

### Preview before execution

Before synchronizing, display a comparison:

- Files to upload.
- Files to download.
- Files to replace.
- Files to delete.
- Conflicting files.
- Unchanged files.

Provide a dry-run mode.

Never silently delete remote files during bidirectional synchronization.

### Sync rules

Support:
- Include patterns.
- Exclude patterns.
- File-size filters.
- File extension filters.
- Hidden-file exclusions.
- Ignore files such as `.git`.
- User-defined sync presets.

### Conflict handling

Allow users to choose:

- Skip.
- Overwrite.
- Keep both.
- Ask individually.
- Apply a rule to all conflicts.

Scheduled synchronization should not run destructive operations without previously established authorization and policy.

---

# 10. Advanced S3 Features

Provide an advanced features area for supported providers.

Implement:

- Object metadata editor.
- Content-Type editor.
- Cache-Control editor.
- Object tagging.
- Presigned URL generation.
- Object version listing.
- Version restore where supported.
- Object deletion markers where supported.
- Server-side copy where supported.
- Storage class selection where supported.
- Bucket region and endpoint information.
- Bucket lifecycle management where supported.
- Bucket versioning controls where supported.
- Bucket CORS configuration where supported.

Use feature detection or explicit provider capability profiles.

Never show an operation as available if the backend does not support it.

Avoid assuming AWS-specific APIs are available in Cloudflare R2.

For destructive administrative actions, require an explicit confirmation.

---

# 11. Search, Filtering, and Navigation

Implement fast navigation for buckets containing large numbers of objects.

Features:

- Prefix-based search.
- Current-directory filtering.
- Recursive object search.
- Search by filename.
- Search by extension.
- Search by size.
- Search by modified date.
- Regex filtering.
- Case-sensitive/insensitive options where practical.
- Search result sorting.
- Cached listing results.
- Manual cache invalidation.

### Large bucket performance

Handle buckets containing hundreds of thousands or millions of objects without freezing the interface.

Use:

- Paginated API requests.
- Incremental loading.
- List virtualization.
- Cancellation of outdated requests.
- Efficient caching.
- Bounded background operations.

Do not attempt to fetch an entire enormous bucket just to display its root directory.

Clarify in the UI whether search covers loaded objects, a prefix, or a full recursive scan.

---

# 12. Built-in File Preview

Provide a file preview panel.

Initial supported file types:

- Plain text.
- JSON.
- YAML.
- XML.
- Markdown.
- Images.
- SVG with safe rendering.
- PDF where safely supported.
- Basic audio/video metadata.
- Common source-code files.

Features:

- Syntax highlighting.
- Line numbers.
- File metadata.
- Image dimensions.
- Content-Type display.
- Object key display.
- Download/open actions.

Use size limits and streaming/ranged access where practical.

Avoid downloading multi-gigabyte objects simply to preview them.

Do not execute untrusted content.

---

# 13. File Editing

Implement a remote editing workflow similar to WinSCP.

Users should be able to:

1. Select a remote text file.
2. Open it in an integrated editor or configured external editor.
3. Modify the file.
4. Save changes.
5. Upload the new version.

Support:

- Configurable external editor.
- Temporary-file management.
- Content-Type preservation.
- Metadata preservation where possible.
- Remote change detection using ETag or other appropriate conditions.
- Conflict warnings.

If another client modifies the remote object during editing, VoidS3 should not silently overwrite the new version.

---

# 14. Bookmarks and Favorites

Allow users to bookmark:

- Connections.
- Buckets.
- Remote prefixes.
- Local folders.
- Paired local/remote folders.

Support opening paired bookmarks directly in the dual-pane view.

Persist navigation history and recently visited locations.

---

# 15. Drag and Drop

Support native drag and drop.

Required operations:

- Windows Explorer to remote bucket.
- Local pane to remote pane.
- Remote pane to local pane.
- Multi-file dragging.
- Directory dragging.
- Moving objects between remote prefixes.

For remote-to-remote operations, prefer server-side copies where supported and appropriate.

Show a preview of the intended action before expensive or destructive bulk operations.

---

# 16. Connection and Transfer Profiles

Allow separate transfer settings per connection.

Example:

**Profile: Cloudflare R2 — Fast Upload**

- Multipart enabled.
- 32 MiB parts.
- 8 concurrent parts.
- 4 concurrent files.
- Automatic retry.
- Resume enabled.

**Profile: Cloudflare R2 — Conservative**

- 16 MiB parts.
- 2 concurrent parts.
- 2 concurrent files.
- Lower memory usage.

Users should be able to create custom transfer profiles and assign them to connections.

These are starting presets, not guaranteed optimal settings.

---

# 17. Logging and Diagnostics

Implement structured logging.

Log:

- Connection lifecycle.
- S3 requests at configurable detail levels.
- Transfer state transitions.
- Retries.
- Network errors.
- Authentication errors.
- Performance statistics.
- Unexpected application errors.

Provide:

- Log viewer.
- Log filtering.
- Log export.
- Diagnostics export.
- Clear log action.
- Configurable verbosity.

Never log:
- Secret access keys.
- Session tokens.
- Authorization headers.
- Sensitive presigned URL signatures.
- Unredacted credentials.

Diagnostics exports must redact sensitive data by default.

---

# 18. Application Settings

Create a comprehensive settings screen.

Sections:

**General**
- Theme.
- Language.
- Startup behavior.
- Minimize to tray.
- Notifications.
- Default view mode.

**Transfers**
- Multipart threshold.
- Multipart size.
- Concurrent requests.
- Concurrent files.
- Retry limits.
- Timeouts.
- Bandwidth limit.
- Resume settings.
- Conflict defaults.

**Network**
- Proxy.
- TLS settings.
- Connection timeout.
- Endpoint overrides.
- Diagnostic options.

**File Manager**
- Double-click behavior.
- Hidden files.
- Sorting.
- Date format.
- Size units.
- Default local directory.
- Default remote prefix.

**Security**
- Credential management.
- Session locking if implemented.
- Connection data export.
- Sensitive data handling.

**Advanced**
- Cache configuration.
- Logging.
- Developer diagnostics.
- Experimental transfer optimizer.

---

# 19. Desktop Integration

On Windows, implement:

- Native file dialogs.
- Explorer integration where reasonable.
- System tray support.
- Native notifications.
- Taskbar progress indicators where supported.
- High-DPI awareness.
- Windows dark-mode compatibility.
- Application icon.
- Installer.
- Uninstaller.
- Crash reporting/logging without leaking credentials.

Support opening VoidS3 through an optional custom protocol or command-line arguments.

Example:

`voids3://connection/my-r2/bucket/assets/`

Validate and safely parse all external protocol inputs.

---

# 20. Command-Line Interface

In addition to the GUI, implement an optional CLI backed by the same Rust core.

Example commands:

```bash
voids3 connections list
voids3 buckets list --connection my-r2

voids3 upload ./file.zip my-r2/mybucket/backups/
voids3 download my-r2/mybucket/file.zip ./downloads/

voids3 sync ./assets my-r2/mybucket/assets --dry-run
voids3 benchmark --connection my-r2 --bucket mybucket
```

The CLI must not require starting the graphical frontend.

Reuse the same provider abstraction, credential management, and transfer engine.

Avoid duplicating transfer logic.

---

# 21. Security Requirements

Treat security as a first-class requirement.

- Secure credential storage.
- No plaintext secret persistence.
- No secret exposure in logs.
- HTTPS by default.
- Explicit warnings for insecure endpoints.
- No implicit disabling of TLS certificate validation.
- Minimize Tauri permissions and exposed commands.
- Validate all IPC inputs.
- Validate local filesystem paths.
- Sanitize remote object keys used as local filenames.
- Prevent path traversal on download.
- Handle Windows reserved filenames.
- Handle conflicting object paths safely.
- Prevent accidental overwrite.
- Avoid unsafe shell command construction.
- Never execute downloaded files automatically.
- Require confirmations for destructive bulk operations.

Do not use Rust `unsafe` unless truly necessary and justified.

Never embed application-wide Cloudflare credentials in the distributed executable.

---

# 22. Performance and Memory Requirements

The application must remain responsive under heavy workloads.

Target scenarios:

- Uploading one 20 GB file.
- Uploading thousands of small files.
- Downloading large objects.
- Listing a prefix containing tens of thousands of objects.
- Handling multiple active transfers.
- Recovering incomplete transfers after a restart.

Requirements:

- Bounded memory consumption.
- Minimal unnecessary allocations.
- No whole-file buffering.
- Asynchronous network operations.
- Minimal IPC overhead.
- Virtualized file lists.
- Bounded queues.
- Backpressure.
- Efficient cancellation.
- No blocking operations on the GUI thread.

Measure real performance and optimize based on profiling.

Do not invent benchmark results.

---

# 23. Error Handling

Create a central error model.

Distinguish between:

- Authentication errors.
- Authorization errors.
- Provider incompatibility.
- Network errors.
- DNS failures.
- TLS failures.
- Request timeouts.
- Rate limiting.
- Missing buckets.
- Missing objects.
- Local filesystem errors.
- Insufficient disk space.
- Transfer conflicts.
- Multipart failures.
- Data integrity failures.
- Application errors.

Show actionable, user-friendly messages.

Detailed technical information should be available in an expandable error panel.

Never show only generic messages such as "Something went wrong" when meaningful diagnostics exist.

---

# 24. Testing Strategy

Implement comprehensive automated testing.

### Rust unit tests

Cover:
- Multipart size calculation.
- Part scheduling.
- Retry logic.
- Transfer state machine.
- Queue priority.
- Path handling.
- S3 key normalization.
- Resume metadata.
- Sync comparison.
- Conflict handling.
- Credential redaction.

### Integration tests

Use MinIO or another locally hosted S3-compatible test service.

Test:
- Upload/download.
- Multipart upload.
- Interrupted transfer recovery.
- Object listing.
- Copy/move/delete.
- Metadata handling.
- Concurrent transfers.
- Permissions errors.
- Folder synchronization.
- Safe cancellation.

### Cloudflare R2 compatibility

Add optional live integration tests using environment-provided credentials.

Never commit live credentials.

Explicitly test:
- EU jurisdiction endpoints.
- Bucket-scoped credentials.
- Multipart uploads.
- Resume behavior.
- Presigned URLs.
- Provider-specific limitations.

### Frontend tests

Use Vitest and React Testing Library.

Cover:
- Connection forms.
- File navigation.
- Transfer queue.
- Error states.
- Settings.
- File selection.
- Search/filtering.

Implement a few end-to-end smoke tests for important desktop workflows.

---

# 25. Project Structure

Use a maintainable Cargo workspace.

Suggested structure:

```text
voids3/
├── apps/
│   ├── desktop/
│   │   ├── src/
│   │   │   ├── components/
│   │   │   ├── features/
│   │   │   ├── hooks/
│   │   │   ├── stores/
│   │   │   ├── lib/
│   │   │   ├── routes/
│   │   │   └── styles/
│   │   ├── src-tauri/
│   │   └── package.json
│   └── cli/
├── crates/
│   ├── voids3-core/
│   ├── voids3-s3/
│   ├── voids3-transfer/
│   ├── voids3-sync/
│   ├── voids3-storage/
│   └── voids3-platform/
├── tests/
├── docs/
├── Cargo.toml
├── package.json
└── README.md
```

Exact structure can be improved if there is a good architectural reason.

Favor modular design without creating unnecessary abstraction layers.

---

# 26. Development Workflow

Follow an incremental, working implementation strategy.

Do not generate hundreds of placeholder files.

### Phase 1 — Foundation

- Create the repository.
- Configure the Rust workspace.
- Configure Tauri 2.
- Configure React and TypeScript.
- Set up Tailwind and shadcn/ui.
- Create the main application shell.
- Implement logging and error handling.
- Establish IPC conventions.
- Configure formatting, linting, and tests.

The application must build and launch at the end of this phase.

### Phase 2 — Connection and Browsing

- Implement secure credentials.
- Implement the S3 client.
- Add Cloudflare R2 support.
- Add connection profiles.
- Add bucket and object listing.
- Implement the local filesystem pane.
- Implement remote navigation.
- Implement basic file operations.

### Phase 3 — Transfer Engine

- Implement single uploads/downloads.
- Implement multipart uploads.
- Implement concurrency control.
- Implement transfer progress.
- Implement retries and cancellation.
- Implement the transfer queue.
- Persist transfer state.
- Implement recovery and resume.

### Phase 4 — Advanced File Management

- Drag and drop.
- Bulk operations.
- Copy/move/rename.
- File properties.
- Bookmarks.
- Search and filtering.
- File preview.
- Remote editing.

### Phase 5 — Synchronization

- Sync comparison.
- Dry-run preview.
- One-way sync.
- Bidirectional sync.
- Conflict resolution.
- Mirror mode.
- Scheduling.

### Phase 6 — Advanced Features

- Provider capabilities.
- Metadata editor.
- Version browsing.
- Presigned URLs.
- Benchmarking.
- Transfer optimization.
- CLI.

### Phase 7 — Production Readiness

- Integration testing.
- Performance profiling.
- Error handling improvements.
- Windows packaging.
- Documentation.
- Crash recovery.
- Security review.

After every phase, ensure the project remains buildable and usable.

---

# 27. Code Quality Requirements

Follow these principles:

- Production-quality Rust.
- Idiomatic asynchronous programming.
- RAII and ownership-based resource management.
- Explicit error handling.
- Small, focused modules.
- Strict TypeScript.
- Reusable React components.
- Clear boundaries between backend and frontend.
- No unnecessary dependencies.
- No hardcoded credentials.
- No fabricated data.
- No fake progress indicators.
- No silently swallowed errors.
- No UI controls that pretend to work.
- No unnecessary global state.

Avoid `unwrap()` and `expect()` in production paths where runtime failure is possible.

Use typed APIs and data structures.

Document non-obvious algorithms and design decisions.

---

# 28. Documentation

Provide:

- README.md.
- Installation instructions.
- Development setup.
- Architecture overview.
- Cloudflare R2 configuration guide.
- AWS S3 configuration guide.
- Security documentation.
- Transfer engine documentation.
- CLI usage.
- Troubleshooting guide.
- Build and release instructions.

Explain how users can configure Cloudflare R2 EU-jurisdiction endpoints and bucket-scoped API credentials.

---

# 29. Acceptance Criteria

VoidS3 is considered functionally ready when:

1. A user can create a Cloudflare R2 connection using valid credentials.
2. Bucket-scoped credentials work without account-wide bucket-list permission.
3. Users can browse local and remote files.
4. Users can upload and download files.
5. Large files use reliable multipart uploads.
6. Multiple files can transfer concurrently.
7. Transfer progress reflects actual transferred bytes.
8. Failed transfers provide actionable errors.
9. Multipart uploads can recover safely from interruption.
10. Files can be copied, renamed, and deleted with appropriate safeguards.
11. Users can perform one-way and bidirectional synchronization with a preview.
12. Credentials are stored securely.
13. The application remains responsive during large transfers.
14. Automated tests cover critical transfer behavior.
15. The Windows desktop application builds and can be installed.

Advanced features must be tested independently before they are considered complete.

---

# 30. Final Instructions

You are responsible for implementing VoidS3, not merely describing how it could be built.

Start by examining the existing repository and development environment.

If the repository is empty, scaffold the project.

Before coding, briefly outline the implementation architecture and identify any major technical risks.

Then begin implementation immediately.

Work through the phases in order and continuously validate the application.

Run formatting, linting, type checking, compilation, and relevant tests after significant changes.

Do not claim that a feature works unless it has been implemented and appropriately verified.

Do not replace complex features with mock implementations.

If functionality cannot be completed in the current iteration, clearly identify what remains unfinished and keep the existing functionality working.

Make sensible engineering decisions independently instead of repeatedly asking for permission on minor details.

Prioritize actual functionality, reliability, performance, and security over decorative polish.

The final goal is a complete, modern, secure, high-performance S3 desktop client named **VoidS3** that users can confidently use as an alternative to WinSCP, Cyberduck, and other professional S3 clients.
