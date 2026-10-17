VoidS3 0.3.1 adds a native close warning when transfers, upload discovery or preparation are pending. Keep open is the default. Choosing Cancel transfers and close stops new work, cancels pending transfers across connections and waits for multipart/partial-download cleanup before exiting. Completed files stay in place.

Cleanup runs in the background. The native window title shows cancellation progress. If cleanup fails or takes over 30 seconds, a second dialog offers Keep open or Close anyway; unfinished records are retained for retry. A forced process termination cannot run orderly cleanup.

Queue operations, settings access, download-path validation, automatic conflict decisions and sync comparison use background workers. Directory listings over 500 entries sort/filter in a Web Worker; totals are cached rather than rescanned on every render. A regression test verifies that shutdown waiting on a blocked SQLite writer does not block the single-thread async executor. Installed-app frame rates and live-provider cleanup have not been profiled or verified.

Windows x64 versions 0.2.0 and newer can discover this release through Settings > Application updates. Older versions require manual installation. Install and restart requires confirmation and an idle transfer queue. Update installation blocks ordinary close attempts and preserves paused records through the installer restart.

No human has read, reviewed, edited, or otherwise manually intervened in this code. It was built for fun by AI agents from a human-supplied specification and visual references. This remains experimental software without a security audit or production-readiness claim.

The installer has a Tauri updater signature and is not Authenticode-signed, so SmartScreen may appear. Verify SHA256SUMS for manual downloads. CLI upgrades remain manual. Provider conditional-write support varies; unsupported operations fail visibly. See docs/status.md, docs/validation.md and docs/roadmap.md for scope, evidence and proposed next features.
