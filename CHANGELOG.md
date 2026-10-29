# Changelog

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
