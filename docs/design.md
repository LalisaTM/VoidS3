# Desktop design refresh

## Design read and audit

VoidS3 is a desktop storage client for developers and storage administrators. The
direction is calm, precise, and practical: native system typography, charcoal or
paper surfaces, and the existing sage accent. Design variance 4, motion intensity
3, visual density 6.

The design-taste-frontend skill excludes dashboards and data tables from its main
marketing workflow. This refresh applies its audit, consistency, accessibility,
and restraint guidance to the existing Radix primitives and owned button
components. It does not add a second design system.

Before the refresh, the shell used 9-12 px labels, many boxed toolbar bands, faint
helper text, an ornamental section number, and repeated connection prompts. The
panes lost useful space on laptop screens, and the page forced an 860 px minimum
width. Existing strengths are the geometric V mark, sage brand color, paired
local/remote navigation, virtualized rows, persistent layout preferences, and
accurate transfer progress.

## Implementation decisions

- Preserve branding, primary navigation, field names and order, file operations,
  keyboard selection, connection management, and all backend behavior.
- Give the workspace title, pane titles, controls, and secondary metadata distinct
  type sizes. Use the operating system's UI fonts without network font requests.
- Use a consistent spacing and radius scale, quieter dividers, and a single accent
  for selected states and primary actions. Keep errors and download indicators
  semantic. Functional helper text must remain readable in both themes.
- Keep file rows compact and virtualized. Size table columns by the actual pane
  width, including when a pane is resized or shown alone.
- Stack panes on narrow screens and expose navigation with a disclosure button.
  On short desktop windows, allow the workspace to scroll rather than clipping
  file actions. Do not rely on horizontal page overflow.
- Use brief opacity/transform transitions for dialogs and menus, plus direct hover
  and press feedback. Respect reduced motion. Do not animate file positions or
  invent transfer activity.
- Keep browser preview visibly disconnected. Empty states explain the actual
  next step; listings and throughput always come from real data.

## Contextual preflight

Marketing heroes, photography, testimonials, pricing, conversion sections, and
scroll storytelling are not applicable to this native file manager. No generated
images, external fonts, or additional component packages are needed. Validate the
dark and light shells, connection and settings dialogs, pane layouts, queue
disclosure, keyboard focus, narrow widths, short windows, and existing component
tests. Native filesystem/keychain behavior is outside this visual change.

## Verification

- Browser checks at 1440 x 900, 1366 x 768, 1280 x 800, 1024 x 768,
  1366 x 600, and 390 x 844. Check both themes, single/dual panes,
  expanded/collapsed queue, navigation disclosure with Escape/focus restoration,
  and scrollable connection/settings dialogs with visible footer actions.
- A temporary browser-only component fixture exercised 500 synthetic entries,
  a long Unicode filename, a resized 300 px pane, and a failed-transfer row.
  The lists rendered 50 rows across two panes, preserved ellipsis, and had no
  horizontal overflow. The fixture was removed and is not part of the app.
- Measured theme tokens meet 4.5:1 for secondary text on selected surfaces and
  3:1 for control borders on raised surfaces. Reduced-motion CSS disables all
  added transitions and animations.
- All 11 frontend tests pass, including navigation and queue disclosure state.
  ESLint, TypeScript/production build, Prettier, and version consistency pass.
  Rust and provider behavior are unchanged; the PR runs the existing CI matrix.
