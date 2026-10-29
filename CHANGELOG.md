# Changelog

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
