# Changelog

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
