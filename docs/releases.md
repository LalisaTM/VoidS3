# Branches, CI, and releases

`main` is the integration branch. Work on `feat/*`, `fix/*`, or `release/*`, commit using Conventional Commit-style subjects, push the branch, and open a PR. No human review is required for this experiment; automated checks are the gate. A release stays experimental until the acceptance criteria in the original brief are verified.

CI runs frontend formatting, strict TypeScript/build, ESLint, Vitest, npm and RustSec dependency audits, version consistency, Rust fmt/Clippy/tests on Windows/Linux/macOS, and a local Moto S3 integration job. Local S3 tests are an emulator contract suite, not proof of cloud compatibility. Actions are pinned to commit SHAs and updated by Dependabot. Workflow permissions default to read-only; only release publication gets contents-write. Main is protected against force pushes/deletion and requires all five CI jobs, including for administrators, without a human-review requirement.

The separate **Build** workflow packages the Windows x64 NSIS installer and CLI on every branch push, fork pull request, and manual run. Same-repository PR events skip a duplicate build because their commits already build on push. It uses read-only repository permissions, requires no signing or provider secrets, and publishes no release. Only main-branch pushes save the release build cache. A newer push cancels an older build for that branch.

To try a branch build, open [Build runs](https://github.com/LalisaTM/VoidS3/actions/workflows/build.yml), select a successful run for the desired branch/commit, and use its summary download link or Artifacts section. GitHub sign-in is required. The `voids3-windows-x64-<commit>` artifact contains the unsigned installer, `voids3-cli-windows-x64.zip`, `build-info.json` (version, commit, ref, and run URL), and `SHA256SUMS.txt`. Artifacts are retained for 14 days. These builds retain the branch's package version; the commit identifies the exact build. Tags remain the source of versioned releases.

Versions follow SemVer, starting at `0.1.0`. Until 1.0, minor releases may change behavior; patch releases fix defects. Set a version with:

```sh
node scripts/version.mjs 0.2.0
npm install --package-lock-only
cargo check
npm run version:check
```

Update CHANGELOG.md and docs/release-notes.md, commit lockfiles, open a release PR, and wait for CI before merging. Wait for the merged main commit's CI too, then tag it with `git tag -a v0.2.0 -m "VoidS3 0.2.0"` and `git push origin v0.2.0`. The Release workflow requires a main-branch ancestor with successful main CI and verifies tag/version consistency.

## Signed updater releases

The release build merges `apps/desktop/src-tauri/tauri.release.conf.json` into the base config, enabling Tauri updater artifacts. Only that step receives `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` from GitHub Actions secrets. Ordinary Build/CI jobs need no signing secrets and leave updater artifact generation disabled. Their installers still contain the public verification key; equal-version development builds do not replace a published version automatically.

Release assets include the NSIS installer, its `.sig`, the CLI ZIP, `latest.json`, and SHA256SUMS. The manifest uses an immutable version-tagged installer URL and embeds the signature contents. Tauri CLI binds the package version into the signature. The workflow verifies the package with the public key embedded in the app, using a mock runtime that never launches an installer or opens production application state. Failed verification prevents publication.

Assets upload into a draft before publication. Normal SemVer versions are marked GitHub latest, including experimental 0.x versions. Explicit SemVer prereleases remain prereleases and do not replace the updater channel. This is necessary because GitHub's `/releases/latest/download/latest.json` excludes prereleases. Already published releases are never overwritten by a workflow rerun; a failed draft upload can be retried.

Windows x64 uses **Settings > Application updates** and optional startup/daily checks. Downloads install only after confirmation and with an idle transfer queue. The installer closes/restarts the app; persisted data and paused transfers remain. Release notes render as plain text. The Rust backend enforces repository-specific HTTPS installer URLs, the embedded public key, signed-version matching, and strictly newer SemVer. Failed checks/downloads/install launches leave a retry path. Checks use a 20-second timeout and downloads a 5-minute timeout.

The first updater-enabled version is 0.2.0. Earlier installs need one manual upgrade. The CLI and other platforms still update manually. Installers remain per-user and lack Authenticode signing, so Windows may show SmartScreen; updater signatures do not confer Authenticode trust. Linux/macOS distribution packages are not configured. Release workflows do not require S3 credentials.

## Signing key custody

The encrypted production key and its generated password are backed up under `%USERPROFILE%\.voids3\updater-signing`, outside Git, in a directory accessible only to the current Windows user and SYSTEM. GitHub stores the same values as encrypted Actions secrets. Keep a secure backup of both files. Never print, commit, or attach either private file to a release. Only the generated public key is embedded in `tauri.conf.json`.

Do not regenerate or replace the signing identity for an existing installed population. Losing the key prevents publishing compatible updates; changing the public key requires a deliberate migration signed by the existing key. The committed test fixture uses an independent test-only identity, and its private key is not committed.

Optional live R2 validation is manually triggered in the `r2-test` environment. Add `R2_TEST_ENDPOINT`, `R2_TEST_BUCKET`, `R2_TEST_ACCESS_KEY`, and `R2_TEST_SECRET_KEY` secrets using a disposable bucket and bucket-scoped credentials. For EU tests, use the EU endpoint. The suite writes a random test prefix and attempts cleanup; it can incur request/storage charges. No live test runs automatically without secrets.
