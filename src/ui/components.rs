use super::*;

// =============================================================================
// Bounded Layout Helpers & Fixed-Height Pill/Icon Painters (Never Stretches!)
// =============================================================================

pub(crate) fn horizontal_row<R>(
    ui: &mut egui::Ui,
    height: f32,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let width = ui.available_width();
    ui.allocate_ui_with_layout(
        Vec2::new(width, height),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_max_height(height);
            add_contents(ui)
        },
    )
}

/// Consistent text input field with white fill (`INPUT_BG`), 1px hairline/strong border (`INPUT_BORDER`),
/// focus ring (`PRIMARY_BLUE`), structured proportional font (`FONT_MD`), and grey placeholder (`TEXT_PLACEHOLDER`).
pub(crate) fn styled_text_edit(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    value: &mut String,
    width: f32,
    hint: Option<&str>,
) -> bool {
    styled_text_edit_ext(ui, id_salt, value, width, hint, false)
}

pub(crate) fn styled_password_edit(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    value: &mut String,
    width: f32,
    hint: Option<&str>,
) -> bool {
    styled_text_edit_ext(ui, id_salt, value, width, hint, true)
}

pub(crate) fn styled_text_edit_ext(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    value: &mut String,
    width: f32,
    hint: Option<&str>,
    is_password: bool,
) -> bool {
    let placeholder_str = hint.unwrap_or("Enter value...");
    let height = INPUT_HEIGHT; // 36.0
    let (outer_rect, outer_resp) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());

    // Single clean white background fill
    ui.painter().rect_filled(outer_rect, CornerRadius::same(6), INPUT_BG);

    // Inner TextEdit child matching singleline text row height (22px) centered vertically
    let text_h = 22.0_f32;
    let text_y = outer_rect.center().y - text_h * 0.5;
    let text_rect = Rect::from_min_size(
        Pos2::new(outer_rect.left() + 10.0, text_y),
        Vec2::new((width - 20.0).max(10.0), text_h),
    );

    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(text_rect));
    let edit = egui::TextEdit::singleline(value)
        .id_salt(id_salt)
        .font(FontId::proportional(FONT_MD))
        .text_color(Color32::from_rgb(8, 8, 8))
        .password(is_password)
        .frame(false)
        .margin(Margin::ZERO)
        .cursor_at_end(true)
        .desired_width(text_rect.width())
        .min_size(Vec2::new(text_rect.width(), text_h))
        .hint_text(
            RichText::new(placeholder_str)
                .size(FONT_MD)
                .color(TEXT_PLACEHOLDER),
        );
    let mut output = edit.show(&mut child_ui);

    if outer_resp.clicked() && !output.response.has_focus() {
        ui.memory_mut(|mem| mem.request_focus(output.response.id));
    }

    let is_focused = output.response.has_focus() || ui.memory(|mem| mem.has_focus(output.response.id));
    let is_hovered = outer_resp.hovered() || output.response.hovered();

    let stroke = if is_focused {
        Stroke::new(1.5_f32, PRIMARY_BLUE)
    } else if is_hovered {
        Stroke::new(1.0_f32, Color32::from_rgb(160, 160, 165))
    } else {
        Stroke::new(1.0_f32, INPUT_BORDER)
    };
    ui.painter().rect_stroke(
        outer_rect,
        CornerRadius::same(6),
        stroke,
        StrokeKind::Inside,
    );

    handle_text_edit_cursor_and_selection(ui.ctx(), &mut output, value);

    output.response.changed()
}

pub(crate) fn handle_text_edit_cursor_and_selection(
    ctx: &egui::Context,
    output: &mut egui::text_edit::TextEditOutput,
    value: &str,
) {
    // Preserve multi-character selections (e.g. from double-click word selection or drag selection)
    if let Some(range) = output.state.cursor.char_range() {
        if range.primary != range.secondary {
            if output.response.dragged() || ctx.is_being_dragged(output.response.id) {
                ctx.request_repaint();
            }
            return;
        }
    }

    // Precise cursor placement on press/click without dragging:
    let pointer_pressed = output.response.hovered() && ctx.input(|i| i.pointer.primary_pressed());
    let is_click = output.response.clicked();
    let gained_focus = output.response.gained_focus();

    if (is_click || pointer_pressed || gained_focus)
        && !output.response.double_clicked()
        && !output.response.triple_clicked()
        && !ctx.input(|i| i.pointer.is_decidedly_dragging())
    {
        if let Some(pos) = output.response.interact_pointer_pos().or_else(|| ctx.pointer_interact_pos()) {
            let rel_x = pos.x - output.galley_pos.x;
            let char_count = value.chars().count();
            let ccursor = if rel_x >= output.galley.size().x {
                egui::text::CCursor::new(char_count)
            } else if rel_x <= 0.0 {
                egui::text::CCursor::new(0)
            } else {
                let sample_pos = Vec2::new(rel_x, (output.galley.size().y * 0.5).max(1.0));
                output.galley.cursor_from_pos(sample_pos).ccursor
            };
            output
                .state
                .cursor
                .set_char_range(Some(egui::text_selection::CCursorRange::one(ccursor)));
            output.state.clone().store(ctx, output.response.id);
        }
    }

    // Smooth drag selection without lag: request continuous repaint while blocking/dragging text
    if output.response.dragged() || ctx.is_being_dragged(output.response.id) {
        ctx.request_repaint();
    }
}

pub(crate) fn sidebar_search_text_edit(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    value: &mut String,
    width: f32,
    hint: Option<&str>,
) -> bool {
    let placeholder_str = hint.unwrap_or("Search projects...");
    let height = 32.0_f32;
    let (rect, outer_resp) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());

    // Single background fill
    ui.painter().rect_filled(rect, CornerRadius::same(R_SM as u8), SB_SURFACE);

    // Left magnifying glass search icon (standard enterprise UX)
    let icon_center = Pos2::new(rect.left() + 14.0, rect.center().y);
    draw_outline_icon(ui.painter(), icon_center, OutlineIcon::Search, SB_TEXT_FAINT);

    // Optional clear button on right when search is non-empty
    let has_clear = !value.is_empty();
    let clear_w = if has_clear { 24.0_f32 } else { 0.0_f32 };
    if has_clear {
        let clear_rect = Rect::from_center_size(
            Pos2::new(rect.right() - 12.0, rect.center().y),
            Vec2::new(16.0, 16.0),
        );
        let clear_resp = ui
            .interact(clear_rect, egui::Id::new("sidebar_search_clear_btn"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let clear_fg = if clear_resp.hovered() { SB_TEXT } else { SB_TEXT_FAINT };
        draw_outline_icon(ui.painter(), clear_rect.center(), OutlineIcon::X, clear_fg);
        if clear_resp.clicked() {
            value.clear();
        }
    }

    // Inner text edit child
    let text_rect = Rect::from_min_max(
        Pos2::new(rect.left() + 28.0, rect.top()),
        Pos2::new(rect.right() - 8.0 - clear_w, rect.bottom()),
    );
    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(text_rect));
    let edit = egui::TextEdit::singleline(value)
        .id_salt(id_salt)
        .font(FontId::proportional(FS_BASE))
        .text_color(SB_TEXT)
        .frame(false)
        .desired_width(text_rect.width())
        .min_size(Vec2::new(text_rect.width(), height))
        .margin(Margin::symmetric(0, 6))
        .hint_text(
            RichText::new(placeholder_str)
                .size(FS_BASE)
                .color(SB_TEXT_FAINT),
        );
    let mut output = edit.show(&mut child_ui);

    let is_focused = output.response.has_focus();
    let is_hovered = outer_resp.hovered() || output.response.hovered();

    // Exactly one single crisp outline (no double outline)
    let stroke = if is_focused {
        Stroke::new(1.5_f32, SB_ACCENT)
    } else if is_hovered {
        Stroke::new(1.0_f32, SB_ACCENT)
    } else {
        Stroke::new(1.0_f32, SB_BORDER_LINE)
    };
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(R_SM as u8),
        stroke,
        StrokeKind::Inside,
    );

    handle_text_edit_cursor_and_selection(ui.ctx(), &mut output, value);

    output.response.changed()
}

/// Syntax-highlighted single-line text editor for pipeline commands, paths, and SCP sources.
/// Colors `{template}` variables in purple, CLI commands in blue, flags (`--flag`) in amber,
/// paths/strings in teal, and shell operators (`&&`, `|`, `;`) in crimson.
pub(crate) fn syntax_highlighted_text_edit(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    value: &mut String,
    width: f32,
    hint: Option<&str>,
) -> bool {
    let placeholder_str = hint.unwrap_or("Enter command or path...");
    let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
        let mut job = build_pipeline_syntax_layout_job(string);
        job.wrap.max_width = wrap_width;
        ui.fonts(|f| f.layout_job(job))
    };
    let edit = egui::TextEdit::singleline(value)
        .id_salt(id_salt)
        .font(FontId::proportional(FONT_MD))
        .background_color(INPUT_BG)
        .desired_width(width)
        .min_size(Vec2::new(width, INPUT_HEIGHT))
        .margin(Margin::symmetric(10, 8))
        .layouter(&mut layouter)
        .hint_text(
            RichText::new(placeholder_str)
                .size(FONT_MD)
                .color(TEXT_PLACEHOLDER),
        );
    let mut output = edit.show(ui);

    handle_text_edit_cursor_and_selection(ui.ctx(), &mut output, value);

    output.response.changed()
}

pub fn build_pipeline_syntax_layout_job(text: &str) -> egui::text::LayoutJob {
    use egui::text::{LayoutJob, TextFormat};
    let mut job = LayoutJob::default();
    let font_id = FontId::proportional(FONT_MD);

    let color_template = PURPLE_PILL_TEXT; // {app_name}, {remote_dir}, etc.
    let color_keyword = BLUE_PILL_TEXT; // npm, pm2, cargo, git, ssh, etc.
    let color_flag = WARN_PILL_TEXT; // --update-env, -rf, etc.
    let color_path = TEAL_PILL_TEXT; // ./dist, /var/www/..., "..."
    let color_operator = FAIL_PILL_TEXT; // &&, ||, |, ;

    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        let b = bytes[i];

        // 1. Whitespace
        if b.is_ascii_whitespace() {
            let start = i;
            while i < len && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            job.append(&text[start..i], 0.0, TextFormat::simple(font_id.clone(), TEXT_PRIMARY));
            continue;
        }

        // 2. Template Variable `{...}`
        if b == b'{' {
            let start = i;
            i += 1;
            while i < len && bytes[i] != b'}' && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < len && bytes[i] == b'}' {
                i += 1;
            }
            job.append(
                &text[start..i],
                0.0,
                TextFormat::simple(font_id.clone(), color_template),
            );
            continue;
        }

        // 3. Quoted String `"..."` or `'...'`
        if b == b'"' || b == b'\'' {
            let quote = b;
            let start = i;
            i += 1;
            while i < len && bytes[i] != quote {
                if bytes[i] == b'{' {
                    break;
                }
                i += 1;
            }
            if i < len && bytes[i] == quote {
                i += 1;
            }
            job.append(&text[start..i], 0.0, TextFormat::simple(font_id.clone(), color_path));
            continue;
        }

        // 4. Operators & Separators (`&&`, `||`, `|`, `;`, `,`)
        if b == b'&' || b == b'|' || b == b';' || b == b',' {
            let start = i;
            i += 1;
            while i < len && (bytes[i] == b'&' || bytes[i] == b'|') {
                i += 1;
            }
            job.append(
                &text[start..i],
                0.0,
                TextFormat::simple(font_id.clone(), color_operator),
            );
            continue;
        }

        // 5. General Token (stop at whitespace, `{`, quote, or operator)
        let start = i;
        while i < len
            && !bytes[i].is_ascii_whitespace()
            && bytes[i] != b'{'
            && bytes[i] != b'"'
            && bytes[i] != b'\''
            && bytes[i] != b'&'
            && bytes[i] != b'|'
            && bytes[i] != b';'
            && bytes[i] != b','
        {
            i += 1;
        }
        let token = &text[start..i];
        let token_color = if token.starts_with('-') {
            color_flag
        } else if token.starts_with("./")
            || token.starts_with("../")
            || token.starts_with('/')
            || token.contains('/')
            || token.contains('\\')
            || token.ends_with(".json")
            || token.ends_with(".js")
            || token.ends_with(".ts")
            || token.ends_with(".ps1")
            || token.ends_with(".env")
        {
            color_path
        } else if token.eq_ignore_ascii_case("npm")
            || token.eq_ignore_ascii_case("npx")
            || token.eq_ignore_ascii_case("yarn")
            || token.eq_ignore_ascii_case("pnpm")
            || token.eq_ignore_ascii_case("node")
            || token.eq_ignore_ascii_case("pm2")
            || token.eq_ignore_ascii_case("cargo")
            || token.eq_ignore_ascii_case("git")
            || token.eq_ignore_ascii_case("docker")
            || token.eq_ignore_ascii_case("systemctl")
            || token.eq_ignore_ascii_case("service")
            || token.eq_ignore_ascii_case("ssh")
            || token.eq_ignore_ascii_case("scp")
            || token.eq_ignore_ascii_case("mkdir")
            || token.eq_ignore_ascii_case("rm")
            || token.eq_ignore_ascii_case("cp")
            || token.eq_ignore_ascii_case("mv")
            || token.eq_ignore_ascii_case("chmod")
            || token.eq_ignore_ascii_case("chown")
            || token.eq_ignore_ascii_case("tar")
            || token.eq_ignore_ascii_case("curl")
            || token.eq_ignore_ascii_case("wget")
            || token.eq_ignore_ascii_case("export")
            || token.eq_ignore_ascii_case("cd")
            || token.eq_ignore_ascii_case("echo")
            || token.eq_ignore_ascii_case("python")
            || token.eq_ignore_ascii_case("pytest")
            || token.eq_ignore_ascii_case("go")
            || token.eq_ignore_ascii_case("cmdkey")
            || token.eq_ignore_ascii_case("security")
            || token.eq_ignore_ascii_case("secret-tool")
            || token.eq_ignore_ascii_case("sudo")
            || token.eq_ignore_ascii_case("run")
            || token.eq_ignore_ascii_case("build")
            || token.eq_ignore_ascii_case("test")
            || token.eq_ignore_ascii_case("install")
            || token.eq_ignore_ascii_case("ci")
            || token.eq_ignore_ascii_case("restart")
            || token.eq_ignore_ascii_case("reload")
            || token.eq_ignore_ascii_case("start")
            || token.eq_ignore_ascii_case("stop")
            || token.eq_ignore_ascii_case("delete")
            || token.eq_ignore_ascii_case("status")
            || token.eq_ignore_ascii_case("store")
        {
            color_keyword
        } else {
            TEXT_PRIMARY
        };

        job.append(token, 0.0, TextFormat::simple(font_id.clone(), token_color));
    }

    job
}

/// Renders a modern modal header according to design tokens:
/// Title (--fs-title 18px, weight 600, --md-text) + description (--fs-small 13px, --md-text-muted, margin-top 4px)
/// + anchored 32x32 close button top-right (radius --r-md 8px, hover bg --md-panel-bg).
pub(crate) fn render_modal_header(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    title: &str,
    subtitle: &str,
) -> bool {
    let full_w = ui.available_width();
    let top_pos = ui.cursor().min;

    // Position the 32x32 close button within the top-right corner of the modal
    let close_rect = Rect::from_min_size(
        Pos2::new(top_pos.x + full_w - 32.0, top_pos.y),
        Vec2::new(32.0, 32.0),
    );
    let close_resp = ui
        .interact(
            close_rect,
            ui.make_persistent_id(id_salt),
            Sense::click(),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand);

    let (title_rect, _) =
        ui.allocate_exact_size(Vec2::new(full_w - 40.0, 26.0), Sense::hover());
    ui.painter().text(
        Pos2::new(title_rect.left(), title_rect.center().y),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(FS_TITLE),
        MD_TEXT,
    );

    let close_bg = if close_resp.hovered() {
        MD_PANEL_BG
    } else {
        Color32::TRANSPARENT
    };
    let close_fg = if close_resp.hovered() {
        MD_TEXT
    } else {
        MD_TEXT_MUTED
    };

    ui.painter()
        .rect_filled(close_rect, CornerRadius::same(R_MD as u8), close_bg);
    let c = close_rect.center();
    let s = Stroke::new(1.6_f32, close_fg);
    ui.painter().line_segment(
        [Pos2::new(c.x - 4.5, c.y - 4.5), Pos2::new(c.x + 4.5, c.y + 4.5)],
        s,
    );
    ui.painter().line_segment(
        [Pos2::new(c.x + 4.5, c.y - 4.5), Pos2::new(c.x - 4.5, c.y + 4.5)],
        s,
    );

    ui.add_space(SP_4);
    ui.label(RichText::new(subtitle).size(FS_SMALL).color(MD_TEXT_MUTED));

    close_resp.on_hover_text("Close dialog (Esc)").clicked()
}

/// Calculates responsive modal width: min(560px, 100vw - 32px); below 480px it becomes a full-screen sheet.
pub(crate) fn modal_width_and_inner(ctx: &egui::Context) -> (f32, f32) {
    let screen_w = ctx.screen_rect().width();
    if screen_w < 480.0 {
        let modal_w = screen_w;
        let inner_w = (screen_w - 24.0).max(200.0);
        (modal_w, inner_w)
    } else {
        let modal_w = 560.0_f32.min(screen_w - 32.0).max(300.0);
        let inner_w = modal_w - 48.0;
        (modal_w, inner_w)
    }
}

/// Standardized dialog window frame adhering to design tokens:
/// width 560px, bg --md-bg, border 1px --md-border, radius --r-xl (14px), padding 24px, shadow 0 24px 60px rgba(2,6,23,0.35)
pub(crate) fn modal_dialog_frame_for(ctx: &egui::Context) -> egui::Frame {
    let screen_w = ctx.screen_rect().width();
    let is_sheet = screen_w < 480.0;
    egui::Frame::new()
        .fill(MD_BG)
        .stroke(Stroke::new(1.0_f32, MD_BORDER))
        .corner_radius(if is_sheet { CornerRadius::ZERO } else { CornerRadius::same(14) })
        .shadow(if is_sheet {
            Shadow::NONE
        } else {
            Shadow {
                offset: [0, 24],
                blur: 60,
                spread: 0,
                color: Color32::from_rgba_unmultiplied(2, 6, 23, 89),
            }
        })
        .inner_margin(if is_sheet { Margin::same(12) } else { Margin::same(24) })
}

/// Unified modal text input (height 36px, font FS_BASE 13px, bg MD_INPUT_BG, border 1px MD_BORDER, radius R_MD 6px).
pub(crate) fn modal_text_input(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    value: &mut String,
    width: f32,
    placeholder: Option<&str>,
) -> egui::Response {
    let hint_str = placeholder.unwrap_or("Enter value...");
    let height = INPUT_HEIGHT; // 36.0
    let (outer_rect, outer_resp) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());

    // Single clean modal input background fill
    ui.painter().rect_filled(outer_rect, CornerRadius::same(R_MD as u8), MD_INPUT_BG);

    // Inner TextEdit child matching singleline text row height (22px) centered vertically
    let text_h = 22.0_f32;
    let text_y = outer_rect.center().y - text_h * 0.5;
    let text_rect = Rect::from_min_size(
        Pos2::new(outer_rect.left() + 12.0, text_y),
        Vec2::new((width - 24.0).max(10.0), text_h),
    );

    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(text_rect));
    let edit = egui::TextEdit::singleline(value)
        .id_salt(id_salt)
        .font(FontId::proportional(FS_BASE))
        .text_color(MD_TEXT)
        .frame(false)
        .margin(Margin::ZERO)
        .cursor_at_end(true)
        .desired_width(text_rect.width())
        .min_size(Vec2::new(text_rect.width(), text_h))
        .hint_text(
            RichText::new(hint_str)
                .size(FS_BASE)
                .color(MD_TEXT_MUTED.gamma_multiply(0.7)),
        );
    let mut output = edit.show(&mut child_ui);

    if outer_resp.clicked() && !output.response.has_focus() {
        ui.memory_mut(|mem| mem.request_focus(output.response.id));
    }

    let is_focused = output.response.has_focus() || ui.memory(|mem| mem.has_focus(output.response.id));
    let is_hovered = outer_resp.hovered() || output.response.hovered();

    let stroke = if is_focused {
        Stroke::new(1.5_f32, PRIMARY_BLUE)
    } else if is_hovered {
        Stroke::new(1.0_f32, ACCENT)
    } else {
        Stroke::new(1.0_f32, MD_BORDER)
    };
    ui.painter().rect_stroke(
        outer_rect,
        CornerRadius::same(R_MD as u8),
        stroke,
        StrokeKind::Inside,
    );

    handle_text_edit_cursor_and_selection(ui.ctx(), &mut output, value);

    output.response
}

/// Unified modal primary action button (height 40px, padding 0 16px, font --fs-base 14px, radius --r-md 8px, bg --accent).
pub(crate) fn modal_primary_button(text: &str, enabled: bool) -> egui::Button<'static> {
    egui::Button::new(
        RichText::new(text.to_string())
            .size(FS_BASE)
            .strong()
            .color(if enabled {
                Color32::WHITE
            } else {
                Color32::from_rgb(148, 155, 168)
            }),
    )
    .fill(if enabled {
        ACCENT
    } else {
        Color32::from_rgb(222, 226, 235)
    })
    .stroke(if enabled {
        Stroke::NONE
    } else {
        Stroke::new(1.0_f32, Color32::from_rgb(205, 212, 224))
    })
    .corner_radius(R_MD)
    .min_size(Vec2::new(0.0, 40.0))
}

/// Unified modal secondary button (height 40px, padding 0 16px, font --fs-base 14px, radius --r-md 8px, white bg, border 1px --md-border, hover --md-panel-bg).
pub(crate) fn modal_secondary_button(text: &str) -> egui::Button<'static> {
    egui::Button::new(
        RichText::new(text.to_string())
            .size(FS_BASE)
            .strong()
            .color(MD_TEXT),
    )
    .fill(Color32::WHITE)
    .stroke(Stroke::new(1.0_f32, MD_BORDER))
    .corner_radius(R_MD)
    .min_size(Vec2::new(0.0, 40.0))
}

/// Renders a tab button with uniform horizontal padding (`16px` left & right) and consistent
/// bordered styling so adjacent tabs have an exact, uniform visual gap.
pub(crate) fn render_uniform_tab_button(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    label: &str,
    is_active: bool,
    height: f32,
) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        FontId::proportional(FONT_MD),
        if is_active {
            Color32::WHITE
        } else {
            TEXT_SECONDARY
        },
    );
    let width = galley.size().x + 32.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let resp = ui
        .interact(rect, ui.make_persistent_id(id_salt), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);

    let (fill, stroke, text_color) = if is_active {
        (
            PRIMARY_BLUE,
            Stroke::new(1.5_f32, PRIMARY_BLUE),
            Color32::WHITE,
        )
    } else if resp.hovered() {
        (
            BLUE_PILL_BG,
            Stroke::new(1.2_f32, PRIMARY_BLUE),
            PRIMARY_BLUE,
        )
    } else {
        (
            CARD_BG,
            Stroke::new(1.0_f32, CARD_BORDER),
            TEXT_SECONDARY,
        )
    };

    ui.painter()
        .rect_filled(rect, CornerRadius::same(6), fill);
    ui.painter()
        .rect_stroke(rect, CornerRadius::same(6), stroke, StrokeKind::Inside);

    let colored_galley = ui.painter().layout_no_wrap(
        label.to_string(),
        FontId::proportional(FONT_MD),
        text_color,
    );
    ui.painter().galley(
        Pos2::new(
            rect.center().x - colored_galley.size().x / 2.0,
            rect.center().y - colored_galley.size().y / 2.0,
        ),
        colored_galley,
        text_color,
    );

    resp
}

