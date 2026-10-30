# Changelog

## [0.10.18] - 2026-10-30

### Fixed
- Sidebar Project Loading Indicator Overlap: relocated the spinning loading indicator from the middle of the project card (`p_rect.center().y`) up to Row 1 (`p_rect.top() + 15.0`) vertically aligned with the project name. This guarantees a clean 10px vertical clearance above Row 2 environment badges, completely eliminating collisions with badge text and backgrounds (such as `PROD v1.1.9`).
- Added Row 1 title width reservation (`avail_name_w = p_rect.right() - 32.0 - content_x`) ensuring project names never collide with the top-right spinner.
- Added clamped width protection in `paint_sidebar_env_badge` to prevent long badge lists from overflowing the project card bounds.

## [0.10.17] - 2026-10-30

### Fixed
- Windows Defender SmartScreen / Unknown Publisher Warning:
  - Added full Windows PE resource metadata (`VERSIONINFO`) in `build.rs` detailing CompanyName ("Shiply"), FileDescription ("Shiply Setup Wizard & Deployment Manager"), ProductName ("Shiply"), FileVersion, and LegalCopyright, eliminating generic anonymous dropper heuristics in Microsoft Defender.
  - Added `unblock_file` helper in `src/installer.rs` and `src/updater.rs` that automatically strips the NTFS Mark-of-the-Web (`Zone.Identifier`) alternate data stream when installing or auto-updating binaries.
  - Implemented automated Authenticode code signing in the Windows CI release workflow using `Set-AuthenticodeSignature` to replace "Unknown publisher" with verified "Shiply".
  - Provided `Shiply-Setup.exe` (branded setup wizard) alongside generic `setup.exe` to avoid heuristic flag collisions.
  - Added 1-click `Shiply-Install.bat` companion launcher and updated `install-windows.ps1` that automatically unblocks the installer before execution to prevent the SmartScreen warning window.

## [0.10.16] - 2026-10-30

### Fixed
- Cross-Platform Compilation for Linux and macOS: removed errant `#[cfg(windows)]` attribute from `draw_shiply_logo` in `src/ui/icons.rs`, restoring logo rendering and resolving unresolved function symbol errors on macOS and Linux.
- Added `#[allow(unused_mut)]` on `install_system_fonts` in `src/ui/app.rs` to eliminate non-Windows unused mutability warnings.
- Added `#[allow(dead_code)]` on `open_windows_credential_manager` in `src/ui/helpers.rs` to eliminate non-Windows dead code warnings.
- Updated GitHub Actions CI/CD workflow `.github/workflows/release.yml` with pinned `ubuntu-24.04` runner and Node 20 environment flag to prevent deprecation notices.

## [0.10.15] - 2026-10-30

### Refactored
- Modularized UI Architecture (Rust Desktop Best Practice): decomposed the 14,000+ line monolithic `src/ui.rs` into a clean, cohesive `src/ui/` module hierarchy (`theme`, `types`, `topbar`, `sidebar`, `modals`, `views`, `components`, `helpers`, `icons`, `app`, and `tests`).
- Idiomatic Modular Structure: cleanly separated design tokens, custom text edit widgets with cursor mechanics, vector icon renderers, modal lifecycles, and views into dedicated files adhering strictly to KISS, DRY, and YAGNI principles.

### Fixed
- GitHub Releases Rate-Limit Bypass: eliminated `Could not reach GitHub Releases` error banner caused by GitHub's strict 60 req/hr unauthenticated API limit on shared IPs/VPNs. Added primary rate-limit-free CDN fallback to `latest.json`, secondary GitHub CLI (`gh release view`) fallback, and automatic authorization token forwarding.
- Removed Topbar and Sidebar White Line: added `.show_separator_line(false)` to both `TopBottomPanel::top` and `SidePanel::left`, completely eliminating the unwanted light separator border between the dark navy topbar and sidebar.

## [0.10.14] - 2026-10-29

### Refactored
- Unified Modal Lifecycle Architecture: abstracted modal presentation, backdrop rendering, smooth 60fps opacity fade, slide translateY entrance, Escape key dismissal, and backdrop outside-click detection into reusable [`prepare_animated_modal`](file:///D:/NTI/Labs/shiply/src/ui.rs#L4157-L4195) and [`animated_modal_window`](file:///D:/NTI/Labs/shiply/src/ui.rs#L4196-L4220) helpers.
- Standardized All 6 Application Modals: migrated `Add Project`, `Edit Project`, `Add Environment Level`, `Bulk Deployment`, `Software Updates`, and `Confirm Deletion` dialogs onto the unified lifecycle, eliminating over 200 lines of repetitive boilerplate across `src/ui.rs` while ensuring consistent animation timing, responsive sizing, and click-to-dismiss behavior across every modal.
- Adherence to DRY & KISS Principles: centralized dialog setup logic, improved code maintainability, and added comprehensive lifecycle unit testing (`test_prepare_animated_modal_and_window_lifecycle`).

## [0.10.13] - 2026-10-29

### Added
- Donation Support Button: added a dedicated heart icon button (`OutlineIcon::Heart`) in the topbar and a "Donate" link in the sidebar footer status row, seamlessly redirecting developers to [Trakteer (faidfadjri)](https://trakteer.id/faidfadjri).
- Smooth Modal Animations: implemented silky 150ms opacity fade and 18px vertical translateY slide-in animations for both "Add New Project" and "Edit Project Settings" dialogs.
- Refresh Button Rotation Animation: added dynamic 360-degree spinning rotation animation on the refresh icon across header and context menu triggers.
- Modern Drag Animation for Projects & Pipelines: replaced plain glowing borders with genuine drag animations featuring soft elevation drop shadows for dragged cards, animated 3px glowing insertion indicator drop lines with circular handles indicating precise top/bottom drop slots, and subtle target card highlights.

### Changed
- 3-Column Environment Cards Grid: updated workspace environment cards layout to use 3 columns on available widths >= 650px (and 2 columns >= 440px), guaranteeing Development, Production, and Add/Import Environment cards sit side-by-side in a single row filling the available horizontal space.
- Add Project Buttons State: "Create Project" and "Auto-Fill" buttons are now always visible in the Add New Project modal, styled in a clean subdued grey fill (`#DEE2EB`) with muted text until required fields are filled, smoothly transitioning to solid brand blue upon population.
- Topbar Black Theme: transformed the top window header into a sleek, unified deep navy-black (`#0B1220`) matching the sidebar palette, featuring crisp white title typography (`#F1F5F9`), subtle dark button hover states (`#1A2438`), and light icon controls.
- Removed Dark Mode Button: eliminated the theme toggle button (`☀️ / 🌙`) from the topbar, streamlining the title bar to essential window management, notifications, and navigation.
- High-Contrast Grey-Blue Slate Canvas: upgraded the main workspace background from pure white to a modern cool grey-blue slate gradient (`#EEF2F8` to `#E2E9F3`), delivering striking visual contrast that makes pure white cards (`#FFFFFF`) pop with clean definition.

### Fixed
- Sync Button Missing Glyph Tofu Box: removed unsupported Unicode geometric triangle characters (`▴`/`▾`) from the Sync button label, eliminating the weird rectangle icon `□` on Windows desktop.
- Sort Dropdown Active Option Contrast: resolved white text on pale/white background in the sidebar sort dropdown menu; selected items now render high-contrast bright white text and checkmark over solid brand blue (`#146EF5`).
- Reset Form Fields on Add Project Re-Open: opening or re-opening the "Add Project" modal now automatically clears all input fields (`new_project_name`, `new_project_path`, `new_project_desc`, etc.), ensuring a clean form every time.
- Text Input Cursor Glitch on First Click: eliminated the 1-frame cursor jump where the cursor briefly flickered at position 0 on mouse down before moving to the clicked position. Refactored `styled_text_edit`, `styled_password_edit`, and `modal_text_input` to vertically center a natural singleline 22px text row (`Margin::ZERO`, `.cursor_at_end(true)`) within the 36px container. Egui's internal cursor hit-testing now evaluates `row.char_at(pos.x)` directly on pointer press/mouse-down, ensuring the cursor immediately appears at the exact clicked character on frame 1 without any initial snap to index 0.

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
