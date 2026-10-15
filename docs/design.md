# Desktop design refresh

## Design read and audit

VoidS3 is a native file manager for developers and storage administrators. The user supplied a Figma Make reference, replacing the earlier sage direction with a charcoal and violet workspace. Design variance 4, motion intensity 3, visual density 7.

The design-taste-frontend skill excludes dashboards and data tables from its marketing workflow. This redesign applies its audit, consistency, readability, and motion guidance to the existing React/Radix components. No second design system, external fonts, or decorative images are added.

The reference establishes the layered square mark, compact top navigation, 226 px connection sidebar, global command bar, flat split panes, path bars, and bottom transfer queue. Its tiny labels are enlarged where needed for readability. Its sample accounts, files, latency, throughput, ETA, TLS version, and app version are excluded from the implementation.

## Implementation decisions

- Match the reference's hierarchy, violet palette, flat dividers, source badges, compact tables, and layered mark. Keep native system fonts and readable text in both themes.
- Wire Files, Transfers, Bookmarks, Search, and Activity to actual application state. Search filters loaded entries; Activity displays current alerts. Storage class is displayed only when the provider returns it.
- Preserve local and remote navigation, settings, connections, properties, copy, signed URLs, text preview, sync, and keyboard selection. Saved folder pairs navigate once to the stored prefix.
- Keep lists virtualized and columns responsive to pane width. Support pointer and keyboard resizing. Stack panes and disclose navigation on narrow screens, with scrollable short-window layouts.
- Keep Pause all, Resume all, and Cancel all visible in the queue header. Expose selected-file deletion in the command bar, context menu, and keyboard. Confirm destructive operations and report partial failures.
- Use pointer dragging between panes while retaining Tauri native Explorer drops. Highlight the actual target folder, use a lightweight drag ghost, and support Escape cancellation. Writable-connection restrictions remain enforced in Rust.
- Use short dialog/menu transitions and immediate feedback. Respect reduced motion. Do not animate file positions or fabricate transfer progress.

## Contextual preflight and verification

Marketing heroes, photography, pricing, testimonials, and scroll storytelling do not fit this workspace. Browser preview remains visibly disconnected and has no filesystem or keychain access.

Browser checks cover 1440 x 900, 1280 x 800, 1024 x 768, and 390 x 844, including both themes, queue disclosure, responsive controls, navigation focus restoration, and dialogs. A temporary browser-only fixture exercises 500 synthetic entries, long Unicode names, storage classes, and a failed transfer; it is removed after inspection. Automated tests cover the meaningful selection, navigation, confirmation, cancellation, and drop-target behavior.

Native GUI execution, real Explorer drops, OS keychain round trips, and live R2 remain outside the verified browser/emulator scope. See [validation.md](validation.md) for the exact evidence and limitations.
