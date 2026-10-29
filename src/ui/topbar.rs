use super::*;

impl ShiplyApp {
    /// Renders the custom frameless Webflow-style topbar with left navigation pills,
    /// 3px brand primary accent strip, theme toggle, draggable window region, notification bell, and window controls.
    pub(crate) fn render_termius_topbar(&mut self, ctx: &egui::Context) {
        let theme = self.theme();
        let topbar_bg = Color32::from_rgb(11, 18, 32); // Deep navy-black topbar
        let pill_bg = Color32::from_rgb(26, 36, 56);   // Dark hover fill for buttons
        let topbar_text = Color32::from_rgb(241, 245, 249); // Clean white text
        let icon_muted = Color32::from_rgb(180, 190, 205);  // Light muted icon

        if let Some(maximized) = ctx.input(|i| i.viewport().maximized) {
            self.window_maximized = maximized;
        }

        egui::TopBottomPanel::top("shiply_termius_topbar")
            .exact_height(42.0)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(topbar_bg)
                    .inner_margin(Margin::symmetric(10, 0))
                    .stroke(Stroke::NONE),
            )
            .show(ctx, |ui| {
                let bar_rect = ui.max_rect();
                let cy = bar_rect.center().y;
                let pill_h = 28.0;
                let mut x = bar_rect.left() + 2.0;

                // 1. Left Hamburger Menu Button (≡) — toggles sidebar visibility or off-canvas drawer
                let menu_rect =
                    Rect::from_center_size(Pos2::new(x + 14.0, cy), Vec2::new(28.0, pill_h));
                let menu_resp = ui.interact(
                    menu_rect,
                    egui::Id::new("topbar_hamburger_btn"),
                    Sense::click(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand);
                if menu_resp.hovered() {
                    ui.painter()
                        .rect_filled(menu_rect, CornerRadius::same(6), pill_bg);
                }
                let menu_fg = if menu_resp.hovered() {
                    Color32::WHITE
                } else {
                    icon_muted
                };
                let mc = menu_rect.center();
                for dy in [-4.0_f32, 0.0, 4.0] {
                    ui.painter().line_segment(
                        [Pos2::new(mc.x - 6.5, mc.y + dy), Pos2::new(mc.x + 6.5, mc.y + dy)],
                        Stroke::new(1.5_f32, menu_fg),
                    );
                }
                if menu_resp.on_hover_text("Toggle Navigation").clicked() {
                    let screen_w = ctx.screen_rect().width();
                    if screen_w < 900.0 {
                        self.sidebar_drawer_open = !self.sidebar_drawer_open;
                    } else {
                        self.sidebar_collapsed = !self.sidebar_collapsed;
                    }
                }
                x = menu_rect.right() + 10.0;

                // Shiply Brand title in topbar
                let logo_rect =
                    Rect::from_center_size(Pos2::new(x + 10.0, cy), Vec2::new(20.0, 20.0));
                draw_shiply_logo(
                    ui.painter(),
                    logo_rect,
                    5,
                    FONT_SM,
                    self.logo_texture.as_ref(),
                );
                x = logo_rect.right() + 8.0;

                ui.painter().text(
                    Pos2::new(x, cy),
                    Align2::LEFT_CENTER,
                    format!("Shiply v{APP_VERSION}"),
                    FontId::proportional(FONT_MD),
                    topbar_text,
                );
                let left_controls_end = x + 100.0;

                // =========================================================================
                // Right-Side Window Actions (Bell, Minimize ─, Maximize/Restore ❐, Close ×)
                // (Dark mode button removed as requested)
                // =========================================================================
                let btn_w = 34.0;
                let btn_gap = 4.0;
                let right_edge = bar_rect.right() - 2.0;

                let close_rect = Rect::from_center_size(
                    Pos2::new(right_edge - btn_w * 0.5, cy),
                    Vec2::new(btn_w, pill_h),
                );
                let max_rect = Rect::from_center_size(
                    Pos2::new(close_rect.left() - btn_gap - btn_w * 0.5, cy),
                    Vec2::new(btn_w, pill_h),
                );
                let min_rect = Rect::from_center_size(
                    Pos2::new(max_rect.left() - btn_gap - btn_w * 0.5, cy),
                    Vec2::new(btn_w, pill_h),
                );
                let bell_rect = Rect::from_center_size(
                    Pos2::new(min_rect.left() - 8.0 - btn_w * 0.5, cy),
                    Vec2::new(btn_w, pill_h),
                );
                let donate_rect = Rect::from_center_size(
                    Pos2::new(bell_rect.left() - btn_gap - btn_w * 0.5, cy),
                    Vec2::new(btn_w, pill_h),
                );

                // Interactive Drag Region in the middle empty space of the topbar
                let drag_left = left_controls_end;
                let drag_right = (donate_rect.left() - 6.0).max(drag_left + 10.0);
                let drag_rect = Rect::from_min_max(
                    Pos2::new(drag_left, bar_rect.top()),
                    Pos2::new(drag_right, bar_rect.bottom()),
                );
                let drag_resp = ui.interact(
                    drag_rect,
                    egui::Id::new("topbar_window_drag_zone"),
                    Sense::click_and_drag(),
                );
                if drag_resp.double_clicked() {
                    self.window_maximized = !self.window_maximized;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(self.window_maximized));
                } else if drag_resp.drag_started_by(egui::PointerButton::Primary) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

                // A. Notification Bell Button (🔔) — acts as Update Information
                let bell_resp = ui.interact(
                    bell_rect,
                    egui::Id::new("topbar_bell_update_btn"),
                    Sense::click(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand);
                if bell_resp.hovered() {
                    ui.painter()
                        .rect_filled(bell_rect, CornerRadius::same(6), pill_bg);
                }
                let bell_fg = if self.available_update.is_some() {
                    theme.warning
                } else if bell_resp.hovered() {
                    Color32::WHITE
                } else {
                    icon_muted
                };
                draw_icon_bell(ui.painter(), bell_rect.center(), bell_fg);
                if self.available_update.is_some() {
                    ui.painter().circle_filled(
                        Pos2::new(bell_rect.center().x + 4.5, bell_rect.center().y - 4.5),
                        3.2,
                        theme.warning,
                    );
                }
                if bell_resp
                    .on_hover_text(if let Some(rel) = &self.available_update {
                        format!("Update Available: {} (Click to view details)", rel.tag_name)
                    } else {
                        "Update Information & Check for Updates (GitHub)".to_string()
                    })
                    .clicked()
                {
                    self.show_update_modal = true;
                    if self.available_update.is_none() && !self.update_checking {
                        spawn_update_check(self.updater_tx.clone(), true);
                    }
                }

                // A2. Donation / Support Button (❤️) — redirects to Trakteer
                let donate_resp = ui.interact(
                    donate_rect,
                    egui::Id::new("topbar_donate_btn"),
                    Sense::click(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Support development on Trakteer (https://trakteer.id/faidfadjri)");
                if donate_resp.hovered() {
                    ui.painter()
                        .rect_filled(donate_rect, CornerRadius::same(6), pill_bg);
                }
                let donate_fg = if donate_resp.hovered() {
                    Color32::from_rgb(255, 95, 125)
                } else {
                    icon_muted
                };
                draw_outline_icon(ui.painter(), donate_rect.center(), OutlineIcon::Heart, donate_fg);
                if donate_resp.clicked() {
                    open_url_in_browser(DONATION_URL);
                }

                // B. Minimize Button (─)
                let min_resp = ui.interact(
                    min_rect,
                    egui::Id::new("topbar_window_minimize_btn"),
                    Sense::click(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand);
                if min_resp.hovered() {
                    ui.painter()
                        .rect_filled(min_rect, CornerRadius::same(6), pill_bg);
                }
                let min_fg = if min_resp.hovered() {
                    Color32::WHITE
                } else {
                    icon_muted
                };
                let min_c = min_rect.center();
                ui.painter().line_segment(
                    [Pos2::new(min_c.x - 5.0, min_c.y), Pos2::new(min_c.x + 5.0, min_c.y)],
                    Stroke::new(1.5_f32, min_fg),
                );
                if min_resp.on_hover_text("Minimize").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }

                // C. Maximize / Restore Button
                let max_resp = ui.interact(
                    max_rect,
                    egui::Id::new("topbar_window_maximize_btn"),
                    Sense::click(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand);
                if max_resp.hovered() {
                    ui.painter()
                        .rect_filled(max_rect, CornerRadius::same(6), pill_bg);
                }
                let max_fg = if max_resp.hovered() {
                    Color32::WHITE
                } else {
                    icon_muted
                };
                draw_window_max_restore_icon(
                    ui.painter(),
                    max_rect.center(),
                    self.window_maximized,
                    max_fg,
                );
                if max_resp
                    .on_hover_text(if self.window_maximized {
                        "Restore Down"
                    } else {
                        "Maximize"
                    })
                    .clicked()
                {
                    self.window_maximized = !self.window_maximized;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(self.window_maximized));
                }

                // D. Close Button (×)
                let close_resp = ui.interact(
                    close_rect,
                    egui::Id::new("topbar_window_close_btn"),
                    Sense::click(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand);
                if close_resp.hovered() {
                    ui.painter().rect_filled(
                        close_rect,
                        CornerRadius::same(6),
                        theme.danger,
                    );
                }
                let close_fg = if close_resp.hovered() {
                    Color32::WHITE
                } else {
                    icon_muted
                };
                let cc = close_rect.center();
                let cs = Stroke::new(1.5_f32, close_fg);
                ui.painter().line_segment(
                    [Pos2::new(cc.x - 4.5, cc.y - 4.5), Pos2::new(cc.x + 4.5, cc.y + 4.5)],
                    cs,
                );
                ui.painter().line_segment(
                    [Pos2::new(cc.x + 4.5, cc.y - 4.5), Pos2::new(cc.x - 4.5, cc.y + 4.5)],
                    cs,
                );
                if close_resp.on_hover_text("Close").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
    }

    /// Provides invisible 5px interactive resize borders around the frameless window edges when unmaximized.
    pub(crate) fn render_window_resize_handles(&self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
            return;
        }
        let screen = ctx.screen_rect();
        let border = 5.0;

        let zones: [(&str, Rect, egui::ResizeDirection, egui::CursorIcon); 8] = [
            (
                "resize_n",
                Rect::from_min_max(
                    Pos2::new(screen.left() + border, screen.top()),
                    Pos2::new(screen.right() - border, screen.top() + border),
                ),
                egui::ResizeDirection::North,
                egui::CursorIcon::ResizeNorth,
            ),
            (
                "resize_s",
                Rect::from_min_max(
                    Pos2::new(screen.left() + border, screen.bottom() - border),
                    Pos2::new(screen.right() - border, screen.bottom()),
                ),
                egui::ResizeDirection::South,
                egui::CursorIcon::ResizeSouth,
            ),
            (
                "resize_w",
                Rect::from_min_max(
                    Pos2::new(screen.left(), screen.top() + border),
                    Pos2::new(screen.left() + border, screen.bottom() - border),
                ),
                egui::ResizeDirection::West,
                egui::CursorIcon::ResizeWest,
            ),
            (
                "resize_e",
                Rect::from_min_max(
                    Pos2::new(screen.right() - border, screen.top() + border),
                    Pos2::new(screen.right(), screen.bottom() - border),
                ),
                egui::ResizeDirection::East,
                egui::CursorIcon::ResizeEast,
            ),
            (
                "resize_nw",
                Rect::from_min_size(screen.left_top(), Vec2::splat(border * 2.0)),
                egui::ResizeDirection::NorthWest,
                egui::CursorIcon::ResizeNorthWest,
            ),
            (
                "resize_ne",
                Rect::from_min_size(
                    Pos2::new(screen.right() - border * 2.0, screen.top()),
                    Vec2::splat(border * 2.0),
                ),
                egui::ResizeDirection::NorthEast,
                egui::CursorIcon::ResizeNorthEast,
            ),
            (
                "resize_sw",
                Rect::from_min_size(
                    Pos2::new(screen.left(), screen.bottom() - border * 2.0),
                    Vec2::splat(border * 2.0),
                ),
                egui::ResizeDirection::SouthWest,
                egui::CursorIcon::ResizeSouthWest,
            ),
            (
                "resize_se",
                Rect::from_min_size(
                    Pos2::new(screen.right() - border * 2.0, screen.bottom() - border * 2.0),
                    Vec2::splat(border * 2.0),
                ),
                egui::ResizeDirection::SouthEast,
                egui::CursorIcon::ResizeSouthEast,
            ),
        ];

        egui::Area::new(egui::Id::new("window_resize_handles_overlay"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::ZERO)
            .show(ctx, |ui| {
                for (id, rect, dir, cursor) in zones {
                    let resp = ui.interact(rect, egui::Id::new(id), Sense::drag());
                    if resp.hovered() {
                        ctx.set_cursor_icon(cursor);
                    }
                    if resp.drag_started_by(egui::PointerButton::Primary) {
                        ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
                    }
                }
            });
    }
}
