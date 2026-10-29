use super::*;

/// Draws a crisp outline notification bell icon (`🔔`) matching the Termius topbar.
pub(crate) fn draw_icon_bell(painter: &egui::Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(1.4_f32, color);
    // Bell dome & body
    painter.line_segment(
        [Pos2::new(c.x - 3.5, c.y - 3.5), Pos2::new(c.x, c.y - 5.5)],
        s,
    );
    painter.line_segment(
        [Pos2::new(c.x, c.y - 5.5), Pos2::new(c.x + 3.5, c.y - 3.5)],
        s,
    );
    painter.line_segment(
        [Pos2::new(c.x - 3.5, c.y - 3.5), Pos2::new(c.x - 4.8, c.y + 2.5)],
        s,
    );
    painter.line_segment(
        [Pos2::new(c.x + 3.5, c.y - 3.5), Pos2::new(c.x + 4.8, c.y + 2.5)],
        s,
    );
    // Base rim
    painter.line_segment(
        [Pos2::new(c.x - 5.5, c.y + 2.5), Pos2::new(c.x + 5.5, c.y + 2.5)],
        s,
    );
    // Clapper at bottom
    painter.line_segment(
        [Pos2::new(c.x - 1.5, c.y + 4.5), Pos2::new(c.x + 1.5, c.y + 4.5)],
        s,
    );
}

/// Draws the Maximize icon (`□`) when normal, and the Termius-style Restore Down icon (`❐`) when maximized.
pub(crate) fn draw_window_max_restore_icon(
    painter: &egui::Painter,
    c: Pos2,
    is_maximized: bool,
    color: Color32,
) {
    let s = Stroke::new(1.3_f32, color);
    if is_maximized {
        // Restore Down: overlapping double rounded rectangles
        let front_rect =
            Rect::from_center_size(Pos2::new(c.x - 1.5, c.y + 1.5), Vec2::new(7.5, 7.5));
        painter.rect_stroke(front_rect, CornerRadius::same(1), s, StrokeKind::Inside);
        let back_tl = Pos2::new(c.x - 1.5, c.y - 4.5);
        let back_tr = Pos2::new(c.x + 4.5, c.y - 4.5);
        let back_br = Pos2::new(c.x + 4.5, c.y + 1.5);
        painter.line_segment([back_tl, back_tr], s);
        painter.line_segment([back_tr, back_br], s);
    } else {
        // Maximize: Single clean rounded square
        let r = Rect::from_center_size(c, Vec2::new(9.0, 9.0));
        painter.rect_stroke(r, CornerRadius::same(1), s, StrokeKind::Inside);
    }
}

// Vector Icon Painters
pub(crate) fn draw_icon_layers(painter: &egui::Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(1.75_f32, color);
    painter.line_segment([Pos2::new(c.x - 6.0, c.y - 3.0), Pos2::new(c.x, c.y - 6.0)], s);
    painter.line_segment([Pos2::new(c.x, c.y - 6.0), Pos2::new(c.x + 6.0, c.y - 3.0)], s);
    painter.line_segment([Pos2::new(c.x + 6.0, c.y - 3.0), Pos2::new(c.x, c.y)], s);
    painter.line_segment([Pos2::new(c.x, c.y), Pos2::new(c.x - 6.0, c.y - 3.0)], s);

    painter.line_segment([Pos2::new(c.x - 6.0, c.y + 1.0), Pos2::new(c.x, c.y + 4.0)], s);
    painter.line_segment([Pos2::new(c.x, c.y + 4.0), Pos2::new(c.x + 6.0, c.y + 1.0)], s);
}

pub(crate) fn draw_icon_git_branch(painter: &egui::Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(1.75_f32, color);
    painter.circle_stroke(Pos2::new(c.x - 3.0, c.y - 4.0), 2.0, s);
    painter.circle_stroke(Pos2::new(c.x - 3.0, c.y + 4.0), 2.0, s);
    painter.circle_stroke(Pos2::new(c.x + 3.5, c.y - 1.5), 2.0, s);
    painter.line_segment(
        [Pos2::new(c.x - 3.0, c.y - 2.0), Pos2::new(c.x - 3.0, c.y + 2.0)],
        s,
    );
    painter.line_segment(
        [Pos2::new(c.x - 3.0, c.y + 1.0), Pos2::new(c.x + 3.5, c.y + 0.5)],
        s,
    );
}

pub(crate) fn draw_icon_server(painter: &egui::Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(1.75_f32, color);
    let top_r = Rect::from_center_size(Pos2::new(c.x, c.y - 2.5), Vec2::new(11.0, 4.0));
    let bot_r = Rect::from_center_size(Pos2::new(c.x, c.y + 2.5), Vec2::new(11.0, 4.0));
    painter.rect_stroke(top_r, CornerRadius::same(1), s, StrokeKind::Inside);
    painter.rect_stroke(bot_r, CornerRadius::same(1), s, StrokeKind::Inside);
}

pub(crate) fn draw_icon_folder(painter: &egui::Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(1.75_f32, color);
    let r = Rect::from_center_size(Pos2::new(c.x, c.y + 1.0), Vec2::new(11.0, 7.5));
    painter.rect_stroke(r, CornerRadius::same(1), s, StrokeKind::Inside);
    painter.line_segment(
        [
            Pos2::new(r.left(), r.top() - 1.5),
            Pos2::new(r.left() + 4.5, r.top() - 1.5),
        ],
        s,
    );
}

pub(crate) fn draw_icon_refresh_rotated(painter: &egui::Painter, c: Pos2, color: Color32, angle: f32) {
    let s = Stroke::new(1.75_f32, color);
    let rot = |p: Pos2| -> Pos2 {
        if angle.abs() < 1e-4 {
            return p;
        }
        let dx = p.x - c.x;
        let dy = p.y - c.y;
        let cos = angle.cos();
        let sin = angle.sin();
        Pos2::new(c.x + dx * cos - dy * sin, c.y + dx * sin + dy * cos)
    };
    painter.circle_stroke(c, 5.0, s);
    let p1 = rot(Pos2::new(c.x + 2.5, c.y - 5.0));
    let p2 = rot(Pos2::new(c.x + 5.5, c.y - 4.5));
    let p3 = rot(Pos2::new(c.x + 4.5, c.y - 1.5));
    painter.line_segment([p1, p2], s);
    painter.line_segment([p2, p3], s);
}

pub(crate) fn draw_icon_refresh(painter: &egui::Painter, c: Pos2, color: Color32) {
    draw_icon_refresh_rotated(painter, c, color, 0.0);
}

pub(crate) fn draw_icon_heart(painter: &egui::Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(1.6_f32, color);
    let pts = [
        Pos2::new(c.x, c.y - 1.5),
        Pos2::new(c.x - 2.5, c.y - 4.5),
        Pos2::new(c.x - 5.0, c.y - 2.5),
        Pos2::new(c.x - 4.5, c.y + 0.5),
        Pos2::new(c.x, c.y + 5.5),
        Pos2::new(c.x + 4.5, c.y + 0.5),
        Pos2::new(c.x + 5.0, c.y - 2.5),
        Pos2::new(c.x + 2.5, c.y - 4.5),
        Pos2::new(c.x, c.y - 1.5),
    ];
    for w in pts.windows(2) {
        painter.line_segment([w[0], w[1]], s);
    }
}

pub(crate) fn draw_icon_plus(painter: &egui::Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(1.75_f32, color);
    painter.line_segment([Pos2::new(c.x - 6.0, c.y), Pos2::new(c.x + 6.0, c.y)], s);
    painter.line_segment([Pos2::new(c.x, c.y - 6.0), Pos2::new(c.x, c.y + 6.0)], s);
}

pub(crate) fn draw_icon_send(painter: &egui::Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(1.75_f32, color);
    let p_top = Pos2::new(c.x + 6.0, c.y - 5.0);
    let p_left = Pos2::new(c.x - 6.0, c.y - 1.0);
    let p_mid = Pos2::new(c.x - 1.0, c.y + 1.5);
    let p_bot = Pos2::new(c.x + 2.0, c.y + 6.0);
    painter.line_segment([p_top, p_left], s);
    painter.line_segment([p_left, p_mid], s);
    painter.line_segment([p_mid, p_bot], s);
    painter.line_segment([p_bot, p_top], s);
    painter.line_segment([p_top, p_mid], s);
}

pub(crate) fn draw_icon_chevron(painter: &egui::Painter, c: Pos2, expanded: bool, color: Color32) {
    let s = Stroke::new(1.75_f32, color);
    if expanded {
        painter.line_segment(
            [Pos2::new(c.x - 4.0, c.y - 2.0), Pos2::new(c.x, c.y + 2.5)],
            s,
        );
        painter.line_segment(
            [Pos2::new(c.x, c.y + 2.5), Pos2::new(c.x + 4.0, c.y - 2.0)],
            s,
        );
    } else {
        painter.line_segment(
            [Pos2::new(c.x - 2.0, c.y - 4.0), Pos2::new(c.x + 2.5, c.y)],
            s,
        );
        painter.line_segment(
            [Pos2::new(c.x + 2.5, c.y), Pos2::new(c.x - 2.0, c.y + 4.0)],
            s,
        );
    }
}

pub(crate) fn draw_icon_check(painter: &egui::Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(1.75_f32, color);
    painter.line_segment(
        [Pos2::new(c.x - 4.0, c.y + 0.5), Pos2::new(c.x - 1.0, c.y + 3.5)],
        s,
    );
    painter.line_segment(
        [Pos2::new(c.x - 1.0, c.y + 3.5), Pos2::new(c.x + 4.5, c.y - 3.0)],
        s,
    );
}

#[cfg(windows)]
/// Draws the official Shiply `logo.ico` texture inside a rounded container (falling back to 'S' if unavailable).
pub(crate) fn draw_shiply_logo(
    painter: &egui::Painter,
    rect: Rect,
    corner_radius: u8,
    font_size: f32,
    logo_texture: Option<&egui::TextureHandle>,
) {
    if let Some(tex) = logo_texture {
        painter.rect_filled(
            rect,
            CornerRadius::same(corner_radius),
            Color32::from_rgba_unmultiplied(30, 41, 59, 60),
        );
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        painter.image(tex.id(), rect, uv, Color32::WHITE);
    } else {
        painter.rect_filled(rect, CornerRadius::same(corner_radius), PRIMARY_BLUE);
        painter.rect_stroke(
            rect,
            CornerRadius::same(corner_radius),
            Stroke::new(1.0_f32, Color32::from_rgb(96, 165, 250)),
            StrokeKind::Inside,
        );
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "S",
            FontId::proportional(font_size),
            Color32::WHITE,
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutlineIcon {
    Trash,
    Refresh,
    ArrowUp,
    ArrowDown,
    Edit,
    RotateCcw,
    Key,
    GripVertical,
    Copy,
    Folder,
    ChevronRight,
    ChevronDown,
    ChevronUp,
    Check,
    X,
    MoreHorizontal,
    Search,
    Heart,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn outline_icon_button(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    icon: OutlineIcon,
    label: Option<&str>,
    fg: Color32,
    bg: Color32,
    hover_bg: Color32,
    border: Color32,
    enabled: bool,
) -> egui::Response {
    let height = 32.0;
    let (width, galley_opt) = if let Some(text) = label {
        let g = ui
            .painter()
            .layout_no_wrap(text.to_string(), FontId::proportional(FONT_SM), fg);
        let w = 10.0 + 14.0 + 6.0 + g.size().x + 12.0;
        (w, Some(g))
    } else {
        (32.0, None)
    };

    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let mut resp = ui.interact(
        rect,
        ui.make_persistent_id(id_salt),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    if enabled {
        resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    }

    let fill = if enabled && resp.hovered() {
        hover_bg
    } else {
        bg
    };
    let stroke_color = if enabled && resp.hovered() {
        fg
    } else {
        border
    };

    ui.painter()
        .rect_filled(rect, CornerRadius::same(6), fill);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(6),
        Stroke::new(1.0_f32, stroke_color),
        StrokeKind::Inside,
    );

    if let Some(galley) = galley_opt {
        let icon_center = Pos2::new(rect.left() + 16.0, rect.center().y);
        draw_outline_icon(ui.painter(), icon_center, icon, fg);
        ui.painter().galley(
            Pos2::new(
                rect.left() + 28.0,
                rect.center().y - galley.size().y / 2.0,
            ),
            galley,
            fg,
        );
    } else {
        draw_outline_icon(ui.painter(), rect.center(), icon, fg);
    }

    resp
}

pub(crate) fn draw_outline_icon(painter: &egui::Painter, c: Pos2, icon: OutlineIcon, color: Color32) {
    let s = Stroke::new(1.75_f32, color);
    match icon {
        OutlineIcon::Trash => {
            // Lid horizontal line
            painter.line_segment(
                [Pos2::new(c.x - 6.0, c.y - 3.5), Pos2::new(c.x + 6.0, c.y - 3.5)],
                s,
            );
            // Lid handle
            painter.line_segment(
                [Pos2::new(c.x - 2.0, c.y - 3.5), Pos2::new(c.x - 2.0, c.y - 5.5)],
                s,
            );
            painter.line_segment(
                [Pos2::new(c.x - 2.0, c.y - 5.5), Pos2::new(c.x + 2.0, c.y - 5.5)],
                s,
            );
            painter.line_segment(
                [Pos2::new(c.x + 2.0, c.y - 5.5), Pos2::new(c.x + 2.0, c.y - 3.5)],
                s,
            );
            // Trash bin body
            painter.line_segment(
                [Pos2::new(c.x - 4.5, c.y - 3.5), Pos2::new(c.x - 3.8, c.y + 6.0)],
                s,
            );
            painter.line_segment(
                [Pos2::new(c.x - 3.8, c.y + 6.0), Pos2::new(c.x + 3.8, c.y + 6.0)],
                s,
            );
            painter.line_segment(
                [Pos2::new(c.x + 3.8, c.y + 6.0), Pos2::new(c.x + 4.5, c.y - 3.5)],
                s,
            );
            // Inner vertical lines
            painter.line_segment(
                [Pos2::new(c.x - 1.5, c.y - 1.0), Pos2::new(c.x - 1.5, c.y + 3.5)],
                s,
            );
            painter.line_segment(
                [Pos2::new(c.x + 1.5, c.y - 1.0), Pos2::new(c.x + 1.5, c.y + 3.5)],
                s,
            );
        }
        OutlineIcon::Refresh => {
            let angle = painter
                .ctx()
                .data(|d| d.get_temp::<f32>(egui::Id::new("refresh_anim_angle")))
                .unwrap_or(0.0);
            draw_icon_refresh_rotated(painter, c, color, angle);
        }
        OutlineIcon::Heart => {
            draw_icon_heart(painter, c, color);
        }
        OutlineIcon::ArrowUp => {
            painter.line_segment(
                [Pos2::new(c.x, c.y + 5.5), Pos2::new(c.x, c.y - 5.0)],
                s,
            );
            painter.line_segment(
                [Pos2::new(c.x - 4.0, c.y - 1.0), Pos2::new(c.x, c.y - 5.0)],
                s,
            );
            painter.line_segment(
                [Pos2::new(c.x + 4.0, c.y - 1.0), Pos2::new(c.x, c.y - 5.0)],
                s,
            );
        }
        OutlineIcon::ArrowDown => {
            painter.line_segment(
                [Pos2::new(c.x, c.y - 5.5), Pos2::new(c.x, c.y + 5.0)],
                s,
            );
            painter.line_segment(
                [Pos2::new(c.x - 4.0, c.y + 1.0), Pos2::new(c.x, c.y + 5.0)],
                s,
            );
            painter.line_segment(
                [Pos2::new(c.x + 4.0, c.y + 1.0), Pos2::new(c.x, c.y + 5.0)],
                s,
            );
        }
        OutlineIcon::Edit => {
            // Crisp tilted pencil outline
            let tip = Pos2::new(c.x - 5.0, c.y + 5.0);
            let p1 = Pos2::new(c.x - 4.5, c.y + 1.8);
            let p2 = Pos2::new(c.x + 2.5, c.y - 5.0);
            let p3 = Pos2::new(c.x + 5.0, c.y - 2.5);
            let p4 = Pos2::new(c.x - 1.8, c.y + 4.5);
            painter.line_segment([tip, p1], s);
            painter.line_segment([p1, p2], s);
            painter.line_segment([p2, p3], s);
            painter.line_segment([p3, p4], s);
            painter.line_segment([p4, tip], s);
        }
        OutlineIcon::RotateCcw => {
            painter.circle_stroke(c, 4.8, s);
            painter.line_segment(
                [Pos2::new(c.x - 2.5, c.y - 5.0), Pos2::new(c.x - 5.5, c.y - 4.5)],
                s,
            );
            painter.line_segment(
                [Pos2::new(c.x - 5.5, c.y - 4.5), Pos2::new(c.x - 4.5, c.y - 1.5)],
                s,
            );
        }
        OutlineIcon::Key => {
            painter.circle_stroke(Pos2::new(c.x - 2.5, c.y), 2.8, s);
            painter.line_segment(
                [Pos2::new(c.x + 0.3, c.y), Pos2::new(c.x + 5.8, c.y)],
                s,
            );
            painter.line_segment(
                [Pos2::new(c.x + 3.4, c.y), Pos2::new(c.x + 3.4, c.y + 2.6)],
                s,
            );
            painter.line_segment(
                [Pos2::new(c.x + 5.4, c.y), Pos2::new(c.x + 5.4, c.y + 2.6)],
                s,
            );
        }
        OutlineIcon::GripVertical => {
            let r = 1.4;
            for dx in [-2.2, 2.2] {
                for dy in [-4.5, 0.0, 4.5] {
                    painter.circle_filled(Pos2::new(c.x + dx, c.y + dy), r, color);
                }
            }
        }
        OutlineIcon::Copy => {
            // Front rectangle
            painter.rect_stroke(
                Rect::from_min_size(Pos2::new(c.x - 3.5, c.y - 2.0), Vec2::new(7.5, 8.5)),
                CornerRadius::same(1),
                s,
                StrokeKind::Inside,
            );
            // Back rectangle top and right lines
            painter.line_segment(
                [Pos2::new(c.x - 1.5, c.y - 5.0), Pos2::new(c.x + 5.5, c.y - 5.0)],
                s,
            );
            painter.line_segment(
                [Pos2::new(c.x + 5.5, c.y - 5.0), Pos2::new(c.x + 5.5, c.y + 3.0)],
                s,
            );
        }
        OutlineIcon::Folder => {
            let r = Rect::from_center_size(Pos2::new(c.x, c.y + 1.0), Vec2::new(12.0, 8.0));
            painter.rect_stroke(r, CornerRadius::same(1), s, StrokeKind::Inside);
            painter.line_segment(
                [Pos2::new(r.left(), r.top() - 2.0), Pos2::new(r.left() + 4.5, r.top() - 2.0)],
                s,
            );
            painter.line_segment(
                [Pos2::new(r.left() + 4.5, r.top() - 2.0), Pos2::new(r.left() + 6.0, r.top())],
                s,
            );
        }
        OutlineIcon::ChevronRight => {
            painter.line_segment([Pos2::new(c.x - 2.0, c.y - 4.0), Pos2::new(c.x + 2.5, c.y)], s);
            painter.line_segment([Pos2::new(c.x + 2.5, c.y), Pos2::new(c.x - 2.0, c.y + 4.0)], s);
        }
        OutlineIcon::ChevronDown => {
            painter.line_segment([Pos2::new(c.x - 4.0, c.y - 2.0), Pos2::new(c.x, c.y + 2.5)], s);
            painter.line_segment([Pos2::new(c.x, c.y + 2.5), Pos2::new(c.x + 4.0, c.y - 2.0)], s);
        }
        OutlineIcon::ChevronUp => {
            painter.line_segment([Pos2::new(c.x - 4.0, c.y + 2.0), Pos2::new(c.x, c.y - 2.5)], s);
            painter.line_segment([Pos2::new(c.x, c.y - 2.5), Pos2::new(c.x + 4.0, c.y + 2.0)], s);
        }
        OutlineIcon::Check => {
            painter.line_segment(
                [Pos2::new(c.x - 4.5, c.y + 0.5), Pos2::new(c.x - 1.0, c.y + 4.0)],
                s,
            );
            painter.line_segment(
                [Pos2::new(c.x - 1.0, c.y + 4.0), Pos2::new(c.x + 5.0, c.y - 3.5)],
                s,
            );
        }
        OutlineIcon::X => {
            painter.line_segment([Pos2::new(c.x - 4.0, c.y - 4.0), Pos2::new(c.x + 4.0, c.y + 4.0)], s);
            painter.line_segment([Pos2::new(c.x + 4.0, c.y - 4.0), Pos2::new(c.x - 4.0, c.y + 4.0)], s);
        }
        OutlineIcon::MoreHorizontal => {
            let r = 1.6_f32;
            for dx in [-4.5_f32, 0.0, 4.5_f32] {
                painter.circle_filled(Pos2::new(c.x + dx, c.y), r, color);
            }
        }
        OutlineIcon::Search => {
            let center = Pos2::new(c.x - 1.5, c.y - 1.5);
            painter.circle_stroke(center, 4.0, s);
            painter.line_segment(
                [
                    Pos2::new(center.x + 2.8, center.y + 2.8),
                    Pos2::new(c.x + 5.5, c.y + 5.5),
                ],
                s,
            );
        }
    }
}

