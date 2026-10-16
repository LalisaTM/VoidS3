VoidS3 0.2.1 adds WinSCP-style upload conflict decisions. Existing destinations show incoming and existing sizes/timestamps, with Replace, Skip, Keep both, and Apply to all remaining conflicts in this upload. Cancel upload discards the entire pending batch. File selections, recursive folder uploads, and drag/drop all use the same check before queueing. Keep both finds an unused numbered filename.

Replacement is conditional on the destination ETag you confirmed, for both single-request and multipart uploads. Changed sources or destinations require a fresh upload confirmation; no unconditional overwrite fallback is used. Retry/restart preserves the original ETag. CLI upload and additions-only sync keep their no-overwrite behavior. Local download conflict choices and sync overwrite/mirror policies remain unfinished.

Windows x64 version 0.2.0 can discover this release through Settings > Application updates. Versions 0.1.1 and earlier require a manual installation. Install and restart requires confirmation and an idle transfer queue. Profiles, credentials and paused transfers persist.

No human has read, reviewed, edited, or otherwise manually intervened in this code. It was built for fun by AI agents from a human-supplied specification and visual references. This remains experimental software without a security audit or production-readiness claim.

The installer has a Tauri updater signature and is not Authenticode-signed, so SmartScreen may appear. Verify SHA256SUMS for manual downloads. CLI upgrades remain manual. Provider conditional-write support varies; unsupported operations fail visibly. See docs/status.md and docs/validation.md for scope and evidence.
