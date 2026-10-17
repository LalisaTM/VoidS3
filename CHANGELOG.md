# Changelog

## 0.3.1 — 2026-10-17

- Intercept native window close and application exit. Show pending transfers and upload batches, default to Keep open, and require an explicit choice to cancel and close.
- Block new transfer admissions during the close check; cancel preparation, discovery, queued/paused/conflicting jobs and active workers after confirmation. Wait for worker cleanup and durable outcomes before exiting.
- Attempt multipart aborts and owned partial-download removal while preserving completed files. Show cleanup failures or a 30-second timeout and offer Keep open or an explicit Close anyway; retain unfinished records for retry.
- Keep close callbacks limited to event suppression and task scheduling. Move bulk queue operations, settings, download-path validation, automatic conflict decisions and sync comparison onto background workers. Add a blocked-storage executor responsiveness regression test and real HTTP shutdown cleanup coverage.
- Sort/filter directory listings over 500 entries in a Web Worker, reuse its dataset across filter changes and reject stale results. Cache listing totals and use a selected-path set during rendering.

## 0.3.0 — 2026-10-16

- Start uploads through bounded background discovery instead of checking every destination first. New small files use conditional uploads directly; conflicts wait per file while other uploads continue.
- Apply conflict choices to the current batch, including later files; stop discovery and deferred decisions with global pause/cancel. Preserve conditional replacement and explicit restart recovery.
- Fetch aggregate queue statistics and at most 100 lightweight rows per page, stop collapsed-row polling, and move durable queue writes off the async executor. Batch idle cancellation/history deletion in SQLite transactions.
- Add five transfer presets with descriptions and editable advanced values.
- Add 10,000-job queue tests, streaming S3 integration coverage, and a prioritized improvement roadmap.

## 0.2.1 — 2026-10-16

- Ask how to handle existing upload destinations, with Replace, Skip, Keep both, sizes/timestamps, batch-scoped Apply to all, and whole-batch cancellation.
- Check recursive folder, picker, selection, and drag/drop uploads before queueing. Allocate unused numbered names for Keep both.
- Persist conditional replacement ETags across pause/restart, protect single and multipart completion against changed destinations, and retain no-overwrite defaults for existing jobs, CLI uploads, and additions-only sync.
- Add frontend regression tests and real HTTP S3 tests for conflict decisions, replacements, changed sources/destinations, and recovery.

## 0.2.0 — 2026-10-15

- Add a signed GitHub Releases self-updater for Windows x64, optional startup/daily checks, release notes, verified download progress, explicit installation confirmation, and a queue-idle installation guard.
- Publish version-bound updater signatures and latest.json only after package verification; retain secret-free development builds.
- Rebuild the desktop workspace around the supplied Figma design, with charcoal/violet themes, compact navigation, flat split panes, working search/bookmarks/activity, and keyboard pane resizing.
- Add global pause/resume/cancel controls across all connections, immediate worker interruption before credential lookup, and retained cleanup failures for retry.
- Add pointer-based file/folder/group transfers between panes and native Explorer drop targeting without disabling Windows file-drop support.
- Add recursive remote-folder downloads and confirmed multi-item deletion through the toolbar, context menu, or Delete key, preserving failed selections for retry.
- Automatically build Windows installer/CLI artifacts with checksums on every branch push, fork pull request, or manual workflow run.

## 0.1.1 — 2026-10-15

- Fix saving a new connection after "Save & test" clears its transient credential fields; the saved OS-keychain reference can now be reused immediately.
- Make the text-preview completion button close the dialog.
- Ignore generated Vite caches and add a regression test for the saved-connection flow.

## 0.1.0 — 2026-10-15

Initial experimental, fully vibecoded release.

- Rust workspace shared by Tauri desktop and CLI, official AWS S3 SDK, SQLite persistence, and OS keychain credentials.
- Dual-pane virtualized browser, provider profiles, explicit buckets, R2 endpoint helpers, bookmarks, settings, file actions, and text preview.
- Streaming transfers, bounded concurrency, multipart part reconciliation, persistent recovery, no-clobber writes, conditional ranged download resume, pause/cancel, and actionable errors.
- Conservative one-way/bidirectional additions-only synchronization preview.
- Automated checks, S3 integration suite, SemVer consistency tooling, dependency updates, and Windows installer/CLI release automation.

Known limitations and unfinished requirements are tracked in docs/status.md. No human review or production-readiness claim.
