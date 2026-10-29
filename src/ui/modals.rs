use super::*;

/// Prepares a modal with smooth opacity fade, slide translateY entrance, backdrop click-to-dismiss, and Escape key handling.
/// Returns `Some(anim_t)` if the modal is currently open or animating, or `None` if completely closed.
pub(crate) fn prepare_animated_modal(
    ctx: &egui::Context,
    modal_id: &str,
    is_open: &mut bool,
) -> Option<f32> {
    let anim_t = ctx.animate_bool(egui::Id::new(("anim_modal", modal_id)), *is_open);
    if anim_t <= 0.005 {
        return None;
    }

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        *is_open = false;
        return None;
    }

    let overlay_color = Color32::from_rgba_premultiplied(0, 0, 0, (180.0 * anim_t) as u8);
    egui::Area::new(egui::Id::new(("modal_backdrop", modal_id)))
        .order(egui::Order::Middle)
        .fixed_pos(Pos2::ZERO)
        .show(ctx, |ui| {
            let screen_rect = ctx.screen_rect();
            let resp = ui.allocate_rect(screen_rect, Sense::click());
            ui.painter().rect_filled(screen_rect, CornerRadius::ZERO, overlay_color);
            if resp.clicked() {
                *is_open = false;
            }
        });

    if !*is_open {
        return None;
    }

    Some(anim_t)
}

/// Constructs a standardized modal window with responsive width, frameless dialog styling, and vertical slide offset.
pub(crate) fn animated_modal_window(
    ctx: &egui::Context,
    title: &str,
    anim_t: f32,
    frame_override: Option<egui::Frame>,
) -> (egui::Window<'static>, f32) {
    let (modal_w, inner_w) = modal_width_and_inner(ctx);
    let slide_y = (1.0 - anim_t) * -18.0;
    let frame = frame_override.unwrap_or_else(|| modal_dialog_frame_for(ctx));
    let window = egui::Window::new(title.to_string())
        .order(egui::Order::Foreground)
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::new(0.0, slide_y))
        .fixed_size(Vec2::new(modal_w, 0.0))
        .frame(frame);
    (window, inner_w)
}
impl ShiplyApp {
    pub fn open_new_project_modal(&mut self) {
        self.new_project_ps1_path.clear();
        self.new_project_name.clear();
        self.new_project_path.clear();
        self.new_project_desc.clear();
        self.new_project_group.clear();
        self.new_project_imported_envs.clear();
        self.new_project_import_notice = None;
        self.show_new_project_modal = true;
    }

    pub(crate) fn render_new_project_modal(&mut self, ctx: &egui::Context) {
        let Some(anim_t) = prepare_animated_modal(ctx, "new_project", &mut self.show_new_project_modal) else {
            return;
        };
        let (window, inner_w) = animated_modal_window(ctx, "Add Project", anim_t, None);
        window.show(ctx, |ui| {
            ui.set_width(inner_w);

                if render_modal_header(
                    ui,
                    "close_new_proj_modal",
                    "Add New Project",
                    "Auto-import a PowerShell deployment script (.ps1) to populate environments and steps, or configure manually.",
                ) {
                    self.show_new_project_modal = false;
                }

                ui.add_space(SP_16);

                // Auto-Import .ps1 Card Box
                egui::Frame::new()
                    .fill(MD_PANEL_BG)
                    .stroke(Stroke::new(1.0_f32, MD_BORDER))
                    .corner_radius(R_LG)
                    .inner_margin(Margin::same(16))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("AUTO-IMPORT FROM POWERSHELL SCRIPT (.PS1)")
                                .size(FS_MICRO)
                                .strong()
                                .color(ACCENT),
                        );
                        ui.add_space(SP_8);
                        horizontal_row(ui, 40.0, |ui| {
                            modal_text_input(
                                ui,
                                "new_project_ps1_path",
                                &mut self.new_project_ps1_path,
                                264.0,
                                Some(r"e.g. C:\Projects\my-app\deploy-dev.ps1"),
                            );
                            ui.add_space(SP_8);
                            let browse_btn = modal_secondary_button("Browse .ps1...");
                            if ui.add(browse_btn).clicked() {
                                if let Some(picked) = pick_ps1_file_dialog() {
                                    self.new_project_ps1_path = picked.clone();
                                    self.apply_ps1_auto_import_to_modal(&picked);
                                }
                            }
                            ui.add_space(SP_8);
                            let can_autofill = !self.new_project_ps1_path.trim().is_empty();
                            let parse_btn = modal_primary_button("Auto-Fill", can_autofill);
                            if ui.add(parse_btn).clicked() && can_autofill {
                                let path_copy = self.new_project_ps1_path.clone();
                                self.apply_ps1_auto_import_to_modal(&path_copy);
                            }
                        });

                        ui.add_space(SP_8);
                        if ui
                            .checkbox(
                                &mut self.new_project_import_siblings,
                                RichText::new(
                                    "Also auto-import sibling deploy-*.ps1 scripts in the same project folder",
                                )
                                .size(FS_SMALL)
                                .color(MD_TEXT_MUTED),
                            )
                            .changed()
                            && !self.new_project_ps1_path.trim().is_empty()
                        {
                            let path_copy = self.new_project_ps1_path.clone();
                            self.apply_ps1_auto_import_to_modal(&path_copy);
                        }

                        if let Some(notice) = &self.new_project_import_notice {
                            ui.add_space(SP_8);
                            let is_err = notice.starts_with("Import error:");
                            ui.label(
                                RichText::new(notice)
                                    .size(FS_SMALL)
                                    .strong()
                                    .color(if is_err {
                                        FAIL_PILL_TEXT
                                    } else {
                                        SUCCESS_PILL_TEXT
                                    }),
                            );
                        }
                    });

                ui.add_space(SP_16);

                // Project Form Fields (vertical gap 16px, label FS_BASE weight 500, margin-bottom 8px)
                ui.label(
                    RichText::new("Project Name")
                        .size(FS_BASE)
                        .strong()
                        .color(MD_TEXT),
                );
                ui.add_space(SP_8);
                modal_text_input(
                    ui,
                    "new_project_name",
                    &mut self.new_project_name,
                    512.0,
                    Some("e.g. my-web-service"),
                );

                ui.add_space(SP_16);
                ui.label(
                    RichText::new("Local Project Directory")
                        .size(FS_BASE)
                        .strong()
                        .color(MD_TEXT),
                );
                ui.add_space(SP_8);
                horizontal_row(ui, 40.0, |ui| {
                    modal_text_input(
                        ui,
                        "new_project_path",
                        &mut self.new_project_path,
                        360.0,
                        Some(r"e.g. C:\Projects\my-web-service"),
                    );
                    ui.add_space(SP_8);
                    let folder_btn = modal_secondary_button("Browse Folder...");
                    if ui.add(folder_btn).clicked() {
                        if let Some(folder) = pick_folder_dialog() {
                            self.new_project_path = folder.clone();
                            if self.new_project_name.trim().is_empty() {
                                self.apply_ps1_auto_import_to_modal(&folder);
                            }
                        }
                    }
                });

                ui.add_space(SP_16);
                ui.label(
                    RichText::new("Description")
                        .size(FS_BASE)
                        .strong()
                        .color(MD_TEXT),
                );
                ui.add_space(SP_8);
                modal_text_input(
                    ui,
                    "new_project_desc",
                    &mut self.new_project_desc,
                    512.0,
                    Some("e.g. Backend API service (Node.js / PM2)"),
                );

                ui.add_space(SP_16);
                ui.label(
                    RichText::new("Group (Optional)")
                        .size(FS_BASE)
                        .strong()
                        .color(MD_TEXT),
                );
                ui.add_space(SP_8);
                modal_text_input(
                    ui,
                    "new_project_group",
                    &mut self.new_project_group,
                    512.0,
                    Some("e.g. Backend, Frontend, Microservices..."),
                );
                let existing_groups = self.storage.existing_project_groups();
                if !existing_groups.is_empty() {
                    ui.add_space(SP_8);
                    horizontal_row(ui, 24.0, |ui| {
                        ui.label(RichText::new("Quick Select:").size(FS_MICRO).color(MD_TEXT_MUTED));
                        for grp in existing_groups {
                            let is_sel = self.new_project_group == grp;
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(&grp).size(FS_MICRO).strong().color(if is_sel {
                                            Color32::WHITE
                                        } else {
                                            MD_TEXT
                                        }),
                                    )
                                    .fill(if is_sel { ACCENT } else { MD_PANEL_BG })
                                    .stroke(Stroke::new(1.0_f32, if is_sel { ACCENT } else { MD_BORDER }))
                                    .corner_radius(R_SM),
                                )
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                self.new_project_group = grp;
                            }
                        }
                    });
                }

                // Footer (top border 1px MD_BORDER, padding-top 16px, margin-top 24px)
                ui.add_space(SP_24);
                draw_divider(ui, MD_BORDER);
                ui.add_space(SP_16);

                horizontal_row(ui, 40.0, |ui| {
                    let env_count = self.new_project_imported_envs.len();
                    ui.label(
                        RichText::new(if env_count > 0 {
                            format!("{env_count} environment(s) ready to import")
                        } else {
                            "Starts empty (you can add or import environments anytime)".to_string()
                        })
                        .size(FS_MICRO)
                        .color(MD_TEXT_MUTED),
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let can_create = !self.new_project_name.trim().is_empty()
                            && !self.new_project_path.trim().is_empty();
                        let create_btn = modal_primary_button("Create Project", can_create);

                        let enter_submitted = can_create && ctx.input(|i| i.key_pressed(egui::Key::Enter));
                        if (ui.add(create_btn).clicked() || enter_submitted) && can_create {
                            let slug = crate::domain::normalize_env_key(&self.new_project_name);
                            let proj = Project {
                                id: if slug.is_empty() {
                                    Uuid::new_v4().to_string()
                                } else {
                                    slug
                                },
                                name: self.new_project_name.trim().to_string(),
                                description: if self.new_project_desc.trim().is_empty() {
                                    "Custom deployment project".to_string()
                                } else {
                                    self.new_project_desc.trim().to_string()
                                },
                                local_path: self.new_project_path.trim().to_string(),
                                environments: self.new_project_imported_envs.clone(),
                                group: if self.new_project_group.trim().is_empty() {
                                    None
                                } else {
                                    Some(self.new_project_group.trim().to_string())
                                },
                                sort_order: Some(self.storage.projects.len() as i32),
                            };
                            let id = proj.id.clone();
                            let first_env = proj
                                .environments
                                .first()
                                .map(|e| e.key.clone())
                                .unwrap_or_default();
                            self.storage.projects.push(proj);
                            let _ = self.storage.save();
                            self.selection = SidebarSelection::Project(id);
                            self.selected_env_key = first_env;
                            self.refresh_selected_project_metadata(true);

                            self.new_project_ps1_path.clear();
                            self.new_project_name.clear();
                            self.new_project_path.clear();
                            self.new_project_desc.clear();
                            self.new_project_group.clear();
                            self.new_project_imported_envs.clear();
                            self.new_project_import_notice = None;
                            self.show_new_project_modal = false;
                        }

                        ui.add_space(SP_8);
                        let cancel_btn = modal_secondary_button("Cancel");
                        if ui.add(cancel_btn).clicked() {
                            self.show_new_project_modal = false;
                        }
                    });
                });
            });
    }

    pub(crate) fn open_edit_project_modal(&mut self, proj_idx: usize) {
        if let Some(proj) = self.storage.projects.get(proj_idx) {
            self.edit_project_id = proj.id.clone();
            self.edit_project_name = proj.name.clone();
            self.edit_project_path = proj.local_path.clone();
            self.edit_project_desc = proj.description.clone();
            self.edit_project_group = proj.group.clone().unwrap_or_default();
            self.edit_project_ps1_path.clear();
            self.edit_project_import_notice = None;
            self.show_edit_project_modal = true;
        }
    }

    pub(crate) fn render_edit_project_modal(&mut self, ctx: &egui::Context) {
        let Some(proj_idx) = self
            .storage
            .projects
            .iter()
            .position(|p| p.id == self.edit_project_id)
        else {
            self.show_edit_project_modal = false;
            return;
        };

        let Some(anim_t) = prepare_animated_modal(ctx, "edit_project", &mut self.show_edit_project_modal) else {
            return;
        };
        let (window, inner_w) = animated_modal_window(ctx, "Edit Project", anim_t, None);
        window.show(ctx, |ui| {
            ui.set_width(inner_w);

                if render_modal_header(
                    ui,
                    "close_edit_proj_modal",
                    "Edit Project Settings",
                    "Update the project name, local directory path, description, or sync environments from a .ps1 script.",
                ) {
                    self.show_edit_project_modal = false;
                }

                ui.add_space(SP_16);

                // Optional: Import/Sync additional .ps1 script into this project
                egui::Frame::new()
                    .fill(MD_PANEL_BG)
                    .stroke(Stroke::new(1.0_f32, MD_BORDER))
                    .corner_radius(R_LG)
                    .inner_margin(Margin::same(16))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("SYNC / IMPORT ENVIRONMENT FROM .PS1 SCRIPT")
                                .size(FS_MICRO)
                                .strong()
                                .color(ACCENT),
                        );
                        ui.add_space(SP_8);
                        horizontal_row(ui, 40.0, |ui| {
                            modal_text_input(
                                ui,
                                "edit_project_ps1_path",
                                &mut self.edit_project_ps1_path,
                                272.0,
                                Some(r"e.g. C:\Projects\my-app\deploy-prod.ps1"),
                            );
                            ui.add_space(SP_8);
                            let browse_btn = modal_secondary_button("Browse .ps1...");
                            if ui.add(browse_btn).clicked() {
                                if let Some(picked) = pick_ps1_file_dialog() {
                                    self.edit_project_ps1_path = picked;
                                }
                            }
                            ui.add_space(SP_8);
                            let can_import = !self.edit_project_ps1_path.trim().is_empty();
                            let imp_btn = modal_primary_button("Import", can_import);
                            if ui.add_enabled(can_import, imp_btn).clicked() && can_import {
                                let p = PathBuf::from(
                                    self.edit_project_ps1_path.trim().trim_matches('"'),
                                );
                                match parse_ps1_file(&p) {
                                    Ok(env_cfg) => {
                                        let env_name = env_cfg.name.clone();
                                        let proj = &mut self.storage.projects[proj_idx];
                                        if let Some(existing) = proj.get_env_mut(&env_cfg.key) {
                                            *existing = env_cfg;
                                        } else {
                                            proj.environments.push(env_cfg);
                                        }
                                        let _ = self.storage.save();
                                        self.edit_project_import_notice = Some(format!(
                                            "Synced environment '{env_name}' from script."
                                        ));
                                    }
                                    Err(err) => {
                                        self.edit_project_import_notice =
                                            Some(format!("Import error: {err}"));
                                    }
                                }
                            }
                        });
                        if let Some(notice) = &self.edit_project_import_notice {
                            ui.add_space(SP_8);
                            let is_err = notice.starts_with("Import error:");
                            ui.label(
                                RichText::new(notice)
                                    .size(FS_SMALL)
                                    .strong()
                                    .color(if is_err {
                                        FAIL_PILL_TEXT
                                    } else {
                                        SUCCESS_PILL_TEXT
                                    }),
                            );
                        }
                    });

                ui.add_space(SP_16);

                // Project Form Fields (vertical gap 16px, label FS_BASE weight 500, margin-bottom 8px)
                ui.label(
                    RichText::new("Project Name")
                        .size(FS_BASE)
                        .strong()
                        .color(MD_TEXT),
                );
                ui.add_space(SP_8);
                modal_text_input(
                    ui,
                    "edit_project_name",
                    &mut self.edit_project_name,
                    512.0,
                    Some("e.g. my-web-service"),
                );

                ui.add_space(SP_16);
                ui.label(
                    RichText::new("Local Project Directory")
                        .size(FS_BASE)
                        .strong()
                        .color(MD_TEXT),
                );
                ui.add_space(SP_8);
                horizontal_row(ui, 40.0, |ui| {
                    modal_text_input(
                        ui,
                        "edit_project_path",
                        &mut self.edit_project_path,
                        360.0,
                        Some(r"e.g. C:\Projects\my-web-service"),
                    );
                    ui.add_space(SP_8);
                    let folder_btn = modal_secondary_button("Browse Folder...");
                    if ui.add(folder_btn).clicked() {
                        if let Some(folder) = pick_folder_dialog() {
                            self.edit_project_path = folder;
                        }
                    }
                });

                ui.add_space(SP_16);
                ui.label(
                    RichText::new("Description")
                        .size(FS_BASE)
                        .strong()
                        .color(MD_TEXT),
                );
                ui.add_space(SP_8);
                modal_text_input(
                    ui,
                    "edit_project_desc",
                    &mut self.edit_project_desc,
                    512.0,
                    Some("e.g. Backend API service (Node.js / PM2)"),
                );

                ui.add_space(SP_16);
                ui.label(
                    RichText::new("Group (Optional)")
                        .size(FS_BASE)
                        .strong()
                        .color(MD_TEXT),
                );
                ui.add_space(SP_8);
                modal_text_input(
                    ui,
                    "edit_project_group",
                    &mut self.edit_project_group,
                    512.0,
                    Some("e.g. Backend, Frontend, Microservices..."),
                );
                let existing_groups = self.storage.existing_project_groups();
                if !existing_groups.is_empty() {
                    ui.add_space(SP_8);
                    horizontal_row(ui, 24.0, |ui| {
                        ui.label(RichText::new("Quick Select:").size(FS_MICRO).color(MD_TEXT_MUTED));
                        for grp in existing_groups {
                            let is_sel = self.edit_project_group == grp;
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(&grp).size(FS_MICRO).strong().color(if is_sel {
                                            Color32::WHITE
                                        } else {
                                            MD_TEXT
                                        }),
                                    )
                                    .fill(if is_sel { ACCENT } else { MD_PANEL_BG })
                                    .stroke(Stroke::new(
                                        1.0_f32,
                                        if is_sel { ACCENT } else { MD_BORDER },
                                    ))
                                    .corner_radius(R_SM),
                                )
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                self.edit_project_group = grp;
                            }
                        }
                    });
                }

                // Footer (top border 1px MD_BORDER, padding-top 16px, margin-top 24px)
                ui.add_space(SP_24);
                draw_divider(ui, MD_BORDER);
                ui.add_space(SP_16);

                let enter_pressed = ctx.input(|i| i.key_pressed(egui::Key::Enter));
                horizontal_row(ui, 40.0, |ui| {
                    ui.label(
                        RichText::new("Changes are saved to local database")
                            .size(FS_MICRO)
                            .color(MD_TEXT_MUTED),
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let can_save = !self.edit_project_name.trim().is_empty()
                            && !self.edit_project_path.trim().is_empty();
                        let save_btn = modal_primary_button("Save Changes", can_save);

                        if (ui.add_enabled(can_save, save_btn).clicked() || enter_pressed)
                            && can_save
                        {
                            let proj = &mut self.storage.projects[proj_idx];
                            proj.name = self.edit_project_name.trim().to_string();
                            proj.local_path = self.edit_project_path.trim().to_string();
                            proj.description = if self.edit_project_desc.trim().is_empty() {
                                "Custom deployment project".to_string()
                            } else {
                                self.edit_project_desc.trim().to_string()
                            };
                            let grp = self.edit_project_group.trim().to_string();
                            proj.group = if grp.is_empty() { None } else { Some(grp) };
                            let updated_name = proj.name.clone();
                            let _ = self.storage.save();
                            self.refresh_selected_project_metadata(false);
                            self.status_banner = Some((
                                format!("Updated project '{updated_name}'."),
                                true,
                            ));
                            self.show_edit_project_modal = false;
                        }

                        ui.add_space(SP_8);
                        let cancel_btn = modal_secondary_button("Cancel");
                        if ui.add(cancel_btn).clicked() {
                            self.show_edit_project_modal = false;
                        }
                    });
                });
            });
    }

    pub(crate) fn render_new_environment_modal(&mut self, ctx: &egui::Context) {
        let SidebarSelection::Project(proj_id) = self.selection.clone() else {
            self.show_new_env_modal = false;
            return;
        };
        let Some(proj_idx) = self.storage.projects.iter().position(|p| p.id == proj_id) else {
            self.show_new_env_modal = false;
            return;
        };

        let Some(anim_t) = prepare_animated_modal(ctx, "new_env", &mut self.show_new_env_modal) else {
            return;
        };

        let existing_envs: Vec<(String, String)> = self.storage.projects[proj_idx]
            .environments
            .iter()
            .map(|e| (e.key.clone(), e.name.clone()))
            .collect();

        let (window, inner_w) = animated_modal_window(ctx, "Add Environment Level", anim_t, None);
        window.show(ctx, |ui| {
            ui.set_width(inner_w);
                if render_modal_header(
                    ui,
                    "close_new_env_modal",
                    "Add Environment Level",
                    "Import an environment from a .ps1 script or create a custom level (dev, staging, production, UAT, QA).",
                ) {
                    self.show_new_env_modal = false;
                }
                ui.add_space(SP_16);

                // Option 1: Import from .ps1 script
                egui::Frame::new()
                    .fill(MD_PANEL_BG)
                    .stroke(Stroke::new(1.0_f32, MD_BORDER))
                    .corner_radius(R_LG)
                    .inner_margin(Margin::same(16))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("IMPORT ENVIRONMENT FROM .PS1 SCRIPT")
                                .size(FS_MICRO)
                                .strong()
                                .color(ACCENT),
                        );
                        ui.add_space(SP_8);
                        horizontal_row(ui, 40.0, |ui| {
                            modal_text_input(
                                ui,
                                "new_env_ps1_path",
                                &mut self.new_env_ps1_path,
                                272.0,
                                Some(r"e.g. C:\Projects\my-app\deploy-staging.ps1"),
                            );
                            ui.add_space(SP_8);
                            let browse_btn = modal_secondary_button("Browse .ps1...");
                            if ui.add(browse_btn).clicked() {
                                if let Some(picked) = pick_ps1_file_dialog() {
                                    self.new_env_ps1_path = picked;
                                }
                            }
                            ui.add_space(SP_8);
                            let can_import = !self.new_env_ps1_path.trim().is_empty();
                            let imp_btn = modal_primary_button("Import", can_import);
                            if ui.add_enabled(can_import, imp_btn).clicked() && can_import {
                                let path = PathBuf::from(
                                    self.new_env_ps1_path.trim().trim_matches('"'),
                                );
                                match parse_ps1_file(&path) {
                                    Ok(env_cfg) => {
                                        let key = env_cfg.key.clone();
                                        let name = env_cfg.name.clone();
                                        let proj = &mut self.storage.projects[proj_idx];
                                        if let Some(existing) = proj.get_env_mut(&key) {
                                            *existing = env_cfg;
                                        } else {
                                            proj.environments.push(env_cfg);
                                        }
                                        let _ = self.storage.save();
                                        self.selected_env_key = key;
                                        self.refresh_selected_project_metadata(true);
                                        self.status_banner = Some((
                                            format!(
                                                "Imported environment '{name}' from '{}'.",
                                                path.display()
                                            ),
                                            true,
                                        ));
                                        self.new_env_ps1_path.clear();
                                        self.show_new_env_modal = false;
                                    }
                                    Err(err) => {
                                        self.status_banner = Some((err, false));
                                    }
                                }
                            }
                        });
                    });

                ui.add_space(SP_16);

                // Option 2: Create by Name
                ui.label(
                    RichText::new("Or Create Custom Environment Name")
                        .size(FS_BASE)
                        .strong()
                        .color(MD_TEXT),
                );
                ui.add_space(SP_8);
                modal_text_input(
                    ui,
                    "new_env_name",
                    &mut self.new_env_name,
                    512.0,
                    Some("e.g. Development, Staging, Production, UAT, QA"),
                );

                if !existing_envs.is_empty() {
                    ui.add_space(SP_16);
                    ui.label(
                        RichText::new("Clone Pipeline Template From:")
                            .size(FS_BASE)
                            .strong()
                            .color(MD_TEXT),
                    );
                    ui.add_space(SP_8);
                    ui.horizontal_wrapped(|ui| {
                        for (key, name) in &existing_envs {
                            let selected = self.new_env_clone_from == *key;
                            let btn = egui::Button::new(
                                RichText::new(name).size(FS_MICRO).strong().color(if selected {
                                    Color32::WHITE
                                } else {
                                    MD_TEXT
                                }),
                            )
                            .fill(if selected { ACCENT } else { MD_PANEL_BG })
                            .stroke(Stroke::new(
                                1.0_f32,
                                if selected { ACCENT } else { MD_BORDER },
                            ))
                            .corner_radius(R_SM);
                            if ui.add(btn).clicked() {
                                self.new_env_clone_from = key.clone();
                            }
                        }
                    });
                }

                // Footer (top border 1px MD_BORDER, padding-top 16px, margin-top 24px)
                ui.add_space(SP_24);
                draw_divider(ui, MD_BORDER);
                ui.add_space(SP_16);

                let enter_pressed = ctx.input(|i| i.key_pressed(egui::Key::Enter));
                horizontal_row(ui, 40.0, |ui| {
                    ui.label(
                        RichText::new("New environments inherit active deployment settings")
                            .size(FS_MICRO)
                            .color(MD_TEXT_MUTED),
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let can_create = !self.new_env_name.trim().is_empty();
                        let create_btn = modal_primary_button("Add Environment", can_create);

                        if (ui.add_enabled(can_create, create_btn).clicked() || enter_pressed)
                            && can_create
                        {
                            let clone_src = if self.new_env_clone_from.is_empty() {
                                None
                            } else {
                                Some(self.new_env_clone_from.as_str())
                            };
                            match self.storage.projects[proj_idx]
                                .add_environment(&self.new_env_name, clone_src)
                            {
                                Ok(new_key) => {
                                    let _ = self.storage.save();
                                    self.selected_env_key = new_key;
                                    self.refresh_selected_project_metadata(true);
                                    self.status_banner = Some((
                                        format!(
                                            "Added environment '{}' successfully.",
                                            self.new_env_name.trim()
                                        ),
                                        true,
                                    ));
                                    self.new_env_name.clear();
                                    self.show_new_env_modal = false;
                                }
                                Err(err) => {
                                    self.status_banner = Some((err, false));
                                }
                            }
                        }

                        ui.add_space(SP_8);
                        let cancel_btn = modal_secondary_button("Cancel");
                        if ui.add(cancel_btn).clicked() {
                            self.show_new_env_modal = false;
                        }
                    });
                });
            });
    }

    pub(crate) fn render_github_update_modal(&mut self, ctx: &egui::Context) {
        let Some(anim_t) = prepare_animated_modal(ctx, "github_update", &mut self.show_update_modal) else {
            return;
        };

        if let Some(release) = self.available_update.clone() {
            let (window, inner_w) = animated_modal_window(ctx, "Software Update Available", anim_t, None);
            window.show(ctx, |ui| {
                ui.set_width(inner_w);
                    if render_modal_header(
                        ui,
                        "close_github_update_modal",
                        "Software Update Available",
                        &format!(
                            "A newer release of Shiply is available on GitHub Releases ({GITHUB_REPO})."
                        ),
                    ) {
                        self.show_update_modal = false;
                    }

                    ui.add_space(SP_12);

                    // Version Comparison Card (Current Version vs. New Version Tag)
                    egui::Frame::new()
                        .fill(MD_PANEL_BG)
                        .stroke(Stroke::new(1.0_f32, MD_BORDER))
                        .corner_radius(R_LG)
                        .inner_margin(Margin::symmetric(14, 12))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            horizontal_row(ui, 26.0, |ui| {
                                ui.label(
                                    RichText::new("Current Version:")
                                        .size(FS_SMALL)
                                        .color(MD_TEXT_MUTED),
                                );
                                render_pill(
                                    ui,
                                    &format!("v{APP_VERSION}"),
                                    MD_INPUT_BG,
                                    MD_TEXT_MUTED,
                                );
                                ui.label(
                                    RichText::new("→")
                                        .size(FS_BASE)
                                        .strong()
                                        .color(ACCENT),
                                );
                                ui.label(
                                    RichText::new("New Release:")
                                        .size(FS_SMALL)
                                        .color(MD_TEXT_MUTED),
                                );
                                render_pill(
                                    ui,
                                    &release.tag_name,
                                    BADGE_DEV_BG,
                                    BADGE_DEV_TEXT,
                                );
                            });
                            ui.add_space(SP_4);
                            ui.label(
                                RichText::new(format!(
                                    "{}  •  Published: {}",
                                    release.name, release.published_at
                                ))
                                .size(FS_MICRO)
                                .color(MD_TEXT_MUTED),
                            );
                        });

                    if self.has_active_deployments() {
                        ui.add_space(SP_8);
                        egui::Frame::new()
                            .fill(BLUE_PILL_BG)
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(191, 219, 254)))
                            .corner_radius(R_SM)
                            .inner_margin(Margin::symmetric(12, 8))
                            .show(ui, |ui| {
                                let active_count = self.active_deployments.len();
                                ui.label(
                                    RichText::new(format!(
                                        "{active_count} active deployment{} in progress. Deployments will continue running uninterrupted and update will apply once completed.",
                                        if active_count > 1 { "s are" } else { " is" }
                                    ))
                                    .size(FS_SMALL)
                                    .color(BLUE_PILL_TEXT),
                                );
                            });
                    }

                    ui.add_space(SP_12);

                    // Release Notes / Changelog section
                    ui.label(
                        RichText::new("RELEASE NOTES / CHANGELOG")
                            .size(FS_MICRO)
                            .strong()
                            .color(MD_TEXT_MUTED),
                    );
                    ui.add_space(SP_4);
                    egui::Frame::new()
                        .fill(MD_INPUT_BG)
                        .stroke(Stroke::new(1.0_f32, MD_BORDER))
                        .corner_radius(R_MD)
                        .inner_margin(Margin::symmetric(14, 10))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ScrollArea::vertical()
                                .id_salt("github_release_notes_scroll")
                                .max_height(190.0)
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    for line in release.body.lines() {
                                        let trimmed = line.trim();
                                        if let Some(h) = trimmed.strip_prefix("### ") {
                                            ui.add_space(SP_4);
                                            ui.label(
                                                RichText::new(h)
                                                    .size(FS_BASE)
                                                    .strong()
                                                    .color(MD_TEXT),
                                            );
                                        } else if let Some(h) = trimmed.strip_prefix("## ") {
                                            ui.add_space(SP_4);
                                            ui.label(
                                                RichText::new(h)
                                                    .size(FS_BASE)
                                                    .strong()
                                                    .color(ACCENT),
                                            );
                                        } else {
                                            ui.label(
                                                RichText::new(line)
                                                    .size(FS_SMALL)
                                                    .color(MD_TEXT_MUTED),
                                            );
                                        }
                                    }
                                });
                        });

                    // Download Progress Bar / Status Indicator
                    if self.update_downloading || self.update_progress > 0.0 {
                        ui.add_space(SP_12);
                        let pct = (self.update_progress * 100.0).clamp(0.0, 100.0);
                        ui.label(
                            RichText::new(format!(
                                "{} ({pct:.0}%)",
                                if self.update_status_text.is_empty() {
                                    "Downloading update..."
                                } else {
                                    &self.update_status_text
                                }
                            ))
                            .size(FS_SMALL)
                            .strong()
                            .color(ACCENT),
                        );
                        ui.add_space(SP_4);
                        let bar_w = ui.available_width();
                        let (bar_rect, _) =
                            ui.allocate_exact_size(Vec2::new(bar_w, 8.0), Sense::hover());
                        ui.painter()
                            .rect_filled(bar_rect, CornerRadius::same(4), MD_PANEL_BG);
                        let fill_w = (bar_w * self.update_progress.clamp(0.02, 1.0)).min(bar_w);
                        let fill_rect = Rect::from_min_size(bar_rect.min, Vec2::new(fill_w, 8.0));
                        ui.painter()
                            .rect_filled(fill_rect, CornerRadius::same(4), ACCENT);
                    }

                    if let Some(err) = &self.update_error {
                        ui.add_space(SP_8);
                        egui::Frame::new()
                            .fill(FAIL_PILL_BG)
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(252, 165, 165)))
                            .corner_radius(R_SM)
                            .inner_margin(Margin::symmetric(12, 8))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(err)
                                        .size(FS_SMALL)
                                        .color(FAIL_PILL_TEXT),
                                );
                            });
                    }

                    ui.add_space(SP_16);
                    draw_divider(ui, MD_BORDER);
                    ui.add_space(SP_12);

                    horizontal_row(ui, 40.0, |ui| {
                        if ui.add(modal_secondary_button("View on GitHub")).clicked() {
                            let url = if release.html_url.is_empty() {
                                GITHUB_RELEASES_PAGE_URL
                            } else {
                                &release.html_url
                            };
                            open_url_in_browser(url);
                        }

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if let Some((_ver, exe_path)) = self.pending_update_restart.clone() {
                                if ui.add(modal_primary_button("Restart Now", true)).clicked() {
                                    if let Err(err) = restart_application(&exe_path) {
                                        self.update_error = Some(err);
                                    } else {
                                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                    }
                                }
                                if ui.add(modal_secondary_button("Dismiss (Restart on Finish)")).clicked() {
                                    self.show_update_modal = false;
                                }
                            } else {
                                let update_btn_label = if self.update_downloading {
                                    "Updating in Background..."
                                } else if self.has_active_deployments() {
                                    "Update (Safe Restart)"
                                } else {
                                    "Update & Restart"
                                };

                                let btn_resp = ui.add(modal_primary_button(update_btn_label, !self.update_downloading));

                                if btn_resp.clicked() && !self.update_downloading {
                                    self.update_error = None;
                                    if release.download_url.trim().is_empty() {
                                        let url = if release.html_url.is_empty() {
                                            GITHUB_RELEASES_PAGE_URL
                                        } else {
                                            &release.html_url
                                        };
                                        open_url_in_browser(url);
                                    } else {
                                        self.update_downloading = true;
                                        self.update_progress = 0.02;
                                        self.update_status_text =
                                            format!("Starting download of {}...", release.asset_name);
                                        spawn_download_and_apply_update(release, self.updater_tx.clone());
                                    }
                                }

                                let cancel_label = if self.update_downloading {
                                    "Dismiss / Background"
                                } else {
                                    "Remind Me Later"
                                };
                                if ui.add(modal_secondary_button(cancel_label)).clicked() {
                                    self.show_update_modal = false;
                                }
                            }
                        });
                    });
                });
        } else {
            // Up-to-date Update Information Dialog (when clicking notification bell or checking)
            let (window, inner_w) = animated_modal_window(ctx, "Update Information", anim_t, None);
            window.show(ctx, |ui| {
                ui.set_width(inner_w);
                    if render_modal_header(
                        ui,
                        "close_update_info_modal",
                        "Update Information",
                        "Check for newer versions published on GitHub Releases.",
                    ) {
                        self.show_update_modal = false;
                    }

                    ui.add_space(SP_12);

                    egui::Frame::new()
                        .fill(SUCCESS_PILL_BG)
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(187, 247, 208)))
                        .corner_radius(R_LG)
                        .inner_margin(Margin::symmetric(14, 12))
                        .show(ui, |ui| {
                            horizontal_row(ui, 24.0, |ui| {
                                render_pill(
                                    ui,
                                    "UP TO DATE",
                                    SUCCESS,
                                    Color32::WHITE,
                                );
                                ui.label(
                                    RichText::new(format!("Shiply v{APP_VERSION}"))
                                        .size(FS_BASE)
                                        .strong()
                                        .color(SUCCESS_PILL_TEXT),
                                );
                            });
                            ui.add_space(SP_4);
                            ui.label(
                                RichText::new(format!(
                                    "You are running the latest version of Shiply. Update checks query {GITHUB_REPO} on startup."
                                ))
                                .size(FS_SMALL)
                                .color(Color32::from_rgb(20, 83, 45)),
                            );
                        });

                    if self.update_checking {
                        ui.add_space(SP_8);
                        ui.label(
                            RichText::new("Checking GitHub Releases...")
                                .size(FS_SMALL)
                                .color(ACCENT),
                        );
                    }

                    ui.add_space(SP_16);
                    draw_divider(ui, MD_BORDER);
                    ui.add_space(SP_12);

                    horizontal_row(ui, 40.0, |ui| {
                        if ui.add(modal_secondary_button("View on GitHub")).clicked() {
                            open_url_in_browser(GITHUB_RELEASES_PAGE_URL);
                        }

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.add(modal_secondary_button("Close")).clicked() {
                                self.show_update_modal = false;
                            }

                            if ui
                                .add(modal_primary_button("Check Again", !self.update_checking))
                                .clicked()
                            {
                                spawn_update_check(self.updater_tx.clone(), true);
                            }
                        });
                    });
                });
        }
    }

    /// Renders a confirmation dialog before deleting a project or an environment.
    pub(crate) fn render_delete_confirmation_modal(&mut self, ctx: &egui::Context) {
        let mut is_open = self.pending_delete_confirm.is_some();
        let Some(anim_t) = prepare_animated_modal(ctx, "delete_confirm", &mut is_open) else {
            if !is_open {
                self.pending_delete_confirm = None;
            }
            return;
        };
        if !is_open {
            self.pending_delete_confirm = None;
            return;
        }

        let Some(action) = self.pending_delete_confirm.clone() else {
            return;
        };

        let mut execute_delete = false;
        let mut dismiss = false;

        let frame = modal_dialog_frame_for(ctx).stroke(Stroke::new(1.0_f32, Color32::from_rgb(252, 165, 165)));
        let (window, inner_w) = animated_modal_window(ctx, "Confirm Deletion", anim_t, Some(frame));
        window.show(ctx, |ui| {
            ui.set_width(inner_w);

                let (modal_title, modal_subtitle, item_name, warning_text) = match &action {
                    ConfirmDeleteAction::Project { project_name, .. } => (
                        "Delete Project",
                        "This will permanently delete the project from Shiply.",
                        project_name.as_str(),
                        "Are you sure you want to delete this project? All associated environments, pipeline configurations, and settings for this project will be removed.",
                    ),
                    ConfirmDeleteAction::Environment { env_name, .. } => (
                        "Delete Environment",
                        "This will permanently remove this environment from the project.",
                        env_name.as_str(),
                        "Are you sure you want to delete this environment? Its pipeline steps and SSH credentials mapping will be deleted.",
                    ),
                };

                if render_modal_header(
                    ui,
                    "close_delete_confirm_modal",
                    modal_title,
                    modal_subtitle,
                ) {
                    dismiss = true;
                }

                ui.add_space(SP_16);

                egui::Frame::new()
                    .fill(FAIL_PILL_BG)
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(254, 202, 202)))
                    .corner_radius(R_LG)
                    .inner_margin(Margin::same(16))
                    .show(ui, |ui| {
                        horizontal_row(ui, 24.0, |ui| {
                            render_pill(
                                ui,
                                "WARNING",
                                Color32::from_rgb(239, 68, 68),
                                Color32::WHITE,
                            );
                            ui.label(
                                RichText::new(format!("Target: {item_name}"))
                                    .size(FS_BASE)
                                    .strong()
                                    .color(FAIL_PILL_TEXT),
                            );
                        });
                        ui.add_space(SP_8);
                        ui.label(
                            RichText::new(warning_text)
                                .size(FS_SMALL)
                                .color(Color32::from_rgb(127, 29, 29)),
                        );
                    });

                // Footer (top border 1px MD_BORDER, padding-top 16px, margin-top 24px)
                ui.add_space(SP_24);
                draw_divider(ui, MD_BORDER);
                ui.add_space(SP_16);

                horizontal_row(ui, 40.0, |ui| {
                    ui.label(
                        RichText::new("Warning: This action cannot be undone.")
                            .size(FS_MICRO)
                            .color(Color32::from_rgb(185, 28, 28)),
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let delete_btn = egui::Button::new(
                            RichText::new("Delete Permanently")
                                .size(FS_BASE)
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(220, 38, 38))
                        .corner_radius(R_MD)
                        .min_size(Vec2::new(160.0, 40.0));

                        if ui.add(delete_btn).clicked() {
                            execute_delete = true;
                        }

                        ui.add_space(SP_8);
                        let cancel_btn = modal_secondary_button("Cancel");
                        if ui.add(cancel_btn).clicked() {
                            dismiss = true;
                        }
                    });
                });
            });

        if execute_delete {
            match action {
                ConfirmDeleteAction::Project {
                    project_id,
                    project_name,
                } => {
                    if let Some(idx) = self
                        .storage
                        .projects
                        .iter()
                        .position(|p| p.id == project_id)
                    {
                        self.storage.projects.remove(idx);
                        let _ = self.storage.save();
                        self.selection = self
                            .storage
                            .projects
                            .first()
                            .map(|p| SidebarSelection::Project(p.id.clone()))
                            .unwrap_or(SidebarSelection::AllDeployments);
                        self.refresh_selected_project_metadata(true);
                        self.status_banner =
                            Some((format!("Removed project '{project_name}'."), true));
                    }
                }
                ConfirmDeleteAction::Environment {
                    project_id,
                    env_key,
                    env_name,
                    ..
                } => {
                    let mut remove_res: Option<Result<String, String>> = None;
                    if let Some(proj) = self
                        .storage
                        .projects
                        .iter_mut()
                        .find(|p| p.id == project_id)
                    {
                        if let Err(err) = proj.remove_environment(&env_key) {
                            remove_res = Some(Err(err));
                        } else {
                            let next_key = proj
                                .environments
                                .first()
                                .map(|e| e.key.clone())
                                .unwrap_or_default();
                            remove_res = Some(Ok(next_key));
                        }
                    }

                    match remove_res {
                        Some(Err(err)) => {
                            self.status_banner = Some((err, false));
                        }
                        Some(Ok(next_key)) => {
                            let _ = self.storage.save();
                            self.selected_env_key = next_key;
                            self.refresh_selected_project_metadata(true);
                            self.status_banner =
                                Some((format!("Removed environment '{env_name}'."), true));
                        }
                        None => {}
                    }
                }
            }
            self.pending_delete_confirm = None;
        } else if dismiss {
            self.pending_delete_confirm = None;
        }
    }
    pub(crate) fn render_bulk_deploy_modal(&mut self, ctx: &egui::Context) {
        let Some(anim_t) = prepare_animated_modal(ctx, "bulk_deploy", &mut self.show_bulk_deploy_modal) else {
            return;
        };
        let (window, inner_w) = animated_modal_window(ctx, "Bulk Deployment", anim_t, None);
        window.show(ctx, |ui| {
            ui.set_width(inner_w);

                if render_modal_header(
                    ui,
                    "close_bulk_deploy_modal",
                    "Bulk Deployment — Select Projects to Deploy",
                    "Choose which projects to deploy, select the target environment, and review each project's auto-bumped release version.",
                ) {
                    self.show_bulk_deploy_modal = false;
                }
                ui.add_space(SP_12);

                // Target Environment Selector + Select All / Clear All
                let env_tabs = self.storage.existing_environment_tabs(None);
                let available_envs: Vec<(String, String)> = env_tabs
                    .into_iter()
                    .filter_map(|(name, key_opt)| key_opt.map(|k| (name, k)))
                    .collect();

                let mut env_changed = false;
                horizontal_row(ui, 34.0, |ui| {
                    ui.label(
                        RichText::new("Target Environment:")
                            .size(FS_BASE)
                            .strong()
                            .color(MD_TEXT),
                    );
                    let current_env_label = available_envs
                        .iter()
                        .find(|(_, k)| k == &self.bulk_target_env_key)
                        .map(|(n, k)| format!("{n} ({k})"))
                        .unwrap_or_else(|| {
                            if self.bulk_target_env_key.is_empty() {
                                "Default (First Environment)".to_string()
                            } else {
                                self.bulk_target_env_key.clone()
                            }
                        });

                    egui::ComboBox::from_id_salt("bulk_target_env_combo")
                        .selected_text(
                            RichText::new(current_env_label)
                                .size(FS_BASE)
                                .strong()
                                .color(ACCENT),
                        )
                        .width(190.0)
                        .show_ui(ui, |ui| {
                            for (env_name, env_key) in &available_envs {
                                let is_sel = &self.bulk_target_env_key == env_key;
                                if ui
                                    .selectable_label(is_sel, format!("{env_name} ({env_key})"))
                                    .clicked()
                                    && !is_sel
                                {
                                    self.bulk_target_env_key = env_key.clone();
                                    env_changed = true;
                                }
                            }
                        });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let clear_btn = egui::Button::new(
                            RichText::new("Clear All")
                                .size(FS_MICRO)
                                .strong()
                                .color(MD_TEXT_MUTED),
                        )
                        .fill(MD_PANEL_BG)
                        .stroke(Stroke::new(1.0_f32, MD_BORDER))
                        .corner_radius(R_SM)
                        .min_size(Vec2::new(72.0, 28.0));
                        if ui.add(clear_btn).clicked() {
                            self.bulk_selected_projects.clear();
                        }

                        ui.add_space(SP_8);
                        let all_btn = egui::Button::new(
                            RichText::new("Select All")
                                .size(FS_MICRO)
                                .strong()
                                .color(ACCENT),
                        )
                        .fill(ACCENT_SOFT)
                        .stroke(Stroke::new(1.0_f32, ACCENT.gamma_multiply(0.4)))
                        .corner_radius(R_SM)
                        .min_size(Vec2::new(76.0, 28.0));
                        if ui.add(all_btn).clicked() {
                            for proj in &self.storage.projects {
                                if !proj.environments.is_empty() {
                                    self.bulk_selected_projects.insert(proj.id.clone());
                                }
                            }
                        }
                    });
                });

                if env_changed {
                    self.sync_bulk_project_versions();
                }

                ui.add_space(SP_8);

                // Project Checklist Table
                egui::Frame::new()
                    .fill(MD_PANEL_BG)
                    .stroke(Stroke::new(1.0_f32, MD_BORDER))
                    .corner_radius(R_MD)
                    .inner_margin(Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ScrollArea::vertical()
                            .id_salt("bulk_projects_list_scroll")
                            .max_height(230.0)
                            .show(ui, |ui| {
                                struct BulkProjectRow {
                                    id: String,
                                    name: String,
                                    resolved_env: Option<(String, String)>,
                                }

                                let proj_snapshots: Vec<BulkProjectRow> = self
                                    .storage
                                    .projects
                                    .iter()
                                    .map(|p| {
                                        let resolved_env = p
                                            .get_env(&self.bulk_target_env_key)
                                            .or_else(|| p.environments.first())
                                            .map(|e| (e.key.clone(), e.name.clone()));
                                        BulkProjectRow {
                                            id: p.id.clone(),
                                            name: p.name.clone(),
                                            resolved_env,
                                        }
                                    })
                                    .collect();

                                for row in proj_snapshots {
                                    let pid = row.id;
                                    let pname = row.name;
                                    let resolved_env = row.resolved_env;
                                    let mut checked = self.bulk_selected_projects.contains(&pid);
                                    let has_env = resolved_env.is_some();
                                    horizontal_row(ui, 38.0, |ui| {
                                        if ui
                                            .add_enabled(
                                                has_env,
                                                egui::Checkbox::new(
                                                    &mut checked,
                                                    RichText::new(&pname)
                                                        .size(FS_BASE)
                                                        .strong()
                                                        .color(MD_TEXT),
                                                ),
                                            )
                                            .changed()
                                        {
                                            if checked {
                                                self.bulk_selected_projects.insert(pid.clone());
                                            } else {
                                                self.bulk_selected_projects.remove(&pid);
                                            }
                                        }

                                        if let Some((ekey, ename)) = &resolved_env {
                                            let (bg, fg) = env_pastel_pill_colors(ekey);
                                            render_pill(ui, &ename.to_uppercase(), bg, fg);
                                        } else {
                                            render_pill(
                                                ui,
                                                "NO ENVIRONMENT",
                                                FAIL_PILL_BG,
                                                FAIL_PILL_TEXT,
                                            );
                                        }

                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            let ver_entry = self
                                                .bulk_project_versions
                                                .entry(pid.clone())
                                                .or_insert_with(|| "1.0.0".to_string());
                                            modal_text_input(
                                                ui,
                                                ("bulk_ver_entry", &pid),
                                                ver_entry,
                                                90.0,
                                                Some("1.0.0"),
                                            );
                                            ui.add_space(SP_4);
                                            ui.label(
                                                RichText::new("Version:")
                                                    .size(FS_MICRO)
                                                    .color(MD_TEXT_MUTED),
                                            );
                                        });
                                    });
                                    draw_divider(ui, MD_BORDER);
                                }
                            });
                    });

                ui.add_space(SP_12);

                // Options + Trigger Action Footer (top border 1px MD_BORDER, padding-top 16px, margin-top 24px)
                ui.add_space(SP_12);
                draw_divider(ui, MD_BORDER);
                ui.add_space(SP_16);

                horizontal_row(ui, 40.0, |ui| {
                    ui.checkbox(
                        &mut self.bulk_bump_pkg,
                        RichText::new("Bump package.json")
                            .size(FS_SMALL)
                            .color(MD_TEXT_MUTED),
                    );
                    ui.add_space(SP_8);
                    ui.checkbox(
                        &mut self.bulk_create_tag,
                        RichText::new("Git Tag")
                            .size(FS_SMALL)
                            .color(MD_TEXT_MUTED),
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let sel_count = self.bulk_selected_projects.len();
                        let run_btn = modal_primary_button(&format!("Deploy {sel_count} Selected"), sel_count > 0);

                        if ui.add_enabled(sel_count > 0, run_btn).clicked() {
                            self.trigger_bulk_deployment();
                        }

                        ui.add_space(SP_8);
                        let cancel_btn = modal_secondary_button("Cancel");
                        if ui.add(cancel_btn).clicked() {
                            self.show_bulk_deploy_modal = false;
                        }
                    });
                });
            });
    }
}
