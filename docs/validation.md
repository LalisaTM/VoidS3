# Validation evidence

Large-batch update validated locally on Windows x64, 2026-10-16:

- Thirty-nine frontend tests cover background upload registration, per-file conflict decisions, future batch policies, presets/custom edits, a 10,000-item summary with only 100 rendered rows, pagination/filter requests and no collapsed-page fetching.
- Rust tests cover bounded discovery chunks, interrupted discovery recovery and saved batch policies, preset validation and 10,000-job aggregation/pagination/transactional cancellation. One local debug-build aggregate plus final 100-row filtered page took approximately 4.6 ms; this is a synthetic queue-processing measurement, not a cloud-throughput or native-frame-rate benchmark.
- Real HTTP Moto integration verifies files completing before discovery finishes, conflicts not blocking fresh files, future batch policies, changed-source refusal, single/multipart replacements, Keep both and global pause/cancel of further admission with the updater guard.
- Strict TypeScript/Vite build, ESLint, Clippy with warnings denied and workspace tests pass. Installed Windows frame-rate tracing, 10,000 real image transfers and live-provider performance remain unverified.

Upload-conflict update validated on Windows x64, 2026-10-16:

- Thirty-seven frontend tests pass, including mixed and apply-to-all choices, cancellation/Escape, missing ETags, immediate conflict-free enqueue, cancelled in-flight preflight, fresh confirmation after failure, and the actual workspace upload entry point.
- Twenty Rust unit tests and workspace/doc-test targets pass. The real HTTP Moto suite additionally verifies folder preflight, whole-batch cancellation, missing decisions, Keep both name collisions, Skip, exact single/multipart replacement content, stale destination refusal after restart, changed-source refusal before any enqueue, and objects created after preflight.
- Strict TypeScript/Vite build, ESLint, Clippy with warnings denied, formatting and version consistency checks pass.
- Collaborative browser DOM checks on a synthetic long-path/Unicode conflict fixture confirm no horizontal overflow at 1280px and 390px, readable stacked comparisons on narrow screens, and initial focus on Skip. Screenshot capture failed in the preview client; no screenshot-based or native desktop visual verification is claimed. The temporary fixture was removed.
- Live cloud and installed-app checks remain unverified as described below.

Local Windows x64 validation on 2026-10-15:

- Rust backend/CLI and Tauri desktop compile with stable Rust 1.99.0.
- Eighteen Rust unit tests and all workspace/doc-test targets pass locally. The optional credentialed integration suite is skipped in normal unit runs and executed separately.
- Updater tests verify an independently signed non-executable fixture, reject altered bytes and inflated announced versions, reject older/equal versions and foreign installer URLs, and exercise the queue-idle installation guard. Two Node release-manifest tests cover version/tag consistency and asset selection. An opt-in release-package test verifies the actual installer against the embedded production public key without launching it.
- Strict TypeScript and Vite production build pass.
- ESLint and Rust Clippy pass with warnings denied.
- Twenty-seven Vitest/React Testing Library tests pass, covering honest browser mode, layouts, connection submission/EU endpoint generation, reusing saved credentials after testing, backend errors, queue controls/progress, file filtering/range/keyboard selection, read-only controls, pane resizing, navigation/bookmark actions, global queue controls across filters, confirmed batch deletion with partial failures, folder/group dragging, Escape cancellation, native drop coordinates, path/size utilities, update checks, installation confirmation, real progress events, offline retry, and keeping Settings open during installation.
- Real HTTP S3 integration against a loopback Moto 5.1.14 server passes: small/multipart uploads, concurrent files, no-clobber remote uploads/local downloads, exact content comparisons, listing, text preview, multipart state reconciliation after simulated interruption, partial download recovery, changed-source refusal, multipart cancellation cleanup, global pause/cancel before queued workers run, copy/delete, folder markers, and a real HTTP GET using a presigned URL.
- The collaborative browser was used to inspect the desktop frontend and connection dialog; browser mode intentionally has no native access.
- `npm audit` reports zero known advisories for the locked frontend dependencies.
- `cargo audit` reports zero vulnerability findings; upstream informational GTK/proc-macro notices remain visible and are documented in SECURITY.md.
- A local optimized Windows x64 NSIS installer builds successfully (approximately 6.4 MiB before the TLS dependency reduction); the CLI release binary also builds.

Hosted results are recorded separately in [CI](https://github.com/LalisaTM/VoidS3/actions/workflows/ci.yml) and [Release](https://github.com/LalisaTM/VoidS3/actions/workflows/release.yml); inspect the run for the exact commit/tag. Live R2 tests have not run because no live credentials were provided. Automatic approval review rejected the combined native credential-store/launch smoke-test command with "blocked by policy" and no more specific reason. Native app/installer execution and OS credential-store round trips therefore remain unverified. Provider permissions and scale/performance must not be inferred from emulator tests.
