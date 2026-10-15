# Architecture

VoidS3 is a Cargo workspace with a separate npm frontend workspace. Desktop and CLI share all provider/transfer code.

| Module                   | Responsibility                                                                                            |
| ------------------------ | --------------------------------------------------------------------------------------------------------- |
| `voids3-core`            | Shared models, errors, settings validation, multipart sizing, portable download-key validation            |
| `voids3-platform`        | Directory listing, safe path mapping, recursive enumeration, folder creation, rename, OS recycling        |
| `voids3-storage`         | SQLite WAL with durable commits; versioned document namespaces; OS keychain references                    |
| `voids3-s3`              | Official AWS SDK configuration; pagination, object APIs, conditional operations, safe error messages      |
| `voids3-transfer`        | Persistent queue, file/request semaphores, streaming I/O, multipart recovery and cancellation             |
| `voids3-sync`            | Deterministic conservative comparison; no timestamp or ETag equality assumptions                          |
| `voids3-app`             | Use cases shared by UI and CLI, recursive upload and additions-only sync orchestration                    |
| `apps/cli`               | Clap command parser and queue lifecycle                                                                   |
| `apps/desktop/src-tauri` | Typed request dispatch and desktop lifecycle; blocking operations moved off GUI thread                    |
| `apps/desktop/src`       | React 19, strict TypeScript, TanStack Query/Virtual, Zustand preferences, Radix and shadcn-style controls |

The IPC boundary carries JSON metadata only. File bodies stream between disk and S3 in Rust. Frontend queries are keyed by connection, prefix, page token, and local preferences. Queue snapshots are polled at 750 ms; upload bytes count confirmed units, and download bytes reflect actual stream writes. The present snapshot protocol sends all retained queue rows, so very large queue histories should be cleared; incremental event pagination is future work.

SQLite stores profile records, transfer settings, and transfer state in separate namespaces. Schema version is `PRAGMA user_version=1`. Credential records are referenced by UUID. Secrets are neither serialized as part of profiles nor included in transfer records. There is no plaintext fallback when a keychain is unavailable.

File and network work runs on Tokio or blocking workers, never the GUI thread. S3 requests use standard SDK retry policy with bounded attempts and connect/attempt timeouts. Transfer scheduling bounds simultaneous files and request bodies; browsing requests are independently paginated. Queue admission is capped for recursive uploads. Completed histories are persisted until explicitly cleared.

The initial implementation uses a process-local scheduler and an OS file lock that rejects a second desktop/CLI instance before opening shared state. Priority metadata is persisted but a reprioritization scheduler is not exposed. These are explicit constraints, not distributed scheduling guarantees. Recovery also compares the persisted connection target with the current endpoint/bucket/region/addressing mode, preventing a profile edit from redirecting an old transfer.
