# VoidS3

[![CI](https://github.com/LalisaTM/VoidS3/actions/workflows/ci.yml/badge.svg)](https://github.com/LalisaTM/VoidS3/actions/workflows/ci.yml) [![Release](https://img.shields.io/github/v/release/LalisaTM/VoidS3?include_prereleases)](https://github.com/LalisaTM/VoidS3/releases) [![License: MIT](https://img.shields.io/badge/License-MIT-c7f9a6)](LICENSE)

A native S3 desktop client built with Rust, Tauri 2, and React 19. Cloudflare R2 first, with configurable S3-compatible endpoints. Windows is the primary target.

## Completely vibecoded, for fun

**This project is completely vibecoded for fun. No human has read, reviewed, modified, edited, or otherwise manually intervened in any of this code.** AI agents handle the implementation, commits, pushing, branching, CI configuration, tests, versioning, and release setup. Humans supplied the idea and specification and started the experiment.

Automated checks are not human review or a security audit. This is an experimental **0.1.1** release, not a claim of production readiness. Use disposable credentials and test buckets first. The original specification is in [docs/brief.md](docs/brief.md); it describes the intended product, including features that are still unfinished.

## What is implemented

- Native dual-pane desktop file browser, paginated S3 listings, virtualized rows, filtering, sorting, multi-selection, navigation history, bookmarks, and dark/light themes.
- Named bucket connections for R2, AWS, B2, Wasabi, MinIO, Spaces, and custom endpoints. R2 region `auto`, EU/FedRAMP endpoint helpers, and explicit buckets without `ListBuckets` permissions.
- OS keychain credentials; non-secret profiles, settings, and transfer history in SQLite. Profile exports exclude credentials.
- Rust streaming uploads/downloads, concurrent files and multipart parts, globally bounded transfer requests, SDK retries, persistent multipart state, pause/resume/cancel, provider part reconciliation, and source-change checks.
- Downloads use ETag-conditional requests, partial files, ranged resume, size verification, and atomic no-clobber finalization on filesystems supporting hard links. ETags are identity tokens, never assumed to be MD5.
- Recursive uploads, local folder creation/rename/recycling, remote folder markers, object properties, server-side copy, conditional object deletion, signed download URLs, and bounded UTF-8 text previews.
- One-way and bidirectional **additions-only** synchronization with preview; existing pairs are preserved as conflicts.
- A CLI using the same provider and transfer engine, independent of the GUI.
- CI for formatting, linting, tests, dependency auditing, live local S3 integration, desktop builds, and version consistency; tagged Windows installer/CLI releases.

Uploads do not overwrite existing objects. Downloads do not overwrite local files. Copy checks destination absence, but S3 copy cannot atomically guarantee it remains absent; avoid concurrent writes to the copy destination. Provider support for conditional operations varies; unsupported requests fail visibly.

## Install

Windows installers and CLI archives are published under [Releases](https://github.com/LalisaTM/VoidS3/releases). Builds are unsigned: Windows may show a SmartScreen warning. No automatic updater is included. Linux and macOS are checked in CI; signed distribution packages for those platforms are unfinished.

## Develop

Install Node.js 22+, a current stable Rust toolchain (the lockfile currently needs Rust 1.94+), and the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/). On Windows, use Visual Studio C++ Build Tools and WebView2.

```sh
npm ci
npm run desktop dev
```

`npm run dev` opens only the frontend preview. Browser mode intentionally has no filesystem, keychain, or S3 access and shows no fake files or transfer data.

```sh
npm run build
npm run lint
npm test
npm run format:check
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
npm run version:check
npm run desktop build
```

Build the CLI with `cargo build --release -p voids3-cli`. Do not run the desktop app and CLI against the same queue concurrently; cross-process queue ownership is not implemented yet.

## Connect Cloudflare R2

Create an R2 API credential with object permissions for the intended bucket. In **New connection**, choose R2, enter the bucket explicitly, use signing region `auto`, and enter one of:

| Jurisdiction | Endpoint                                                |
| ------------ | ------------------------------------------------------- |
| Default      | `https://<account-id>.r2.cloudflarestorage.com`         |
| EU           | `https://<account-id>.eu.r2.cloudflarestorage.com`      |
| FedRAMP      | `https://<account-id>.fedramp.r2.cloudflarestorage.com` |

Use the endpoint for the bucket's jurisdiction, as described in [Cloudflare's data location documentation](https://developers.cloudflare.com/r2/reference/data-location/). Test uses `ListObjectsV2` for the configured bucket/prefix, not account-wide `ListBuckets`. Multipart recovery additionally needs permission to list parts and abort multipart uploads. Live R2 behavior requires optional credentialed tests; local S3 tests alone do not establish R2 compatibility.

AWS users should set the actual region and its matching S3 endpoint, for example `us-east-1` and `https://s3.us-east-1.amazonaws.com`. For temporary credentials, supply the session token. For MinIO on a trusted local machine, explicitly allow HTTP.

See [configuration](docs/configuration.md), [architecture](docs/architecture.md), [security](SECURITY.md), [transfers](docs/transfers.md), [CLI usage](docs/cli.md), and [release workflow](docs/releases.md).

## Limits and remaining work

See [the feature status matrix](docs/status.md). Full conflict-resolution sync, mirror deletion, scheduled sync, parallel ranged downloads, per-connection transfer presets, remote editing, metadata/tag/version/lifecycle/CORS administration, benchmarks/optimizer, grid view, native remote-to-Explorer drag, tray/taskbar integrations, bandwidth limiting, custom CA/proxy UI, signed installers, and a credential-redacted diagnostics viewer are not implemented. Throughput charts reflect observed confirmed-byte changes and are not benchmark results. Large-scale performance and live provider compatibility are not yet established.

## License

[MIT](LICENSE). Contributions change the no-human-intervention provenance and must update the disclosure if a human reviews or edits code.
