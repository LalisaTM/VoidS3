VoidS3 0.3.0 removes the batch-wide upload preflight. Background discovery feeds a bounded worker backlog, and uploads start while the remaining folder is still being discovered. New small files use conditional uploads directly. Existing files wait individually for Replace, Skip or Keep both while other files continue. Apply to all covers later conflicts in the same batch. Pause all / Cancel all also stop discovery and deferred conflict work.

The desktop requests a small queue summary plus at most 100 lightweight rows per page instead of repeatedly transferring and rendering the entire queue. Durable queue writes run on blocking workers. Settings adds Balanced, Many small files, Large files, Low impact and Unreliable connection presets with editable advanced settings. Restart VoidS3 after changing file/request concurrency. Conditional replacement protection remains in place.

Incomplete discovery is marked interrupted after restart; already admitted transfers recover paused. Select the folder again to discover unqueued files. Live-cloud throughput and installed-app frame rates have not been benchmarked. Local tests cover 10,000-job queue bounds and real HTTP S3 discovery, conflicts and global stops.

Windows x64 versions 0.2.0 and newer can discover this release through Settings > Application updates. Older versions require a manual installation. Install and restart requires confirmation and an idle transfer queue. Profiles, credentials and paused transfers persist.

No human has read, reviewed, edited, or otherwise manually intervened in this code. It was built for fun by AI agents from a human-supplied specification and visual references. This remains experimental software without a security audit or production-readiness claim.

The installer has a Tauri updater signature and is not Authenticode-signed, so SmartScreen may appear. Verify SHA256SUMS for manual downloads. CLI upgrades remain manual. Provider conditional-write support varies; unsupported operations fail visibly. See docs/status.md, docs/validation.md and docs/roadmap.md for scope, evidence and proposed next features.
