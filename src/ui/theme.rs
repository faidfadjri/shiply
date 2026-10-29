use eframe::egui::Color32;

// =============================================================================
// DESIGN TOKENS (Webflow-Style Light First System with Optional Dark Theme)
// =============================================================================

// Typography scale (bold tight headings, small medium-weight UI text, monospace for technical strings)
pub const FS_TITLE: f32 = 18.0; // Modal & section title (18px, weight 600)
pub const FS_BRAND: f32 = 16.0; // "Shiply" brand name (16px, weight 600)
pub const FS_BASE: f32 = 13.0;  // UI / body text (13px, weight 500)
pub const FS_SMALL: f32 = 12.0; // Secondary metadata, subtitles, helper text (12px, weight 400)
pub const FS_MICRO: f32 = 11.0; // Badges, mono tags, versions, status (11px, weight 500)

// Spacing scale: ONLY 4, 8, 12, 16, 20, 24 px
pub const SP_4: f32 = 4.0;
pub const SP_8: f32 = 8.0;
pub const SP_12: f32 = 12.0;
pub const SP_16: f32 = 16.0;
pub const SP_20: f32 = 20.0;
pub const SP_24: f32 = 24.0;

// Radius scale: buttons/inputs 6px, cards 10px, badges 4px (crisp, no big pills), modals 14px
pub const R_SM: f32 = 4.0;  // badges (4px crisp)
pub const R_MD: f32 = 6.0;  // buttons, inputs, segmented controls (6px)
pub const R_LG: f32 = 10.0; // feature cards, plain containers (10px)
pub const R_XL: f32 = 14.0; // modal dialogs (14px)

// Standard component heights
pub const BTN_HEIGHT: f32 = 32.0;
pub const BTN_PRIMARY_HEIGHT: f32 = 36.0;
pub const INPUT_HEIGHT: f32 = 36.0;
pub const TABLE_ROW_HEIGHT: f32 = 48.0;
pub const DONATION_URL: &str = "https://trakteer.id/faidfadjri";
pub const TAB_HEIGHT: f32 = 40.0;
pub const ICON_HIT_AREA: f32 = 32.0;
pub const SIDEBAR_EXPANDED_WIDTH: f32 = 248.0;
pub const SIDEBAR_COLLAPSED_WIDTH: f32 = 64.0;

// Theme definition struct for central theme control
#[derive(Debug, Clone)]
pub struct Theme {
    pub is_dark: bool,
    pub bg: Color32,
    pub bg_gradient_top: Color32,
    pub bg_gradient_bottom: Color32,
    pub surface: Color32,
    pub surface_2: Color32,
    pub surface_tint_top: Color32,
    pub surface_tint_bottom: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub text_subtle: Color32,
    pub primary: Color32,
    pub primary_hover: Color32,
    pub primary_active: Color32,
    pub primary_soft: Color32,
    pub primary_soft_border: Color32,
    pub ink: Color32,
    pub ink_fg: Color32,
    pub success: Color32,
    pub success_soft: Color32,
    pub danger: Color32,
    pub danger_soft: Color32,
    pub warning: Color32,
    pub warning_soft: Color32,
    pub purple: Color32,
    pub purple_soft: Color32,
    pub shadow_sm: Color32,
    pub shadow_hover: Color32,
    pub focus_ring: Color32,
}

impl Theme {
    pub fn light() -> Self {
        Self {
            is_dark: false,
            bg: Color32::from_rgb(235, 239, 246), // Cool grey-blue slate canvas
            bg_gradient_top: Color32::from_rgb(238, 242, 248), // #EEF2F8
            bg_gradient_bottom: Color32::from_rgb(226, 233, 243), // #E2E9F3
            surface: Color32::from_rgb(255, 255, 255), // Pure white cards (#FFFFFF)
            surface_2: Color32::from_rgb(243, 246, 251), // #F3F6FB subtle hover fill
            surface_tint_top: Color32::from_rgb(255, 255, 255), // Crisp white top for cards
            surface_tint_bottom: Color32::from_rgb(248, 250, 253), // Soft tint for cards
            border: Color32::from_rgb(218, 224, 233), // Hairline border
            border_strong: Color32::from_rgb(203, 213, 225), // Strong border for inputs
            text: Color32::from_rgb(8, 8, 8), // #080808
            text_muted: Color32::from_rgb(90, 90, 90), // #5A5A5A
            text_subtle: Color32::from_rgb(138, 138, 143), // #8A8A8F
            primary: Color32::from_rgb(20, 110, 245), // #146EF5
            primary_hover: Color32::from_rgb(15, 94, 215), // #0F5ED7
            primary_active: Color32::from_rgb(11, 79, 184), // #0B4FB8
            primary_soft: Color32::from_rgb(231, 240, 254), // #E7F0FE
            primary_soft_border: Color32::from_rgb(207, 224, 253), // #CFE0FD
            ink: Color32::from_rgb(8, 8, 8), // #080808
            ink_fg: Color32::from_rgb(255, 255, 255),
            success: Color32::from_rgb(10, 143, 60), // #0A8F3C
            success_soft: Color32::from_rgb(230, 247, 236), // #E6F7EC
            danger: Color32::from_rgb(238, 29, 54), // #EE1D36
            danger_soft: Color32::from_rgb(254, 236, 238), // #FEECEE
            warning: Color32::from_rgb(194, 106, 0), // #C26A00
            warning_soft: Color32::from_rgb(255, 243, 223), // #FFF3DF
            purple: Color32::from_rgb(122, 61, 255), // #7A3DFF
            purple_soft: Color32::from_rgb(240, 234, 255), // #F0EAFF
            shadow_sm: Color32::from_rgba_unmultiplied(8, 8, 8, 10),
            shadow_hover: Color32::from_rgba_unmultiplied(20, 110, 245, 26),
            focus_ring: Color32::from_rgba_unmultiplied(20, 110, 245, 64),
        }
    }

    pub fn dark() -> Self {
        Self {
            is_dark: true,
            bg: Color32::from_rgb(8, 8, 8), // #080808
            bg_gradient_top: Color32::from_rgb(8, 8, 8),
            bg_gradient_bottom: Color32::from_rgb(14, 17, 25),
            surface: Color32::from_rgb(16, 16, 18), // #101012
            surface_2: Color32::from_rgb(24, 24, 27), // #18181B
            surface_tint_top: Color32::from_rgb(18, 21, 31), // #12151F
            surface_tint_bottom: Color32::from_rgb(14, 17, 25), // #0E1119
            border: Color32::from_rgb(38, 38, 43), // #26262B
            border_strong: Color32::from_rgb(52, 52, 58), // #34343A
            text: Color32::from_rgb(245, 245, 245), // #F5F5F5
            text_muted: Color32::from_rgb(160, 160, 168), // #A0A0A8
            text_subtle: Color32::from_rgb(112, 112, 122), // #70707A
            primary: Color32::from_rgb(59, 130, 246), // #3B82F6
            primary_hover: Color32::from_rgb(96, 165, 250),
            primary_active: Color32::from_rgb(37, 99, 235),
            primary_soft: Color32::from_rgb(18, 32, 63), // #12203F
            primary_soft_border: Color32::from_rgb(30, 58, 114),
            ink: Color32::from_rgb(245, 245, 245), // #F5F5F5
            ink_fg: Color32::from_rgb(8, 8, 8),
            success: Color32::from_rgb(34, 197, 94),
            success_soft: Color32::from_rgb(14, 44, 28),
            danger: Color32::from_rgb(239, 68, 68),
            danger_soft: Color32::from_rgb(50, 18, 22),
            warning: Color32::from_rgb(245, 158, 11),
            warning_soft: Color32::from_rgb(48, 30, 12),
            purple: Color32::from_rgb(168, 85, 247),
            purple_soft: Color32::from_rgb(40, 20, 60),
            shadow_sm: Color32::from_rgba_unmultiplied(0, 0, 0, 40),
            shadow_hover: Color32::from_rgba_unmultiplied(59, 130, 246, 35),
            focus_ring: Color32::from_rgba_unmultiplied(59, 130, 246, 75),
        }
    }
}

// Light theme color constants (Single source of truth)
pub const CANVAS_BG: Color32 = Color32::from_rgb(255, 255, 255); // #FFFFFF
pub const CANVAS_GRADIENT_BOTTOM: Color32 = Color32::from_rgb(245, 247, 255); // #F5F7FF
pub const CARD_BG: Color32 = Color32::from_rgb(255, 255, 255); // #FFFFFF
pub const CARD_BORDER: Color32 = Color32::from_rgb(230, 230, 234); // #E6E6EA hairline
pub const SUBTLE_BG: Color32 = Color32::from_rgb(245, 245, 247); // #F5F5F7
pub const CARD_TINT_TOP: Color32 = Color32::from_rgb(247, 248, 255); // #F7F8FF
pub const CARD_TINT_BOTTOM: Color32 = Color32::from_rgb(240, 243, 253); // #F0F3FD
pub const PILL_CONTAINER_BG: Color32 = Color32::from_rgb(245, 245, 247); // #F5F5F7

pub const INPUT_BG: Color32 = Color32::from_rgb(255, 255, 255); // #FFFFFF
pub const INPUT_BG_HOVER: Color32 = Color32::from_rgb(245, 245, 247); // #F5F5F7
pub const INPUT_BORDER: Color32 = Color32::from_rgb(212, 212, 218); // #D4D4DA
pub const INPUT_BORDER_HOVER: Color32 = Color32::from_rgb(170, 170, 180);

pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(8, 8, 8); // #080808
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(90, 90, 90); // #5A5A5A
pub const TEXT_MUTED: Color32 = Color32::from_rgb(138, 138, 143); // #8A8A8F
pub const TEXT_PLACEHOLDER: Color32 = Color32::from_rgb(138, 138, 143);

pub const PRIMARY_BLUE: Color32 = Color32::from_rgb(20, 110, 245); // #146EF5
pub const PRIMARY_BLUE_HOVER: Color32 = Color32::from_rgb(15, 94, 215); // #0F5ED7
pub const PRIMARY_BLUE_ACTIVE: Color32 = Color32::from_rgb(11, 79, 184); // #0B4FB8
pub const BLUE_PILL_BG: Color32 = Color32::from_rgb(231, 240, 254); // #E7F0FE
pub const BLUE_PILL_TEXT: Color32 = Color32::from_rgb(20, 110, 245); // #146EF5
pub const BLUE_PILL_BORDER: Color32 = Color32::from_rgb(207, 224, 253); // #CFE0FD
pub const INK: Color32 = Color32::from_rgb(8, 8, 8);

pub const SUCCESS: Color32 = Color32::from_rgb(10, 143, 60); // #0A8F3C
pub const SUCCESS_PILL_BG: Color32 = Color32::from_rgb(230, 247, 236); // #E6F7EC
pub const SUCCESS_PILL_TEXT: Color32 = Color32::from_rgb(10, 143, 60);

pub const FAIL_PILL_BG: Color32 = Color32::from_rgb(254, 236, 238); // #FEECEE
pub const FAIL_PILL_TEXT: Color32 = Color32::from_rgb(238, 29, 54); // #EE1D36

pub const WARN_PILL_BG: Color32 = Color32::from_rgb(255, 243, 223); // #FFF3DF
pub const WARN_PILL_TEXT: Color32 = Color32::from_rgb(194, 106, 0); // #C26A00

pub const PURPLE_PILL_BG: Color32 = Color32::from_rgb(240, 234, 255); // #F0EAFF
pub const PURPLE_PILL_TEXT: Color32 = Color32::from_rgb(122, 61, 255); // #7A3DFF

pub const TEAL_PILL_BG: Color32 = Color32::from_rgb(204, 251, 241);
pub const TEAL_PILL_TEXT: Color32 = Color32::from_rgb(20, 184, 166); // #14B8A6

// Step-type 3px left accent bar colors
pub const STEP_LOCAL_ACCENT: Color32 = Color32::from_rgb(20, 110, 245); // #146EF5
pub const STEP_VERIFY_ACCENT: Color32 = Color32::from_rgb(245, 158, 11); // #F59E0B
pub const STEP_SCP_ACCENT: Color32 = Color32::from_rgb(122, 61, 255); // #7A3DFF
pub const STEP_SSH_ACCENT: Color32 = Color32::from_rgb(20, 184, 166); // #14B8A6

// Sidebar Dark Theme Colors (keeps dark background)
pub const SB_BG: Color32 = Color32::from_rgb(11, 18, 32);                          // #0B1220 deep dark navy
pub const SB_SURFACE: Color32 = Color32::from_rgb(17, 26, 46);                     // #111A2E surface
pub const SB_SURFACE_HOVER: Color32 = Color32::from_rgb(22, 33, 58);               // #16213A hover
pub const SB_BORDER: Color32 = Color32::from_rgba_premultiplied(15, 15, 15, 15);    // rgba(255,255,255,0.06)
pub const SB_BORDER_LINE: Color32 = Color32::from_rgb(28, 38, 60);                  // solid hairline border #1C263C
pub const SB_TEXT: Color32 = Color32::from_rgb(230, 234, 242);                     // #E6EAF2
pub const SB_TEXT_MUTED: Color32 = Color32::from_rgb(139, 150, 173);               // #8B96AD
pub const SB_TEXT_FAINT: Color32 = Color32::from_rgb(100, 112, 138);               // #64708A
pub const SB_ACCENT: Color32 = Color32::from_rgb(59, 130, 246);                    // #3B82F6
pub const SB_ACCENT_SOFT: Color32 = Color32::from_rgba_premultiplied(8, 18, 35, 36); // rgba(59,130,246,0.14)
pub const ACCENT: Color32 = Color32::from_rgb(20, 110, 245);                        // #146EF5 (primary brand blue)
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(231, 240, 254);                  // #E7F0FE (soft blue)

pub const BADGE_DEV_BG: Color32 = Color32::from_rgba_premultiplied(9, 21, 40, 41);  // rgba(59,130,246,0.16)
pub const BADGE_DEV_TEXT: Color32 = Color32::from_rgb(125, 178, 255);              // #7DB2FF
pub const BADGE_PROD_BG: Color32 = Color32::from_rgba_premultiplied(27, 14, 40, 41); // rgba(168,85,247,0.16)
pub const BADGE_PROD_TEXT: Color32 = Color32::from_rgb(196, 161, 255);             // #C4A1FF

pub const DESTRUCTIVE_TEXT: Color32 = Color32::from_rgb(238, 29, 54); // #EE1D36
pub const DESTRUCTIVE_HOVER_BG: Color32 = Color32::from_rgba_premultiplied(18, 2, 4, 20);
pub const DESTRUCTIVE_ACTIVE_BG: Color32 = Color32::from_rgba_premultiplied(35, 4, 8, 38);

// Modal Colors
pub const MD_OVERLAY: Color32 = Color32::from_rgba_premultiplied(3, 3, 3, 102); // rgba(8,8,8,0.40)
pub const MD_BG: Color32 = Color32::WHITE;
pub const MD_BORDER: Color32 = Color32::from_rgb(230, 230, 234);
pub const MD_TEXT: Color32 = Color32::from_rgb(8, 8, 8);
pub const MD_TEXT_MUTED: Color32 = Color32::from_rgb(90, 90, 90);
pub const MD_INPUT_BG: Color32 = Color32::WHITE;
pub const MD_PANEL_BG: Color32 = Color32::from_rgb(247, 248, 255);
pub const MD_FOCUS_RING_COLOR: Color32 = Color32::from_rgba_premultiplied(5, 27, 61, 64);

// Terminal & Log Console (dark high-contrast anchor)
pub const TERM_HEADER_BG: Color32 = Color32::from_rgb(18, 18, 24);
pub const TERM_BODY_BG: Color32 = Color32::from_rgb(11, 11, 15); // #0B0B0F
pub const TERM_GREEN: Color32 = Color32::from_rgb(10, 143, 60);
pub const TERM_RED: Color32 = Color32::from_rgb(238, 29, 54);
pub const TERM_YELLOW: Color32 = Color32::from_rgb(194, 106, 0);

// Preserved font size definitions
pub const FONT_XL: f32 = 20.0;
pub const FONT_LG: f32 = 15.0;
pub const FONT_MD: f32 = 13.0;
pub const FONT_SM: f32 = 11.5;
pub const FONT_XS: f32 = 10.5;
pub const POPUP_FONT_SIZE: f32 = 13.0;
pub const SIDEBAR_TEXT_MUTED: Color32 = Color32::from_rgb(138, 138, 143);

pub const ENV_CARD_HEIGHT: f32 = 96.0;

