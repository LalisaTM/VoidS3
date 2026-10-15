# Changelog

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
