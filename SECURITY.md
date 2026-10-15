# Security and provenance

VoidS3 is fully AI-written experimental software with no human code review or security audit. Do not treat a green CI run as a security guarantee. Report vulnerabilities privately through GitHub's private vulnerability reporting for this repository; do not publish credentials or presigned URLs in issues.

Credentials are stored through keyring 3 in Windows Credential Manager, macOS Keychain, or Linux Secret Service. There is no plaintext fallback. Linux requires an unlocked Secret Service session. The React connection form necessarily handles newly entered credentials transiently and sends them to Rust once; saved secrets cannot be retrieved through IPC. Non-secret profile exports contain endpoints, bucket names, and prefixes and may still reveal infrastructure information.

SQLite stores non-secret profiles and transfer metadata, including local paths and remote keys. OS user permissions protect the application data directory. The current application does not encrypt path/history data. The desktop and CLI share this directory; do not operate them concurrently against the same queue.

HTTPS is mandatory unless explicitly disabled per profile for a trusted local service. TLS certificate validation is never disabled. Connection forms accept endpoint origins only; embedded credentials, query strings, fragments, and endpoint paths are rejected. Credentials are not accepted through external URLs or shell commands.

SDK raw/debug request logging is not enabled. Provider errors include a service code and remediation rather than signed request structures. Signed URLs are generated only on request and copied to the clipboard, never logged or saved. They grant access until expiration and should be treated as secrets.

Downloads reject traversal, absolute paths, Windows reserved device names, invalid characters, and existing symlink ancestors when deriving paths from remote keys. Partial files use random transfer IDs. Finalization uses a same-directory hard link without replacing an existing destination. A malicious local process modifying the same directories can still race path checks; the application is not a sandbox against other processes running as the same OS user. File-size and ETag checks do not constitute end-to-end cryptographic integrity verification.

Remote previews are UTF-8 text, capped at 512 KiB and rendered as escaped text. HTML/SVG content is not executed. The webview's content security policy denies external scripts, frames, and plugins. IPC is available to the local main window, with native dialog selection and core event permissions. No shell or arbitrary executable-opening plugin is granted.

Uploads and downloads default to creating new destinations only. Remote copy preflights absence but cannot atomically reserve a destination; do not concurrently write its destination. Destructive GUI actions require confirmation. Conditional deletion fails if the provider rejects its precondition or does not support it. Mirror deletion is unavailable.

Release executables are unsigned. No updater or telemetry is installed. Dependencies are locked and checked in CI; an audit can only detect published advisories, not unknown vulnerabilities.
