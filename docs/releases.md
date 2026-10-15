# Branches, CI, and releases

`main` is the integration branch. Work on `feat/*`, `fix/*`, or `release/*`, commit using Conventional Commit-style subjects, push the branch, and open a PR. No human review is required for this experiment; automated checks are the gate. A release stays experimental until the acceptance criteria in the original brief are verified.

CI runs frontend formatting, strict TypeScript/build, ESLint, Vitest, npm and RustSec dependency audits, version consistency, Rust fmt/Clippy/tests on Windows/Linux/macOS, and a local Moto S3 integration job. Local S3 tests are an emulator contract suite, not proof of cloud compatibility. Actions are pinned to commit SHAs and updated by Dependabot. Workflow permissions default to read-only; only release publication gets contents-write. Main is protected against force pushes/deletion and requires all five CI jobs, including for administrators, without a human-review requirement.

Versions follow SemVer, starting at `0.1.0`. Until 1.0, minor releases may change behavior; patch releases fix defects. Set a version with:

```sh
node scripts/version.mjs 0.1.1
npm install --package-lock-only
cargo check
npm run version:check
```

Update CHANGELOG.md and docs/release-notes.md, commit lockfiles, open a release PR, and wait for CI before merging. Then tag the merged main commit with `git tag -a v0.1.1 -m "VoidS3 0.1.1"` and `git push origin v0.1.1`. The Release workflow verifies tag/version consistency, builds the Windows NSIS installer and CLI, creates SHA256SUMS, uploads build artifacts, and publishes a GitHub prerelease. No npm/crates.io publishing is configured.

The installer is per-user and unsigned. Windows may show SmartScreen; do not claim Authenticode trust. Linux/macOS packages and signing keys are not configured. Release workflows do not require S3 credentials.

Optional live R2 validation is manually triggered in the `r2-test` environment. Add `R2_TEST_ENDPOINT`, `R2_TEST_BUCKET`, `R2_TEST_ACCESS_KEY`, and `R2_TEST_SECRET_KEY` secrets using a disposable bucket and bucket-scoped credentials. For EU tests, use the EU endpoint. The suite writes a random test prefix and attempts cleanup; it can incur request/storage charges. No live test runs automatically without secrets.
