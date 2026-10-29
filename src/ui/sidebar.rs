use super::*;

impl ShiplyApp {
    pub(crate) fn render_sidebar_drawer(&mut self, ctx: &egui::Context) {
        let screen_rect = ctx.screen_rect();

        egui::Area::new(egui::Id::new("sidebar_drawer_overlay"))
            .fixed_pos(screen_rect.min)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let (backdrop_rect, backdrop_resp) =
                    ui.allocate_exact_size(screen_rect.size(), Sense::click());
                ui.painter()
                    .rect_filled(backdrop_rect, CornerRadius::ZERO, MD_OVERLAY);
                if backdrop_resp.clicked() {
                    self.sidebar_drawer_open = false;
                }

                let drawer_rect = Rect::from_min_size(
                    screen_rect.min,
                    Vec2::new(SIDEBAR_EXPANDED_WIDTH, screen_rect.height()),
                );
                let mut drawer_ui = ui.new_child(egui::UiBuilder::new().max_rect(drawer_rect));
                drawer_ui
                    .painter()
                    .rect_filled(drawer_rect, CornerRadius::ZERO, SB_BG);
                drawer_ui.painter().line_segment(
                    [
                        Pos2::new(drawer_rect.right(), drawer_rect.top()),
                        Pos2::new(drawer_rect.right(), drawer_rect.bottom()),
                    ],
                    Stroke::new(1.0_f32, SB_BORDER_LINE),
                );

                let mut inner_ui = drawer_ui.new_child(
                    egui::UiBuilder::new().max_rect(drawer_rect.shrink2(Vec2::new(10.0, 0.0))),
                );
                inner_ui.visuals_mut().override_text_color = Some(SB_TEXT);
                let mut sort_trigger_rect_out = None;
                self.render_sidebar_content(&mut inner_ui, false, ctx, &mut sort_trigger_rect_out);
                if let Some(r) = sort_trigger_rect_out {
                    self.render_sidebar_sort_dropdown(ctx, r);
                }
                self.render_sidebar_custom_context_menu(ctx);
            });
    }

    pub(crate) fn render_sidebar(&mut self, ctx: &egui::Context, anim_t: f32) {
        let is_rail = anim_t < 0.35;
        let width = if is_rail {
            SIDEBAR_COLLAPSED_WIDTH
        } else {
            SIDEBAR_COLLAPSED_WIDTH
                + (SIDEBAR_EXPANDED_WIDTH - SIDEBAR_COLLAPSED_WIDTH)
                    * ((anim_t - 0.35) / 0.65).clamp(0.0, 1.0)
        };
        let pad_x = if is_rail { 4.0 } else { 10.0 };
        let mut sort_trigger_rect_out = None;

        egui::SidePanel::left("shiply_sidebar")
            .resizable(false)
            .show_separator_line(false)
            .exact_width(width)
            .frame(
                egui::Frame::new()
                    .fill(SB_BG)
                    .inner_margin(Margin::symmetric(pad_x as i8, 0))
                    .stroke(Stroke::NONE),
            )
            .show(ctx, |ui| {
                // Hairline right border separating sidebar from central workspace
                let r = ui.max_rect();
                ui.painter().line_segment(
                    [Pos2::new(r.right(), r.top()), Pos2::new(r.right(), r.bottom())],
                    Stroke::new(1.0_f32, SB_BORDER_LINE),
                );
                ui.visuals_mut().override_text_color = Some(SB_TEXT);
                self.render_sidebar_content(ui, is_rail, ctx, &mut sort_trigger_rect_out);
            });

        if let Some(r) = sort_trigger_rect_out {
            self.render_sidebar_sort_dropdown(ctx, r);
        }
        self.render_sidebar_custom_context_menu(ctx);
    }

    pub(crate) fn render_sidebar_content(
        &mut self,
        ui: &mut egui::Ui,
        is_rail: bool,
        _ctx: &egui::Context,
        sort_trigger_rect_out: &mut Option<Rect>,
    ) {
        let inner_rect = ui.available_rect_before_wrap();
        let full_w = inner_rect.width();
        let theme = self.theme();

        if is_rail {
            // ==================== COLLAPSED RAIL MODE (64px) ====================
            // 1. Pinned Brand Top Bar (52px)
            let brand_rect = Rect::from_min_size(inner_rect.min, Vec2::new(full_w, 52.0));
            let brand_ui = ui.new_child(egui::UiBuilder::new().max_rect(brand_rect));
            let logo_rect = Rect::from_center_size(brand_rect.center(), Vec2::new(26.0, 26.0));
            draw_shiply_logo(
                brand_ui.painter(),
                logo_rect,
                6,
                FS_TITLE,
                self.logo_texture.as_ref(),
            );
            brand_ui.painter().line_segment(
                [
                    Pos2::new(brand_rect.left(), brand_rect.bottom()),
                    Pos2::new(brand_rect.right(), brand_rect.bottom()),
                ],
                Stroke::new(1.0_f32, SB_BORDER_LINE),
            );

            // 2. Pinned Footer (132px)
            let footer_h = 132.0_f32;
            let footer_rect = Rect::from_min_size(
                Pos2::new(inner_rect.left(), inner_rect.bottom() - footer_h),
                Vec2::new(full_w, footer_h),
            );
            let mut footer_ui = ui.new_child(egui::UiBuilder::new().max_rect(footer_rect));
            footer_ui.spacing_mut().item_spacing = Vec2::ZERO;
            footer_ui.painter().line_segment(
                [
                    Pos2::new(footer_rect.left(), footer_rect.top()),
                    Pos2::new(footer_rect.right(), footer_rect.top()),
                ],
                Stroke::new(1.0_f32, SB_BORDER_LINE),
            );

            let btn_size = Vec2::new(32.0, 32.0);
            let center_x = footer_rect.center().x;

            // Sync Button
            let sync_rect = Rect::from_center_size(Pos2::new(center_x, footer_rect.top() + 20.0), btn_size);
            let sync_resp = footer_ui
                .interact(sync_rect, egui::Id::new("rail_sync_btn"), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Sync App Directory Files");
            if sync_resp.hovered() {
                footer_ui
                    .painter()
                    .rect_filled(sync_rect, CornerRadius::same(R_MD as u8), SB_SURFACE_HOVER);
            }
            draw_icon_refresh(
                footer_ui.painter(),
                sync_rect.center(),
                if sync_resp.hovered() { SB_TEXT } else { SB_TEXT_MUTED },
            );
            if sync_resp.clicked() {
                self.sidebar_refreshing_until = Some(Instant::now() + Duration::from_millis(400));
                self.storage.reset_sync_cache();
                if let Some(msg) = self.storage.sync_from_app_directory() {
                    self.refresh_selected_project_metadata(true);
                    self.status_banner = Some((msg, true));
                } else {
                    self.status_banner = Some((
                        format!(
                            "Synced app directory ({})",
                            self.storage.app_data_dir().display()
                        ),
                        true,
                    ));
                }
            }

            // Folder Button
            let dir_rect = Rect::from_center_size(Pos2::new(center_x, footer_rect.top() + 54.0), btn_size);
            let dir_resp = footer_ui
                .interact(dir_rect, egui::Id::new("rail_folder_btn"), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Open App Folder (JSON / .ps1)");
            if dir_resp.hovered() {
                footer_ui
                    .painter()
                    .rect_filled(dir_rect, CornerRadius::same(R_MD as u8), SB_SURFACE_HOVER);
            }
            draw_icon_folder(
                footer_ui.painter(),
                dir_rect.center(),
                if dir_resp.hovered() { SB_TEXT } else { SB_TEXT_MUTED },
            );
            if dir_resp.clicked() {
                let dir = self.storage.app_data_dir();
                let _ = fs::create_dir_all(&dir);
                open_directory_in_os(&dir);
            }

            // Updates Button
            let upd_rect = Rect::from_center_size(Pos2::new(center_x, footer_rect.top() + 88.0), btn_size);
            let upd_resp = footer_ui
                .interact(upd_rect, egui::Id::new("rail_upd_btn"), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(if self.available_update.is_some() {
                    "Update Available! Click to view"
                } else {
                    "Check for Updates"
                });
            if upd_resp.hovered() {
                footer_ui
                    .painter()
                    .rect_filled(upd_rect, CornerRadius::same(R_MD as u8), SB_SURFACE_HOVER);
            }
            let upd_color = if self.available_update.is_some() {
                theme.warning
            } else if upd_resp.hovered() {
                SB_TEXT
            } else {
                SB_TEXT_MUTED
            };
            draw_icon_refresh(footer_ui.painter(), upd_rect.center(), upd_color);
            if upd_resp.clicked() {
                if self.available_update.is_some() {
                    self.show_update_modal = true;
                } else if !self.update_checking {
                    spawn_update_check(self.updater_tx.clone(), true);
                }
            }

            // Status Green Dot
            let dot_pos = Pos2::new(center_x, footer_rect.top() + 118.0);
            let dot_color = if self.available_update.is_some() {
                theme.warning
            } else {
                theme.success
            };
            let status_msg = if let Some(rel) = &self.available_update {
                format!("Update Available: {}", rel.tag_name)
            } else {
                format!("Up to date (v{APP_VERSION})")
            };
            let dot_rect = Rect::from_center_size(dot_pos, Vec2::new(14.0, 14.0));
            footer_ui
                .interact(dot_rect, egui::Id::new("rail_status_dot"), Sense::hover())
                .on_hover_text(status_msg);
            footer_ui.painter().circle_filled(dot_pos, 3.5, dot_color);

            // 3. Middle Scrollable Rail Content
            let middle_rect = Rect::from_min_max(
                Pos2::new(inner_rect.left(), brand_rect.bottom() + 8.0),
                Pos2::new(inner_rect.right(), footer_rect.top() - 6.0),
            );
            let mut middle_ui = ui.new_child(egui::UiBuilder::new().max_rect(middle_rect));
            ScrollArea::vertical()
                .id_salt("sidebar_rail_middle")
                .auto_shrink([false, false])
                .show(&mut middle_ui, |ui| {
                    let full_w = ui.available_width();
                    let center_x = full_w / 2.0;

                    // Nav item 1: All Deployments
                    let is_overview = matches!(self.selection, SidebarSelection::AllDeployments);
                    let all_rect = Rect::from_center_size(Pos2::new(center_x, ui.cursor().min.y + 18.0), btn_size);
                    let all_resp = ui
                        .interact(all_rect, egui::Id::new("rail_all_deploy_btn"), Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text("All Deployments");
                    let all_bg = if is_overview {
                        SB_ACCENT_SOFT
                    } else if all_resp.hovered() {
                        SB_SURFACE_HOVER
                    } else {
                        Color32::TRANSPARENT
                    };
                    let all_fg = if is_overview {
                        SB_ACCENT
                    } else if all_resp.hovered() {
                        SB_TEXT
                    } else {
                        SB_TEXT_MUTED
                    };
                    ui.painter().rect_filled(all_rect, CornerRadius::same(R_MD as u8), all_bg);
                    draw_icon_layers(ui.painter(), all_rect.center(), all_fg);
                    if all_resp.clicked() {
                        self.selection = SidebarSelection::AllDeployments;
                    }
                    ui.add_space(40.0);

                    // Nav item 2: Bulk Deploy
                    if !self.storage.projects.is_empty() {
                        let is_bulk = self.show_bulk_deploy_modal;
                        let bulk_rect = Rect::from_center_size(Pos2::new(center_x, ui.cursor().min.y + 18.0), btn_size);
                        let bulk_resp = ui
                            .interact(bulk_rect, egui::Id::new("rail_bulk_deploy_btn"), Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text("Bulk Deploy...");
                        let bulk_bg = if is_bulk {
                            SB_ACCENT_SOFT
                        } else if bulk_resp.hovered() {
                            SB_SURFACE_HOVER
                        } else {
                            Color32::TRANSPARENT
                        };
                        let bulk_fg = if is_bulk {
                            SB_ACCENT
                        } else if bulk_resp.hovered() {
                            SB_TEXT
                        } else {
                            SB_TEXT_MUTED
                        };
                        ui.painter().rect_filled(bulk_rect, CornerRadius::same(R_MD as u8), bulk_bg);
                        draw_icon_send(ui.painter(), bulk_rect.center(), bulk_fg);
                        if bulk_resp.clicked() {
                            self.open_bulk_deploy_modal();
                        }
                        ui.add_space(40.0);
                    }

                    // Separator
                    let sep_y = ui.cursor().min.y + 4.0;
                    ui.painter().line_segment(
                        [Pos2::new(center_x - 14.0, sep_y), Pos2::new(center_x + 14.0, sep_y)],
                        Stroke::new(1.0_f32, SB_BORDER_LINE),
                    );
                    ui.add_space(12.0);

                    // Add Project Button (+)
                    let add_rect = Rect::from_center_size(Pos2::new(center_x, ui.cursor().min.y + 16.0), btn_size);
                    let add_resp = ui
                        .interact(add_rect, egui::Id::new("rail_add_proj_btn"), Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text("Add Project");
                    let add_bg = if add_resp.hovered() { SB_SURFACE_HOVER } else { Color32::TRANSPARENT };
                    ui.painter().rect_filled(add_rect, CornerRadius::same(R_MD as u8), add_bg);
                    draw_icon_plus(ui.painter(), add_rect.center(), if add_resp.hovered() { SB_TEXT } else { SB_TEXT_MUTED });
                    if add_resp.clicked() {
                        self.open_new_project_modal();
                    }
                    ui.add_space(36.0);

                    // Project Avatars
                    let mut clicked_project_id = None;
                    for project in &self.storage.projects {
                        let is_selected = matches!(&self.selection, SidebarSelection::Project(id) if id == &project.id);
                        let p_avatar_rect = Rect::from_center_size(Pos2::new(center_x, ui.cursor().min.y + 18.0), Vec2::new(32.0, 32.0));
                        let p_resp = ui
                            .interact(p_avatar_rect, egui::Id::new(("rail_proj", &project.id)), Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand);

                        let bg_fill = if is_selected {
                            SB_ACCENT_SOFT
                        } else if p_resp.hovered() {
                            SB_SURFACE_HOVER
                        } else {
                            SB_SURFACE
                        };
                        let stroke = if is_selected {
                            Stroke::new(2.0_f32, SB_ACCENT)
                        } else {
                            Stroke::new(1.0_f32, SB_BORDER_LINE)
                        };

                        ui.painter().rect_filled(p_avatar_rect, CornerRadius::same(8), bg_fill);
                        ui.painter().rect_stroke(p_avatar_rect, CornerRadius::same(8), stroke, StrokeKind::Inside);

                        let initial = project.name.chars().next().unwrap_or('P').to_uppercase().to_string();
                        let fg_color = if is_selected { SB_ACCENT } else { SB_TEXT };
                        ui.painter().text(
                            p_avatar_rect.center(),
                            Align2::CENTER_CENTER,
                            initial,
                            FontId::proportional(FS_BASE),
                            fg_color,
                        );

                        let mut env_summary = String::new();
                        for env in &project.environments {
                            if let Some(dep) = self.storage.last_succeeded_deployment(&project.id, &env.key) {
                                env_summary.push_str(&format!("\n• {}: v{}", env.name, dep.version));
                            } else {
                                env_summary.push_str(&format!("\n• {}: —", env.name));
                            }
                        }
                        let tooltip = format!("{}{}", project.name, env_summary);
                        let p_resp = p_resp.on_hover_text(tooltip);

                        if p_resp.secondary_clicked() {
                            if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
                                self.sidebar_context_menu = Some(SidebarContextMenuState {
                                    project_id: project.id.clone(),
                                    click_pos: pos,
                                    submenu_open: false,
                                });
                            }
                        }

                        if p_resp.clicked() {
                            clicked_project_id = Some(project.id.clone());
                        }

                        ui.add_space(38.0);
                    }

                    if let Some(pid) = clicked_project_id {
                        self.sidebar_context_menu = None;
                        let was_different = match &self.selection {
                            SidebarSelection::Project(cur) => cur != &pid,
                            SidebarSelection::AllDeployments => true,
                        };
                        self.selection = SidebarSelection::Project(pid.clone());
                        if was_different {
                            self.project_switch_loading_id = Some(pid);
                            self.project_switch_loading_start = Some(Instant::now());
                        }
                        self.refresh_selected_project_metadata(true);
                    }
                });
        } else {
            // ==================== EXPANDED SIDEBAR MODE (248px) ====================
            // 1. Pinned Brand Top Bar (52px)
            let brand_rect = Rect::from_min_size(inner_rect.min, Vec2::new(full_w, 52.0));
            let brand_ui = ui.new_child(egui::UiBuilder::new().max_rect(brand_rect));
            {
                let logo_rect = Rect::from_min_size(
                    Pos2::new(brand_rect.left() + 2.0, brand_rect.center().y - 13.0),
                    Vec2::new(26.0, 26.0),
                );
                draw_shiply_logo(
                    brand_ui.painter(),
                    logo_rect,
                    6,
                    FS_TITLE,
                    self.logo_texture.as_ref(),
                );
                brand_ui.painter().text(
                    Pos2::new(logo_rect.right() + 8.0, brand_rect.center().y),
                    Align2::LEFT_CENTER,
                    "Shiply",
                    FontId::proportional(FS_BRAND),
                    SB_TEXT,
                );

                // Version badge: mono 11px, bg --sb-surface, text --sb-text-muted, radius 4px
                let ver_text = format!("v{APP_VERSION}");
                let ver_galley = brand_ui.painter().layout_no_wrap(
                    ver_text,
                    FontId::monospace(FS_MICRO),
                    SB_TEXT_MUTED,
                );
                let badge_pad_x = 6.0_f32;
                let badge_h = 20.0_f32;
                let badge_w = ver_galley.size().x + badge_pad_x * 2.0;
                let badge_rect = Rect::from_min_size(
                    Pos2::new(
                        brand_rect.right() - badge_w - 2.0,
                        brand_rect.center().y - badge_h / 2.0,
                    ),
                    Vec2::new(badge_w, badge_h),
                );
                brand_ui
                    .painter()
                    .rect_filled(badge_rect, CornerRadius::same(R_SM as u8), SB_SURFACE);
                brand_ui.painter().galley(
                    Pos2::new(
                        badge_rect.left() + badge_pad_x,
                        badge_rect.center().y - ver_galley.size().y / 2.0,
                    ),
                    ver_galley,
                    SB_TEXT_MUTED,
                );

                // Hairline bottom border
                brand_ui.painter().line_segment(
                    [
                        Pos2::new(brand_rect.left() - 10.0, brand_rect.bottom()),
                        Pos2::new(brand_rect.right() + 10.0, brand_rect.bottom()),
                    ],
                    Stroke::new(1.0_f32, SB_BORDER_LINE),
                );
            }

            // 2. Pinned Footer (138px) with zero-spacing to guarantee no bottom overflow
            let footer_h = 138.0_f32;
            let footer_rect = Rect::from_min_size(
                Pos2::new(inner_rect.left(), inner_rect.bottom() - footer_h),
                Vec2::new(full_w, footer_h),
            );
            let mut footer_ui = ui.new_child(egui::UiBuilder::new().max_rect(footer_rect));
            footer_ui.spacing_mut().item_spacing = Vec2::ZERO;
            {
                footer_ui.painter().line_segment(
                    [
                        Pos2::new(footer_rect.left() - 10.0, footer_rect.top()),
                        Pos2::new(footer_rect.right() + 10.0, footer_rect.top()),
                    ],
                    Stroke::new(1.0_f32, SB_BORDER_LINE),
                );

                // 3 Ghost Buttons (30px high, radius 6px)
                // Action 1: Sync App Directory Files
                let sync_rect = Rect::from_min_size(
                    Pos2::new(footer_rect.left(), footer_rect.top() + 8.0),
                    Vec2::new(full_w, 30.0),
                );
                let sync_resp = footer_ui
                    .interact(sync_rect, egui::Id::new("sidebar_sync_btn"), Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                let sync_bg = if sync_resp.hovered() {
                    SB_SURFACE_HOVER
                } else {
                    Color32::TRANSPARENT
                };
                let sync_fg = if sync_resp.hovered() {
                    SB_TEXT
                } else {
                    SB_TEXT_MUTED
                };
                footer_ui
                    .painter()
                    .rect_filled(sync_rect, CornerRadius::same(R_MD as u8), sync_bg);
                draw_icon_refresh(
                    footer_ui.painter(),
                    Pos2::new(sync_rect.left() + 14.0, sync_rect.center().y),
                    sync_fg,
                );
                footer_ui.painter().text(
                    Pos2::new(sync_rect.left() + 30.0, sync_rect.center().y),
                    Align2::LEFT_CENTER,
                    "Sync App Files",
                    FontId::proportional(FS_BASE),
                    sync_fg,
                );
                if sync_resp.clicked() {
                    self.sidebar_refreshing_until =
                        Some(Instant::now() + Duration::from_millis(400));
                    self.storage.reset_sync_cache();
                    if let Some(msg) = self.storage.sync_from_app_directory() {
                        self.refresh_selected_project_metadata(true);
                        self.status_banner = Some((msg, true));
                    } else {
                        self.status_banner = Some((
                            format!(
                                "Synced app directory ({})",
                                self.storage.app_data_dir().display()
                            ),
                            true,
                        ));
                    }
                }

                // Action 2: Open App Folder
                let dir_rect = Rect::from_min_size(
                    Pos2::new(footer_rect.left(), footer_rect.top() + 40.0),
                    Vec2::new(full_w, 30.0),
                );
                let dir_resp = footer_ui
                    .interact(dir_rect, egui::Id::new("sidebar_folder_btn"), Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                let dir_bg = if dir_resp.hovered() {
                    SB_SURFACE_HOVER
                } else {
                    Color32::TRANSPARENT
                };
                let dir_fg = if dir_resp.hovered() {
                    SB_TEXT
                } else {
                    SB_TEXT_MUTED
                };
                footer_ui
                    .painter()
                    .rect_filled(dir_rect, CornerRadius::same(R_MD as u8), dir_bg);
                draw_icon_folder(
                    footer_ui.painter(),
                    Pos2::new(dir_rect.left() + 14.0, dir_rect.center().y),
                    dir_fg,
                );
                footer_ui.painter().text(
                    Pos2::new(dir_rect.left() + 30.0, dir_rect.center().y),
                    Align2::LEFT_CENTER,
                    "Open App Folder",
                    FontId::proportional(FS_BASE),
                    dir_fg,
                );
                if dir_resp
                    .on_hover_text(
                        "Open the app folder where you can paste shiply_state.json or deploy-*.ps1 files",
                    )
                    .clicked()
                {
                    let dir = self.storage.app_data_dir();
                    let _ = fs::create_dir_all(&dir);
                    open_directory_in_os(&dir);
                }

                // Action 3: Check for Updates
                let upd_rect = Rect::from_min_size(
                    Pos2::new(footer_rect.left(), footer_rect.top() + 72.0),
                    Vec2::new(full_w, 30.0),
                );
                let upd_resp = footer_ui
                    .interact(upd_rect, egui::Id::new("sidebar_update_btn"), Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                let (upd_label, upd_fg) = if let Some(rel) = &self.available_update {
                    (
                        format!("Update: {}!", rel.tag_name),
                        theme.warning,
                    )
                } else if self.update_checking {
                    ("Checking Releases...".to_string(), SB_ACCENT)
                } else if upd_resp.hovered() {
                    ("Check for Updates".to_string(), SB_TEXT)
                } else {
                    ("Check for Updates".to_string(), SB_TEXT_MUTED)
                };
                let upd_bg = if upd_resp.hovered() {
                    SB_SURFACE_HOVER
                } else {
                    Color32::TRANSPARENT
                };
                footer_ui
                    .painter()
                    .rect_filled(upd_rect, CornerRadius::same(R_MD as u8), upd_bg);
                draw_icon_refresh(
                    footer_ui.painter(),
                    Pos2::new(upd_rect.left() + 14.0, upd_rect.center().y),
                    upd_fg,
                );
                footer_ui.painter().text(
                    Pos2::new(upd_rect.left() + 30.0, upd_rect.center().y),
                    Align2::LEFT_CENTER,
                    upd_label,
                    FontId::proportional(FS_BASE),
                    upd_fg,
                );
                if upd_resp
                    .on_hover_text(format!(
                        "Check GitHub Releases ({GITHUB_REPO}) for updates"
                    ))
                    .clicked()
                {
                    if self.available_update.is_some() {
                        self.show_update_modal = true;
                    } else if !self.update_checking {
                        spawn_update_check(self.updater_tx.clone(), true);
                    }
                }

                // Status Row: 6px green circle + "Up to date" / "Installed v..." + action link
                let stat_rect = Rect::from_min_size(
                    Pos2::new(footer_rect.left(), footer_rect.top() + 108.0),
                    Vec2::new(full_w, 20.0),
                );
                let install_state = check_install_state(&default_install_dir(), APP_VERSION);
                let (dot_color, status_text, action_text) = match &install_state {
                    InstallState::NotInstalled => {
                        (SB_ACCENT, "Not installed".to_string(), "Install (.exe)")
                    }
                    InstallState::InstalledCurrent { version, .. } => {
                        (theme.success, format!("Installed v{version}"), "Reinstall")
                    }
                    InstallState::UpdateReady { running_version, .. } => (
                        theme.warning,
                        format!("Update to v{running_version}"),
                        "Apply",
                    ),
                };

                footer_ui.painter().circle_filled(
                    Pos2::new(stat_rect.left() + 10.0, stat_rect.center().y),
                    3.5,
                    dot_color,
                );

                let st_galley = footer_ui.painter().layout_no_wrap(
                    status_text,
                    FontId::monospace(FS_MICRO),
                    SB_TEXT_MUTED,
                );
                footer_ui.painter().galley(
                    Pos2::new(
                        stat_rect.left() + 20.0,
                        stat_rect.center().y - st_galley.size().y / 2.0,
                    ),
                    st_galley.clone(),
                    SB_TEXT_MUTED,
                );

                let link_x = stat_rect.left() + 20.0 + st_galley.size().x + 8.0;
                let link_galley = footer_ui.painter().layout_no_wrap(
                    action_text.to_string(),
                    FontId::proportional(FS_MICRO),
                    SB_TEXT_FAINT,
                );
                let link_rect = Rect::from_min_size(
                    Pos2::new(link_x, stat_rect.center().y - link_galley.size().y / 2.0),
                    link_galley.size(),
                );
                let link_resp = footer_ui
                    .interact(
                        link_rect,
                        egui::Id::new("sidebar_reinstall_link"),
                        Sense::click(),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                let link_color = if link_resp.hovered() {
                    SB_ACCENT
                } else {
                    SB_TEXT_FAINT
                };
                footer_ui
                    .painter()
                    .galley(link_rect.min, link_galley, link_color);

                if link_resp.clicked() {
                    match install_or_update_self() {
                        Ok(msg) => self.status_banner = Some((msg, true)),
                        Err(err) => self.status_banner = Some((err, false)),
                    }
                }

                // Donate link on sidebar status line
                let don_x = link_rect.right() + 8.0;
                let don_galley = footer_ui.painter().layout_no_wrap(
                    "Donate".to_string(),
                    FontId::proportional(FS_MICRO),
                    SB_TEXT_FAINT,
                );
                let don_rect = Rect::from_min_size(
                    Pos2::new(don_x, stat_rect.center().y - don_galley.size().y / 2.0),
                    don_galley.size(),
                );
                let don_resp = footer_ui
                    .interact(
                        don_rect,
                        egui::Id::new("sidebar_donate_link"),
                        Sense::click(),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text("Support Shiply development on Trakteer (https://trakteer.id/faidfadjri)");
                let don_color = if don_resp.hovered() {
                    Color32::from_rgb(255, 105, 135)
                } else {
                    SB_TEXT_FAINT
                };
                footer_ui
                    .painter()
                    .galley(don_rect.min, don_galley, don_color);

                if don_resp.clicked() {
                    open_url_in_browser(DONATION_URL);
                }
            }

            // 3. Middle Scrollable Content (Nav items + Projects header + Search + Projects list)
            let middle_rect = Rect::from_min_max(
                Pos2::new(inner_rect.left(), brand_rect.bottom() + 10.0),
                Pos2::new(inner_rect.right(), footer_rect.top() - 6.0),
            );
            let mut middle_ui = ui.new_child(egui::UiBuilder::new().max_rect(middle_rect));
            middle_ui.spacing_mut().scroll.bar_width = 4.0;
            middle_ui.spacing_mut().scroll.bar_inner_margin = 1.0;

            ScrollArea::vertical()
                .id_salt("sidebar_scrollable_middle")
                .auto_shrink([false, false])
                .show(&mut middle_ui, |ui| {
                    let full_w = ui.available_width();

                    // Nav Item 1: All Deployments (36px high, radius 6px, icon 16px, label 13px/500)
                    let is_overview = matches!(self.selection, SidebarSelection::AllDeployments);
                    let (nav_rect, nav_resp) =
                        ui.allocate_exact_size(Vec2::new(full_w, 36.0), Sense::click());
                    let nav_resp = nav_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                    let nav_bg = if is_overview {
                        SB_ACCENT_SOFT
                    } else if nav_resp.hovered() {
                        SB_SURFACE_HOVER
                    } else {
                        Color32::TRANSPARENT
                    };
                    let nav_fg = if is_overview {
                        SB_ACCENT
                    } else if nav_resp.hovered() {
                        SB_TEXT
                    } else {
                        SB_TEXT_MUTED
                    };
                    ui.painter()
                        .rect_filled(nav_rect, CornerRadius::same(R_MD as u8), nav_bg);
                    draw_icon_layers(
                        ui.painter(),
                        Pos2::new(nav_rect.left() + 16.0, nav_rect.center().y),
                        nav_fg,
                    );
                    ui.painter().text(
                        Pos2::new(nav_rect.left() + 34.0, nav_rect.center().y),
                        Align2::LEFT_CENTER,
                        "All Deployments",
                        FontId::proportional(FS_BASE),
                        nav_fg,
                    );
                    if nav_resp.clicked() {
                        self.selection = SidebarSelection::AllDeployments;
                    }

                    // Nav Item 2: Bulk Deploy (36px high)
                    if !self.storage.projects.is_empty() {
                        ui.add_space(3.0);
                        let is_bulk_active = self.show_bulk_deploy_modal;
                        let (bulk_rect, bulk_resp) =
                            ui.allocate_exact_size(Vec2::new(full_w, 36.0), Sense::click());
                        let bulk_resp = bulk_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                        let bulk_bg = if is_bulk_active {
                            SB_ACCENT_SOFT
                        } else if bulk_resp.hovered() {
                            SB_SURFACE_HOVER
                        } else {
                            Color32::TRANSPARENT
                        };
                        let bulk_fg = if is_bulk_active {
                            SB_ACCENT
                        } else if bulk_resp.hovered() {
                            SB_TEXT
                        } else {
                            SB_TEXT_MUTED
                        };
                        ui.painter()
                            .rect_filled(bulk_rect, CornerRadius::same(R_MD as u8), bulk_bg);
                        draw_icon_send(
                            ui.painter(),
                            Pos2::new(bulk_rect.left() + 16.0, bulk_rect.center().y),
                            bulk_fg,
                        );
                        ui.painter().text(
                            Pos2::new(bulk_rect.left() + 34.0, bulk_rect.center().y),
                            Align2::LEFT_CENTER,
                            "Bulk Deploy...",
                            FontId::proportional(FS_BASE),
                            bulk_fg,
                        );
                        if bulk_resp
                            .on_hover_text("Select multiple projects to deploy sequentially")
                            .clicked()
                        {
                            self.open_bulk_deploy_modal();
                        }
                    }

                    ui.add_space(14.0);

                    // Projects Header Row (28px)
                    let (hdr_rect, _) =
                        ui.allocate_exact_size(Vec2::new(full_w, 28.0), Sense::hover());
                    ui.painter().text(
                        Pos2::new(hdr_rect.left() + 2.0, hdr_rect.center().y),
                        Align2::LEFT_CENTER,
                        "PROJECTS",
                        FontId::proportional(FS_SMALL),
                        SB_TEXT_FAINT,
                    );

                    let add_w = 46.0_f32;
                    let sort_w = 78.0_f32;

                    // "+ ADD" Button
                    let add_btn_rect = Rect::from_min_size(
                        Pos2::new(hdr_rect.right() - add_w, hdr_rect.center().y - 12.0),
                        Vec2::new(add_w, 24.0),
                    );
                    let add_btn_resp = ui
                        .interact(
                            add_btn_rect,
                            egui::Id::new("sidebar_add_proj_btn"),
                            Sense::click(),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    let add_bg = if add_btn_resp.hovered() {
                        SB_SURFACE_HOVER
                    } else {
                        SB_SURFACE
                    };
                    ui.painter()
                        .rect_filled(add_btn_rect, CornerRadius::same(R_SM as u8), add_bg);
                    ui.painter().rect_stroke(
                        add_btn_rect,
                        CornerRadius::same(R_SM as u8),
                        Stroke::new(1.0_f32, SB_BORDER_LINE),
                        StrokeKind::Inside,
                    );
                    ui.painter().text(
                        add_btn_rect.center(),
                        Align2::CENTER_CENTER,
                        "+ ADD",
                        FontId::proportional(FS_MICRO),
                        SB_TEXT,
                    );
                    if add_btn_resp.clicked() {
                        self.open_new_project_modal();
                    }

                    // Sort Control
                    let sort_area_rect = Rect::from_min_size(
                        Pos2::new(
                            hdr_rect.right() - add_w - 6.0 - sort_w,
                            hdr_rect.center().y - 12.0,
                        ),
                        Vec2::new(sort_w, 24.0),
                    );
                    *sort_trigger_rect_out = Some(sort_area_rect);

                    let sort_btn_resp = ui
                        .interact(
                            sort_area_rect,
                            egui::Id::new("sidebar_project_sort_trigger"),
                            Sense::click(),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    if sort_btn_resp.clicked() {
                        self.sidebar_sort_dropdown_open = !self.sidebar_sort_dropdown_open;
                    }

                    let is_sort_open = self.sidebar_sort_dropdown_open;
                    let sort_bg = if is_sort_open || sort_btn_resp.hovered() {
                        SB_SURFACE_HOVER
                    } else {
                        SB_SURFACE
                    };
                    let sort_stroke = if is_sort_open {
                        Stroke::new(1.5_f32, SB_ACCENT)
                    } else {
                        Stroke::new(1.0_f32, SB_BORDER_LINE)
                    };
                    ui.painter()
                        .rect_filled(sort_area_rect, CornerRadius::same(R_SM as u8), sort_bg);
                    ui.painter().rect_stroke(
                        sort_area_rect,
                        CornerRadius::same(R_SM as u8),
                        sort_stroke,
                        StrokeKind::Inside,
                    );

                    let sort_label = match self.project_sort_mode {
                        ProjectSortMode::Custom => "Custom",
                        ProjectSortMode::NameAsc => "A-Z",
                        ProjectSortMode::NameDesc => "Z-A",
                        ProjectSortMode::RecentDeploy => "Recent",
                    };
                    ui.painter().text(
                        Pos2::new(sort_area_rect.left() + 8.0, sort_area_rect.center().y),
                        Align2::LEFT_CENTER,
                        sort_label,
                        FontId::proportional(FS_MICRO),
                        SB_TEXT,
                    );
                    draw_outline_icon(
                        ui.painter(),
                        Pos2::new(sort_area_rect.right() - 10.0, sort_area_rect.center().y),
                        if is_sort_open {
                            OutlineIcon::ChevronUp
                        } else {
                            OutlineIcon::ChevronDown
                        },
                        SB_TEXT_MUTED,
                    );

                    ui.add_space(8.0);

                    // Search input
                    sidebar_search_text_edit(
                        ui,
                        "sidebar_project_search_box",
                        &mut self.project_search_query,
                        full_w,
                        Some("Search projects..."),
                    );

                    ui.add_space(8.0);

                    // Skeleton loading on initial load or refresh:
                    let is_refreshing = self
                        .sidebar_refreshing_until
                        .as_ref()
                        .is_some_and(|t| Instant::now() < *t);
                    if is_refreshing && self.storage.projects.is_empty() {
                        ui.ctx().request_repaint();
                        let time = ui.input(|i| i.time);
                        for _ in 0..3 {
                            let (skel_rect, _) =
                                ui.allocate_exact_size(Vec2::new(full_w, 56.0), Sense::hover());
                            ui.painter().rect_filled(
                                skel_rect,
                                CornerRadius::same(R_MD as u8),
                                SB_SURFACE,
                            );
                            ui.painter().rect_stroke(
                                skel_rect,
                                CornerRadius::same(R_MD as u8),
                                Stroke::new(1.0_f32, SB_BORDER_LINE),
                                StrokeKind::Inside,
                            );

                            let name_w = (skel_rect.width() - 34.0) * 0.6;
                            let name_rect = Rect::from_min_size(
                                Pos2::new(skel_rect.left() + 20.0, skel_rect.top() + 10.0),
                                Vec2::new(name_w, 14.0),
                            );
                            paint_shimmer_rect(ui.painter(), name_rect, 4.0, time);

                            let pill1 = Rect::from_min_size(
                                Pos2::new(skel_rect.left() + 20.0, skel_rect.top() + 30.0),
                                Vec2::new(44.0, 18.0),
                            );
                            paint_shimmer_rect(ui.painter(), pill1, R_SM, time);
                            let pill2 = Rect::from_min_size(
                                Pos2::new(skel_rect.left() + 20.0 + 50.0, skel_rect.top() + 30.0),
                                Vec2::new(52.0, 18.0),
                            );
                            paint_shimmer_rect(ui.painter(), pill2, R_SM, time);

                            ui.add_space(6.0);
                        }
                    } else if self.storage.projects.is_empty() {
                        let (empty_rect, _) =
                            ui.allocate_exact_size(Vec2::new(full_w, 56.0), Sense::hover());
                        ui.painter().rect_stroke(
                            empty_rect,
                            CornerRadius::same(R_MD as u8),
                            Stroke::new(1.0_f32, SB_BORDER_LINE),
                            StrokeKind::Inside,
                        );
                        ui.painter().text(
                            empty_rect.center(),
                            Align2::CENTER_CENTER,
                            "No projects yet.\nClick '+ ADD' or import .ps1",
                            FontId::proportional(FS_SMALL),
                            SB_TEXT_MUTED,
                        );
                    } else {
                        let is_pointer_released = ui.input(|i| i.pointer.any_released());
                        let pointer_pos = ui.input(|i| i.pointer.hover_pos());

                        if self.dragged_project_idx.is_some() {
                            ui.ctx().request_repaint();
                        }

                        let query = self.project_search_query.trim().to_lowercase();
                        let has_groups = self.storage.projects.iter().any(|p| {
                            p.group.as_ref().is_some_and(|g| !g.trim().is_empty())
                        });

                        let mut group_sections: Vec<(Option<String>, Vec<usize>)> = Vec::new();
                        if has_groups {
                            for (idx, p) in self.storage.projects.iter().enumerate() {
                                if !query.is_empty() && !p.name.to_lowercase().contains(&query) {
                                    continue;
                                }
                                let grp = p
                                    .group
                                    .as_ref()
                                    .map(|g| g.trim().to_string())
                                    .filter(|g| !g.is_empty());
                                if let Some(pos) =
                                    group_sections.iter().position(|(g, _)| g.as_ref() == grp.as_ref())
                                {
                                    group_sections[pos].1.push(idx);
                                } else {
                                    group_sections.push((grp, vec![idx]));
                                }
                            }
                            group_sections.sort_by(|(g1, _), (g2, _)| match (g1, g2) {
                                (Some(a), Some(b)) => a.to_lowercase().cmp(&b.to_lowercase()),
                                (Some(_), None) => std::cmp::Ordering::Less,
                                (None, Some(_)) => std::cmp::Ordering::Greater,
                                (None, None) => std::cmp::Ordering::Equal,
                            });
                        } else {
                            let indices: Vec<usize> = self
                                .storage
                                .projects
                                .iter()
                                .enumerate()
                                .filter(|(_, p)| query.is_empty() || p.name.to_lowercase().contains(&query))
                                .map(|(idx, _)| idx)
                                .collect();
                            group_sections.push((None, indices));
                        }

                        let mut clicked_project_id = None;
                        let mut drag_reorder = None;

                        for (grp_opt, proj_indices) in group_sections {
                            if proj_indices.is_empty() {
                                continue;
                            }
                            if has_groups {
                                let group_title = grp_opt.as_deref().unwrap_or("General");
                                let group_key =
                                    grp_opt.clone().unwrap_or_else(|| "_general".to_string());
                                let is_collapsed =
                                    self.collapsed_project_groups.contains(&group_key);

                                let (grp_rect, grp_resp) =
                                    ui.allocate_exact_size(Vec2::new(full_w, 26.0), Sense::click());
                                let grp_resp = grp_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                                if grp_resp.hovered() {
                                    ui.painter().rect_filled(
                                        grp_rect,
                                        CornerRadius::same(R_SM as u8),
                                        SB_SURFACE_HOVER,
                                    );
                                }

                                let chev = if is_collapsed {
                                    OutlineIcon::ChevronRight
                                } else {
                                    OutlineIcon::ChevronDown
                                };
                                draw_outline_icon(
                                    ui.painter(),
                                    Pos2::new(grp_rect.left() + 8.0, grp_rect.center().y),
                                    chev,
                                    SB_TEXT_FAINT,
                                );
                                ui.painter().text(
                                    Pos2::new(grp_rect.left() + 20.0, grp_rect.center().y),
                                    Align2::LEFT_CENTER,
                                    group_title.to_uppercase(),
                                    FontId::proportional(FS_MICRO),
                                    SB_TEXT_FAINT,
                                );

                                paint_pill_right_aligned(
                                    ui.painter(),
                                    Pos2::new(grp_rect.right() - 4.0, grp_rect.center().y),
                                    &format!("{}", proj_indices.len()),
                                    SB_SURFACE,
                                    SB_TEXT_MUTED,
                                );

                                if grp_resp.clicked() {
                                    if is_collapsed {
                                        self.collapsed_project_groups.remove(&group_key);
                                    } else {
                                        self.collapsed_project_groups.insert(group_key.clone());
                                    }
                                }

                                ui.add_space(3.0);

                                if is_collapsed {
                                    continue;
                                }
                            }

                            for proj_idx in proj_indices {
                                let project = &self.storage.projects[proj_idx];
                                let is_selected = matches!(&self.selection, SidebarSelection::Project(id) if id == &project.id);
                                let is_being_dragged = self.dragged_project_idx == Some(proj_idx);
                                let is_switching = self.project_switch_loading_id.as_deref()
                                    == Some(&project.id);

                                // Project Card: 56px high, radius 8px
                                let (p_rect, p_resp) =
                                    ui.allocate_exact_size(Vec2::new(full_w, 56.0), Sense::click());
                                let p_resp =
                                    p_resp.on_hover_cursor(egui::CursorIcon::PointingHand);

                                let fill = if is_selected || is_switching {
                                    Color32::from_rgb(17, 30, 56)
                                } else if p_resp.hovered() {
                                    SB_SURFACE_HOVER
                                } else {
                                    SB_SURFACE
                                };
                                if is_being_dragged {
                                    let shadow_rect = p_rect.translate(Vec2::new(0.0, 3.0)).expand(1.0);
                                    ui.painter().rect_filled(
                                        shadow_rect,
                                        CornerRadius::same(8),
                                        Color32::from_rgba_premultiplied(0, 0, 0, 80),
                                    );
                                }
                                ui.painter()
                                    .rect_filled(p_rect, CornerRadius::same(8), fill);

                                // Drag handle on left
                                let grip_rect = Rect::from_min_size(
                                    Pos2::new(p_rect.left() + 2.0, p_rect.top() + 6.0),
                                    Vec2::new(14.0, 44.0),
                                );
                                let grip_resp = ui
                                    .interact(
                                        grip_rect,
                                        egui::Id::new(("sidebar_proj_grip", &project.id)),
                                        Sense::drag(),
                                    )
                                    .on_hover_cursor(egui::CursorIcon::Grab);

                                if grip_resp.drag_started() || grip_resp.dragged() {
                                    self.dragged_project_idx = Some(proj_idx);
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                                }

                                let grip_color = if is_being_dragged || grip_resp.hovered() {
                                    SB_ACCENT
                                } else if p_resp.hovered() {
                                    SB_TEXT_FAINT
                                } else {
                                    Color32::TRANSPARENT
                                };
                                draw_outline_icon(
                                    ui.painter(),
                                    grip_rect.center(),
                                    OutlineIcon::GripVertical,
                                    grip_color,
                                );

                                // Reordering drop preview & animation
                                if let Some(from_idx) = self.dragged_project_idx {
                                    if from_idx != proj_idx {
                                        if let Some(pos) = pointer_pos {
                                            if p_rect.contains(pos) {
                                                let is_top_half = pos.y < p_rect.center().y;
                                                let line_y = if is_top_half {
                                                    p_rect.top() - 2.0
                                                } else {
                                                    p_rect.bottom() + 2.0
                                                };
                                                let insert_line = [
                                                    Pos2::new(p_rect.left() + 6.0, line_y),
                                                    Pos2::new(p_rect.right() - 6.0, line_y),
                                                ];
                                                // Drop insertion glow & line
                                                ui.painter().line_segment(
                                                    insert_line,
                                                    Stroke::new(4.0_f32, SB_ACCENT.gamma_multiply(0.4)),
                                                );
                                                ui.painter().line_segment(
                                                    insert_line,
                                                    Stroke::new(2.5_f32, SB_ACCENT),
                                                );
                                                ui.painter().circle_filled(
                                                    Pos2::new(p_rect.left() + 6.0, line_y),
                                                    3.5,
                                                    SB_ACCENT,
                                                );
                                                ui.painter().rect_filled(
                                                    p_rect,
                                                    CornerRadius::same(8),
                                                    Color32::from_rgba_premultiplied(20, 110, 245, 25),
                                                );
                                                if is_pointer_released {
                                                    drag_reorder = Some((from_idx, proj_idx));
                                                }
                                            }
                                        }
                                    }
                                }

                                // Active project: 1px accent border, 2px left bar
                                let stroke = if is_selected || is_switching {
                                    Stroke::new(1.0_f32, Color32::from_rgb(30, 58, 102))
                                } else {
                                    Stroke::new(1.0_f32, SB_BORDER_LINE)
                                };
                                ui.painter().rect_stroke(
                                    p_rect,
                                    CornerRadius::same(8),
                                    stroke,
                                    StrokeKind::Inside,
                                );

                                if is_selected || is_switching {
                                    let bar_rect = Rect::from_min_max(
                                        Pos2::new(p_rect.left(), p_rect.top() + 8.0),
                                        Pos2::new(p_rect.left() + 2.0, p_rect.bottom() - 8.0),
                                    );
                                    ui.painter().rect_filled(
                                        bar_rect,
                                        CornerRadius { nw: 0, ne: 2, se: 2, sw: 0 },
                                        SB_ACCENT,
                                    );
                                }

                                let content_x = p_rect.left() + 20.0;
                                let is_deploying = self.is_project_deploying(&project.id);

                                // Row 1: Project Name (13px, weight 500, --sb-text)
                                let name_color = SB_TEXT;
                                let avail_name_w = (p_rect.right() - 26.0 - content_x).max(20.0);
                                let mut job = egui::text::LayoutJob::simple_singleline(
                                    project.name.clone(),
                                    FontId::proportional(FS_BASE),
                                    name_color,
                                );
                                job.wrap.max_width = avail_name_w;
                                job.wrap.max_rows = 1;
                                job.wrap.break_anywhere = false;
                                let name_galley = ui.painter().layout_job(job);
                                ui.painter().galley(
                                    Pos2::new(content_x, p_rect.top() + 8.0),
                                    name_galley,
                                    name_color,
                                );

                                // Spinner slot on the right
                                if is_deploying || is_switching {
                                    ui.ctx().request_repaint();
                                    let spinner_center =
                                        Pos2::new(p_rect.right() - 16.0, p_rect.center().y);
                                    let spinner_color = if is_deploying {
                                        SB_ACCENT
                                    } else {
                                        SB_TEXT_FAINT
                                    };
                                    draw_circle_loading_spinner(
                                        ui.painter(),
                                        spinner_center,
                                        6.0,
                                        spinner_color,
                                        ui.input(|i| i.time),
                                    );
                                }

                                let p_resp = if is_deploying {
                                    p_resp.on_hover_text(format!(
                                        "{}\n(A deployment is currently running)",
                                        project.name
                                    ))
                                } else {
                                    p_resp.on_hover_text(&project.name)
                                };

                                if p_resp.secondary_clicked() {
                                    if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
                                        self.sidebar_context_menu = Some(SidebarContextMenuState {
                                            project_id: project.id.clone(),
                                            click_pos: pos,
                                            submenu_open: false,
                                        });
                                    }
                                }

                                // Row 2: Env Badges below (11px mono, radius 4px)
                                let row2_y = p_rect.top() + 30.0;
                                if is_switching {
                                    ui.ctx().request_repaint();
                                    let time = ui.input(|i| i.time);
                                    let pill1 = Rect::from_min_size(
                                        Pos2::new(content_x, row2_y),
                                        Vec2::new(44.0, 18.0),
                                    );
                                    paint_shimmer_rect(ui.painter(), pill1, R_SM, time);
                                    let pill2 = Rect::from_min_size(
                                        Pos2::new(content_x + 50.0, row2_y),
                                        Vec2::new(52.0, 18.0),
                                    );
                                    paint_shimmer_rect(ui.painter(), pill2, R_SM, time);
                                } else if project.environments.is_empty() {
                                    paint_sidebar_env_badge(
                                        ui.painter(),
                                        Pos2::new(content_x, row2_y),
                                        "NO ENVS",
                                        SB_SURFACE,
                                        SB_TEXT_FAINT,
                                    );
                                } else {
                                    let mut pill_x = content_x;
                                    for env in project.environments.iter().take(3) {
                                        let badge = env.badge_label();
                                        let is_env_running = self.is_env_deploying(&project.id, &env.key)
                                            || self.storage.deployments.iter().any(|d| {
                                                d.project_id == project.id
                                                    && d.environment == env.key
                                                    && (d.status == DeploymentStatus::Running
                                                        || self.is_deployment_active(&d.id))
                                            });
                                        let key_lower = env.key.to_lowercase();
                                        let is_prod = key_lower.contains("prod");

                                        let (bg, fg) = if is_prod {
                                            (BADGE_PROD_BG, BADGE_PROD_TEXT)
                                        } else {
                                            (BADGE_DEV_BG, BADGE_DEV_TEXT)
                                        };

                                        let label = if is_env_running {
                                            format!("{badge} Deploying...")
                                        } else if let Some(ok_dep) = self
                                            .storage
                                            .last_succeeded_deployment(&project.id, &env.key)
                                        {
                                            format!("{badge} v{}", ok_dep.version)
                                        } else {
                                            format!("{badge} —")
                                        };

                                        let w = paint_sidebar_env_badge(
                                            ui.painter(),
                                            Pos2::new(pill_x, row2_y),
                                            &label,
                                            bg,
                                            fg,
                                        );
                                        pill_x += w + 6.0;
                                    }
                                }

                                if p_resp.clicked() {
                                    clicked_project_id = Some(project.id.clone());
                                }

                                ui.add_space(6.0);
                            }
                        }

                        if let Some((from, to)) = drag_reorder {
                            self.storage.reorder_project(from, to);
                            self.project_sort_mode = ProjectSortMode::Custom;
                            self.dragged_project_idx = None;
                        } else if is_pointer_released {
                            self.dragged_project_idx = None;
                        }

                        if let Some(pid) = clicked_project_id {
                            self.sidebar_context_menu = None;
                            let was_different = match &self.selection {
                                SidebarSelection::Project(cur) => cur != &pid,
                                SidebarSelection::AllDeployments => true,
                            };
                            self.selection = SidebarSelection::Project(pid.clone());
                            if was_different {
                                self.project_switch_loading_id = Some(pid);
                                self.project_switch_loading_start = Some(Instant::now());
                            }
                            self.refresh_selected_project_metadata(true);
                        }
                    }
                });
        }
    }

    pub(crate) fn render_sidebar_sort_dropdown(&mut self, ctx: &egui::Context, trigger_rect: Rect) {
        if !self.sidebar_sort_dropdown_open {
            return;
        }

        let menu_w = 192.0_f32;
        let menu_h = 4.0 + (32.0 * 4.0) + (3.0 * 2.0) + 4.0; // 142px
        let screen = ctx.screen_rect();

        // Right-align with the trigger so it never clips off the sidebar
        let mut menu_pos = Pos2::new(trigger_rect.right() - menu_w, trigger_rect.bottom() + 4.0);
        // Guard if below screen
        if menu_pos.y + menu_h > screen.bottom() - 8.0 {
            menu_pos.y = (trigger_rect.top() - menu_h - 4.0).max(screen.top() + 8.0);
        }
        let menu_rect = Rect::from_min_size(menu_pos, Vec2::new(menu_w, menu_h));

        // Close on outside click
        if ctx.input(|i| i.pointer.any_pressed()) {
            if let Some(ptr) = ctx.input(|i| i.pointer.latest_pos()) {
                if !menu_rect.contains(ptr) && !trigger_rect.contains(ptr) {
                    self.sidebar_sort_dropdown_open = false;
                    return;
                }
            }
        }

        // Close on Escape or Tab
        if ctx.input(|i| i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Tab)) {
            self.sidebar_sort_dropdown_open = false;
            return;
        }

        // Keyboard navigation: ArrowUp, ArrowDown, Enter
        let modes = [
            ProjectSortMode::Custom,
            ProjectSortMode::NameAsc,
            ProjectSortMode::NameDesc,
            ProjectSortMode::RecentDeploy,
        ];
        let current_idx = modes.iter().position(|m| *m == self.project_sort_mode).unwrap_or(0);
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
            let next_idx = (current_idx + 1) % modes.len();
            self.project_sort_mode = modes[next_idx];
            self.storage.sort_projects(modes[next_idx]);
        } else if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
            let prev_idx = if current_idx == 0 { modes.len() - 1 } else { current_idx - 1 };
            self.project_sort_mode = modes[prev_idx];
            self.storage.sort_projects(modes[prev_idx]);
        } else if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            self.sidebar_sort_dropdown_open = false;
            return;
        }

        egui::Area::new(egui::Id::new("sidebar_sort_dropdown_area"))
            .order(egui::Order::Foreground)
            .fixed_pos(menu_pos)
            .show(ctx, |ui| {
                // Drop shadow 0 12px 32px rgba(0,0,0,0.45)
                let shadow_rect = menu_rect.expand(2.0).translate(Vec2::new(0.0, 4.0));
                ui.painter().rect_filled(
                    shadow_rect,
                    CornerRadius::same(R_MD as u8),
                    Color32::from_rgba_premultiplied(0, 0, 0, 115),
                );

                // Container: min-width 192px, bg --sb-surface, border 1px --sb-border, radius --r-md, padding 4px
                ui.painter().rect_filled(menu_rect, CornerRadius::same(R_MD as u8), SB_SURFACE);
                ui.painter().rect_stroke(
                    menu_rect,
                    CornerRadius::same(R_MD as u8),
                    Stroke::new(1.0_f32, SB_BORDER),
                    StrokeKind::Inside,
                );

                let options = [
                    (ProjectSortMode::Custom, "Custom / Drag"),
                    (ProjectSortMode::NameAsc, "Name (A-Z)"),
                    (ProjectSortMode::NameDesc, "Name (Z-A)"),
                    (ProjectSortMode::RecentDeploy, "Recently Deployed"),
                ];

                for (idx, (mode, label)) in options.iter().enumerate() {
                    let item_y = menu_pos.y + 4.0 + idx as f32 * 34.0;
                    let item_rect = Rect::from_min_size(
                        Pos2::new(menu_pos.x + 4.0, item_y),
                        Vec2::new(menu_w - 8.0, 32.0),
                    );
                    let item_resp = ui.interact(
                        item_rect,
                        egui::Id::new(("sidebar_sort_opt", idx)),
                        Sense::click(),
                    ).on_hover_cursor(egui::CursorIcon::PointingHand);

                    let is_selected = self.project_sort_mode == *mode;
                    let (bg, text_color, check_color) = if is_selected {
                        (PRIMARY_BLUE, Color32::WHITE, Color32::WHITE)
                    } else if item_resp.hovered() {
                        (SB_SURFACE_HOVER, SB_TEXT, SB_TEXT_MUTED)
                    } else {
                        (Color32::TRANSPARENT, SB_TEXT_MUTED, Color32::TRANSPARENT)
                    };

                    ui.painter().rect_filled(item_rect, CornerRadius::same(R_SM as u8), bg);

                    // Text: --fs-small
                    ui.painter().text(
                        Pos2::new(item_rect.left() + 10.0, item_rect.center().y),
                        Align2::LEFT_CENTER,
                        *label,
                        FontId::proportional(FS_SMALL),
                        text_color,
                    );

                    // Selected item: 14px check icon (SVG) on the right
                    if is_selected {
                        let check_pos = Pos2::new(item_rect.right() - 14.0, item_rect.center().y);
                        draw_outline_icon(ui.painter(), check_pos, OutlineIcon::Check, check_color);
                    }

                    if item_resp.clicked() {
                        self.project_sort_mode = *mode;
                        self.storage.sort_projects(*mode);
                        self.sidebar_sort_dropdown_open = false;
                    }
                }
            });
    }

    pub(crate) fn render_sidebar_custom_context_menu(&mut self, ctx: &egui::Context) {
        let state = match &self.sidebar_context_menu {
            Some(s) => s.clone(),
            None => return,
        };

        let project = match self.storage.projects.iter().find(|p| p.id == state.project_id) {
            Some(p) => p.clone(),
            None => {
                self.sidebar_context_menu = None;
                return;
            }
        };

        let total_projects = self.storage.projects.len();
        let proj_idx = self.storage.projects.iter().position(|p| p.id == state.project_id).unwrap_or(0);

        let can_move_up = proj_idx > 0;
        let can_move_down = proj_idx + 1 < total_projects;
        let has_move_sep = can_move_up || can_move_down;

        let existing_groups = self.storage.existing_project_groups();
        let is_set_group_disabled = existing_groups.is_empty() && project.group.is_none();

        // Calculate menu height:
        let mut items_count = 4; // Edit Project, Set Group..., Duplicate Project, Refresh Metadata
        if can_move_up { items_count += 1; }
        if can_move_down { items_count += 1; }
        items_count += 1; // Delete Project

        let seps_count = if has_move_sep { 2 } else { 1 };
        let menu_w = 220.0_f32;
        let menu_h = 8.0 + (items_count as f32 * 32.0) + (seps_count as f32 * 9.0);

        let screen = ctx.screen_rect();
        let mut pos = state.click_pos;

        // Viewport collision flip:
        // If bottom overflow: flip upward!
        if pos.y + menu_h > screen.bottom() - 8.0 {
            pos.y = (pos.y - menu_h).max(screen.top() + 8.0);
        } else {
            pos.y = pos.y.max(screen.top() + 8.0);
        }

        // If right overflow: flip left!
        if pos.x + menu_w > screen.right() - 8.0 {
            pos.x = (pos.x - menu_w).max(screen.left() + 8.0);
        } else {
            pos.x = pos.x.max(screen.left() + 8.0);
        }

        let menu_rect = Rect::from_min_size(pos, Vec2::new(menu_w, menu_h));

        // Submenu geometry (if open):
        let submenu_w = 190.0_f32;
        let submenu_items_count = existing_groups.len() + if project.group.is_some() { 1 } else { 0 };
        let submenu_seps = if project.group.is_some() && !existing_groups.is_empty() { 1 } else { 0 };
        let submenu_h = 8.0 + (submenu_items_count as f32 * 32.0) + (submenu_seps as f32 * 9.0);

        let sub_x = if menu_rect.right() + 4.0 + submenu_w <= screen.right() - 8.0 {
            menu_rect.right() + 4.0
        } else {
            (menu_rect.left() - 4.0 - submenu_w).max(screen.left() + 8.0)
        };
        // Align submenu y to Set Group item
        let set_group_y_offset = 4.0 + (if can_move_up { 32.0 } else { 0.0 })
            + (if can_move_down { 32.0 } else { 0.0 })
            + (if has_move_sep { 9.0 } else { 0.0 })
            + 32.0; // after Edit Project
        let mut sub_y = pos.y + set_group_y_offset;
        if sub_y + submenu_h > screen.bottom() - 8.0 {
            sub_y = (screen.bottom() - 8.0 - submenu_h).max(screen.top() + 8.0);
        }
        let submenu_rect = Rect::from_min_size(Pos2::new(sub_x, sub_y), Vec2::new(submenu_w, submenu_h));

        // Outside click handling:
        if ctx.input(|i| i.pointer.any_pressed()) {
            if let Some(ptr) = ctx.input(|i| i.pointer.latest_pos()) {
                let in_main = menu_rect.contains(ptr);
                let in_sub = state.submenu_open && submenu_rect.contains(ptr);
                if !in_main && !in_sub {
                    self.sidebar_context_menu = None;
                    return;
                }
            }
        }

        // Close on Escape
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.sidebar_context_menu = None;
            return;
        }

        // Close on scroll
        if ctx.input(|i| i.raw_scroll_delta.length_sq() > 0.0) {
            self.sidebar_context_menu = None;
            return;
        }

        let mut next_submenu_open = state.submenu_open;
        let mut action_move_up = false;
        let mut action_move_down = false;
        let mut action_edit = false;
        let mut action_duplicate = false;
        let mut action_refresh = false;
        let mut action_delete = false;
        let mut action_set_group = None;

        egui::Area::new(egui::Id::new("sidebar_custom_context_menu_area"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                // Drop shadow
                let shadow_rect = menu_rect.expand(2.0).translate(Vec2::new(0.0, 4.0));
                ui.painter().rect_filled(
                    shadow_rect,
                    CornerRadius::same(R_MD as u8),
                    Color32::from_rgba_premultiplied(0, 0, 0, 115),
                );

                // Container
                ui.painter().rect_filled(menu_rect, CornerRadius::same(R_MD as u8), SB_SURFACE);
                ui.painter().rect_stroke(
                    menu_rect,
                    CornerRadius::same(R_MD as u8),
                    Stroke::new(1.0_f32, SB_BORDER),
                    StrokeKind::Inside,
                );

                let mut cur_y = menu_rect.top() + 4.0;

                // Move Up
                if can_move_up {
                    let r = Rect::from_min_size(Pos2::new(menu_rect.left() + 4.0, cur_y), Vec2::new(menu_w - 8.0, 32.0));
                    let resp = ui.interact(r, egui::Id::new("ctx_move_up"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                    let bg = if resp.hovered() { SB_SURFACE_HOVER } else { Color32::TRANSPARENT };
                    ui.painter().rect_filled(r, CornerRadius::same(R_SM as u8), bg);
                    draw_outline_icon(ui.painter(), Pos2::new(r.left() + 16.0, r.center().y), OutlineIcon::ArrowUp, SB_TEXT);
                    ui.painter().text(Pos2::new(r.left() + 32.0, r.center().y), Align2::LEFT_CENTER, "Move Up", FontId::proportional(FS_SMALL), SB_TEXT);
                    if resp.clicked() { action_move_up = true; }
                    cur_y += 32.0;
                }

                // Move Down
                if can_move_down {
                    let r = Rect::from_min_size(Pos2::new(menu_rect.left() + 4.0, cur_y), Vec2::new(menu_w - 8.0, 32.0));
                    let resp = ui.interact(r, egui::Id::new("ctx_move_down"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                    let bg = if resp.hovered() { SB_SURFACE_HOVER } else { Color32::TRANSPARENT };
                    ui.painter().rect_filled(r, CornerRadius::same(R_SM as u8), bg);
                    draw_outline_icon(ui.painter(), Pos2::new(r.left() + 16.0, r.center().y), OutlineIcon::ArrowDown, SB_TEXT);
                    ui.painter().text(Pos2::new(r.left() + 32.0, r.center().y), Align2::LEFT_CENTER, "Move Down", FontId::proportional(FS_SMALL), SB_TEXT);
                    if resp.clicked() { action_move_down = true; }
                    cur_y += 32.0;
                }

                if has_move_sep {
                    cur_y += 4.0;
                    ui.painter().line_segment(
                        [Pos2::new(menu_rect.left() + 8.0, cur_y), Pos2::new(menu_rect.right() - 8.0, cur_y)],
                        Stroke::new(1.0_f32, SB_BORDER),
                    );
                    cur_y += 5.0;
                }

                // Edit Project
                {
                    let r = Rect::from_min_size(Pos2::new(menu_rect.left() + 4.0, cur_y), Vec2::new(menu_w - 8.0, 32.0));
                    let resp = ui.interact(r, egui::Id::new("ctx_edit"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                    let bg = if resp.hovered() { SB_SURFACE_HOVER } else { Color32::TRANSPARENT };
                    ui.painter().rect_filled(r, CornerRadius::same(R_SM as u8), bg);
                    draw_outline_icon(ui.painter(), Pos2::new(r.left() + 16.0, r.center().y), OutlineIcon::Edit, SB_TEXT);
                    ui.painter().text(Pos2::new(r.left() + 32.0, r.center().y), Align2::LEFT_CENTER, "Edit Project", FontId::proportional(FS_SMALL), SB_TEXT);
                    if resp.clicked() { action_edit = true; }
                    cur_y += 32.0;
                }

                // Set Group...
                {
                    let r = Rect::from_min_size(Pos2::new(menu_rect.left() + 4.0, cur_y), Vec2::new(menu_w - 8.0, 32.0));
                    let resp = if is_set_group_disabled {
                        ui.interact(r, egui::Id::new("ctx_set_group"), Sense::hover())
                            .on_hover_cursor(egui::CursorIcon::NotAllowed)
                            .on_hover_text("No existing groups. Assign groups in Edit Project.")
                    } else {
                        ui.interact(r, egui::Id::new("ctx_set_group"), Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                    };

                    let bg = if !is_set_group_disabled && (resp.hovered() || state.submenu_open) {
                        SB_SURFACE_HOVER
                    } else {
                        Color32::TRANSPARENT
                    };
                    ui.painter().rect_filled(r, CornerRadius::same(R_SM as u8), bg);

                    let fg = if is_set_group_disabled {
                        Color32::from_rgba_premultiplied(55, 60, 69, 102) // opacity 0.4
                    } else {
                        SB_TEXT
                    };

                    draw_outline_icon(ui.painter(), Pos2::new(r.left() + 16.0, r.center().y), OutlineIcon::Folder, fg);
                    ui.painter().text(Pos2::new(r.left() + 32.0, r.center().y), Align2::LEFT_CENTER, "Set Group...", FontId::proportional(FS_SMALL), fg);
                    draw_outline_icon(ui.painter(), Pos2::new(r.right() - 14.0, r.center().y), OutlineIcon::ChevronRight, fg);

                    if !is_set_group_disabled && (resp.hovered() || resp.clicked()) {
                        next_submenu_open = true;
                    }
                    cur_y += 32.0;
                }

                // Duplicate Project
                {
                    let r = Rect::from_min_size(Pos2::new(menu_rect.left() + 4.0, cur_y), Vec2::new(menu_w - 8.0, 32.0));
                    let resp = ui.interact(r, egui::Id::new("ctx_dup"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                    let bg = if resp.hovered() { SB_SURFACE_HOVER } else { Color32::TRANSPARENT };
                    ui.painter().rect_filled(r, CornerRadius::same(R_SM as u8), bg);
                    draw_outline_icon(ui.painter(), Pos2::new(r.left() + 16.0, r.center().y), OutlineIcon::Copy, SB_TEXT);
                    ui.painter().text(Pos2::new(r.left() + 32.0, r.center().y), Align2::LEFT_CENTER, "Duplicate Project", FontId::proportional(FS_SMALL), SB_TEXT);
                    if resp.clicked() { action_duplicate = true; }
                    cur_y += 32.0;
                }

                // Refresh Metadata
                {
                    let r = Rect::from_min_size(Pos2::new(menu_rect.left() + 4.0, cur_y), Vec2::new(menu_w - 8.0, 32.0));
                    let resp = ui.interact(r, egui::Id::new("ctx_refresh"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                    let bg = if resp.hovered() { SB_SURFACE_HOVER } else { Color32::TRANSPARENT };
                    let fg = if resp.hovered() { SB_TEXT } else { SB_TEXT_MUTED };
                    ui.painter().rect_filled(r, CornerRadius::same(R_SM as u8), bg);
                    draw_outline_icon(ui.painter(), Pos2::new(r.left() + 16.0, r.center().y), OutlineIcon::Refresh, fg);
                    ui.painter().text(Pos2::new(r.left() + 32.0, r.center().y), Align2::LEFT_CENTER, "Refresh Metadata", FontId::proportional(FS_SMALL), fg);
                    if resp.clicked() { action_refresh = true; }
                    cur_y += 32.0;
                }

                // Separator before Delete
                cur_y += 4.0;
                ui.painter().line_segment(
                    [Pos2::new(menu_rect.left() + 8.0, cur_y), Pos2::new(menu_rect.right() - 8.0, cur_y)],
                    Stroke::new(1.0_f32, SB_BORDER),
                );
                cur_y += 5.0;

                // Delete Project
                {
                    let r = Rect::from_min_size(Pos2::new(menu_rect.left() + 4.0, cur_y), Vec2::new(menu_w - 8.0, 32.0));
                    let resp = ui.interact(r, egui::Id::new("ctx_delete"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                    let bg = if resp.is_pointer_button_down_on() {
                        DESTRUCTIVE_ACTIVE_BG
                    } else if resp.hovered() {
                        DESTRUCTIVE_HOVER_BG
                    } else {
                        Color32::TRANSPARENT
                    };
                    ui.painter().rect_filled(r, CornerRadius::same(R_SM as u8), bg);
                    draw_outline_icon(ui.painter(), Pos2::new(r.left() + 16.0, r.center().y), OutlineIcon::Trash, DESTRUCTIVE_TEXT);
                    ui.painter().text(Pos2::new(r.left() + 32.0, r.center().y), Align2::LEFT_CENTER, "Delete Project", FontId::proportional(FS_SMALL), DESTRUCTIVE_TEXT);
                    if resp.clicked() { action_delete = true; }
                }
            });

        // Submenu for Set Group...
        if state.submenu_open && !is_set_group_disabled {
            egui::Area::new(egui::Id::new("sidebar_custom_submenu_area"))
                .order(egui::Order::Foreground)
                .fixed_pos(Pos2::new(sub_x, sub_y))
                .show(ctx, |ui| {
                    // Submenu drop shadow
                    let shadow_rect = submenu_rect.expand(2.0).translate(Vec2::new(0.0, 4.0));
                    ui.painter().rect_filled(
                        shadow_rect,
                        CornerRadius::same(R_MD as u8),
                        Color32::from_rgba_premultiplied(0, 0, 0, 115),
                    );

                    // Submenu container
                    ui.painter().rect_filled(submenu_rect, CornerRadius::same(R_MD as u8), SB_SURFACE);
                    ui.painter().rect_stroke(
                        submenu_rect,
                        CornerRadius::same(R_MD as u8),
                        Stroke::new(1.0_f32, SB_BORDER),
                        StrokeKind::Inside,
                    );

                    let mut sub_cur_y = submenu_rect.top() + 4.0;
                    for (g_idx, grp) in existing_groups.iter().enumerate() {
                        let r = Rect::from_min_size(Pos2::new(submenu_rect.left() + 4.0, sub_cur_y), Vec2::new(submenu_w - 8.0, 32.0));
                        let resp = ui.interact(r, egui::Id::new(("ctx_sub_grp", g_idx)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                        let is_cur = project.group.as_deref() == Some(grp.as_str());
                        let bg = if resp.hovered() { SB_SURFACE_HOVER } else if is_cur { ACCENT_SOFT } else { Color32::TRANSPARENT };
                        let fg = if is_cur { ACCENT } else { SB_TEXT };

                        ui.painter().rect_filled(r, CornerRadius::same(R_SM as u8), bg);
                        if is_cur {
                            draw_outline_icon(ui.painter(), Pos2::new(r.left() + 16.0, r.center().y), OutlineIcon::Check, ACCENT);
                        }
                        ui.painter().text(Pos2::new(r.left() + 32.0, r.center().y), Align2::LEFT_CENTER, grp, FontId::proportional(FS_SMALL), fg);
                        if resp.clicked() {
                            action_set_group = Some(Some(grp.clone()));
                        }
                        sub_cur_y += 32.0;
                    }

                    if project.group.is_some() {
                        if !existing_groups.is_empty() {
                            sub_cur_y += 4.0;
                            ui.painter().line_segment(
                                [Pos2::new(submenu_rect.left() + 8.0, sub_cur_y), Pos2::new(submenu_rect.right() - 8.0, sub_cur_y)],
                                Stroke::new(1.0_f32, SB_BORDER),
                            );
                            sub_cur_y += 5.0;
                        }

                        let r = Rect::from_min_size(Pos2::new(submenu_rect.left() + 4.0, sub_cur_y), Vec2::new(submenu_w - 8.0, 32.0));
                        let resp = ui.interact(r, egui::Id::new("ctx_sub_remove_grp"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                        let bg = if resp.hovered() { SB_SURFACE_HOVER } else { Color32::TRANSPARENT };
                        ui.painter().rect_filled(r, CornerRadius::same(R_SM as u8), bg);
                        draw_outline_icon(ui.painter(), Pos2::new(r.left() + 16.0, r.center().y), OutlineIcon::X, SB_TEXT_MUTED);
                        ui.painter().text(Pos2::new(r.left() + 32.0, r.center().y), Align2::LEFT_CENTER, "Remove from Group", FontId::proportional(FS_SMALL), SB_TEXT_MUTED);
                        if resp.clicked() {
                            action_set_group = Some(None);
                        }
                    }
                });
        }

        if let Some(ref mut cur_state) = self.sidebar_context_menu {
            cur_state.submenu_open = next_submenu_open;
        }

        // Execute any clicked action
        if action_move_up {
            self.storage.move_project_up(proj_idx);
            self.project_sort_mode = ProjectSortMode::Custom;
            self.sidebar_context_menu = None;
        } else if action_move_down {
            self.storage.move_project_down(proj_idx);
            self.project_sort_mode = ProjectSortMode::Custom;
            self.sidebar_context_menu = None;
        } else if action_edit {
            self.open_edit_project_modal(proj_idx);
            self.sidebar_context_menu = None;
        } else if let Some(grp_opt) = action_set_group {
            self.storage.set_project_group(&project.id, grp_opt);
            self.sidebar_context_menu = None;
        } else if action_duplicate {
            if let Some(new_proj) = self.storage.duplicate_project(&project.id) {
                self.selection = SidebarSelection::Project(new_proj.id.clone());
                self.refresh_selected_project_metadata(true);
                self.status_banner = Some((
                    format!("Duplicated project successfully as '{}'", new_proj.name),
                    true,
                ));
            }
            self.sidebar_context_menu = None;
        } else if action_refresh {
            self.selection = SidebarSelection::Project(project.id.clone());
            self.refresh_selected_project_metadata(true);
            self.sidebar_context_menu = None;
        } else if action_delete {
            self.pending_delete_confirm = Some(ConfirmDeleteAction::Project {
                project_id: project.id.clone(),
                project_name: project.name.clone(),
            });
            self.sidebar_context_menu = None;
        }
    }

}
