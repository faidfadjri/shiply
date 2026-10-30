use super::*;

impl ShiplyApp {
    pub(crate) fn render_first_time_empty_state(&mut self, ui: &mut egui::Ui) {
        ui.add_space(28.0);
        let avail_w = ui.available_width();
        egui::Frame::new()
            .fill(CARD_BG)
            .stroke(Stroke::new(1.0_f32, CARD_BORDER))
            .corner_radius(12.0)
            .inner_margin(Margin::symmetric(36, 36))
            .show(ui, |ui| {
                ui.set_width((avail_w - 72.0).max(320.0));
                ui.vertical_centered(|ui| {
                    let (icon_rect, _) =
                        ui.allocate_exact_size(Vec2::new(48.0, 48.0), Sense::hover());
                    draw_shiply_logo(
                        ui.painter(),
                        icon_rect,
                        12,
                        24.0,
                        self.logo_texture.as_ref(),
                    );

                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("Welcome to Shiply")
                            .size(FONT_XL)
                            .strong()
                            .color(TEXT_PRIMARY),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(
                            "No projects or environments configured yet. Auto-import a PowerShell deployment script (.ps1),\ncreate a project manually, or paste your shiply_state.json into the app directory.",
                        )
                        .size(FONT_MD)
                        .color(TEXT_SECONDARY),
                    );
                    ui.add_space(22.0);

                    ui.horizontal(|ui| {
                        let total_btns_w = 510.0;
                        let pad = ((ui.available_width() - total_btns_w) / 2.0).max(0.0);
                        ui.add_space(pad);

                        let import_btn = egui::Button::new(
                            RichText::new("Auto-Import deploy.ps1 Script")
                                .size(FONT_MD)
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(PRIMARY_BLUE)
                        .corner_radius(8.0)
                        .min_size(Vec2::new(235.0, 40.0));

                        if ui.add(import_btn).clicked() {
                            self.open_new_project_modal();
                            if let Some(picked) = pick_ps1_file_dialog() {
                                self.new_project_ps1_path = picked.clone();
                                self.apply_ps1_auto_import_to_modal(&picked);
                            }
                        }

                        ui.add_space(8.0);

                        let add_btn = egui::Button::new(
                            RichText::new("+ Add Project")
                                .size(FONT_MD)
                                .strong()
                                .color(TEXT_PRIMARY),
                        )
                        .fill(SUBTLE_BG)
                        .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                        .corner_radius(8.0)
                        .min_size(Vec2::new(130.0, 40.0));

                        if ui.add(add_btn).clicked() {
                            self.open_new_project_modal();
                        }

                        ui.add_space(8.0);

                        let folder_btn = egui::Button::new(
                            RichText::new("Open App Folder")
                                .size(FONT_MD)
                                .strong()
                                .color(TEXT_SECONDARY),
                        )
                        .fill(CARD_BG)
                        .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                        .corner_radius(8.0)
                        .min_size(Vec2::new(130.0, 40.0));

                        if ui.add(folder_btn).clicked() {
                            let dir = self.storage.app_data_dir();
                            let _ = fs::create_dir_all(&dir);
                            open_directory_in_os(&dir);
                        }
                    });
                });
            });

        ui.add_space(18.0);
        self.render_onboarding_guideline_card(ui);
        ui.add_space(16.0);
        self.render_os_credentials_guideline_card(ui);
    }
    pub(crate) fn render_all_deployments_overview(&mut self, ui: &mut egui::Ui) {
        let mut open_bulk_modal = false;
        let mut toggle_select_all = false;
        let all_project_ids: Vec<String> = self
            .storage
            .projects
            .iter()
            .map(|p| p.id.clone())
            .collect();
        let all_selected = !all_project_ids.is_empty()
            && all_project_ids
                .iter()
                .all(|id| self.bulk_selected_projects.contains(id));
        let selected_count = all_project_ids
            .iter()
            .filter(|id| self.bulk_selected_projects.contains(*id))
            .count();

        horizontal_row(ui, 38.0, |ui| {
            ui.label(
                RichText::new("All Deployments")
                    .size(FONT_XL)
                    .strong()
                    .color(TEXT_PRIMARY),
            );
            if selected_count > 0 {
                render_pill(
                    ui,
                    &format!("{selected_count} SELECTED"),
                    BLUE_PILL_BG,
                    BLUE_PILL_TEXT,
                );
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);

                let bulk_label = if selected_count > 0 {
                    format!("Bulk Deploy ({selected_count} Selected)...")
                } else {
                    "Bulk Deploy Projects...".to_string()
                };
                let bulk_btn = egui::Button::new(
                    RichText::new(bulk_label)
                        .size(FONT_MD)
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(PRIMARY_BLUE)
                .corner_radius(7.0)
                .min_size(Vec2::new(190.0, 34.0));

                if ui
                    .add(bulk_btn)
                    .on_hover_text("Deploy multiple selected projects sequentially with auto-bumped versions")
                    .clicked()
                {
                    open_bulk_modal = true;
                }

                let sel_btn_label = if all_selected {
                    "Clear Selection"
                } else {
                    "Select All Projects"
                };
                let sel_btn = egui::Button::new(
                    RichText::new(sel_btn_label)
                        .size(FONT_SM)
                        .strong()
                        .color(TEXT_SECONDARY),
                )
                .fill(CARD_BG)
                .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                .corner_radius(7.0)
                .min_size(Vec2::new(128.0, 34.0));

                if ui.add(sel_btn).clicked() {
                    toggle_select_all = true;
                }
            });
        });

        if toggle_select_all {
            if all_selected {
                self.bulk_selected_projects.clear();
            } else {
                for id in &all_project_ids {
                    self.bulk_selected_projects.insert(id.clone());
                }
            }
        }
        if open_bulk_modal {
            self.open_bulk_deploy_modal();
        }

        ui.label(
            RichText::new(
                "Select projects below to run a Bulk Deployment, or click a project card to open its workspace.",
            )
            .size(FONT_MD)
            .color(TEXT_SECONDARY),
        );
        ui.add_space(14.0);

        // Live Bulk Deployment Queue Progress Banner (when a bulk deployment is running)
        if !self.bulk_deploy_queue.is_empty()
            || (self.bulk_total_count > 0 && self.has_active_deployments())
        {
            let remaining = self.bulk_deploy_queue.len();
            let current_idx = self.bulk_total_count.saturating_sub(remaining);
            let total = self.bulk_total_count.max(1);
            let mut cancel_queue = false;

            egui::Frame::new()
                .fill(BLUE_PILL_BG)
                .stroke(Stroke::new(1.0_f32, PRIMARY_BLUE))
                .corner_radius(8.0)
                .inner_margin(Margin::symmetric(16, 10))
                .show(ui, |ui| {
                    horizontal_row(ui, 30.0, |ui| {
                        ui.ctx().request_repaint();
                        let time = ui.input(|i| i.time);
                        let (sp_rect, _) =
                            ui.allocate_exact_size(Vec2::new(14.0, 14.0), Sense::hover());
                        draw_circle_loading_spinner(
                            ui.painter(),
                            sp_rect.center(),
                            5.0,
                            BLUE_PILL_TEXT,
                            time,
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(format!(
                                "BULK DEPLOYMENT IN PROGRESS — Running {current_idx} of {total} ({remaining} queued)"
                            ))
                            .size(FONT_SM)
                            .strong()
                            .color(BLUE_PILL_TEXT),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if remaining > 0
                                && outline_icon_button(
                                    ui,
                                    "cancel_bulk_queue_btn",
                                    OutlineIcon::Trash,
                                    Some("Cancel Remaining Queue"),
                                    FAIL_PILL_TEXT,
                                    FAIL_PILL_BG,
                                    Color32::from_rgb(254, 202, 202),
                                    Color32::from_rgb(252, 165, 165),
                                    true,
                                )
                                .clicked()
                            {
                                cancel_queue = true;
                            }
                        });
                    });
                });
            if cancel_queue {
                self.bulk_deploy_queue.clear();
                self.bulk_total_count = 0;
                self.status_banner = Some((
                    "Cleared remaining projects from the Bulk Deployment queue.".to_string(),
                    false,
                ));
            }
            ui.add_space(12.0);
        }

        let proj_count = self.storage.projects.len().max(1);
        let spacing = 14.0;
        let card_w = ((ui.available_width() - spacing * (proj_count.saturating_sub(1) as f32))
            / (proj_count as f32))
            .max(240.0);

        let mut clicked_proj = None;
        let mut toggled_bulk_proj = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(spacing, 14.0);
            for project in &self.storage.projects {
                let is_checked = self.bulk_selected_projects.contains(&project.id);
                let (rect, resp) =
                    ui.allocate_exact_size(Vec2::new(card_w, 108.0), Sense::click());
                let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);

                // Checkbox hit target in top-right of the project card
                let chk_rect = Rect::from_center_size(
                    Pos2::new(rect.right() - 24.0, rect.top() + 24.0),
                    Vec2::new(22.0, 22.0),
                );
                let chk_resp = ui
                    .interact(
                        chk_rect,
                        egui::Id::new(("bulk_chk_card", &project.id)),
                        Sense::click(),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand);

                draw_elevated_card(
                    ui.painter(),
                    rect,
                    CARD_BG,
                    Stroke::new(
                        if is_checked {
                            2.0_f32
                        } else if resp.hovered() {
                            1.5_f32
                        } else {
                            1.0_f32
                        },
                        if is_checked || resp.hovered() {
                            PRIMARY_BLUE
                        } else {
                            CARD_BORDER
                        },
                    ),
                );

                // Draw selection checkbox at top-right of project card
                ui.painter().rect_filled(
                    chk_rect,
                    CornerRadius::same(5),
                    if is_checked { PRIMARY_BLUE } else { SUBTLE_BG },
                );
                ui.painter().rect_stroke(
                    chk_rect,
                    CornerRadius::same(5),
                    Stroke::new(
                        1.2_f32,
                        if is_checked || chk_resp.hovered() {
                            PRIMARY_BLUE
                        } else {
                            INPUT_BORDER
                        },
                    ),
                    StrokeKind::Inside,
                );
                if is_checked {
                    draw_icon_check(ui.painter(), chk_rect.center(), Color32::WHITE);
                }

                ui.painter().text(
                    Pos2::new(rect.left() + 18.0, rect.top() + 24.0),
                    Align2::LEFT_CENTER,
                    &project.name,
                    FontId::proportional(FONT_LG),
                    TEXT_PRIMARY,
                );
                ui.painter().text(
                    Pos2::new(rect.left() + 18.0, rect.top() + 46.0),
                    Align2::LEFT_CENTER,
                    &project.local_path,
                    FontId::proportional(FONT_SM),
                    TEXT_MUTED,
                );

                let mut px = rect.left() + 18.0;
                let py = rect.top() + 78.0;
                for env in &project.environments {
                    let badge = env.badge_label();
                    let is_env_running = self.storage.deployments.iter().any(|d| {
                        d.project_id == project.id
                            && d.environment == env.key
                            && d.status == DeploymentStatus::Running
                    });
                    let w = if is_env_running {
                        ui.ctx().request_repaint();
                        let time = ui.input(|i| i.time);
                        paint_running_pill_left_aligned(
                            ui.painter(),
                            Pos2::new(px, py),
                            &format!("{badge}: RUNNING"),
                            WARN_PILL_BG,
                            WARN_PILL_TEXT,
                            time,
                        )
                    } else {
                        let (bg, fg) = env_pastel_pill_colors(&env.key);
                        let ver_label = self
                            .storage
                            .last_succeeded_deployment(&project.id, &env.key)
                            .map(|d| format!("{badge}: v{}", d.version))
                            .unwrap_or_else(|| format!("{badge}: —"));
                        paint_pill_left_aligned(
                            ui.painter(),
                            Pos2::new(px, py),
                            &ver_label,
                            bg,
                            fg,
                        )
                    };
                    px += w + 6.0;
                }
                if chk_resp
                    .on_hover_text("Select or deselect this project for Bulk Deployment")
                    .clicked()
                {
                    toggled_bulk_proj = Some(project.id.clone());
                } else if resp.clicked() {
                    clicked_proj = Some(project.id.clone());
                }
            }
        });

        if let Some(tid) = toggled_bulk_proj {
            if self.bulk_selected_projects.contains(&tid) {
                self.bulk_selected_projects.remove(&tid);
            } else {
                self.bulk_selected_projects.insert(tid);
            }
        } else if let Some(pid) = clicked_proj {
            self.selection = SidebarSelection::Project(pid.clone());
            self.project_switch_loading_id = Some(pid);
            self.project_switch_loading_start = Some(Instant::now());
            self.refresh_selected_project_metadata(true);
            return;
        }

        ui.add_space(18.0);
        self.render_onboarding_guideline_card(ui);
        ui.add_space(16.0);
        self.render_os_credentials_guideline_card(ui);
        ui.add_space(20.0);
        self.render_history_and_logs_view(ui, None);
    }

    pub(crate) fn render_onboarding_guideline_card(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(CARD_BG)
            .stroke(Stroke::new(1.0_f32, CARD_BORDER))
            .corner_radius(10.0)
            .inner_margin(Margin::symmetric(20, 14))
            .show(ui, |ui| {
                horizontal_row(ui, 30.0, |ui| {
                    render_pill(ui, "ONBOARDING", BLUE_PILL_BG, BLUE_PILL_TEXT);
                    ui.label(
                        RichText::new("QUICK-START DEPLOYMENT GUIDELINE")
                            .size(FONT_MD)
                            .strong()
                            .color(TEXT_PRIMARY),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let toggle_lbl = if self.show_onboarding_guide {
                            "Hide Guide"
                        } else {
                            "Show Guide"
                        };
                        let btn = egui::Button::new(
                            RichText::new(toggle_lbl)
                                .size(FONT_XS)
                                .strong()
                                .color(TEXT_SECONDARY),
                        )
                        .fill(SUBTLE_BG)
                        .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                        .corner_radius(6.0)
                        .min_size(Vec2::new(86.0, 28.0));
                        if ui.add(btn).clicked() {
                            self.show_onboarding_guide = !self.show_onboarding_guide;
                        }
                    });
                });

                if !self.show_onboarding_guide {
                    return;
                }

                ui.add_space(8.0);
                draw_divider(ui, SUBTLE_BG);
                ui.add_space(10.0);

                let steps = [
                    (
                        "STEP 1",
                        "Add or Auto-Import Project",
                        "Click '+ ADD' in the sidebar or auto-import a deploy-*.ps1 script to populate project directories, remote server configs, and steps.",
                        BLUE_PILL_BG,
                        BLUE_PILL_TEXT,
                    ),
                    (
                        "STEP 2",
                        "Setup OS Credentials",
                        "Store your SSH password securely in Windows Credential Manager, Linux libsecret, or macOS Keychain — zero plain text passwords.",
                        TEAL_PILL_BG,
                        TEAL_PILL_TEXT,
                    ),
                    (
                        "STEP 3",
                        "Configure Pipeline Steps",
                        "Customize Local Command, Verify Path, SCP Upload, and Remote SSH steps with real-time syntax highlighting and {template} variables.",
                        PURPLE_PILL_BG,
                        PURPLE_PILL_TEXT,
                    ),
                    (
                        "STEP 4",
                        "Deploy, Bulk Deploy & Rollback",
                        "Pick target Git branches, verify auto-bumped versions, run sequential Bulk Deployments, or perform 1-click Rollback anytime.",
                        SUCCESS_PILL_BG,
                        SUCCESS_PILL_TEXT,
                    ),
                ];

                let spacing = 12.0;
                let step_w = ((ui.available_width() - spacing * 3.0) / 4.0).max(185.0);
                const ONBOARDING_CARD_HEIGHT: f32 = 138.0;
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(spacing, 10.0);
                    for (badge, title, desc, bg, fg) in steps {
                        egui::Frame::new()
                            .fill(SUBTLE_BG)
                            .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                            .corner_radius(8.0)
                            .inner_margin(Margin::symmetric(14, 12))
                            .show(ui, |ui| {
                                ui.set_width((step_w - 28.0).max(150.0));
                                ui.set_height(ONBOARDING_CARD_HEIGHT);
                                render_pill(ui, badge, bg, fg);
                                ui.add_space(6.0);
                                ui.label(
                                    RichText::new(title)
                                        .size(FONT_MD)
                                        .strong()
                                        .color(TEXT_PRIMARY),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(desc).size(FONT_SM).color(TEXT_SECONDARY),
                                );
                            });
                    }
                });
            });
    }

    pub(crate) fn render_os_credentials_guideline_card(&mut self, ui: &mut egui::Ui) {
        let target_example = if self.cred_guide_target_input.trim().is_empty() {
            "prod-ssh-server".to_string()
        } else {
            self.cred_guide_target_input.trim().to_string()
        };
        let user_example = if self.cred_guide_user_input.trim().is_empty() {
            "deploy".to_string()
        } else {
            self.cred_guide_user_input.trim().to_string()
        };
        let vault_ready = read_os_vault_credential(&target_example).is_some();

        egui::Frame::new()
            .fill(CARD_BG)
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(191, 219, 254)))
            .corner_radius(10.0)
            .inner_margin(Margin::symmetric(20, 14))
            .show(ui, |ui| {
                horizontal_row(ui, 32.0, |ui| {
                    let (ic_r, _) =
                        ui.allocate_exact_size(Vec2::new(20.0, 20.0), Sense::hover());
                    draw_outline_icon(ui.painter(), ic_r.center(), OutlineIcon::Key, PRIMARY_BLUE);
                    ui.label(
                        RichText::new("OS CREDENTIALS SETUP GUIDELINE (WINDOWS | LINUX | MAC)")
                            .size(FONT_MD)
                            .strong()
                            .color(PRIMARY_BLUE),
                    );
                    if vault_ready {
                        render_pill(
                            ui,
                            "TARGET FOUND IN VAULT",
                            SUCCESS_PILL_BG,
                            SUCCESS_PILL_TEXT,
                        );
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let toggle_lbl = if self.show_credential_guide {
                            "Hide Guide"
                        } else {
                            "Show Guide"
                        };
                        let btn = egui::Button::new(
                            RichText::new(toggle_lbl)
                                .size(FONT_XS)
                                .strong()
                                .color(TEXT_SECONDARY),
                        )
                        .fill(SUBTLE_BG)
                        .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                        .corner_radius(6.0)
                        .min_size(Vec2::new(86.0, 28.0));
                        if ui.add(btn).clicked() {
                            self.show_credential_guide = !self.show_credential_guide;
                        }
                    });
                });

                if !self.show_credential_guide {
                    return;
                }

                ui.add_space(6.0);
                ui.label(
                    RichText::new(
                        "Shiply reads your SSH password securely from your operating system's native credential vault so you never hardcode passwords in scripts or config files.",
                    )
                    .size(FONT_SM)
                    .color(TEXT_SECONDARY),
                );
                ui.add_space(10.0);

                // Interactive Target & Username customizer + Verify button
                horizontal_row(ui, 34.0, |ui| {
                    ui.label(
                        RichText::new("Vault Target:")
                            .size(FONT_SM)
                            .strong()
                            .color(TEXT_SECONDARY),
                    );
                    styled_text_edit(
                        ui,
                        "cred_guide_target_input",
                        &mut self.cred_guide_target_input,
                        170.0,
                        Some("e.g. prod-ssh-server"),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("SSH User:")
                            .size(FONT_SM)
                            .strong()
                            .color(TEXT_SECONDARY),
                    );
                    styled_text_edit(
                        ui,
                        "cred_guide_user_input",
                        &mut self.cred_guide_user_input,
                        130.0,
                        Some("e.g. deploy"),
                    );
                    ui.add_space(6.0);
                    if outline_icon_button(
                        ui,
                        "verify_cred_guide_target_btn",
                        OutlineIcon::Refresh,
                        Some("Verify in OS Vault"),
                        TEXT_PRIMARY,
                        SUBTLE_BG,
                        INPUT_BG_HOVER,
                        CARD_BORDER,
                        true,
                    )
                    .clicked()
                    {
                        if read_os_vault_credential(&target_example).is_some() {
                            self.status_banner = Some((
                                format!("Verified! Credential target '{target_example}' is ready in your OS Vault."),
                                true,
                            ));
                        } else {
                            self.status_banner = Some((
                                format!("Target '{target_example}' was not found in your OS Vault yet. Follow the steps below to add it."),
                                false,
                            ));
                        }
                    }
                });

                ui.add_space(10.0);

                // Platform Tab Bar: Windows | Linux | Mac (uniform gap & styling)
                horizontal_row(ui, 34.0, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
                    for (tab, label) in [
                        (CredentialOsTab::Windows, "Windows (Credential Manager)"),
                        (CredentialOsTab::Linux, "Linux (libsecret / secret-tool)"),
                        (CredentialOsTab::Mac, "Mac (macOS Keychain)"),
                    ] {
                        let is_active = self.cred_guide_os_tab == tab;
                        if render_uniform_tab_button(
                            ui,
                            ("cred_os_tab", label),
                            label,
                            is_active,
                            32.0,
                        )
                        .clicked()
                        {
                            self.cred_guide_os_tab = tab;
                        }
                    }
                });

                ui.add_space(10.0);

                egui::Frame::new()
                    .fill(SUBTLE_BG)
                    .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                    .corner_radius(8.0)
                    .inner_margin(Margin::symmetric(16, 12))
                    .show(ui, |ui| match self.cred_guide_os_tab {
                        CredentialOsTab::Windows => {
                            let cmdkey_cmd = format!(
                                "cmdkey /generic:\"{target_example}\" /user:\"{user_example}\" /pass:\"YOUR_SSH_PASSWORD\""
                            );
                            horizontal_row(ui, 30.0, |ui| {
                                ui.label(
                                    RichText::new("Option 1 — Windows Credential Manager (GUI):")
                                        .size(FONT_SM)
                                        .strong()
                                        .color(TEXT_PRIMARY),
                                );
                                #[cfg(windows)]
                                {
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        let open_cm_btn = egui::Button::new(
                                            RichText::new("Open Windows Credential Manager")
                                                .size(FONT_XS)
                                                .strong()
                                                .color(Color32::WHITE),
                                        )
                                        .fill(PRIMARY_BLUE)
                                        .corner_radius(6.0)
                                        .min_size(Vec2::new(210.0, 28.0));
                                        if ui.add(open_cm_btn).clicked() {
                                            open_windows_credential_manager();
                                        }
                                    });
                                }
                            });
                            ui.label(
                                RichText::new(format!(
                                    "  1. Open Control Panel -> User Accounts -> Credential Manager -> Windows Credentials.\n  2. Under 'Generic Credentials', click 'Add a generic credential'.\n  3. Set Internet or network address = '{target_example}', User name = '{user_example}', and Password = your SSH password.\n  4. Click OK, then click 'Verify in OS Vault' above."
                                ))
                                .size(FONT_SM)
                                .color(TEXT_SECONDARY),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new("Option 2 — PowerShell / CMD One-Line Command:")
                                    .size(FONT_SM)
                                    .strong()
                                    .color(TEXT_PRIMARY),
                            );
                            ui.add_space(4.0);
                            horizontal_row(ui, 34.0, |ui| {
                                let mut preview = cmdkey_cmd.clone();
                                syntax_highlighted_text_edit(
                                    ui,
                                    "cred_cmdkey_preview",
                                    &mut preview,
                                    (ui.available_width() - 125.0).max(220.0),
                                    None,
                                );
                                let copy_btn = egui::Button::new(
                                    RichText::new("Copy Command")
                                        .size(FONT_SM)
                                        .strong()
                                        .color(TEXT_PRIMARY),
                                )
                                .fill(CARD_BG)
                                .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                                .corner_radius(6.0)
                                .min_size(Vec2::new(110.0, 32.0));
                                if ui.add(copy_btn).clicked() {
                                    ui.ctx().copy_text(cmdkey_cmd);
                                    self.status_banner = Some((
                                        "Copied Windows cmdkey command to clipboard!".to_string(),
                                        true,
                                    ));
                                }
                            });
                        }
                        CredentialOsTab::Linux => {
                            let linux_cmd = format!(
                                "secret-tool store --label=\"Shiply ({target_example})\" target \"{target_example}\" username \"{user_example}\""
                            );
                            ui.label(
                                RichText::new("Linux Secret Service (libsecret / GNOME Keyring / KWallet):")
                                    .size(FONT_SM)
                                    .strong()
                                    .color(TEXT_PRIMARY),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "  1. Ensure libsecret-tools is installed (Debian/Ubuntu: sudo apt install libsecret-tools | Fedora: sudo dnf install libsecret).\n  2. Run the command below in your terminal and enter your SSH password when prompted.\n  3. In your Project's Environment & SSH Settings, set OS Credential Vault Target = '{target_example}'."
                                ))
                                .size(FONT_SM)
                                .color(TEXT_SECONDARY),
                            );
                            ui.add_space(8.0);
                            horizontal_row(ui, 34.0, |ui| {
                                let mut preview = linux_cmd.clone();
                                syntax_highlighted_text_edit(
                                    ui,
                                    "cred_linux_preview",
                                    &mut preview,
                                    (ui.available_width() - 125.0).max(220.0),
                                    None,
                                );
                                let copy_btn = egui::Button::new(
                                    RichText::new("Copy Command")
                                        .size(FONT_SM)
                                        .strong()
                                        .color(TEXT_PRIMARY),
                                )
                                .fill(CARD_BG)
                                .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                                .corner_radius(6.0)
                                .min_size(Vec2::new(110.0, 32.0));
                                if ui.add(copy_btn).clicked() {
                                    ui.ctx().copy_text(linux_cmd);
                                    self.status_banner = Some((
                                        "Copied Linux secret-tool command to clipboard!".to_string(),
                                        true,
                                    ));
                                }
                            });
                        }
                        CredentialOsTab::Mac => {
                            let mac_cmd = format!(
                                "security add-generic-password -U -s \"{target_example}\" -a \"{user_example}\" -w \"YOUR_SSH_PASSWORD\""
                            );
                            ui.label(
                                RichText::new("macOS Keychain Access (GUI or Terminal CLI):")
                                    .size(FONT_SM)
                                    .strong()
                                    .color(TEXT_PRIMARY),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "  1. GUI: Open Keychain Access -> File -> New Password Item. Set Keychain Item Name = '{target_example}', Account Name = '{user_example}', and enter your SSH password.\n  2. CLI: Or run the one-line macOS security command below in Terminal.\n  3. In your Project's Environment & SSH Settings, set OS Credential Vault Target = '{target_example}'."
                                ))
                                .size(FONT_SM)
                                .color(TEXT_SECONDARY),
                            );
                            ui.add_space(8.0);
                            horizontal_row(ui, 34.0, |ui| {
                                let mut preview = mac_cmd.clone();
                                syntax_highlighted_text_edit(
                                    ui,
                                    "cred_mac_preview",
                                    &mut preview,
                                    (ui.available_width() - 125.0).max(220.0),
                                    None,
                                );
                                let copy_btn = egui::Button::new(
                                    RichText::new("Copy Command")
                                        .size(FONT_SM)
                                        .strong()
                                        .color(TEXT_PRIMARY),
                                )
                                .fill(CARD_BG)
                                .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                                .corner_radius(6.0)
                                .min_size(Vec2::new(110.0, 32.0));
                                if ui.add(copy_btn).clicked() {
                                    ui.ctx().copy_text(mac_cmd);
                                    self.status_banner = Some((
                                        "Copied macOS Keychain security command to clipboard!".to_string(),
                                        true,
                                    ));
                                }
                            });
                        }
                    });
            });
    }
    pub(crate) fn render_project_workspace(&mut self, ui: &mut egui::Ui, project_id: &str) {
        if self.cached_project_id.as_deref() != Some(project_id) {
            self.refresh_selected_project_metadata(true);
        }

        let Some(proj_idx) = self
            .storage
            .projects
            .iter()
            .position(|p| p.id == project_id)
        else {
            ui.label("Project not found.");
            return;
        };

        let proj_name = self.storage.projects[proj_idx].name.clone();

        // If currently in the loading skeleton transition for switching active project:
        if self.project_switch_loading_id.as_deref() == Some(project_id) {
            if let Some(start) = self.project_switch_loading_start {
                if start.elapsed() < Duration::from_millis(300) {
                    render_project_workspace_skeleton(ui, &proj_name);
                    return;
                }
            }
            self.project_switch_loading_id = None;
            self.project_switch_loading_start = None;
        }

        if self.storage.projects[proj_idx]
            .get_env(&self.selected_env_key)
            .is_none()
        {
            self.selected_env_key = self.storage.projects[proj_idx]
                .environments
                .first()
                .map(|e| e.key.clone())
                .unwrap_or_default();
        }

        let proj_desc = self.storage.projects[proj_idx].description.clone();
        let proj_path = self.storage.projects[proj_idx].local_path.clone();
        let path_exists = PathBuf::from(&proj_path).exists();
        let avail_w = ui.available_width();
        let theme = self.theme();

        // 1. Top Project Header Bar (Fixed 36px height — all right-side controls vertically centered at 32px height)
        let mut delete_project_clicked = false;
        let mut edit_project_clicked = false;
        let mut duplicate_project_clicked = false;
        let mut selected_branch_changed = None;

        horizontal_row(ui, 36.0, |ui| {
            let title_resp = ui.add(
                egui::Label::new(
                    RichText::new(&proj_name)
                        .size(FONT_XL)
                        .strong()
                        .color(TEXT_PRIMARY),
                )
                .sense(Sense::click()),
            );
            title_resp.context_menu(|ui| {
                ui.set_min_width(180.0);
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);
                ui.spacing_mut().button_padding = Vec2::new(10.0, 6.0);

                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("✎  Edit Project")
                                .size(POPUP_FONT_SIZE)
                                .color(TEXT_PRIMARY),
                        )
                        .fill(CARD_BG),
                    )
                    .clicked()
                {
                    edit_project_clicked = true;
                    ui.close_menu();
                }

                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("⧉  Duplicate Project")
                                .size(POPUP_FONT_SIZE)
                                .color(TEXT_PRIMARY),
                        )
                        .fill(CARD_BG),
                    )
                    .clicked()
                {
                    duplicate_project_clicked = true;
                    ui.close_menu();
                }

                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("↻  Refresh Metadata")
                                .size(POPUP_FONT_SIZE)
                                .color(TEXT_SECONDARY),
                        )
                        .fill(CARD_BG),
                    )
                    .clicked()
                {
                    self.refresh_selected_project_metadata(true);
                    ui.close_menu();
                }

                ui.separator();

                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("🗑  Delete Project")
                                .size(POPUP_FONT_SIZE)
                                .color(FAIL_PILL_TEXT),
                        )
                        .fill(CARD_BG),
                    )
                    .clicked()
                {
                    delete_project_clicked = true;
                    ui.close_menu();
                }
            });

            let (r_txt, r_bg, r_fg) = if path_exists {
                ("READY", SUCCESS_PILL_BG, SUCCESS_PILL_TEXT)
            } else {
                ("PATH MISSING", FAIL_PILL_BG, FAIL_PILL_TEXT)
            };
            render_pill(ui, r_txt, r_bg, r_fg);

            let right_w = ui.available_width();
            let screen_w = ui.ctx().screen_rect().width();
            let is_narrow_header = screen_w < 720.0 || right_w < 340.0;

            ui.allocate_ui_with_layout(
                Vec2::new(right_w, 36.0),
                Layout::right_to_left(Align::Center),
                |ui| {
                    ui.set_min_height(36.0);
                    ui.set_max_height(36.0);
                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);

                    if is_narrow_header {
                        // Below 720px: move Edit/Duplicate/Refresh/Delete into a more-horizontal menu
                        let more_resp = outline_icon_button(
                            ui,
                            ("hdr_more_menu_btn", project_id),
                            OutlineIcon::MoreHorizontal,
                            None,
                            theme.text_muted,
                            theme.surface,
                            theme.surface_2,
                            theme.border,
                            true,
                        )
                        .on_hover_text("Project actions");

                        more_resp.context_menu(|ui| {
                            ui.set_min_width(160.0);
                            ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);
                            ui.spacing_mut().button_padding = Vec2::new(10.0, 6.0);

                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("✎  Edit Project")
                                            .size(POPUP_FONT_SIZE)
                                            .color(theme.text),
                                    )
                                    .fill(theme.surface),
                                )
                                .clicked()
                            {
                                edit_project_clicked = true;
                                ui.close_menu();
                            }

                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("⧉  Duplicate Project")
                                            .size(POPUP_FONT_SIZE)
                                            .color(theme.text),
                                    )
                                    .fill(theme.surface),
                                )
                                .clicked()
                            {
                                duplicate_project_clicked = true;
                                ui.close_menu();
                            }

                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("↻  Refresh Metadata")
                                            .size(POPUP_FONT_SIZE)
                                            .color(theme.text_muted),
                                    )
                                    .fill(theme.surface),
                                )
                                .clicked()
                            {
                                self.refresh_selected_project_metadata(true);
                                ui.close_menu();
                            }

                            ui.separator();

                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("🗑  Delete Project")
                                            .size(POPUP_FONT_SIZE)
                                            .color(theme.danger),
                                    )
                                    .fill(theme.surface),
                                )
                                .clicked()
                            {
                                delete_project_clicked = true;
                                ui.close_menu();
                            }
                        });
                    } else {
                        if outline_icon_button(
                            ui,
                            ("hdr_delete_proj", project_id),
                            OutlineIcon::Trash,
                            Some("Delete"),
                            theme.danger,
                            theme.danger_soft,
                            theme.danger_soft,
                            theme.danger,
                            true,
                        )
                        .on_hover_text("Delete this project")
                        .clicked()
                        {
                            delete_project_clicked = true;
                        }

                        if outline_icon_button(
                            ui,
                            ("hdr_refresh_proj", project_id),
                            OutlineIcon::Refresh,
                            Some("Refresh"),
                            theme.text_muted,
                            theme.surface,
                            theme.surface_2,
                            theme.border,
                            true,
                        )
                        .on_hover_text("Refresh Git branches, commits, and package.json version")
                        .clicked()
                        {
                            self.refresh_selected_project_metadata(true);
                        }

                        if outline_icon_button(
                            ui,
                            ("hdr_dup_proj", project_id),
                            OutlineIcon::Copy,
                            Some("Duplicate"),
                            theme.text,
                            theme.surface,
                            theme.surface_2,
                            theme.border,
                            true,
                        )
                        .on_hover_text("Duplicate this project and all its environments")
                        .clicked()
                        {
                            duplicate_project_clicked = true;
                        }

                        if outline_icon_button(
                            ui,
                            ("hdr_edit_proj", project_id),
                            OutlineIcon::Edit,
                            Some("Edit Project"),
                            theme.text,
                            theme.surface,
                            theme.surface_2,
                            theme.border,
                            true,
                        )
                        .on_hover_text("Edit project name, local directory path, or description")
                        .clicked()
                        {
                            edit_project_clicked = true;
                        }
                    }

                    // Git Branch Selector Dropdown — exact 32px height to match Edit/Refresh/Delete buttons
                    if !self.cached_git_branches.is_empty() {
                        let current_b = if self.selected_git_branch.is_empty() {
                            "main".to_string()
                        } else {
                            self.selected_git_branch.clone()
                        };
                        let commit_suffix = self
                            .cached_git_meta
                            .commit_short
                            .as_deref()
                            .map(|c| format!(" @ {c}"))
                            .unwrap_or_default();

                        ui.scope(|ui| {
                            ui.spacing_mut().interact_size.y = 32.0;
                            ui.spacing_mut().button_padding = Vec2::new(10.0, 6.0);
                            egui::ComboBox::from_id_salt("header_git_branch_selector")
                                .selected_text(
                                    RichText::new(format!("Branch: {current_b}{commit_suffix}"))
                                        .size(FONT_SM)
                                        .strong()
                                        .color(theme.text),
                                )
                                .width(if is_narrow_header { 130.0 } else { 185.0 })
                                .show_ui(ui, |ui| {
                                    for branch in self.cached_git_branches.clone() {
                                        let is_sel = self.selected_git_branch == branch;
                                        if ui.selectable_label(is_sel, &branch).clicked() && !is_sel
                                        {
                                            selected_branch_changed = Some(branch);
                                        }
                                    }
                                });
                        });
                    }
                },
            );
        });

        if edit_project_clicked {
            self.open_edit_project_modal(proj_idx);
        }

        if duplicate_project_clicked {
            if let Some(new_proj) = self.storage.duplicate_project(project_id) {
                self.selection = SidebarSelection::Project(new_proj.id.clone());
                self.refresh_selected_project_metadata(true);
                self.status_banner = Some((
                    format!("Duplicated project successfully as '{}'", new_proj.name),
                    true,
                ));
            }
        }

        if delete_project_clicked {
            self.pending_delete_confirm = Some(ConfirmDeleteAction::Project {
                project_id: project_id.to_string(),
                project_name: proj_name.clone(),
            });
        }

        if let Some(new_branch) = selected_branch_changed {
            self.selected_git_branch = new_branch.clone();
            let path = PathBuf::from(&proj_path);
            match checkout_git_branch(&path, &new_branch) {
                Ok(msg) => {
                    self.refresh_selected_project_metadata(false);
                    self.status_banner = Some((msg, true));
                }
                Err(err) => {
                    self.status_banner = Some((
                        format!("Selected branch '{new_branch}' for deploy ({err})"),
                        false,
                    ));
                }
            }
        }

        ui.add_space(8.0);
        draw_divider(ui, CARD_BORDER);
        ui.add_space(6.0);

        // 2. Sub-header description & folder path row (Fixed 24px height)
        let (sub_rect, _) = ui.allocate_exact_size(Vec2::new(avail_w, 24.0), Sense::hover());
        draw_icon_server(
            ui.painter(),
            Pos2::new(sub_rect.left() + 8.0, sub_rect.center().y),
            TEXT_MUTED,
        );
        let desc_galley = ui.painter().layout_no_wrap(
            proj_desc,
            FontId::proportional(FONT_SM),
            TEXT_SECONDARY,
        );
        let desc_w = desc_galley.size().x;
        ui.painter().galley(
            Pos2::new(
                sub_rect.left() + 22.0,
                sub_rect.center().y - desc_galley.size().y / 2.0,
            ),
            desc_galley,
            TEXT_SECONDARY,
        );

        let sep_x = sub_rect.left() + 22.0 + desc_w + 14.0;
        ui.painter().line_segment(
            [
                Pos2::new(sep_x, sub_rect.top() + 5.0),
                Pos2::new(sep_x, sub_rect.bottom() - 5.0),
            ],
            Stroke::new(1.0_f32, CARD_BORDER),
        );
        draw_icon_folder(
            ui.painter(),
            Pos2::new(sep_x + 16.0, sub_rect.center().y),
            TEXT_MUTED,
        );
        ui.painter().text(
            Pos2::new(sep_x + 30.0, sub_rect.center().y),
            Align2::LEFT_CENTER,
            &proj_path,
            FontId::proportional(FONT_SM),
            TEXT_SECONDARY,
        );

        ui.add_space(14.0);

        // 3. Environment Cards Grid — Responsive columns (3 columns fill workspace width >= 650px, 2 columns >= 440px, 1 column < 440px)
        let env_summaries: Vec<(String, String)> = self.storage.projects[proj_idx]
            .environments
            .iter()
            .map(|e| (e.key.clone(), e.name.clone()))
            .collect();

        let mut new_selected_env_key = None;
        let mut delete_env_key = None;
        let card_spacing = 14.0;
        let cols = if avail_w >= 650.0 {
            3
        } else if avail_w >= 440.0 {
            2
        } else {
            1
        };
        let card_w = if cols == 1 {
            avail_w
        } else {
            ((avail_w - card_spacing * (cols - 1) as f32) / cols as f32).floor()
        };

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(card_spacing, 14.0);

            for (env_key, env_name) in &env_summaries {
                let is_active = &self.selected_env_key == env_key;
                let last_ok = self.storage.last_succeeded_deployment(project_id, env_key);
                let latest_any = self.storage.latest_deployment(project_id, env_key);

                let (card_rect, card_resp) = ui.allocate_exact_size(
                    Vec2::new(card_w, ENV_CARD_HEIGHT),
                    Sense::click(),
                );
                let card_resp = card_resp.on_hover_cursor(egui::CursorIcon::PointingHand);

                // Outline Trash delete button rect at top-right of the card
                let del_rect = Rect::from_center_size(
                    Pos2::new(card_rect.right() - 22.0, card_rect.top() + 24.0),
                    Vec2::new(24.0, 24.0),
                );
                let del_resp = ui
                    .interact(
                        del_rect,
                        egui::Id::new(("del_env_card_btn", project_id, env_key)),
                        Sense::click(),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand);

                let stroke = if is_active {
                    Stroke::new(2.0_f32, theme.primary)
                } else if card_resp.hovered() {
                    Stroke::new(1.5_f32, theme.primary_hover)
                } else {
                    Stroke::new(1.0_f32, theme.border)
                };

                draw_elevated_card(ui.painter(), card_rect, theme.surface_tint_top, stroke);

                // Top-left: Environment pastel pill
                let is_prod = env_key.to_lowercase().contains("prod");
                let (pill_bg, pill_fg) = if is_prod {
                    (theme.purple_soft, theme.purple)
                } else {
                    (theme.primary_soft, theme.primary)
                };
                paint_pill_left_aligned(
                    ui.painter(),
                    Pos2::new(card_rect.left() + 16.0, card_rect.top() + 24.0),
                    &env_name.to_uppercase(),
                    pill_bg,
                    pill_fg,
                );

                // Top-right (left of delete button): Status pill or "NOT DEPLOYED"
                if let Some(latest) = latest_any {
                    let (s_bg, s_fg) = status_pill_colors(latest.status);
                    if latest.status == DeploymentStatus::Running {
                        ui.ctx().request_repaint();
                        let time = ui.input(|i| i.time);
                        paint_running_pill_right_aligned(
                            ui.painter(),
                            Pos2::new(del_rect.left() - 6.0, card_rect.top() + 24.0),
                            &latest.status.label().to_uppercase(),
                            s_bg,
                            s_fg,
                            time,
                        );
                    } else {
                        paint_pill_right_aligned(
                            ui.painter(),
                            Pos2::new(del_rect.left() - 6.0, card_rect.top() + 24.0),
                            &latest.status.label().to_uppercase(),
                            s_bg,
                            s_fg,
                        );
                    }
                } else {
                    ui.painter().text(
                        Pos2::new(del_rect.left() - 8.0, card_rect.top() + 24.0),
                        Align2::RIGHT_CENTER,
                        "NOT DEPLOYED",
                        FontId::proportional(FONT_XS),
                        theme.text_muted,
                    );
                }

                // Paint the outline Trash delete environment button
                ui.painter().rect_filled(
                    del_rect,
                    CornerRadius::same(5),
                    if del_resp.hovered() {
                        theme.danger_soft
                    } else {
                        theme.surface_2
                    },
                );
                ui.painter().rect_stroke(
                    del_rect,
                    CornerRadius::same(5),
                    Stroke::new(
                        1.0_f32,
                        if del_resp.hovered() {
                            theme.danger
                        } else {
                            theme.border
                        },
                    ),
                    StrokeKind::Inside,
                );
                draw_outline_icon(
                    ui.painter(),
                    del_rect.center(),
                    OutlineIcon::Trash,
                    if del_resp.hovered() {
                        theme.danger
                    } else {
                        theme.text_muted
                    },
                );

                // Bottom area of card: Active version or "No Active Build"
                if let Some(ok_dep) = last_ok {
                    ui.painter().text(
                        Pos2::new(card_rect.left() + 16.0, card_rect.top() + 58.0),
                        Align2::LEFT_CENTER,
                        format!("Active: v{}", ok_dep.version),
                        FontId::proportional(FONT_LG),
                        theme.text,
                    );
                    ui.painter().text(
                        Pos2::new(card_rect.left() + 16.0, card_rect.top() + 78.0),
                        Align2::LEFT_CENTER,
                        "CURRENT VERSION",
                        FontId::monospace(FONT_XS),
                        theme.text_muted,
                    );
                } else {
                    ui.painter().text(
                        Pos2::new(card_rect.left() + 16.0, card_rect.top() + 62.0),
                        Align2::LEFT_CENTER,
                        "No Active Build",
                        FontId::proportional(FONT_LG),
                        theme.text_subtle,
                    );
                }

                if del_resp.on_hover_text("Delete this environment").clicked() {
                    delete_env_key = Some((env_key.clone(), env_name.clone()));
                } else if card_resp.clicked() {
                    new_selected_env_key = Some(env_key.clone());
                }
            }

            // "+ ADD / IMPORT ENVIRONMENT" Card with exact same 96px height!
            let (add_rect, add_resp) = ui.allocate_exact_size(
                Vec2::new(card_w, ENV_CARD_HEIGHT),
                Sense::click(),
            );
            let add_resp = add_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
            let add_bg = if add_resp.hovered() {
                theme.surface
            } else {
                theme.surface_2
            };
            let add_stroke = if add_resp.hovered() {
                Stroke::new(1.5_f32, theme.primary)
            } else {
                Stroke::new(1.0_f32, theme.border)
            };
            ui.painter()
                .rect_filled(add_rect, CornerRadius::same(10), add_bg);
            ui.painter().rect_stroke(
                add_rect,
                CornerRadius::same(10),
                add_stroke,
                StrokeKind::Inside,
            );

            let plus_center = Pos2::new(add_rect.center().x, add_rect.center().y - 10.0);
            draw_icon_plus(ui.painter(), plus_center, theme.text_muted);
            ui.painter().text(
                Pos2::new(add_rect.center().x, add_rect.center().y + 14.0),
                Align2::CENTER_CENTER,
                "ADD / IMPORT ENVIRONMENT",
                FontId::proportional(FONT_SM),
                theme.text_muted,
            );

            if add_resp.clicked() {
                self.new_env_clone_from = self.selected_env_key.clone();
                self.show_new_env_modal = true;
            }
        });

        if let Some((del_key, del_name)) = delete_env_key {
            self.pending_delete_confirm = Some(ConfirmDeleteAction::Environment {
                project_id: project_id.to_string(),
                project_name: proj_name.clone(),
                env_key: del_key,
                env_name: del_name,
            });
        } else if let Some(env_key) = new_selected_env_key {
            self.servers_edit_buffer = None;
            self.selected_env_key = env_key;
            self.refresh_selected_project_metadata(true);
        }

        if self.storage.projects[proj_idx].environments.is_empty() {
            ui.add_space(16.0);
            egui::Frame::new()
                .fill(CARD_BG)
                .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                .corner_radius(10.0)
                .inner_margin(Margin::symmetric(24, 24))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("No Environments in This Project Yet")
                                .size(FONT_LG)
                                .strong()
                                .color(TEXT_PRIMARY),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(
                                "Click 'ADD / IMPORT ENVIRONMENT' above to import a deploy-dev.ps1 / deploy-prod.ps1 script or add a custom environment.",
                            )
                            .size(FONT_MD)
                            .color(TEXT_SECONDARY),
                        );
                    });
                });
            return;
        }

        ui.add_space(16.0);

        // 4. Version Control, Rollback & Deploy Action Card (Fixed 78px height)
        let active_env_name = self.storage.projects[proj_idx]
            .get_env(&self.selected_env_key)
            .map(|e| e.name.clone())
            .unwrap_or_else(|| self.selected_env_key.clone());
        let rollback_candidate = self
            .storage
            .previous_rollback_candidate(project_id, &self.selected_env_key)
            .cloned();
        let mut sync_trigger_rect = None;
        self.render_deploy_action_bar(
            ui,
            &active_env_name,
            rollback_candidate,
            &mut sync_trigger_rect,
        );

        ui.add_space(18.0);

        // 5. Clean Section Tabs — Identical 10px gap and consistent bordered styling
        let enabled_count = self.storage.projects[proj_idx]
            .get_env(&self.selected_env_key)
            .map(|e| e.steps.iter().filter(|s| s.enabled).count())
            .unwrap_or(0);
        let total_steps = self.storage.projects[proj_idx]
            .get_env(&self.selected_env_key)
            .map(|e| e.steps.len())
            .unwrap_or(0);
        let history_count = self
            .storage
            .filter_deployments(Some(project_id), Some(&self.selected_env_key), None)
            .len();

        egui::ScrollArea::horizontal()
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                horizontal_row(ui, TAB_HEIGHT, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(10.0, 0.0);
                    for (tab, label) in [
                        (
                            WorkspaceTab::PipelineSteps,
                            format!("Pipeline Steps ({enabled_count}/{total_steps})"),
                        ),
                        (
                            WorkspaceTab::HistoryAndLogs,
                            format!("Deployments & Logs ({history_count})"),
                        ),
                        (
                            WorkspaceTab::ServerSettings,
                            "Environment & SSH Settings".to_string(),
                        ),
                    ] {
                        let is_active = self.active_tab == tab;
                        let btn_resp = render_uniform_tab_button(
                            ui,
                            ("workspace_tab_btn", project_id, &label),
                            &label,
                            is_active,
                            TAB_HEIGHT,
                        );
                        if is_active {
                            btn_resp.scroll_to_me(Some(Align::Center));
                        }
                        if btn_resp.clicked() {
                            self.active_tab = tab;
                        }
                    }
                });
            });

        ui.add_space(14.0);

        // 6. Render Active Full-Width Tab Content
        match self.active_tab {
            WorkspaceTab::PipelineSteps => {
                self.render_pipeline_steps_tab(ui, proj_idx);
            }
            WorkspaceTab::HistoryAndLogs => {
                let pid = self.storage.projects[proj_idx].id.clone();
                self.render_history_and_logs_view(ui, Some(&pid));
            }
            WorkspaceTab::ServerSettings => {
                self.render_server_settings_tab(ui, proj_idx);
            }
        }

        if let Some(r) = sync_trigger_rect {
            self.render_sync_version_dropdown(ui.ctx(), r, project_id);
        }
    }

    pub fn sync_version_candidates(&self, project_id: &str) -> Vec<SyncVersionCandidate> {
        let mut candidates = Vec::new();
        let Some(proj) = self.storage.projects.iter().find(|p| p.id == project_id) else {
            return candidates;
        };

        // 1. Collect latest deployed versions from other environments in this project
        for env in &proj.environments {
            if env.key == self.selected_env_key {
                continue;
            }
            if let Some(dep) = self
                .storage
                .last_succeeded_deployment(project_id, &env.key)
                .or_else(|| self.storage.latest_deployment(project_id, &env.key))
            {
                let ver = dep.version.trim().to_string();
                if !ver.is_empty()
                    && !candidates
                        .iter()
                        .any(|c: &SyncVersionCandidate| c.version == ver && c.source_name == env.name)
                {
                    candidates.push(SyncVersionCandidate {
                        source_name: env.name.clone(),
                        env_key: Some(env.key.clone()),
                        version: ver,
                        is_latest: false,
                    });
                }
            }
        }

        // 2. Fall back to package.json if present and not already in candidates
        if let Some(pkg_ver) = &self.cached_pkg_version {
            let ver = pkg_ver.trim().to_string();
            if !ver.is_empty()
                && ver != "0.0.0"
                && !candidates.iter().any(|c| c.version == ver)
            {
                candidates.push(SyncVersionCandidate {
                    source_name: "package.json".to_string(),
                    env_key: None,
                    version: ver,
                    is_latest: false,
                });
            }
        }

        // 3. Mark the most recent candidate as is_latest
        if !candidates.is_empty() {
            let most_recent_ver = self
                .storage
                .deployments
                .iter()
                .filter(|d| d.project_id == project_id && d.environment != self.selected_env_key)
                .max_by_key(|d| d.started_at)
                .map(|d| d.version.clone());

            let mut matched = false;
            if let Some(recent_ver) = most_recent_ver {
                if let Some(cand) = candidates.iter_mut().find(|c| c.version == recent_ver) {
                    cand.is_latest = true;
                    matched = true;
                }
            }
            if !matched {
                candidates[0].is_latest = true;
            }
        }

        candidates
    }

    pub fn apply_synced_version(&mut self, version: &str, source_name: &str) {
        self.deploy_version_input = version.trim().to_string();
        self.status_banner = Some((
            format!("Synced version to v{} from {}.", version.trim(), source_name),
            true,
        ));
        self.sync_version_dropdown_open = false;
    }

    pub(crate) fn render_sync_version_dropdown(
        &mut self,
        ctx: &egui::Context,
        trigger_rect: Rect,
        project_id: &str,
    ) {
        if !self.sync_version_dropdown_open {
            return;
        }

        let candidates = self.sync_version_candidates(project_id);
        let item_h = 32.0_f32;
        let header_h = 28.0_f32;
        let pad = 6.0_f32;
        let row_count = if candidates.is_empty() { 1 } else { candidates.len() };
        let menu_w = 270.0_f32;
        let menu_h = pad * 2.0
            + header_h
            + (row_count as f32 * item_h)
            + ((row_count.saturating_sub(1)) as f32 * 4.0);

        let screen = ctx.screen_rect();
        let mut menu_pos = Pos2::new(trigger_rect.left(), trigger_rect.bottom() + 4.0);
        if menu_pos.x + menu_w > screen.right() - 10.0 {
            menu_pos.x = (trigger_rect.right() - menu_w).max(screen.left() + 10.0);
        }
        if menu_pos.y + menu_h > screen.bottom() - 10.0 {
            menu_pos.y = (trigger_rect.top() - menu_h - 4.0).max(screen.top() + 10.0);
        }
        let menu_rect = Rect::from_min_size(menu_pos, Vec2::new(menu_w, menu_h));

        // Outside click to close
        if ctx.input(|i| i.pointer.any_pressed()) {
            if let Some(ptr) = ctx.input(|i| i.pointer.latest_pos()) {
                if !menu_rect.contains(ptr) && !trigger_rect.contains(ptr) {
                    self.sync_version_dropdown_open = false;
                    return;
                }
            }
        }

        // Close on Esc / Tab
        if ctx.input(|i| i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Tab)) {
            self.sync_version_dropdown_open = false;
            return;
        }

        let mut chosen = None;

        egui::Area::new(egui::Id::new("sync_version_dropdown_area"))
            .order(egui::Order::Foreground)
            .fixed_pos(menu_pos)
            .show(ctx, |ui| {
                let shadow_rect = menu_rect.expand(2.0);
                ui.painter().rect_filled(
                    shadow_rect,
                    CornerRadius::same(8),
                    Color32::from_black_alpha(45),
                );

                ui.painter().rect_filled(menu_rect, CornerRadius::same(8), CARD_BG);
                ui.painter().rect_stroke(
                    menu_rect,
                    CornerRadius::same(8),
                    Stroke::new(1.0_f32, CARD_BORDER),
                    StrokeKind::Inside,
                );

                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(menu_rect.shrink(pad))
                        .layout(Layout::top_down(Align::Min)),
                );

                horizontal_row(&mut child, 22.0, |ui| {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("SYNC VERSION ACROSS ENVIRONMENTS")
                            .size(FONT_XS)
                            .strong()
                            .color(TEXT_MUTED),
                    );
                });
                draw_divider(&mut child, SUBTLE_BG);
                child.add_space(4.0);

                if candidates.is_empty() {
                    child.add_space(4.0);
                    child.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("No other deployed versions found.")
                                .size(FONT_SM)
                                .color(TEXT_MUTED),
                        );
                    });
                } else {
                    for cand in &candidates {
                        let is_current = self.deploy_version_input.trim() == cand.version.trim();
                        let (rect, resp) = child.allocate_exact_size(
                            Vec2::new(child.available_width(), 32.0),
                            Sense::click(),
                        );
                        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                        let bg = if resp.hovered() {
                            SUBTLE_BG
                        } else if cand.is_latest {
                            Color32::from_rgb(239, 246, 255)
                        } else {
                            Color32::TRANSPARENT
                        };
                        child.painter().rect_filled(rect, CornerRadius::same(6), bg);

                        // Icon on left
                        let icon_c = Pos2::new(rect.left() + 16.0, rect.center().y);
                        draw_outline_icon(
                            child.painter(),
                            icon_c,
                            if cand.env_key.is_some() {
                                OutlineIcon::Refresh
                            } else {
                                OutlineIcon::Folder
                            },
                            if cand.is_latest {
                                PRIMARY_BLUE
                            } else {
                                TEXT_SECONDARY
                            },
                        );

                        // Source name
                        let name_text = if cand.is_latest {
                            format!("{} (Latest)", cand.source_name)
                        } else {
                            cand.source_name.clone()
                        };
                        child.painter().text(
                            Pos2::new(rect.left() + 32.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            name_text,
                            FontId::proportional(FONT_SM),
                            if cand.is_latest {
                                PRIMARY_BLUE
                            } else {
                                TEXT_PRIMARY
                            },
                        );

                        // Version badge on right
                        let ver_text = format!("v{}", cand.version);
                        let ver_color = if is_current {
                            Color32::from_rgb(22, 163, 74)
                        } else {
                            TEXT_PRIMARY
                        };
                        child.painter().text(
                            Pos2::new(rect.right() - 10.0, rect.center().y),
                            Align2::RIGHT_CENTER,
                            ver_text,
                            FontId::proportional(FONT_SM),
                            ver_color,
                        );

                        if resp.clicked() {
                            chosen = Some((cand.version.clone(), cand.source_name.clone()));
                        }
                    }
                }
            });

        if let Some((v, s)) = chosen {
            self.apply_synced_version(&v, &s);
            ctx.request_repaint();
        }
    }

    pub(crate) fn render_deploy_action_bar(
        &mut self,
        ui: &mut egui::Ui,
        env_display_name: &str,
        rollback_candidate: Option<DeploymentRecord>,
        sync_trigger_rect_out: &mut Option<Rect>,
    ) {
        let mut trigger_deploy = false;
        let mut trigger_rollback = None;
        let avail_w = ui.available_width();
        let screen_w = ui.ctx().screen_rect().width();
        let theme = self.theme();

        let is_two_rows = screen_w < 900.0 || avail_w < 700.0;
        let is_mobile_deploy = screen_w < 600.0 || avail_w < 480.0;
        let bar_h = if is_two_rows { 122.0 } else { 78.0 };

        let (bar_rect, _) = ui.allocate_exact_size(Vec2::new(avail_w, bar_h), Sense::hover());
        draw_elevated_card(
            ui.painter(),
            bar_rect,
            theme.surface,
            Stroke::new(1.0_f32, theme.border),
        );

        ui.painter().text(
            Pos2::new(bar_rect.left() + 20.0, bar_rect.top() + 16.0),
            Align2::LEFT_CENTER,
            "VERSION CONTROL & RELEASE",
            FontId::monospace(FONT_XS),
            theme.text_muted,
        );

        let row1_rect = Rect::from_min_max(
            Pos2::new(bar_rect.left() + 20.0, bar_rect.top() + 28.0),
            Pos2::new(
                if is_two_rows { bar_rect.right() - 20.0 } else { bar_rect.right() - 280.0 },
                bar_rect.top() + 66.0,
            ),
        );

        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("deploy_bar_controls")
                .max_rect(row1_rect)
                .layout(Layout::left_to_right(Align::Center)),
        );

        styled_text_edit(
            &mut child,
            "deploy_version_input",
            &mut self.deploy_version_input,
            92.0,
            Some("1.0.0"),
        );
        let ver_id = child.make_persistent_id("deploy_version_input");
        self.is_version_input_focused = child.memory(|m| m.has_focus(ver_id));
        child.add_space(6.0);

        for (lbl, bump) in [
            ("+Patch", VersionBump::Patch),
            ("+Minor", VersionBump::Minor),
            ("+Major", VersionBump::Major),
        ] {
            let b = egui::Button::new(
                RichText::new(lbl)
                    .size(FONT_SM)
                    .strong()
                    .color(theme.text),
            )
            .fill(theme.surface)
            .stroke(Stroke::new(1.0_f32, theme.border_strong))
            .corner_radius(R_MD as u8)
            .min_size(Vec2::new(54.0, BTN_HEIGHT));
            if child.add(b).clicked() {
                self.deploy_version_input = bump_semver(&self.deploy_version_input, bump);
            }
        }

        child.add_space(4.0);
        let project_id_owned = match &self.selection {
            SidebarSelection::Project(pid) => pid.clone(),
            SidebarSelection::AllDeployments => String::new(),
        };
        let project_id = project_id_owned.as_str();
        let sync_candidates = self.sync_version_candidates(project_id);
        let sync_btn_text = "Sync";
        let sync_tooltip = if sync_candidates.is_empty() {
            "Sync version across environments (no other deployed versions found yet)".to_string()
        } else if let Some(latest) = sync_candidates.iter().find(|c| c.is_latest) {
            format!(
                "Sync version across environments (latest: v{} from {})",
                latest.version, latest.source_name
            )
        } else {
            "Sync version across environments".to_string()
        };

        let sync_resp = outline_icon_button(
            &mut child,
            ("sync_version_btn", project_id),
            OutlineIcon::Refresh,
            Some(sync_btn_text),
            if self.sync_version_dropdown_open {
                theme.primary
            } else {
                theme.text
            },
            if self.sync_version_dropdown_open {
                theme.primary_soft
            } else {
                theme.surface
            },
            theme.surface_2,
            if self.sync_version_dropdown_open {
                theme.primary
            } else {
                theme.border_strong
            },
            true,
        )
        .on_hover_text(sync_tooltip);

        *sync_trigger_rect_out = Some(sync_resp.rect);

        if sync_resp.clicked() {
            if sync_candidates.len() == 1 {
                let cand = &sync_candidates[0];
                self.apply_synced_version(&cand.version, &cand.source_name);
            } else {
                self.sync_version_dropdown_open = !self.sync_version_dropdown_open;
            }
        }

        child.add_space(10.0);
        let sep_x = child.cursor().min.x;
        child.painter().line_segment(
            [
                Pos2::new(sep_x, row1_rect.top() + 4.0),
                Pos2::new(sep_x, row1_rect.bottom() - 4.0),
            ],
            Stroke::new(1.0_f32, theme.border),
        );
        child.add_space(10.0);

        child.checkbox(
            &mut self.bump_package_json,
            RichText::new("Bump package.json")
                .size(FONT_SM)
                .strong()
                .color(theme.text),
        );
        child.add_space(8.0);
        child.checkbox(
            &mut self.create_git_tag,
            RichText::new("Git Tag")
                .size(FONT_SM)
                .strong()
                .color(theme.text_muted),
        );

        // Deploy Button + Rollback Button
        let is_busy = self.is_env_deploying(project_id, &self.selected_env_key);
        let btn_text = if is_busy {
            "Deploying...".to_string()
        } else {
            let ver = self.deploy_version_input.trim();
            if ver.is_empty() {
                format!("Deploy to {env_display_name}")
            } else {
                format!("Deploy to {env_display_name} (v{ver})")
            }
        };

        let deploy_galley = ui.painter().layout_no_wrap(
            btn_text,
            FontId::proportional(FS_BASE),
            Color32::WHITE,
        );
        let deploy_icon_w = 14.0;
        let deploy_gap = 8.0;
        let deploy_pad_x = 16.0;
        let deploy_content_w = deploy_icon_w + deploy_gap + deploy_galley.size().x;
        let btn_h = BTN_PRIMARY_HEIGHT; // 36px

        let rollback_info = rollback_candidate.as_ref().map(|prev_dep| {
            let rb_text = format!("Rollback v{}", prev_dep.version);
            let rb_galley = ui.painter().layout_no_wrap(
                rb_text,
                FontId::proportional(FONT_SM),
                theme.warning,
            );
            let rb_icon_w = 14.0;
            let rb_gap = 6.0;
            let rb_pad_x = 12.0;
            let rb_content_w = rb_icon_w + rb_gap + rb_galley.size().x;
            let rb_w = (rb_content_w + rb_pad_x * 2.0).max(120.0);
            (rb_galley, rb_icon_w, rb_gap, rb_content_w, rb_w)
        });

        let (btn_rect, rb_rect_opt) = if is_two_rows {
            // Row 2 below controls
            let row2_y = bar_rect.top() + 74.0;
            if is_mobile_deploy {
                // Full width deploy button below 600px
                let w = bar_rect.width() - 40.0;
                let b_rect = Rect::from_min_size(Pos2::new(bar_rect.left() + 20.0, row2_y), Vec2::new(w, btn_h));
                (b_rect, None)
            } else {
                let b_w = (deploy_content_w + deploy_pad_x * 2.0).max(180.0);
                let rb_w = rollback_info.as_ref().map(|(_, _, _, _, w)| *w).unwrap_or(0.0);
                let b_rect = Rect::from_min_size(
                    Pos2::new(bar_rect.right() - 20.0 - b_w, row2_y),
                    Vec2::new(b_w, btn_h),
                );
                let r_rect = if rb_w > 0.0 {
                    Some(Rect::from_min_size(
                        Pos2::new(b_rect.left() - 10.0 - rb_w, row2_y + (btn_h - BTN_HEIGHT) / 2.0),
                        Vec2::new(rb_w, BTN_HEIGHT),
                    ))
                } else {
                    None
                };
                (b_rect, r_rect)
            }
        } else {
            // Single row: right-aligned
            let mut b_w = (deploy_content_w + deploy_pad_x * 2.0).max(180.0);
            let rb_w = rollback_info.as_ref().map(|(_, _, _, _, w)| *w).unwrap_or(0.0);
            let total_needed = b_w + if rb_w > 0.0 { rb_w + 10.0 } else { 0.0 };
            let max_r_w = (bar_rect.right() - 20.0 - (child.cursor().min.x + 16.0)).max(180.0);
            if total_needed > max_r_w {
                b_w = (b_w - (total_needed - max_r_w)).max(140.0);
            }
            let b_rect = Rect::from_min_size(
                Pos2::new(bar_rect.right() - 20.0 - b_w, bar_rect.center().y - btn_h / 2.0),
                Vec2::new(b_w, btn_h),
            );
            let r_rect = if rb_w > 0.0 {
                Some(Rect::from_min_size(
                    Pos2::new(b_rect.left() - 10.0 - rb_w, bar_rect.center().y - BTN_HEIGHT / 2.0),
                    Vec2::new(rb_w, BTN_HEIGHT),
                ))
            } else {
                None
            };
            (b_rect, r_rect)
        };

        let btn_resp = ui
            .interact(btn_rect, egui::Id::new("primary_deploy_btn"), Sense::click())
            .on_hover_cursor(if is_busy {
                egui::CursorIcon::NotAllowed
            } else {
                egui::CursorIcon::PointingHand
            });
        let btn_bg = if is_busy {
            theme.text_subtle
        } else if btn_resp.is_pointer_button_down_on() {
            theme.primary_active
        } else if btn_resp.hovered() {
            theme.primary_hover
        } else {
            theme.primary
        };
        ui.painter()
            .rect_filled(btn_rect, CornerRadius::same(R_MD as u8), btn_bg);

        // Center icon and text together
        let deploy_start_x = (btn_rect.center().x - deploy_content_w / 2.0)
            .max(btn_rect.left() + 10.0);
        draw_icon_send(
            ui.painter(),
            Pos2::new(deploy_start_x + deploy_icon_w / 2.0, btn_rect.center().y),
            Color32::WHITE,
        );
        let text_pos = Pos2::new(
            deploy_start_x + deploy_icon_w + deploy_gap,
            btn_rect.center().y - deploy_galley.size().y / 2.0,
        );
        ui.painter().galley(text_pos, deploy_galley, Color32::WHITE);

        if !is_busy && btn_resp.clicked() {
            trigger_deploy = true;
        }

        // Quick Rollback Button
        if let (Some(prev_dep), Some(rb_rect), Some((rb_galley, rb_icon_w, rb_gap, rb_content_w, _))) =
            (rollback_candidate, rb_rect_opt, rollback_info)
        {
            let rb_resp = ui
                .interact(
                    rb_rect,
                    egui::Id::new("quick_rollback_btn"),
                    Sense::click(),
                )
                .on_hover_cursor(if is_busy {
                    egui::CursorIcon::NotAllowed
                } else {
                    egui::CursorIcon::PointingHand
                });
            let rb_bg = if rb_resp.hovered() && !is_busy {
                theme.warning_soft
            } else {
                theme.surface
            };
            ui.painter().rect_filled(
                rb_rect,
                CornerRadius::same(R_MD as u8),
                rb_bg,
            );
            ui.painter().rect_stroke(
                rb_rect,
                CornerRadius::same(R_MD as u8),
                Stroke::new(1.0_f32, theme.warning),
                StrokeKind::Inside,
            );

            let rb_start_x = (rb_rect.center().x - rb_content_w / 2.0)
                .max(rb_rect.left() + 8.0);
            draw_outline_icon(
                ui.painter(),
                Pos2::new(rb_start_x + rb_icon_w / 2.0, rb_rect.center().y),
                OutlineIcon::RotateCcw,
                theme.warning,
            );
            let rb_text_pos = Pos2::new(
                rb_start_x + rb_icon_w + rb_gap,
                rb_rect.center().y - rb_galley.size().y / 2.0,
            );
            ui.painter().galley(rb_text_pos, rb_galley, theme.warning);

            if !is_busy
                && rb_resp
                    .on_hover_text(format!(
                        "Rollback {} to previous successful release v{}",
                        env_display_name, prev_dep.version
                    ))
                    .clicked()
            {
                trigger_rollback = Some(prev_dep);
            }
        }

        if let Some(rb_record) = trigger_rollback {
            self.trigger_redeploy_from_record(&rb_record, true);
        } else if trigger_deploy {
            self.trigger_current_environment_deploy();
        }
    }

    /// Feature #6: Redesigned Pipeline Steps Tab with non-overlapping 2-row step cards!
    pub(crate) fn render_pipeline_steps_tab(&mut self, ui: &mut egui::Ui, proj_idx: usize) {
        let env_key = self.selected_env_key.clone();
        let mut config_changed = false;

        let env_name = self.storage.projects[proj_idx]
            .get_env(&env_key)
            .map(|e| e.name.clone())
            .unwrap_or_else(|| env_key.clone());

        horizontal_row(ui, 36.0, |ui| {
            ui.label(
                RichText::new(format!("{} PIPELINE STEPS", env_name.to_uppercase()))
                    .size(FONT_MD)
                    .strong()
                    .color(TEXT_PRIMARY),
            );
            ui.label(
                RichText::new("— Add or customize any step type:")
                    .size(FONT_SM)
                    .color(TEXT_MUTED),
            );

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                let project = &mut self.storage.projects[proj_idx];
                if let Some(env_cfg) = project.get_env_mut(&env_key) {
                    let remote_btn = egui::Button::new(
                        RichText::new("+ Remote SSH")
                            .size(FONT_SM)
                            .strong()
                            .color(TEAL_PILL_TEXT),
                    )
                    .fill(TEAL_PILL_BG)
                    .stroke(Stroke::new(1.0_f32, TEAL_PILL_TEXT))
                    .corner_radius(6.0);
                    if ui
                        .add(remote_btn)
                        .on_hover_text("Add a Remote SSH command step executed on target servers")
                        .clicked()
                    {
                        env_cfg.steps.push(PipelineStep::new(
                            "Remote SSH Command",
                            StepKind::RemoteCommand {
                                command: "pm2 restart {app_name} --update-env".to_string(),
                            },
                        ));
                        config_changed = true;
                    }

                    let scp_btn = egui::Button::new(
                        RichText::new("+ SCP Upload")
                            .size(FONT_SM)
                            .strong()
                            .color(PURPLE_PILL_TEXT),
                    )
                    .fill(PURPLE_PILL_BG)
                    .stroke(Stroke::new(1.0_f32, PURPLE_PILL_TEXT))
                    .corner_radius(6.0);
                    if ui
                        .add(scp_btn)
                        .on_hover_text(
                            "Add an SCP file upload step (transfers local files & optional staged .env to {remote_dir})",
                        )
                        .clicked()
                    {
                        env_cfg.steps.push(PipelineStep::new(
                            "Upload Artifacts via SCP",
                            StepKind::TransferFiles {
                                sources: vec![
                                    "./dist".to_string(),
                                    "./package.json".to_string(),
                                    "./package-lock.json".to_string(),
                                ],
                                stage_env_file: true,
                            },
                        ));
                        config_changed = true;
                    }

                    let verify_btn = egui::Button::new(
                        RichText::new("+ Verify Path")
                            .size(FONT_SM)
                            .strong()
                            .color(WARN_PILL_TEXT),
                    )
                    .fill(WARN_PILL_BG)
                    .stroke(Stroke::new(1.0_f32, WARN_PILL_TEXT))
                    .corner_radius(6.0);
                    if ui
                        .add(verify_btn)
                        .on_hover_text(
                            "Add a local file/folder verification guard (e.g. ./dist/server.js)",
                        )
                        .clicked()
                    {
                        env_cfg.steps.push(PipelineStep::new(
                            "Verify Build Output",
                            StepKind::VerifyLocalPath {
                                path: "./dist/server.js".to_string(),
                            },
                        ));
                        config_changed = true;
                    }

                    let local_btn = egui::Button::new(
                        RichText::new("+ Local Command")
                            .size(FONT_SM)
                            .strong()
                            .color(BLUE_PILL_TEXT),
                    )
                    .fill(BLUE_PILL_BG)
                    .stroke(Stroke::new(1.0_f32, BLUE_PILL_TEXT))
                    .corner_radius(6.0);
                    if ui
                        .add(local_btn)
                        .on_hover_text("Add a local shell command step (e.g. npm run build)")
                        .clicked()
                    {
                        env_cfg.steps.push(PipelineStep::new(
                            "Build Application",
                            StepKind::LocalCommand {
                                command: "npm run build".to_string(),
                                allow_failure: false,
                                shell: None,
                            },
                        ));
                        config_changed = true;
                    }
                }
            });
        });

        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "Available template variables: {remote_dir}, {remote_user}, {app_name}, {http_proxy}, {env_source}, {version}",
            )
            .size(FONT_SM)
            .color(TEXT_MUTED),
        );
        ui.add_space(10.0);

        let theme = self.theme();
        let project = &mut self.storage.projects[proj_idx];
        let Some(env_cfg) = project.get_env_mut(&env_key) else {
            return;
        };

        let mut move_up_idx = None;
        let mut move_down_idx = None;
        let mut delete_idx = None;
        let mut drag_reorder = None;
        let total_steps = env_cfg.steps.len();

        let pointer_pos = ui.input(|i| i.pointer.hover_pos());
        let is_pointer_released = ui.input(|i| i.pointer.any_released());

        if self.dragged_step_idx.is_some() {
            ui.ctx().request_repaint();
        }

        for (idx, step) in env_cfg.steps.iter_mut().enumerate() {
            let is_being_dragged = self.dragged_step_idx == Some(idx);
            let card_fill = if is_being_dragged {
                theme.primary_soft
            } else if !step.enabled {
                theme.surface_2
            } else {
                theme.surface
            };
            let step_border_color = if is_being_dragged {
                theme.primary
            } else {
                theme.border
            };
            let step_accent_color = match &step.kind {
                StepKind::LocalCommand { .. } => Color32::from_rgb(20, 110, 245), // local #146EF5
                StepKind::VerifyLocalPath { .. } => Color32::from_rgb(245, 158, 11), // verify #F59E0B
                StepKind::TransferFiles { .. } => Color32::from_rgb(122, 61, 255), // scp #7A3DFF
                StepKind::RemoteCommand { .. } => Color32::from_rgb(20, 184, 166), // ssh #14B8A6
            };

            let frame_resp = egui::Frame::new()
                .fill(card_fill)
                .stroke(Stroke::new(if is_being_dragged { 2.0_f32 } else { 1.0_f32 }, step_border_color))
                .corner_radius(10.0)
                .inner_margin(Margin::symmetric(16, 12))
                .show(ui, |ui| {
                    // Row 1: Drag Handle + Enable Checkbox + Step # + Interactive Type Dropdown + Step Name + Option Checkbox | Up / Down / Delete Buttons
                    horizontal_row(ui, 32.0, |ui| {
                        // Drag Handle: Vertical grip dots
                        let handle_size = Vec2::new(18.0, 26.0);
                        let (handle_rect, handle_resp) = ui.allocate_exact_size(handle_size, Sense::drag());
                        let handle_resp = handle_resp.on_hover_cursor(if is_being_dragged {
                            egui::CursorIcon::Grabbing
                        } else {
                            egui::CursorIcon::Grab
                        });

                        if handle_resp.hovered() || is_being_dragged {
                            ui.painter().rect_filled(
                                handle_rect,
                                CornerRadius::same(4),
                                if is_being_dragged {
                                    BLUE_PILL_BG
                                } else {
                                    SUBTLE_BG
                                },
                            );
                        }

                        if handle_resp.drag_started() || handle_resp.dragged() {
                            self.dragged_step_idx = Some(idx);
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                        }

                        let handle_color = if is_being_dragged || handle_resp.hovered() {
                            PRIMARY_BLUE
                        } else {
                            TEXT_MUTED
                        };
                        draw_outline_icon(ui.painter(), handle_rect.center(), OutlineIcon::GripVertical, handle_color);
                        handle_resp.on_hover_text("Drag to reorder this pipeline step");
                        ui.add_space(3.0);

                        if ui.checkbox(&mut step.enabled, "").changed() {
                            config_changed = true;
                        }
                        ui.label(
                            RichText::new(format!("#{:02}", idx + 1))
                                .size(FONT_SM)
                                .strong()
                                .color(TEXT_MUTED),
                        );

                        let (type_label, _bg, fg) = match &step.kind {
                            StepKind::LocalCommand { .. } => {
                                ("LOCAL", BLUE_PILL_BG, BLUE_PILL_TEXT)
                            }
                            StepKind::VerifyLocalPath { .. } => {
                                ("VERIFY PATH", WARN_PILL_BG, WARN_PILL_TEXT)
                            }
                            StepKind::TransferFiles { .. } => {
                                ("SCP UPLOAD", PURPLE_PILL_BG, PURPLE_PILL_TEXT)
                            }
                            StepKind::RemoteCommand { .. } => {
                                ("REMOTE SSH", TEAL_PILL_BG, TEAL_PILL_TEXT)
                            }
                        };

                        // Interactive Step Type Selector Dropdown so user can switch step type anytime!
                        egui::ComboBox::from_id_salt(("step_kind_selector", &step.id))
                            .selected_text(
                                RichText::new(type_label).size(FONT_XS).strong().color(fg),
                            )
                            .width(120.0)
                            .show_ui(ui, |ui| {
                                let is_local = matches!(step.kind, StepKind::LocalCommand { .. });
                                if ui
                                    .selectable_label(is_local, "LOCAL — Local Command")
                                    .clicked()
                                    && !is_local
                                {
                                    step.kind = StepKind::LocalCommand {
                                        command: "npm run build".to_string(),
                                        allow_failure: false,
                                        shell: None,
                                    };
                                    config_changed = true;
                                }

                                let is_verify =
                                    matches!(step.kind, StepKind::VerifyLocalPath { .. });
                                if ui
                                    .selectable_label(is_verify, "VERIFY PATH — Check File/Dir")
                                    .clicked()
                                    && !is_verify
                                {
                                    step.kind = StepKind::VerifyLocalPath {
                                        path: "./dist/server.js".to_string(),
                                    };
                                    config_changed = true;
                                }

                                let is_scp = matches!(step.kind, StepKind::TransferFiles { .. });
                                if ui
                                    .selectable_label(is_scp, "SCP UPLOAD — Transfer Files")
                                    .clicked()
                                    && !is_scp
                                {
                                    step.kind = StepKind::TransferFiles {
                                        sources: vec![
                                            "./dist".to_string(),
                                            "./package.json".to_string(),
                                            "./package-lock.json".to_string(),
                                        ],
                                        stage_env_file: true,
                                    };
                                    config_changed = true;
                                }

                                let is_remote =
                                    matches!(step.kind, StepKind::RemoteCommand { .. });
                                if ui
                                    .selectable_label(is_remote, "REMOTE SSH — Server Command")
                                    .clicked()
                                    && !is_remote
                                {
                                    step.kind = StepKind::RemoteCommand {
                                        command: "pm2 restart {app_name} --update-env".to_string(),
                                    };
                                    config_changed = true;
                                }
                            });

                        if styled_text_edit(
                            ui,
                            ("step_name", &step.id),
                            &mut step.name,
                            210.0,
                            Some("Step name (e.g. Build Application)"),
                        ) {
                            config_changed = true;
                        }

                        // Step-type specific toggle placed cleanly in Row 1 with plenty of space
                        match &mut step.kind {
                            StepKind::LocalCommand {
                                allow_failure,
                                shell,
                                ..
                            } => {
                                ui.add_space(6.0);
                                let current_shell = shell.unwrap_or_default();
                                egui::ComboBox::from_id_salt(("step_shell_selector", &step.id))
                                    .selected_text(
                                        RichText::new(current_shell.display_name())
                                            .size(FONT_XS)
                                            .color(TEXT_SECONDARY),
                                    )
                                    .width(88.0)
                                    .show_ui(ui, |ui| {
                                        for &s in LocalShell::all() {
                                            let is_selected =
                                                shell.map_or(s == current_shell, |cur| cur == s);
                                            if ui
                                                .selectable_label(is_selected, s.display_name())
                                                .clicked()
                                            {
                                                *shell = Some(s);
                                                config_changed = true;
                                            }
                                        }
                                    });

                                ui.add_space(6.0);
                                if ui
                                    .checkbox(
                                        allow_failure,
                                        RichText::new("Continue on non-zero exit")
                                            .size(FONT_SM)
                                            .color(TEXT_SECONDARY),
                                    )
                                    .changed()
                                {
                                    config_changed = true;
                                }
                            }
                            StepKind::TransferFiles { stage_env_file, .. } => {
                                ui.add_space(6.0);
                                if ui
                                    .checkbox(
                                        stage_env_file,
                                        RichText::new("Stage .env file")
                                            .size(FONT_SM)
                                            .color(TEXT_SECONDARY),
                                    )
                                    .changed()
                                {
                                    config_changed = true;
                                }
                            }
                            _ => {}
                        }

                        // Right-aligned Reorder & Delete actions using crisp vector outline icons!
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                            if outline_icon_button(
                                ui,
                                ("step_del_btn", &step.id),
                                OutlineIcon::Trash,
                                None,
                                FAIL_PILL_TEXT,
                                FAIL_PILL_BG,
                                Color32::from_rgb(254, 202, 202),
                                Color32::from_rgb(252, 165, 165),
                                true,
                            )
                            .on_hover_text("Delete step")
                            .clicked()
                            {
                                delete_idx = Some(idx);
                            }

                            let can_down = idx + 1 < total_steps;
                            if outline_icon_button(
                                ui,
                                ("step_down_btn", &step.id),
                                OutlineIcon::ArrowDown,
                                None,
                                if can_down {
                                    TEXT_SECONDARY
                                } else {
                                    TEXT_PLACEHOLDER
                                },
                                SUBTLE_BG,
                                INPUT_BG_HOVER,
                                CARD_BORDER,
                                can_down,
                            )
                            .on_hover_text("Move step down")
                            .clicked()
                                && can_down
                            {
                                move_down_idx = Some(idx);
                            }

                            let can_up = idx > 0;
                            if outline_icon_button(
                                ui,
                                ("step_up_btn", &step.id),
                                OutlineIcon::ArrowUp,
                                None,
                                if can_up {
                                    TEXT_SECONDARY
                                } else {
                                    TEXT_PLACEHOLDER
                                },
                                SUBTLE_BG,
                                INPUT_BG_HOVER,
                                CARD_BORDER,
                                can_up,
                            )
                            .on_hover_text("Move step up")
                            .clicked()
                                && can_up
                            {
                                move_up_idx = Some(idx);
                            }
                        });
                    });

                    ui.add_space(8.0);

                    // Row 2: Full-width Command / Path / SCP Sources field
                    horizontal_row(ui, 34.0, |ui| {
                        let (field_label, placeholder) = match &step.kind {
                            StepKind::LocalCommand { .. } => (
                                "Command:",
                                "e.g. npm run build",
                            ),
                            StepKind::VerifyLocalPath { .. } => (
                                "Verify Path:",
                                "e.g. ./dist/index.js or ./dist",
                            ),
                            StepKind::TransferFiles { .. } => (
                                "SCP Sources:",
                                "e.g. ./dist, ./package.json, ./package-lock.json",
                            ),
                            StepKind::RemoteCommand { .. } => (
                                "Remote SSH:",
                                "e.g. pm2 restart {app_name} --update-env",
                            ),
                        };

                        ui.add_sized(
                            [88.0, 24.0],
                            egui::Label::new(
                                RichText::new(field_label)
                                    .size(FONT_SM)
                                    .strong()
                                    .color(TEXT_SECONDARY),
                            ),
                        );

                        let input_w = (ui.available_width() - 4.0).max(200.0);
                        match &mut step.kind {
                            StepKind::LocalCommand { command, .. } => {
                                if syntax_highlighted_text_edit(
                                    ui,
                                    ("step_local_cmd", &step.id),
                                    command,
                                    input_w,
                                    Some(placeholder),
                                ) {
                                    config_changed = true;
                                }
                            }
                            StepKind::VerifyLocalPath { path } => {
                                if syntax_highlighted_text_edit(
                                    ui,
                                    ("step_verify_path", &step.id),
                                    path,
                                    input_w,
                                    Some(placeholder),
                                ) {
                                    config_changed = true;
                                }
                            }
                            StepKind::TransferFiles { sources, .. } => {
                                let sources_text = match &mut self.step_scp_sources_buffer {
                                    Some((id, buf)) if id == &step.id => buf,
                                    _ => {
                                        let joined = sources.join(", ");
                                        self.step_scp_sources_buffer =
                                            Some((step.id.clone(), joined));
                                        &mut self.step_scp_sources_buffer.as_mut().unwrap().1
                                    }
                                };
                                if syntax_highlighted_text_edit(
                                    ui,
                                    ("step_transfer_sources", &step.id),
                                    sources_text,
                                    input_w,
                                    Some(placeholder),
                                ) {
                                    *sources = sources_text
                                        .split(',')
                                        .map(|s| s.trim().to_string())
                                        .filter(|s| !s.is_empty())
                                        .collect();
                                    config_changed = true;
                                }
                            }
                            StepKind::RemoteCommand { command } => {
                                if syntax_highlighted_text_edit(
                                    ui,
                                    ("step_remote_cmd", &step.id),
                                    command,
                                    input_w,
                                    Some(placeholder),
                                ) {
                                    config_changed = true;
                                }
                            }
                        }
                    });
                });

            let card_rect = frame_resp.response.rect;
            let bar_rect = Rect::from_min_max(
                Pos2::new(card_rect.left(), card_rect.top() + 6.0),
                Pos2::new(card_rect.left() + 3.0, card_rect.bottom() - 6.0),
            );
            ui.painter().rect_filled(
                bar_rect,
                CornerRadius { nw: 0, ne: 3, se: 3, sw: 0 },
                if step.enabled { step_accent_color } else { theme.text_subtle },
            );
            if is_being_dragged {
                let shadow_rect = card_rect.translate(Vec2::new(0.0, 4.0)).expand(1.0);
                ui.painter().rect_filled(
                    shadow_rect,
                    CornerRadius::same(10),
                    Color32::from_rgba_premultiplied(0, 0, 0, 40),
                );
            }
            if let Some(from_idx) = self.dragged_step_idx {
                if from_idx != idx {
                    if let Some(pos) = pointer_pos {
                        if card_rect.contains(pos) {
                            let is_top_half = pos.y < card_rect.center().y;
                            let line_y = if is_top_half {
                                card_rect.top() - 3.0
                            } else {
                                card_rect.bottom() + 3.0
                            };
                            let insert_line = [
                                Pos2::new(card_rect.left() + 6.0, line_y),
                                Pos2::new(card_rect.right() - 6.0, line_y),
                            ];
                            // Animated drop insertion glow & line
                            ui.painter().line_segment(
                                insert_line,
                                Stroke::new(5.0_f32, PRIMARY_BLUE.gamma_multiply(0.4)),
                            );
                            ui.painter().line_segment(
                                insert_line,
                                Stroke::new(2.5_f32, PRIMARY_BLUE),
                            );
                            ui.painter().circle_filled(
                                Pos2::new(card_rect.left() + 6.0, line_y),
                                4.0,
                                PRIMARY_BLUE,
                            );
                            ui.painter().rect_filled(
                                card_rect,
                                CornerRadius::same(10),
                                PRIMARY_BLUE.gamma_multiply(0.08),
                            );
                            if is_pointer_released {
                                drag_reorder = Some((from_idx, idx));
                            }
                        }
                    }
                }
            }

            ui.add_space(8.0);
        }

        if let Some((from, to)) = drag_reorder {
            env_cfg.reorder_step(from, to);
            self.dragged_step_idx = None;
            config_changed = true;
        } else if is_pointer_released {
            self.dragged_step_idx = None;
        }

        if let Some(i) = move_up_idx {
            env_cfg.move_step_up(i);
            config_changed = true;
        }
        if let Some(i) = move_down_idx {
            env_cfg.move_step_down(i);
            config_changed = true;
        }
        if let Some(i) = delete_idx {
            env_cfg.steps.remove(i);
            config_changed = true;
        }

        if config_changed {
            self.mark_config_dirty();
        }
    }

    pub(crate) fn render_server_settings_tab(&mut self, ui: &mut egui::Ui, proj_idx: usize) {
        let env_key = self.selected_env_key.clone();
        let mut config_changed = false;
        let mut delete_current_env = false;
        let mut verify_vault_clicked = false;
        let mut open_cred_guide_on_overview = false;
        let is_two_col = ui.ctx().screen_rect().width() >= 800.0 && ui.available_width() >= 550.0;
        let field_width = if is_two_col {
            (ui.available_width() - 280.0).clamp(240.0, 520.0)
        } else {
            (ui.available_width() - 36.0).max(200.0)
        };

        let current_env_name = self.storage.projects[proj_idx]
            .get_env(&env_key)
            .map(|e| e.name.clone())
            .unwrap_or_else(|| env_key.clone());

        horizontal_row(ui, 34.0, |ui| {
            ui.label(
                RichText::new(format!(
                    "{} — ENVIRONMENT & SSH SETTINGS",
                    current_env_name.to_uppercase()
                ))
                .size(FONT_MD)
                .strong()
                .color(TEXT_PRIMARY),
            );
            if self.cached_vault_found {
                render_pill(
                    ui,
                    "OS CREDENTIAL VAULT READY",
                    SUCCESS_PILL_BG,
                    SUCCESS_PILL_TEXT,
                );
            } else {
                render_pill(
                    ui,
                    "SSH KEY / MANUAL PASSWORD",
                    WARN_PILL_BG,
                    WARN_PILL_TEXT,
                );
            }

            // Feature #5: Always allow deleting the environment + quick toggle for Credential Setup Guide
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
                if outline_icon_button(
                    ui,
                    ("del_env_settings_btn", &env_key),
                    OutlineIcon::Trash,
                    Some("Delete Environment"),
                    FAIL_PILL_TEXT,
                    FAIL_PILL_BG,
                    Color32::from_rgb(254, 202, 202),
                    Color32::from_rgb(252, 165, 165),
                    true,
                )
                .on_hover_text("Delete this environment")
                .clicked()
                {
                    delete_current_env = true;
                }

                if outline_icon_button(
                    ui,
                    ("toggle_cred_guide_btn", &env_key),
                    OutlineIcon::Key,
                    Some("OS Credential Guide (Win / Linux / Mac)"),
                    PRIMARY_BLUE,
                    BLUE_PILL_BG,
                    Color32::from_rgb(191, 219, 254),
                    PRIMARY_BLUE,
                    true,
                )
                .on_hover_text("Open the multi-platform OS Credentials setup guideline on the All Deployments page")
                .clicked()
                {
                    open_cred_guide_on_overview = true;
                }
            });
        });

        ui.add_space(10.0);

        let mut guide_vault_target = String::new();
        let mut guide_remote_user = String::new();

        egui::Frame::new()
            .fill(CARD_BG)
            .stroke(Stroke::new(1.0_f32, CARD_BORDER))
            .corner_radius(10.0)
            .inner_margin(Margin::symmetric(22, 20))
            .show(ui, |ui| {
                let project = &mut self.storage.projects[proj_idx];
                egui::Grid::new("clean_server_settings_grid")
                    .num_columns(if is_two_col { 2 } else { 1 })
                    .spacing(if is_two_col { [28.0, 14.0] } else { [0.0, 6.0] })
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("Local Project Directory:")
                                .size(FONT_MD)
                                .color(TEXT_SECONDARY)
                                .strong(),
                        );
                        if !is_two_col {
                            ui.end_row();
                        }
                        if styled_text_edit(
                            ui,
                            "srv_local_path",
                            &mut project.local_path,
                            field_width,
                            Some(r"e.g. C:\Projects\my-web-service"),
                        ) {
                            config_changed = true;
                        }
                        ui.end_row();
                        if !is_two_col {
                            ui.add_space(6.0);
                            ui.end_row();
                        }

                        if let Some(env_cfg) = project.get_env_mut(&env_key) {
                            ui.label(
                                RichText::new("Environment Display Name:")
                                    .size(FONT_MD)
                                    .color(TEXT_SECONDARY)
                                    .strong(),
                            );
                            if !is_two_col {
                                ui.end_row();
                            }
                            if styled_text_edit(
                                ui,
                                ("srv_env_name", &env_key),
                                &mut env_cfg.name,
                                field_width,
                                Some("e.g. Development, Staging, Production"),
                            ) {
                                config_changed = true;
                            }
                            ui.end_row();
                            if !is_two_col {
                                ui.add_space(6.0);
                                ui.end_row();
                            }

                            ui.label(
                                RichText::new("Application Name (PM2 / Dir):")
                                    .size(FONT_MD)
                                    .color(TEXT_SECONDARY)
                                    .strong(),
                            );
                            if !is_two_col {
                                ui.end_row();
                            }
                            if styled_text_edit(
                                ui,
                                ("srv_app_name", &env_key),
                                &mut env_cfg.app_name,
                                field_width,
                                Some("e.g. my-web-service-dev"),
                            ) {
                                config_changed = true;
                            }
                            ui.end_row();
                            if !is_two_col {
                                ui.add_space(6.0);
                                ui.end_row();
                            }

                            ui.label(
                                RichText::new("Target Servers (comma-separated):")
                                    .size(FONT_MD)
                                    .color(TEXT_SECONDARY)
                                    .strong(),
                            );
                            if !is_two_col {
                                ui.end_row();
                            }
                            let servers_text = match &mut self.servers_edit_buffer {
                                Some((k, buf)) if k == &env_key => buf,
                                _ => {
                                    self.servers_edit_buffer =
                                        Some((env_key.clone(), env_cfg.servers.join(", ")));
                                    &mut self.servers_edit_buffer.as_mut().unwrap().1
                                }
                            };
                            if styled_text_edit(
                                ui,
                                ("srv_servers", &env_key),
                                servers_text,
                                field_width,
                                Some("e.g. 192.168.1.10, 192.168.1.11"),
                            ) {
                                env_cfg.servers = servers_text
                                    .split(',')
                                    .map(|s| s.trim().to_string())
                                    .filter(|s| !s.is_empty())
                                    .collect();
                                config_changed = true;
                            }
                            ui.end_row();
                            if !is_two_col {
                                ui.add_space(6.0);
                                ui.end_row();
                            }

                            ui.label(
                                RichText::new("Remote Directory Path:")
                                    .size(FONT_MD)
                                    .color(TEXT_SECONDARY)
                                    .strong(),
                            );
                            if !is_two_col {
                                ui.end_row();
                            }
                            if styled_text_edit(
                                ui,
                                ("srv_remote_dir", &env_key),
                                &mut env_cfg.remote_dir,
                                field_width,
                                Some("e.g. /var/www/my-web-service or /home/deploy/apps/my-app"),
                            ) {
                                config_changed = true;
                            }
                            ui.end_row();
                            if !is_two_col {
                                ui.add_space(6.0);
                                ui.end_row();
                            }

                            ui.label(
                                RichText::new("Remote SSH Username:")
                                    .size(FONT_MD)
                                    .color(TEXT_SECONDARY)
                                    .strong(),
                            );
                            if !is_two_col {
                                ui.end_row();
                            }
                            if styled_text_edit(
                                ui,
                                ("srv_remote_user", &env_key),
                                &mut env_cfg.remote_user,
                                field_width,
                                Some("e.g. deploy or ubuntu"),
                            ) {
                                config_changed = true;
                            }
                            ui.end_row();
                            if !is_two_col {
                                ui.add_space(6.0);
                                ui.end_row();
                            }

                            ui.label(
                                RichText::new("OS Credential Vault Target:")
                                    .size(FONT_MD)
                                    .color(TEXT_SECONDARY)
                                    .strong(),
                            );
                            if !is_two_col {
                                ui.end_row();
                            }
                            ui.horizontal(|ui| {
                                if styled_text_edit(
                                    ui,
                                    ("srv_vault_target", &env_key),
                                    &mut env_cfg.vault_target,
                                    field_width - 102.0,
                                    Some("e.g. prod-ssh-server or my-vault-target"),
                                ) {
                                    self.cached_vault_found =
                                        read_os_vault_credential(&env_cfg.vault_target).is_some();
                                    config_changed = true;
                                }
                                if outline_icon_button(
                                    ui,
                                    ("verify_vault_btn", &env_key),
                                    OutlineIcon::Refresh,
                                    Some("Verify"),
                                    TEXT_PRIMARY,
                                    SUBTLE_BG,
                                    INPUT_BG_HOVER,
                                    CARD_BORDER,
                                    true,
                                )
                                .on_hover_text("Check if this target exists in your OS Credential Manager / Keychain")
                                .clicked()
                                {
                                    verify_vault_clicked = true;
                                }
                            });
                            ui.end_row();
                            if !is_two_col {
                                ui.add_space(6.0);
                                ui.end_row();
                            }

                            ui.label(
                                RichText::new("SSH Password Override:")
                                    .size(FONT_MD)
                                    .color(TEXT_SECONDARY)
                                    .strong(),
                            );
                            if !is_two_col {
                                ui.end_row();
                            }
                            styled_password_edit(
                                ui,
                                ("srv_manual_pwd", &env_key),
                                &mut self.manual_ssh_password,
                                field_width,
                                Some("Optional — leave blank to use OS Credential Vault or SSH Key"),
                            );
                            ui.end_row();
                            if !is_two_col {
                                ui.add_space(6.0);
                                ui.end_row();
                            }

                            ui.label(
                                RichText::new("Environment File Source (.env):")
                                    .size(FONT_MD)
                                    .color(TEXT_SECONDARY)
                                    .strong(),
                            );
                            if !is_two_col {
                                ui.end_row();
                            }
                            let env_src_text = match &mut self.server_env_source_buffer {
                                Some((k, buf)) if k == &env_key => buf,
                                _ => {
                                    self.server_env_source_buffer =
                                        Some((env_key.clone(), env_cfg.env_source.clone().unwrap_or_default()));
                                    &mut self.server_env_source_buffer.as_mut().unwrap().1
                                }
                            };
                            if styled_text_edit(
                                ui,
                                ("srv_env_source", &env_key),
                                env_src_text,
                                field_width,
                                Some("Optional — e.g. ./.env.dev or ./.env.production"),
                            ) {
                                env_cfg.env_source = if env_src_text.trim().is_empty() {
                                    None
                                } else {
                                    Some(env_src_text.clone())
                                };
                                config_changed = true;
                            }
                            ui.end_row();
                            if !is_two_col {
                                ui.add_space(6.0);
                                ui.end_row();
                            }

                            ui.label(
                                RichText::new("Outbound HTTP Proxy:")
                                    .size(FONT_MD)
                                    .color(TEXT_SECONDARY)
                                    .strong(),
                            );
                            if !is_two_col {
                                ui.end_row();
                            }
                            let proxy_text = match &mut self.server_http_proxy_buffer {
                                Some((k, buf)) if k == &env_key => buf,
                                _ => {
                                    self.server_http_proxy_buffer =
                                        Some((env_key.clone(), env_cfg.http_proxy.clone().unwrap_or_default()));
                                    &mut self.server_http_proxy_buffer.as_mut().unwrap().1
                                }
                            };
                            if styled_text_edit(
                                ui,
                                ("srv_http_proxy", &env_key),
                                proxy_text,
                                field_width,
                                Some("Optional — e.g. http://proxy.example.com:8080"),
                            ) {
                                env_cfg.http_proxy = if proxy_text.trim().is_empty() {
                                    None
                                } else {
                                    Some(proxy_text.clone())
                                };
                                config_changed = true;
                            }
                            ui.end_row();
                            if !is_two_col {
                                ui.add_space(6.0);
                                ui.end_row();
                            }

                            guide_vault_target = env_cfg.vault_target.trim().to_string();
                            guide_remote_user = env_cfg.remote_user.trim().to_string();
                        }
                    });
            });

        if open_cred_guide_on_overview {
            if !guide_vault_target.is_empty() {
                self.cred_guide_target_input = guide_vault_target.clone();
            }
            if !guide_remote_user.is_empty() {
                self.cred_guide_user_input = guide_remote_user.clone();
            }
            self.show_credential_guide = true;
            self.selection = SidebarSelection::AllDeployments;
        } else if verify_vault_clicked {
            let _ = self.storage.save();
            self.refresh_selected_project_metadata(false);
            if self.cached_vault_found {
                self.status_banner = Some((
                    format!(
                        "Verified! Found credential for target '{guide_vault_target}' in OS Vault."
                    ),
                    true,
                ));
            } else {
                if !guide_vault_target.is_empty() {
                    self.cred_guide_target_input = guide_vault_target.clone();
                }
                if !guide_remote_user.is_empty() {
                    self.cred_guide_user_input = guide_remote_user.clone();
                }
                self.show_credential_guide = true;
                self.status_banner = Some((
                    format!(
                        "No credential found for target '{guide_vault_target}' in OS Vault. Click 'OS Credential Guide (Win / Linux / Mac)' for setup instructions."
                    ),
                    false,
                ));
            }
        } else if delete_current_env {
            self.servers_edit_buffer = None;
            self.flush_pending_save();
            if let Err(err) = self.storage.projects[proj_idx].remove_environment(&env_key) {
                self.status_banner = Some((err, false));
            } else {
                let _ = self.storage.save();
                self.selected_env_key = self.storage.projects[proj_idx]
                    .environments
                    .first()
                    .map(|e| e.key.clone())
                    .unwrap_or_default();
                self.refresh_selected_project_metadata(true);
                self.status_banner = Some((
                    format!("Removed environment '{current_env_name}'."),
                    true,
                ));
            }
        } else if config_changed {
            self.mark_config_dirty();
        }
    }

    pub(crate) fn render_history_and_logs_view(&mut self, ui: &mut egui::Ui, project_filter: Option<&str>) {
        let is_project_view = project_filter.is_some();
        let project_envs: Vec<(String, String)> = if let Some(pid) = project_filter {
            self.storage
                .projects
                .iter()
                .find(|p| p.id == pid)
                .map(|p| {
                    p.environments
                        .iter()
                        .map(|e| (e.name.clone(), e.key.clone()))
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let filter_options = if !is_project_view {
            let opts = self.storage.existing_environment_tabs(None);
            if self.history_env_filter.is_some()
                && !opts.iter().any(|(_, k)| *k == self.history_env_filter)
            {
                self.history_env_filter = None;
            }
            opts
        } else {
            Vec::new()
        };

        let mut switched_env_key = None;
        let mut switched_history_filter = None;

        // Section Header + Filter Pills (Fixed 34px height)
        let theme = self.theme();
        horizontal_row(ui, 34.0, |ui| {
            ui.label(
                RichText::new("DEPLOYMENT HISTORY")
                    .size(FONT_MD)
                    .strong()
                    .color(theme.text),
            );

            if is_project_view {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                    for (env_name, env_key) in project_envs.into_iter().rev() {
                        let is_active = self.selected_env_key == env_key;
                        let btn = egui::Button::new(
                            RichText::new(env_name).size(FONT_SM).strong().color(if is_active {
                                theme.ink_fg
                            } else {
                                theme.text_muted
                            }),
                        )
                        .fill(if is_active { theme.ink } else { theme.surface_2 })
                        .stroke(Stroke::new(
                            1.0_f32,
                            if is_active { theme.ink } else { theme.border },
                        ))
                        .corner_radius(R_MD as u8);
                        if ui.add(btn).clicked() {
                            switched_env_key = Some(env_key);
                        }
                    }
                });
            } else {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                    for (label, filter_val) in filter_options.into_iter().rev() {
                        let is_active = self.history_env_filter == filter_val;
                        let btn = egui::Button::new(
                            RichText::new(label).size(FONT_SM).strong().color(if is_active {
                                theme.ink_fg
                            } else {
                                theme.text_muted
                            }),
                        )
                        .fill(if is_active { theme.ink } else { theme.surface_2 })
                        .stroke(Stroke::new(
                            1.0_f32,
                            if is_active { theme.ink } else { theme.border },
                        ))
                        .corner_radius(R_MD as u8);
                        if ui.add(btn).clicked() {
                            switched_history_filter = Some(filter_val);
                        }
                    }
                });
            }
        });

        if let Some(env_key) = switched_env_key {
            self.selected_env_key = env_key;
            self.history_visible_limit = 5;
            self.refresh_selected_project_metadata(true);
        } else if let Some(filter_val) = switched_history_filter {
            self.history_env_filter = filter_val;
            self.history_visible_limit = 5;
        }

        ui.add_space(10.0);

        // When viewing a project workspace, deployment history strictly filters by the active environment
        let effective_env_filter = if is_project_view {
            if self.selected_env_key.is_empty() {
                None
            } else {
                Some(self.selected_env_key.as_str())
            }
        } else {
            self.history_env_filter.as_deref()
        };

        let filtered: Vec<DeploymentRecord> = self
            .storage
            .filter_deployments(project_filter, effective_env_filter, None)
            .into_iter()
            .cloned()
            .collect();

        let mut redeploy_target: Option<(DeploymentRecord, bool)> = None;

        let is_stacked_view = ui.ctx().screen_rect().width() < 900.0 || ui.available_width() < 640.0;
        let total_count = filtered.len();
        let visible_count = total_count.min(self.history_visible_limit.max(5));

        // Structured Deployment History Table Card
        egui::Frame::new()
            .fill(CARD_BG)
            .stroke(Stroke::new(1.0_f32, CARD_BORDER))
            .corner_radius(10.0)
            .inner_margin(Margin::symmetric(20, 14))
            .show(ui, |ui| {
                if !is_stacked_view {
                    horizontal_row(ui, 22.0, |ui| {
                        ui.add_sized(
                            [110.0, 20.0],
                            egui::Label::new(
                                RichText::new("STATUS")
                                    .size(FONT_XS)
                                    .strong()
                                    .color(TEXT_MUTED),
                            ),
                        );
                        ui.add_sized(
                            [80.0, 20.0],
                            egui::Label::new(
                                RichText::new("ENV")
                                    .size(FONT_XS)
                                    .strong()
                                    .color(TEXT_MUTED),
                            ),
                        );
                        ui.add_sized(
                            [95.0, 20.0],
                            egui::Label::new(
                                RichText::new("VERSION")
                                    .size(FONT_XS)
                                    .strong()
                                    .color(TEXT_MUTED),
                            ),
                        );
                        ui.add_sized(
                            [280.0, 20.0],
                            egui::Label::new(
                                RichText::new("DETAILS & GIT CHANGES")
                                    .size(FONT_XS)
                                    .strong()
                                    .color(TEXT_MUTED),
                            ),
                        );
                        ui.add_sized(
                            [150.0, 20.0],
                            egui::Label::new(
                                RichText::new("TIMESTAMP")
                                    .size(FONT_XS)
                                    .strong()
                                    .color(TEXT_MUTED),
                            ),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new("ACTIONS")
                                    .size(FONT_XS)
                                    .strong()
                                    .color(TEXT_MUTED),
                            );
                        });
                    });

                    ui.add_space(6.0);
                    draw_divider(ui, CARD_BORDER);
                }

                if filtered.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        let msg = if is_project_view {
                            format!("No deployments recorded for '{}' yet.", self.selected_env_key)
                        } else {
                            "No deployments recorded yet.".to_string()
                        };
                        ui.label(
                            RichText::new(msg)
                                .size(FONT_MD)
                                .color(TEXT_MUTED),
                        );
                        ui.add_space(20.0);
                    });
                }

                if is_stacked_view {
                    // Mobile / narrow stacked card view (< 900px)
                    for (idx, dep) in filtered.iter().take(visible_count).enumerate() {
                        if idx > 0 {
                            draw_divider(ui, SUBTLE_BG);
                        }
                        ui.add_space(6.0);
                        let is_selected = self.selected_deployment_id.as_deref() == Some(&dep.id);

                        // Line 1: Status + Env + Version (left), Timestamp (right)
                        horizontal_row(ui, 24.0, |ui| {
                            let (s_rect, _) =
                                ui.allocate_exact_size(Vec2::new(96.0, 22.0), Sense::hover());
                            let (s_bg, s_fg) = status_pill_colors(dep.status);
                            if dep.status == DeploymentStatus::Running {
                                ui.ctx().request_repaint();
                                let time = ui.input(|i| i.time);
                                paint_running_pill_left_aligned(
                                    ui.painter(),
                                    Pos2::new(s_rect.left(), s_rect.center().y),
                                    &dep.status.label().to_uppercase(),
                                    s_bg,
                                    s_fg,
                                    time,
                                );
                            } else {
                                paint_pill_left_aligned(
                                    ui.painter(),
                                    Pos2::new(s_rect.left(), s_rect.center().y),
                                    &dep.status.label().to_uppercase(),
                                    s_bg,
                                    s_fg,
                                );
                            }

                            let (_, env_fg) = env_pastel_pill_colors(&dep.environment);
                            ui.label(
                                RichText::new(format_env_badge(&dep.environment))
                                    .size(FONT_SM)
                                    .strong()
                                    .color(env_fg),
                            );

                            ui.label(
                                RichText::new(format!("v{}", dep.version))
                                    .size(FONT_MD)
                                    .strong()
                                    .color(TEXT_PRIMARY),
                            );

                            if dep.is_rollback {
                                ui.label(
                                    RichText::new("ROLLBACK")
                                        .size(FONT_XS)
                                        .strong()
                                        .color(WARN_PILL_TEXT),
                                );
                            }

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new(
                                        dep.started_at.format("%Y-%m-%d %H:%M").to_string(),
                                    )
                                    .size(FONT_SM)
                                    .color(TEXT_MUTED),
                                );
                            });
                        });

                        ui.add_space(4.0);

                        // Line 2: Git branch @ commit + git_changes / server summary
                        horizontal_row(ui, 20.0, |ui| {
                            let (icon_r, _) =
                                ui.allocate_exact_size(Vec2::new(14.0, 14.0), Sense::hover());
                            draw_icon_git_branch(
                                ui.painter(),
                                Pos2::new(icon_r.left(), icon_r.top() + 3.0),
                                TEXT_MUTED,
                            );
                            let git_line = match (&dep.git_branch, &dep.git_commit) {
                                (Some(b), Some(c)) => format!("{b} @ {c}"),
                                _ => dep.project_name.clone(),
                            };
                            ui.label(
                                RichText::new(git_line)
                                    .size(FONT_SM)
                                    .color(TEXT_SECONDARY),
                            );

                            let sub_detail = dep
                                .git_changes
                                .first()
                                .cloned()
                                .unwrap_or_else(|| dep.servers.join(", "));
                            let truncated = if sub_detail.chars().count() > 36 {
                                let s: String = sub_detail.chars().take(34).collect();
                                format!("{s}…")
                            } else {
                                sub_detail
                            };
                            let det_resp = ui.label(
                                RichText::new(format!("— {truncated}"))
                                    .size(FONT_SM)
                                    .color(TEXT_MUTED),
                            );
                            if !dep.git_changes.is_empty() {
                                det_resp.on_hover_text(format!(
                                    "Code Changes in v{}:\n• {}",
                                    dep.version,
                                    dep.git_changes.join("\n• ")
                                ));
                            }
                        });

                        ui.add_space(6.0);

                        // Line 3: Duration (left), Action buttons (right)
                        horizontal_row(ui, 28.0, |ui| {
                            let dur_str = dep
                                .duration_ms
                                .map(|ms| format!("took {:.1}s", ms as f64 / 1000.0))
                                .unwrap_or_else(|| "running...".to_string());
                            ui.label(
                                RichText::new(dur_str)
                                    .size(FONT_SM)
                                    .color(TEXT_MUTED),
                            );

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                                let is_busy =
                                    self.is_env_deploying(&dep.project_id, &dep.environment);

                                if outline_icon_button(
                                    ui,
                                    ("stacked_rerun_btn", &dep.id),
                                    OutlineIcon::Refresh,
                                    None,
                                    TEXT_SECONDARY,
                                    SUBTLE_BG,
                                    PILL_CONTAINER_BG,
                                    CARD_BORDER,
                                    !is_busy,
                                )
                                .on_hover_text("Re-run this deployment version and pipeline")
                                .clicked()
                                    && !is_busy
                                {
                                    redeploy_target = Some((dep.clone(), false));
                                }

                                if dep.status == DeploymentStatus::Succeeded
                                    && outline_icon_button(
                                        ui,
                                        ("stacked_rollback_btn", &dep.id),
                                        OutlineIcon::RotateCcw,
                                        Some("Rollback"),
                                        WARN_PILL_TEXT,
                                        WARN_PILL_BG,
                                        Color32::from_rgb(253, 230, 138),
                                        Color32::from_rgb(251, 191, 36),
                                        !is_busy,
                                    )
                                    .on_hover_text(format!(
                                        "Rollback {} to v{}",
                                        dep.environment, dep.version
                                    ))
                                    .clicked()
                                    && !is_busy
                                {
                                    redeploy_target = Some((dep.clone(), true));
                                }

                                let logs_btn = egui::Button::new(
                                    RichText::new(if is_selected {
                                        "Viewing Logs"
                                    } else {
                                        "View Logs"
                                    })
                                    .size(FONT_SM)
                                    .strong()
                                    .color(TEXT_PRIMARY),
                                )
                                .fill(if is_selected {
                                    PILL_CONTAINER_BG
                                } else {
                                    SUBTLE_BG
                                })
                                .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                                .corner_radius(6.0);

                                if ui.add(logs_btn).clicked() {
                                    self.selected_deployment_id = Some(dep.id.clone());
                                    self.console_collapsed = false;
                                }
                            });
                        });
                        ui.add_space(6.0);
                    }
                } else {
                    // Desktop normal table (>= 900px, 48px row height)
                    for (idx, dep) in filtered.iter().take(visible_count).enumerate() {
                        if idx > 0 {
                            draw_divider(ui, SUBTLE_BG);
                        }
                        ui.add_space(6.0);
                        let is_selected = self.selected_deployment_id.as_deref() == Some(&dep.id);
                        let row_h = TABLE_ROW_HEIGHT;

                        horizontal_row(ui, row_h, |ui| {
                            // STATUS column
                            let (s_rect, _) =
                                ui.allocate_exact_size(Vec2::new(110.0, row_h), Sense::hover());
                            let (s_bg, s_fg) = status_pill_colors(dep.status);
                            if dep.status == DeploymentStatus::Running {
                                ui.ctx().request_repaint();
                                let time = ui.input(|i| i.time);
                                paint_running_pill_left_aligned(
                                    ui.painter(),
                                    Pos2::new(s_rect.left(), s_rect.center().y),
                                    &dep.status.label().to_uppercase(),
                                    s_bg,
                                    s_fg,
                                    time,
                                );
                            } else {
                                paint_pill_left_aligned(
                                    ui.painter(),
                                    Pos2::new(s_rect.left(), s_rect.center().y),
                                    &dep.status.label().to_uppercase(),
                                    s_bg,
                                    s_fg,
                                );
                            }

                            // ENV column
                            let (e_rect, _) =
                                ui.allocate_exact_size(Vec2::new(80.0, row_h), Sense::hover());
                            let (_, env_fg) = env_pastel_pill_colors(&dep.environment);
                            ui.painter().text(
                                Pos2::new(e_rect.left(), e_rect.center().y),
                                Align2::LEFT_CENTER,
                                format_env_badge(&dep.environment),
                                FontId::proportional(FONT_MD),
                                env_fg,
                            );

                            // VERSION column (+ ROLLBACK tag if rollback)
                            let (v_rect, _) =
                                ui.allocate_exact_size(Vec2::new(95.0, row_h), Sense::hover());
                            ui.painter().text(
                                Pos2::new(
                                    v_rect.left(),
                                    if dep.is_rollback {
                                        v_rect.top() + 14.0
                                    } else {
                                        v_rect.center().y
                                    },
                                ),
                                Align2::LEFT_CENTER,
                                format!("v{}", dep.version),
                                FontId::proportional(FONT_MD),
                                TEXT_PRIMARY,
                            );
                            if dep.is_rollback {
                                ui.painter().text(
                                    Pos2::new(v_rect.left(), v_rect.top() + 32.0),
                                    Align2::LEFT_CENTER,
                                    "ROLLBACK",
                                    FontId::proportional(FONT_XS),
                                    WARN_PILL_TEXT,
                                );
                            }

                            // DETAILS & GIT CHANGES column (Feature #10)
                            let (d_rect, d_resp) =
                                ui.allocate_exact_size(Vec2::new(280.0, row_h), Sense::hover());
                            draw_icon_git_branch(
                                ui.painter(),
                                Pos2::new(d_rect.left() + 6.0, d_rect.top() + 14.0),
                                TEXT_MUTED,
                            );
                            let git_line = match (&dep.git_branch, &dep.git_commit) {
                                (Some(b), Some(c)) => format!("{b} @ {c}"),
                                _ => dep.project_name.clone(),
                            };
                            ui.painter().text(
                                Pos2::new(d_rect.left() + 18.0, d_rect.top() + 14.0),
                                Align2::LEFT_CENTER,
                                git_line,
                                FontId::proportional(FONT_SM),
                                TEXT_SECONDARY,
                            );

                            // Second line in DETAILS: Latest commit subject or server list
                            let sub_detail = dep
                                .git_changes
                                .first()
                                .cloned()
                                .unwrap_or_else(|| dep.servers.join(", "));
                            let truncated_detail = if sub_detail.chars().count() > 38 {
                                let s: String = sub_detail.chars().take(36).collect();
                                format!("{s}…")
                            } else {
                                sub_detail
                            };
                            ui.painter().text(
                                Pos2::new(d_rect.left() + 6.0, d_rect.top() + 33.0),
                                Align2::LEFT_CENTER,
                                truncated_detail,
                                FontId::proportional(FONT_SM),
                                TEXT_MUTED,
                            );
                            if !dep.git_changes.is_empty() {
                                d_resp.on_hover_text(format!(
                                    "Code Changes in v{}:\n• {}",
                                    dep.version,
                                    dep.git_changes.join("\n• ")
                                ));
                            }

                            // TIMESTAMP column
                            let (t_rect, _) =
                                ui.allocate_exact_size(Vec2::new(150.0, row_h), Sense::hover());
                            ui.painter().text(
                                Pos2::new(t_rect.left(), t_rect.top() + 14.0),
                                Align2::LEFT_CENTER,
                                dep.started_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                                FontId::proportional(FONT_SM),
                                TEXT_PRIMARY,
                            );
                            let dur_str = dep
                                .duration_ms
                                .map(|ms| format!("took {:.1}s", ms as f64 / 1000.0))
                                .unwrap_or_else(|| "running...".to_string());
                            ui.painter().text(
                                Pos2::new(t_rect.left(), t_rect.top() + 33.0),
                                Align2::LEFT_CENTER,
                                dur_str,
                                FontId::proportional(FONT_SM),
                                TEXT_MUTED,
                            );

                            // ACTIONS column (Rollback + Re-run + View Logs) using outline icons
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                                let is_busy =
                                    self.is_env_deploying(&dep.project_id, &dep.environment);

                                if outline_icon_button(
                                    ui,
                                    ("hist_rerun_btn", &dep.id),
                                    OutlineIcon::Refresh,
                                    None,
                                    TEXT_SECONDARY,
                                    SUBTLE_BG,
                                    PILL_CONTAINER_BG,
                                    CARD_BORDER,
                                    !is_busy,
                                )
                                .on_hover_text("Re-run this deployment version and pipeline")
                                .clicked()
                                    && !is_busy
                                {
                                    redeploy_target = Some((dep.clone(), false));
                                }

                                if dep.status == DeploymentStatus::Succeeded
                                    && outline_icon_button(
                                        ui,
                                        ("hist_rollback_btn", &dep.id),
                                        OutlineIcon::RotateCcw,
                                        Some("Rollback"),
                                        WARN_PILL_TEXT,
                                        WARN_PILL_BG,
                                        Color32::from_rgb(253, 230, 138),
                                        Color32::from_rgb(251, 191, 36),
                                        !is_busy,
                                    )
                                    .on_hover_text(format!(
                                        "Rollback {} to v{}",
                                        dep.environment, dep.version
                                    ))
                                    .clicked()
                                    && !is_busy
                                {
                                    redeploy_target = Some((dep.clone(), true));
                                }

                                let logs_btn = egui::Button::new(
                                    RichText::new(if is_selected {
                                        "Viewing Logs"
                                    } else {
                                        "View Logs"
                                    })
                                    .size(FONT_SM)
                                    .strong()
                                    .color(TEXT_PRIMARY),
                                )
                                .fill(if is_selected {
                                    PILL_CONTAINER_BG
                                } else {
                                    SUBTLE_BG
                                })
                                .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                                .corner_radius(6.0);

                                if ui.add(logs_btn).clicked() {
                                    self.selected_deployment_id = Some(dep.id.clone());
                                    self.console_collapsed = false;
                                }
                            });
                        });
                        ui.add_space(6.0);
                    }
                }

                if total_count > visible_count {
                    if visible_count > 0 {
                        draw_divider(ui, SUBTLE_BG);
                    }
                    ui.add_space(10.0);
                    ui.vertical_centered(|ui| {
                        let remaining = total_count - visible_count;
                        let next_chunk = remaining.min(10);
                        let btn_text = format!("Show More (+{next_chunk} of {remaining} remaining)...");
                        let show_more_btn = egui::Button::new(
                            RichText::new(btn_text)
                                .size(FONT_SM)
                                .strong()
                                .color(PRIMARY_BLUE),
                        )
                        .fill(SUBTLE_BG)
                        .stroke(Stroke::new(1.0_f32, PRIMARY_BLUE.gamma_multiply(0.4)))
                        .corner_radius(6.0)
                        .min_size(Vec2::new(240.0, 32.0));

                        if ui
                            .add(show_more_btn)
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            self.history_visible_limit = self.history_visible_limit.saturating_add(10);
                        }
                    });
                    ui.add_space(4.0);
                } else if total_count > 5 {
                    draw_divider(ui, SUBTLE_BG);
                    ui.add_space(8.0);
                    ui.vertical_centered(|ui| {
                        horizontal_row(ui, 24.0, |ui| {
                            ui.label(
                                RichText::new(format!("Showing all {total_count} deployment records."))
                                    .size(FONT_XS)
                                    .color(TEXT_MUTED),
                            );
                            let collapse_btn = egui::Button::new(
                                RichText::new("Collapse to Last 5")
                                    .size(FONT_XS)
                                    .color(TEXT_SECONDARY),
                            )
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE);
                            if ui
                                .add(collapse_btn)
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                self.history_visible_limit = 5;
                            }
                        });
                    });
                    ui.add_space(4.0);
                }
            });

        if let Some((target, is_rb)) = redeploy_target {
            self.trigger_redeploy_from_record(&target, is_rb);
        }

        ui.add_space(18.0);

        let selected_dep = self
            .selected_deployment_id
            .as_ref()
            .and_then(|id| filtered.iter().find(|d| &d.id == id))
            .or_else(|| filtered.first())
            .cloned();

        if let Some(dep) = &selected_dep {
            if self.selected_deployment_id.as_ref() != Some(&dep.id) {
                self.selected_deployment_id = Some(dep.id.clone());
            }
        }

        // Code changes to display:
        // Strictly show code changes of the active project.
        // If a deployment of the active project is selected and has recorded git_changes, show them.
        // Otherwise, if viewing a project workspace, fall back to the active project's current git branch commits (cached_git_changes).
        let active_project_name = project_filter.and_then(|pid| {
            self.storage
                .projects
                .iter()
                .find(|p| p.id == pid)
                .map(|p| p.name.clone())
        });

        let code_changes_data: Option<(String, Vec<String>)> = if let Some(dep) = &selected_dep {
            if !dep.git_changes.is_empty() {
                Some((
                    format!(
                        "CODE CHANGES — {} v{} ({} entry/entries)",
                        dep.project_name,
                        dep.version,
                        dep.git_changes.len()
                    ),
                    dep.git_changes.clone(),
                ))
            } else if let Some(pname) = &active_project_name {
                if !self.cached_git_changes.is_empty() {
                    Some((
                        format!(
                            "CODE CHANGES — {} (Current Branch - {} entry/entries)",
                            pname,
                            self.cached_git_changes.len()
                        ),
                        self.cached_git_changes.clone(),
                    ))
                } else {
                    None
                }
            } else {
                None
            }
        } else if let Some(pname) = &active_project_name {
            if !self.cached_git_changes.is_empty() {
                Some((
                    format!(
                        "CODE CHANGES — {} (Current Branch - {} entry/entries)",
                        pname,
                        self.cached_git_changes.len()
                    ),
                    self.cached_git_changes.clone(),
                ))
            } else {
                None
            }
        } else {
            None
        };

        if let Some((hdr_title, changes_lines)) = code_changes_data {
            egui::Frame::new()
                .fill(CARD_BG)
                .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                .corner_radius(10.0)
                .inner_margin(Margin::symmetric(18, 12))
                .show(ui, |ui| {
                    horizontal_row(ui, 26.0, |ui| {
                        let (chev_r, chev_resp) =
                            ui.allocate_exact_size(Vec2::new(18.0, 18.0), Sense::click());
                        let chev_resp = chev_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                        draw_icon_chevron(
                            ui.painter(),
                            chev_r.center(),
                            !self.changes_collapsed,
                            TEXT_PRIMARY,
                        );
                        draw_icon_git_branch(
                            ui.painter(),
                            Pos2::new(chev_r.right() + 10.0, chev_r.center().y),
                            PRIMARY_BLUE,
                        );
                        ui.add_space(18.0);
                        let hdr_btn = egui::Button::new(
                            RichText::new(&hdr_title)
                                .size(FONT_SM)
                                .strong()
                                .color(TEXT_PRIMARY),
                        )
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::NONE);
                        if chev_resp.clicked() || ui.add(hdr_btn).clicked() {
                            self.changes_collapsed = !self.changes_collapsed;
                        }
                    });

                    if !self.changes_collapsed {
                        ui.add_space(6.0);
                        draw_divider(ui, SUBTLE_BG);
                        ui.add_space(6.0);
                        for change_line in &changes_lines {
                            ui.label(
                                RichText::new(format!("• {change_line}"))
                                    .size(FONT_SM)
                                    .color(TEXT_SECONDARY),
                            );
                        }
                    }
                });
            ui.add_space(14.0);
        }

        if let Some(dep) = selected_dep {
            // Collapsible CONSOLE OUTPUT Section (Fixed 32px header row)
            horizontal_row(ui, 32.0, |ui| {
                let (chev_r, chev_resp) =
                    ui.allocate_exact_size(Vec2::new(18.0, 18.0), Sense::click());
                let chev_resp = chev_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                draw_icon_chevron(
                    ui.painter(),
                    chev_r.center(),
                    !self.console_collapsed,
                    TEXT_PRIMARY,
                );
                let title_resp = ui.add(
                    egui::Button::new(
                        RichText::new("CONSOLE OUTPUT")
                            .size(FONT_MD)
                            .strong()
                            .color(TEXT_PRIMARY),
                    )
                    .fill(Color32::TRANSPARENT)
                    .stroke(Stroke::NONE),
                );
                if chev_resp.clicked() || title_resp.clicked() {
                    self.console_collapsed = !self.console_collapsed;
                }

                ui.label(
                    RichText::new(format!(
                        "— {} ({}) v{}",
                        dep.project_name,
                        format_env_badge(&dep.environment),
                        dep.version
                    ))
                    .size(FONT_SM)
                    .strong()
                    .color(TEXT_MUTED),
                );
                render_light_status_badge(ui, dep.status);

                if dep.status == DeploymentStatus::Running
                    || self.is_deployment_active(&dep.id)
                {
                    ui.add_space(8.0);
                    ui.ctx().request_repaint();
                    let time = ui.input(|i| i.time);
                    let (spin_r, _) =
                        ui.allocate_exact_size(Vec2::new(14.0, 14.0), Sense::hover());
                    draw_circle_loading_spinner(
                        ui.painter(),
                        spin_r.center(),
                        5.0,
                        PRIMARY_BLUE,
                        time,
                    );
                    ui.label(
                        RichText::new("Pipeline running...")
                            .size(FONT_SM)
                            .strong()
                            .color(PRIMARY_BLUE),
                    );
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let toggle_lbl = if self.console_collapsed {
                        "Expand Console"
                    } else {
                        "Collapse Console"
                    };
                    let btn = egui::Button::new(
                        RichText::new(toggle_lbl)
                            .size(FONT_SM)
                            .strong()
                            .color(TEXT_SECONDARY),
                    )
                    .fill(CARD_BG)
                    .stroke(Stroke::new(1.0_f32, CARD_BORDER));
                    if ui.add(btn).clicked() {
                        self.console_collapsed = !self.console_collapsed;
                    }

                    if !self.console_collapsed && !dep.step_logs.is_empty() {
                        let all_steps_collapsed =
                            self.collapsed_step_indices.len() >= dep.step_logs.len();
                        let steps_btn_lbl = if all_steps_collapsed {
                            "Expand All Steps"
                        } else {
                            "Collapse All Steps"
                        };
                        let steps_btn = egui::Button::new(
                            RichText::new(steps_btn_lbl)
                                .size(FONT_SM)
                                .strong()
                                .color(TEXT_SECONDARY),
                        )
                        .fill(CARD_BG)
                        .stroke(Stroke::new(1.0_f32, CARD_BORDER));
                        if ui.add(steps_btn).clicked() {
                            if all_steps_collapsed {
                                self.collapsed_step_indices.clear();
                            } else {
                                self.collapsed_step_indices = (0..dep.step_logs.len()).collect();
                            }
                        }
                    }
                });
            });

            if !self.console_collapsed {
                ui.add_space(10.0);
                self.render_terminal_window(ui, &dep);
            }
        }
    }

    pub(crate) fn render_terminal_window(&mut self, ui: &mut egui::Ui, dep: &DeploymentRecord) {
        egui::Frame::new()
            .fill(TERM_BODY_BG)
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(30, 41, 59)))
            .corner_radius(10.0)
            .shadow(Shadow {
                offset: [0, 4],
                blur: 12,
                spread: 0,
                color: Color32::from_black_alpha(25),
            })
            .show(ui, |ui| {
                let avail_w = ui.available_width();
                let (top_rect, _) =
                    ui.allocate_exact_size(Vec2::new(avail_w, 36.0), Sense::hover());
                ui.painter().rect_filled(
                    top_rect,
                    CornerRadius {
                        nw: 10,
                        ne: 10,
                        sw: 0,
                        se: 0,
                    },
                    TERM_HEADER_BG,
                );
                let mut dot_x = top_rect.left() + 18.0;
                for dot_color in [TERM_RED, TERM_YELLOW, TERM_GREEN] {
                    ui.painter().circle_filled(
                        Pos2::new(dot_x, top_rect.center().y),
                        5.0,
                        dot_color,
                    );
                    dot_x += 16.0;
                }
                ui.painter().text(
                    Pos2::new(dot_x + 8.0, top_rect.center().y),
                    Align2::LEFT_CENTER,
                    "TERMINAL OUTPUT",
                    FontId::proportional(FONT_XS),
                    SIDEBAR_TEXT_MUTED,
                );

                if dep.status == DeploymentStatus::Running
                    || self.is_deployment_active(&dep.id)
                {
                    ui.ctx().request_repaint();
                    let time = ui.input(|i| i.time);
                    let running_lbl_galley = ui.painter().layout_no_wrap(
                        "RUNNING PIPELINE".to_string(),
                        FontId::proportional(FONT_XS),
                        Color32::from_rgb(56, 189, 248),
                    );
                    let r_text_w = running_lbl_galley.size().x;
                    let r_spinner_r = 4.2;
                    let r_spinner_x = top_rect.right() - 20.0 - r_text_w - 8.0 - r_spinner_r;
                    let r_spinner_c = Pos2::new(r_spinner_x, top_rect.center().y);
                    draw_circle_loading_spinner(
                        ui.painter(),
                        r_spinner_c,
                        r_spinner_r,
                        Color32::from_rgb(56, 189, 248),
                        time,
                    );
                    ui.painter().galley(
                        Pos2::new(
                            r_spinner_x + r_spinner_r + 6.0,
                            top_rect.center().y - running_lbl_galley.size().y / 2.0,
                        ),
                        running_lbl_galley,
                        Color32::from_rgb(56, 189, 248),
                    );
                }

                egui::Frame::new()
                    .fill(TERM_BODY_BG)
                    .corner_radius(CornerRadius {
                        nw: 0,
                        ne: 0,
                        sw: 10,
                        se: 10,
                    })
                    .inner_margin(Margin::symmetric(20, 16))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        if dep.step_logs.is_empty() {
                            ui.label(
                                RichText::new("Initializing deployment pipeline...")
                                    .size(FONT_SM)
                                    .color(SIDEBAR_TEXT_MUTED),
                            );
                        }

                        for (i, step_log) in dep.step_logs.iter().enumerate() {
                            let is_step_collapsed = self.collapsed_step_indices.contains(&i);
                            let color = match step_log.status {
                                DeploymentStatus::Succeeded => TERM_GREEN,
                                DeploymentStatus::Failed => TERM_RED,
                                DeploymentStatus::Running => TERM_YELLOW,
                                DeploymentStatus::Cancelled => SIDEBAR_TEXT_MUTED,
                            };

                            let step_resp = horizontal_row(ui, 24.0, |ui| {
                                let (ic_r, _) =
                                    ui.allocate_exact_size(Vec2::new(14.0, 14.0), Sense::hover());
                                if step_log.status == DeploymentStatus::Succeeded {
                                    draw_icon_check(ui.painter(), ic_r.center(), color);
                                } else if step_log.status == DeploymentStatus::Running {
                                    ui.ctx().request_repaint();
                                    let time = ui.input(|i| i.time);
                                    draw_circle_loading_spinner(
                                        ui.painter(),
                                        ic_r.center(),
                                        4.8,
                                        color,
                                        time,
                                    );
                                } else {
                                    ui.painter().circle_filled(ic_r.center(), 4.5, color);
                                }

                                ui.label(
                                    RichText::new(format!(
                                        "[{}/{}] {}",
                                        i + 1,
                                        dep.step_logs.len(),
                                        step_log.step_name
                                    ))
                                    .size(FONT_MD)
                                    .strong()
                                    .color(color),
                                );
                                ui.label(
                                    RichText::new(format!("({} ms)", step_log.duration_ms))
                                        .size(FONT_SM)
                                        .color(Color32::from_rgb(100, 116, 139)),
                                );

                                let (ch_r, _) =
                                    ui.allocate_exact_size(Vec2::new(14.0, 14.0), Sense::hover());
                                draw_icon_chevron(
                                    ui.painter(),
                                    ch_r.center(),
                                    !is_step_collapsed,
                                    Color32::from_rgb(100, 116, 139),
                                );
                            })
                            .response
                            .interact(Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand);

                            if step_resp
                                .on_hover_text("Click to collapse or expand this step")
                                .clicked()
                            {
                                if is_step_collapsed {
                                    self.collapsed_step_indices.remove(&i);
                                } else {
                                    self.collapsed_step_indices.insert(i);
                                }
                            }

                            if !is_step_collapsed {
                                ui.indent(format!("term_step_indent_{i}"), |ui| {
                                    ui.label(
                                        RichText::new(format!("$ {}", step_log.command_summary))
                                            .size(FONT_SM)
                                            .color(Color32::from_rgb(100, 116, 139)),
                                    );
                                    for line in step_log.output.lines() {
                                        let trimmed = line.trim();
                                        let line_color = if trimmed.starts_with("[OK]")
                                            || trimmed.starts_with("Updated ")
                                        {
                                            TERM_GREEN
                                        } else if trimmed.starts_with("[FAILED]")
                                            || trimmed.starts_with("[ERROR]")
                                        {
                                            TERM_RED
                                        } else {
                                            Color32::from_rgb(226, 232, 240)
                                        };
                                        ui.label(
                                            RichText::new(trimmed).size(FONT_SM).color(line_color),
                                        );
                                    }
                                });
                            }
                            ui.add_space(10.0);
                        }

                        if dep.status == DeploymentStatus::Running
                            || self.is_deployment_active(&dep.id)
                        {
                            ui.add_space(4.0);
                            horizontal_row(ui, 24.0, |ui| {
                                let (ic_r, _) =
                                    ui.allocate_exact_size(Vec2::new(14.0, 14.0), Sense::hover());
                                ui.ctx().request_repaint();
                                let time = ui.input(|i| i.time);
                                draw_circle_loading_spinner(
                                    ui.painter(),
                                    ic_r.center(),
                                    5.0,
                                    Color32::from_rgb(56, 189, 248),
                                    time,
                                );
                                let active_step_name = dep
                                    .step_logs
                                    .last()
                                    .map(|s| s.step_name.as_str())
                                    .unwrap_or("Running pipeline");
                                ui.label(
                                    RichText::new(format!("Executing: {active_step_name}..."))
                                        .size(FONT_MD)
                                        .color(Color32::from_rgb(56, 189, 248)),
                                );
                            });
                        }
                    });
            });
    }
}
