# Changelog

## [0.10.12] - 2026-10-29

### Fixed
- Search Project Field Double Outline: eliminated duplicate nested focus frames in `sidebar_search_text_edit` by configuring the inner `TextEdit` with `.frame(false)` and rendering a single crisp accent border with rounded corners. Added a left magnifying glass search icon and a quick-clear (`X`) button when the input is populated.
- Frameless Topbar Clean Border: removed the dark horizontal stroke beneath the top window panel (`Stroke::NONE`), delivering a seamless frameless header aligned with standard enterprise desktop software.
- Text Edit Cursor Snap & Glitch: resolved the issue where clicking into prefilled input fields (such as Target Servers, app name, remote directory) jumped to character 0 or glitched out. Sampling now occurs at the vertical center of the text line, preventing negative Y offsets from triggering line-0 snaps. Clicking to the right of text places the cursor at the end of the line, while multi-character drag selections and double-click word highlights are strictly protected from collapsing.
- Sidebar Over-Height & Windows Taskbar Clipping: fixed the sidebar footer overflowing past the bottom of the window into the Windows taskbar. By eliminating unwanted implicit item spacing (`item_spacing = Vec2::ZERO`) and establishing a pinned 138px expanded / 132px rail footer container, all footer action buttons and the version status row remain cleanly visible above the taskbar with comfortable breathing margins.

## [0.10.11] - 2026-10-29

### Changed
- Dark Sidebar: restored and preserved the deep navy-black dark background (`#0B1220`) and high-contrast dark theme tokens across the sidebar in expanded mode, collapsed 64px rail mode, and off-canvas mobile drawer.
- Window Topbar: removed the 3px blue line on top of the window and vertically centered the topbar elements cleanly.
- Sidebar Search: added dedicated dark styling `sidebar_search_text_edit` with smooth cursor positioning, instant text blocking, and dark borders matching sidebar design tokens.

## [0.10.10] - 2026-10-29

### Added
- Webflow-Style Light First Theme: clean white canvas, brand blue accents (`#146EF5`), near-black typography (`#080808`), soft lavender-blue tint backgrounds, hairline borders (`#E6E6EA`), and crisp radii across all components, with full dark mode support.
- Top Signature Accent: 3px solid `--primary` strip across the very top edge of the window.
- Fully Responsive Layout Matrix:
  - Responsive Sidebar: expanded (248px) at $\ge 1200\text{px}$, collapsed rail (64px) with 32px initial avatars at $900\text{px}-1199\text{px}$, off-canvas drawer with `rgba(8,8,8,0.4)` overlay below $900\text{px}$.
  - Environment Cards: auto-responsive 3 columns ($\ge 1200\text{px}$), 2 columns ($900\text{px}-1199\text{px}$), and 1 column below $900\text{px}$.
  - Project Header: below $720\text{px}$ keeps primary Deploy button visible and moves project actions into a `MoreHorizontal` ("⋯") menu.
  - Workspace Tabs: horizontally scrollable single row with hidden scrollbars, auto-scrolling active tab into view (`scroll_to_me`), uniform 40px height.
  - Deployments History: desktop 48px row table at $\ge 900\text{px}$, responsive stacked cards below $900\text{px}$ with status, version, git commit details, duration, and action buttons.
  - Settings & Forms: 2 columns at $\ge 800\text{px}$, 1 column stack below $800\text{px}$.
  - Modals & Sheets: responsive dialogs scaling to `min(560px, 100vw - 32px)`, converting to a full-screen sheet below $480\text{px}$.
  - Window Size: minimum inner window dimensions enforced at $800\times 560$.

### Fixed
- Text Input Cursor Snap: clicking a prefilled field now places the cursor accurately where clicked rather than snapping to the beginning of the field.
- Text Drag Selection Lag: fixed mouse drag lag and text blocking failure by requesting continuous frame repaints during active selection interactions.

## [0.10.9] - 2026-10-29

### Fixed
- Versioning: if a deployment fails at any step during execution, `package.json` is automatically reverted to its exact original version via RAII rollback guards, preventing failed runs from leaving behind a dirty git working directory or premature version bumps.
- Auto-Bumping: `next_auto_bumped_version` now strictly computes the next version from succeeded deployments only, ensuring failed deployments do not bump the auto-suggested patch version on subsequent retries.

## [0.10.8] - 2026-10-29

### Added
- Environment-level Deployment Concurrency: users can now deploy multiple environments simultaneously on the same project (e.g. running both Development and Production deployments at the same time on the same project). Duplicate deployment prevention is strictly scoped to the exact `(project_id, environment_key)` pair.
- Multi-Environment Live Switching: switching between environment tabs dynamically prioritizes and streams the live console output for the currently viewed environment while other environments continue running concurrently in the background.
- Sidebar & Action Bar Isolation: action bar Deploy and Quick Rollback buttons, as well as sidebar environment badge indicators, now independently track and display the deployment status of each environment.

## [0.10.7] - 2026-10-29

### Changed
- Concurrency: decoupled active deployment locking from a global app-level mutex to per-project isolation (`active_deployments`). Deploying a backend project now prevents only duplicate runs for the same project, allowing frontend or any other project to be deployed simultaneously in parallel.
- Continuous Deployments on Update: downloading or receiving an in-app update notification no longer interrupts active deployment processes. Restarts are safely deferred until all active deployments and queued bulk deployments finish to completion, with an in-modal status banner and non-blocking dismissal allowing full visibility into active pipeline logs.
- UI: per-project deploy and rollback buttons, sidebar loading spinners, and console indicators now track each project's individual deployment state without blocking other projects.

## [0.10.6] - 2026-10-29

### Added
- Deployment History: filtered to strictly show the active environment of the selected project (e.g. opening backend project and selecting development displays only development deployments; switching environments dynamically updates history and tab counts).
- Version Sync: added smooth and clean "Sync" button across environments with 1-click sync for single candidate environments and an elevated custom dropdown for picking between sibling environments or `package.json`.
- Console Pipeline Indicator: added animated circular loading indicator on running pipelines across all console touchpoints (Console Output section header, Terminal window title bar, and live execution status at the bottom of the log stream).

## [0.10.5] - 2026-10-29

### Fixed
- UI: fixed 4 visual/UX bugs in the sidebar:
  - Replaced native select popup with a dark custom sort dropdown matching sidebar tokens.
  - Replaced broken Unicode glyphs (tofu boxes "□") with unified inline vector outline icons.
  - Redesigned project context menu into a cohesive dark container with consistent 16px icon slot alignment, disabled state styling, and upward/leftward viewport collision flip.
  - Fixed sidebar footer clipping by increasing height to 172px, adding 8px bottom clearance and 16px bottom padding to guarantee text descenders are never clipped.

## [0.10.4] - 2026-10-29

### Changed
- UI: modernized sidebar and dialogs, unified design tokens, skeleton loading for the project list, fixed project name wrapping while loading. No functional changes.
