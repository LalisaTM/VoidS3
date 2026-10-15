# Validation evidence

Local Windows x64 validation on 2026-10-15:

- Rust backend/CLI and Tauri desktop compile with stable Rust 1.99.0.
- Fifteen Rust unit tests and all workspace/doc-test targets pass locally. The optional credentialed integration suite is skipped in normal unit runs and executed separately.
- Strict TypeScript and Vite production build pass.
- ESLint and Rust Clippy pass with warnings denied.
- Twenty-three Vitest/React Testing Library tests pass, covering honest browser mode, layouts, connection submission/EU endpoint generation, reusing saved credentials after testing, backend errors, queue controls/progress, file filtering/range/keyboard selection, read-only controls, pane resizing, navigation/bookmark actions, global queue controls across filters, confirmed batch deletion with partial failures, folder/group dragging, Escape cancellation, native drop coordinates, and path/size utilities.
- Real HTTP S3 integration against a loopback Moto 5.1.14 server passes: small/multipart uploads, concurrent files, no-clobber remote uploads/local downloads, exact content comparisons, listing, text preview, multipart state reconciliation after simulated interruption, partial download recovery, changed-source refusal, multipart cancellation cleanup, global pause/cancel before queued workers run, copy/delete, folder markers, and a real HTTP GET using a presigned URL.
- The collaborative browser was used to inspect the desktop frontend and connection dialog; browser mode intentionally has no native access.
- `npm audit` reports zero known advisories for the locked frontend dependencies.
- `cargo audit` reports zero vulnerability findings; upstream informational GTK/proc-macro notices remain visible and are documented in SECURITY.md.
- A local optimized Windows x64 NSIS installer builds successfully (approximately 6.4 MiB before the TLS dependency reduction); the CLI release binary also builds.

Hosted results are recorded separately in [CI](https://github.com/LalisaTM/VoidS3/actions/workflows/ci.yml) and [Release](https://github.com/LalisaTM/VoidS3/actions/workflows/release.yml); inspect the run for the exact commit/tag. Live R2 tests have not run because no live credentials were provided. Automatic approval review rejected the combined native credential-store/launch smoke-test command with "blocked by policy" and no more specific reason. Native app/installer execution and OS credential-store round trips therefore remain unverified. Provider permissions and scale/performance must not be inferred from emulator tests.
