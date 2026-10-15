# Validation evidence

Local Windows x64 validation on 2026-10-15:

- Rust backend/CLI and Tauri desktop compile with stable Rust 1.99.0.
- Strict TypeScript and Vite production build pass.
- ESLint and Rust Clippy pass with warnings denied.
- Seven Vitest/React Testing Library tests pass, covering honest browser mode, layouts, connection submission/EU endpoint generation, backend errors, queue controls/progress, and path/size utilities.
- Real HTTP S3 integration against a loopback Moto 5.1.14 server passes: small/multipart uploads, concurrent files, no-clobber remote uploads/local downloads, exact content comparisons, listing, text preview, multipart state reconciliation after simulated interruption, partial download recovery, changed-source refusal, multipart cancellation cleanup, copy/delete, folder markers, and presigned URL generation.
- The collaborative browser was used to inspect the desktop frontend and connection dialog; browser mode intentionally has no native access.
- `npm audit` reports zero known advisories for the locked frontend dependencies.

Full cross-platform CI, tagged release packaging, additional signed-URL HTTP validation, and the final workspace test run are being recorded before the first release. See GitHub Actions for authoritative run results. Live R2 tests have not run because no live credentials were provided. Windows installer launch, provider permissions, scale/performance, and OS credential-store integration are distinct from the emulator contract tests and must not be inferred from them.
