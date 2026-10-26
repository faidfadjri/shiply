use crate::credentials::read_os_vault_credential;
use crate::domain::{
    format_env_badge, DeploymentRecord, DeploymentStatus, PipelineStep, StepKind,
};
use crate::executor::{run_deployment, DeploymentRequest, ExecutorEvent};
use crate::installer::{
    check_install_state, default_install_dir, install_or_update_self, InstallState, APP_VERSION,
};
use crate::presets::{create_custom_project, default_projects};
use crate::storage::AppStorage;
use crate::version::{
    bump_semver, read_git_metadata, read_package_json_version, GitMetadata, VersionBump,
};
use eframe::egui::{self, Color32, FontId, RichText, ScrollArea, Stroke, Vec2};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
enum SidebarSelection {
    AllDeployments,
    Project(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkspaceTab {
    PipelineSteps,
    HistoryAndLogs,
    ServerSettings,
}

pub struct ShiplyApp {
    storage: AppStorage,
    selection: SidebarSelection,
    selected_env_key: String,
    active_tab: WorkspaceTab,
    history_env_filter: Option<String>,
    selected_deployment_id: Option<String>,

    // Deployment version & release controls
    deploy_version_input: String,
    bump_package_json: bool,
    create_git_tag: bool,
    manual_ssh_password: String,

    // Cached project metadata (Git + package.json + OS Vault check)
    cached_project_id: Option<String>,
    cached_pkg_version: Option<String>,
    cached_git_meta: GitMetadata,
    cached_vault_found: bool,

    // Background execution channel
    event_tx: Sender<ExecutorEvent>,
    event_rx: Receiver<ExecutorEvent>,
    active_deployment_id: Option<String>,

    // New Project Modal state
    show_new_project_modal: bool,
    new_project_name: String,
    new_project_path: String,
    new_project_desc: String,

    // New Environment Modal state
    show_new_env_modal: bool,
    new_env_name: String,
    new_env_clone_from: String,

    // Status banner
    status_banner: Option<(String, bool)>,
}

impl ShiplyApp {
    pub fn new(storage: AppStorage) -> Self {
        let (event_tx, event_rx) = mpsc::channel();
        let initial_project_id = storage.projects.first().map(|p| p.id.clone());
        let initial_env_key = storage
            .projects
            .first()
            .and_then(|p| p.environments.first())
            .map(|e| e.key.clone())
            .unwrap_or_else(|| "dev".to_string());
        let selection = match &initial_project_id {
            Some(id) => SidebarSelection::Project(id.clone()),
            None => SidebarSelection::AllDeployments,
        };
        let initial_selected_dep = storage.deployments.first().map(|d| d.id.clone());

        let mut app = Self {
            storage,
            selection,
            selected_env_key: initial_env_key.clone(),
            active_tab: WorkspaceTab::PipelineSteps,
            history_env_filter: None,
            selected_deployment_id: initial_selected_dep,
            deploy_version_input: "1.0.0".to_string(),
            bump_package_json: true,
            create_git_tag: false,
            manual_ssh_password: String::new(),
            cached_project_id: None,
            cached_pkg_version: None,
            cached_git_meta: GitMetadata::default(),
            cached_vault_found: false,
            event_tx,
            event_rx,
            active_deployment_id: None,
            show_new_project_modal: false,
            new_project_name: String::new(),
            new_project_path: String::new(),
            new_project_desc: String::new(),
            show_new_env_modal: false,
            new_env_name: String::new(),
            new_env_clone_from: initial_env_key,
            status_banner: None,
        };
        app.refresh_selected_project_metadata(true);
        app
    }

    pub fn set_status_banner(&mut self, message: String, is_success: bool) {
        self.status_banner = Some((message, is_success));
    }

    fn refresh_selected_project_metadata(&mut self, update_version_input: bool) {
        let SidebarSelection::Project(proj_id) = &self.selection else {
            return;
        };
        let Some(proj) = self.storage.projects.iter().find(|p| &p.id == proj_id) else {
            return;
        };

        // Ensure selected_env_key exists on this project
        if proj.get_env(&self.selected_env_key).is_none() {
            if let Some(first_env) = proj.environments.first() {
                self.selected_env_key = first_env.key.clone();
            }
        }

        let path = PathBuf::from(&proj.local_path);
        self.cached_project_id = Some(proj.id.clone());
        self.cached_pkg_version = read_package_json_version(&path);
        self.cached_git_meta = read_git_metadata(&path);

        if let Some(env_cfg) = proj.get_env(&self.selected_env_key) {
            self.cached_vault_found = read_os_vault_credential(&env_cfg.vault_target).is_some();
        }

        if update_version_input {
            if let Some(pkg_ver) = &self.cached_pkg_version {
                if pkg_ver != "0.0.0" {
                    self.deploy_version_input = pkg_ver.clone();
                    return;
                }
            }
            if let Some(last_ok) = self
                .storage
                .last_succeeded_deployment(proj_id, &self.selected_env_key)
            {
                self.deploy_version_input = last_ok.version.clone();
            } else if let Some(pkg_ver) = &self.cached_pkg_version {
                self.deploy_version_input = pkg_ver.clone();
            } else {
                self.deploy_version_input = "1.0.0".to_string();
            }
        }
    }

    fn poll_executor_events(&mut self, ctx: &egui::Context) {
        let mut received = false;
        while let Ok(event) = self.event_rx.try_recv() {
            received = true;
            match event {
                ExecutorEvent::Updated(record) => {
                    self.selected_deployment_id = Some(record.id.clone());
                    self.storage.upsert_deployment(record);
                }
                ExecutorEvent::Finished(record) => {
                    let ok = record.status == DeploymentStatus::Succeeded;
                    let msg = format!(
                        "Deployment of {} ({}) v{} {}",
                        record.project_name,
                        format_env_badge(&record.environment),
                        record.version,
                        record.status.label()
                    );
                    self.status_banner = Some((msg, ok));
                    self.selected_deployment_id = Some(record.id.clone());
                    self.active_deployment_id = None;
                    self.storage.upsert_deployment(record);
                    self.refresh_selected_project_metadata(false);
                }
            }
        }
        if received || self.active_deployment_id.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn start_deployment(&mut self, req: DeploymentRequest) {
        if self.active_deployment_id.is_some() {
            self.status_banner = Some((
                "A deployment is already running. Please wait for it to finish.".to_string(),
                false,
            ));
            return;
        }

        let dep_id = req.deployment_id.clone();
        self.active_deployment_id = Some(dep_id.clone());
        self.selected_deployment_id = Some(dep_id);
        self.active_tab = WorkspaceTab::HistoryAndLogs;
        let tx = self.event_tx.clone();

        thread::spawn(move || {
            run_deployment(req, Some(&tx));
        });
    }

    fn trigger_current_environment_deploy(&mut self) {
        let SidebarSelection::Project(proj_id) = &self.selection else {
            return;
        };
        let Some(proj) = self.storage.projects.iter().find(|p| &p.id == proj_id) else {
            return;
        };
        let Some(env_cfg) = proj.get_env(&self.selected_env_key).cloned() else {
            return;
        };

        let version = if self.deploy_version_input.trim().is_empty() {
            "1.0.0".to_string()
        } else {
            self.deploy_version_input.trim().to_string()
        };

        let manual_pw = if self.manual_ssh_password.is_empty() {
            None
        } else {
            Some(self.manual_ssh_password.clone())
        };

        let req = DeploymentRequest {
            deployment_id: Uuid::new_v4().to_string(),
            project_id: proj.id.clone(),
            project_name: proj.name.clone(),
            local_path: PathBuf::from(&proj.local_path),
            env_config: env_cfg,
            version,
            bump_package_json: self.bump_package_json,
            create_git_tag_on_success: self.create_git_tag,
            manual_password: manual_pw,
            skip_ssh_preflight: false,
        };

        let _ = self.storage.save();
        self.start_deployment(req);
    }

    fn trigger_redeploy_from_record(&mut self, record: &DeploymentRecord) {
        let Some(proj) = self
            .storage
            .projects
            .iter()
            .find(|p| p.id == record.project_id)
        else {
            self.status_banner = Some((
                format!("Project '{}' not found.", record.project_name),
                false,
            ));
            return;
        };
        let Some(mut env_cfg) = proj
            .get_env(&record.environment)
            .or_else(|| proj.environments.first())
            .cloned()
        else {
            return;
        };

        if !record.executed_steps.is_empty() {
            env_cfg.steps = record.executed_steps.clone();
        }

        let manual_pw = if self.manual_ssh_password.is_empty() {
            None
        } else {
            Some(self.manual_ssh_password.clone())
        };

        let req = DeploymentRequest {
            deployment_id: Uuid::new_v4().to_string(),
            project_id: proj.id.clone(),
            project_name: proj.name.clone(),
            local_path: PathBuf::from(&proj.local_path),
            env_config: env_cfg,
            version: record.version.clone(),
            bump_package_json: self.bump_package_json,
            create_git_tag_on_success: self.create_git_tag,
            manual_password: manual_pw,
            skip_ssh_preflight: false,
        };

        self.start_deployment(req);
    }

    fn configure_style(ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = Vec2::new(10.0, 8.0);
        style.spacing.button_padding = Vec2::new(10.0, 6.0);
        style.spacing.indent = 16.0;

        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(13, 15, 20);
        visuals.window_fill = Color32::from_rgb(18, 21, 28);
        visuals.faint_bg_color = Color32::from_rgb(20, 24, 33);
        visuals.extreme_bg_color = Color32::from_rgb(9, 11, 16);
        visuals.widgets.noninteractive.bg_stroke =
            Stroke::new(1.0_f32, Color32::from_rgb(36, 42, 56));
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(24, 29, 40);
        visuals.widgets.inactive.corner_radius = 6.0.into();
        visuals.widgets.hovered.corner_radius = 6.0.into();
        visuals.widgets.active.corner_radius = 6.0.into();
        style.visuals = visuals;
        ctx.set_style(style);
    }
}

impl eframe::App for ShiplyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_executor_events(ctx);
        Self::configure_style(ctx);

        self.render_sidebar(ctx);
        self.render_new_project_modal(ctx);
        self.render_new_environment_modal(ctx);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(13, 15, 20))
                    .inner_margin(egui::Margin::symmetric(20, 16)),
            )
            .show(ctx, |ui| {
                if let Some((msg, is_ok)) = &self.status_banner.clone() {
                    let bg = if *is_ok {
                        Color32::from_rgb(18, 72, 46)
                    } else {
                        Color32::from_rgb(88, 26, 32)
                    };
                    egui::Frame::new()
                        .fill(bg)
                        .corner_radius(6.0)
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(msg).color(Color32::WHITE).strong());
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("✕").clicked() {
                                            self.status_banner = None;
                                        }
                                    },
                                );
                            });
                        });
                    ui.add_space(10.0);
                }

                match self.selection.clone() {
                    SidebarSelection::AllDeployments => {
                        self.render_all_deployments_overview(ui);
                    }
                    SidebarSelection::Project(proj_id) => {
                        self.render_project_workspace(ui, &proj_id);
                    }
                }
            });
    }
}

impl ShiplyApp {
    fn render_sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("shiply_sidebar")
            .resizable(false)
            .exact_width(240.0)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(16, 19, 26))
                    .inner_margin(egui::Margin::symmetric(14, 16))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(30, 35, 48))),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("▲ Shiply")
                            .font(FontId::proportional(20.0))
                            .strong()
                            .color(Color32::WHITE),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new("v0.1")
                                .small()
                                .color(Color32::from_rgb(120, 130, 150)),
                        );
                    });
                });
                ui.add_space(12.0);

                let is_overview = matches!(self.selection, SidebarSelection::AllDeployments);
                let overview_btn = egui::Button::new(
                    RichText::new("All Deployments")
                        .size(13.5)
                        .color(if is_overview {
                            Color32::WHITE
                        } else {
                            Color32::from_rgb(185, 195, 215)
                        }),
                )
                .fill(if is_overview {
                    Color32::from_rgb(37, 99, 235)
                } else {
                    Color32::from_rgb(22, 27, 38)
                })
                .min_size(Vec2::new(ui.available_width(), 34.0));

                if ui.add(overview_btn).clicked() {
                    self.selection = SidebarSelection::AllDeployments;
                }

                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("PROJECTS")
                            .size(11.5)
                            .strong()
                            .color(Color32::from_rgb(130, 140, 165)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("+ Add").clicked() {
                            self.show_new_project_modal = true;
                        }
                    });
                });
                ui.add_space(4.0);

                let mut clicked_project_id = None;
                for project in &self.storage.projects {
                    let is_selected = matches!(
                        &self.selection,
                        SidebarSelection::Project(id) if id == &project.id
                    );
                    let summary_parts: Vec<String> = project
                        .environments
                        .iter()
                        .take(3)
                        .map(|env| {
                            self.storage
                                .last_succeeded_deployment(&project.id, &env.key)
                                .map(|d| format!("{} v{}", env.badge_label(), d.version))
                                .unwrap_or_else(|| format!("{} —", env.badge_label()))
                        })
                        .collect();

                    let frame_fill = if is_selected {
                        Color32::from_rgb(28, 37, 56)
                    } else {
                        Color32::from_rgb(20, 24, 34)
                    };
                    let stroke = if is_selected {
                        Stroke::new(1.5_f32, Color32::from_rgb(70, 130, 240))
                    } else {
                        Stroke::new(1.0_f32, Color32::from_rgb(32, 38, 50))
                    };

                    let resp = egui::Frame::new()
                        .fill(frame_fill)
                        .stroke(stroke)
                        .corner_radius(7.0)
                        .inner_margin(egui::Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(
                                RichText::new(&project.name)
                                    .size(14.0)
                                    .strong()
                                    .color(Color32::WHITE),
                            );
                            ui.add_space(2.0);
                            ui.label(
                                RichText::new(summary_parts.join("  •  "))
                                    .size(11.0)
                                    .color(Color32::from_rgb(145, 160, 185)),
                            );
                        })
                        .response
                        .interact(egui::Sense::click());

                    if resp.clicked() {
                        clicked_project_id = Some(project.id.clone());
                    }
                    ui.add_space(4.0);
                }

                if let Some(pid) = clicked_project_id {
                    self.selection = SidebarSelection::Project(pid);
                    self.refresh_selected_project_metadata(true);
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    let install_state = check_install_state(&default_install_dir(), APP_VERSION);
                    let install_btn_label = match &install_state {
                        InstallState::NotInstalled => "⬇ Install App (.exe + Shortcuts)".to_string(),
                        InstallState::InstalledCurrent { version, .. } => {
                            format!("✔ Installed v{version} (Reinstall)")
                        }
                        InstallState::UpdateReady {
                            installed_version,
                            running_version,
                            ..
                        } => format!("⬆ Update App (v{installed_version} -> v{running_version})"),
                    };

                    if ui
                        .small_button(install_btn_label)
                        .on_hover_text(
                            "Installs or updates Shiply in %LOCALAPPDATA%\\Programs\\Shiply and creates Desktop & Start Menu shortcuts",
                        )
                        .clicked()
                    {
                        match install_or_update_self() {
                            Ok(msg) => self.status_banner = Some((msg, true)),
                            Err(err) => self.status_banner = Some((err, false)),
                        }
                    }

                    if ui
                        .small_button("↺ Reset Presets")
                        .on_hover_text(
                            "Restores default ndm-dq-subscription and indy-subscription presets",
                        )
                        .clicked()
                    {
                        self.storage.projects = default_projects();
                        let _ = self.storage.save();
                        self.refresh_selected_project_metadata(true);
                        self.status_banner =
                            Some(("Restored default project presets.".to_string(), true));
                    }
                    if let Some(active_id) = &self.active_deployment_id {
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(format!("⏳ Deploying ({})...", &active_id[..8]))
                                .color(Color32::from_rgb(250, 190, 60))
                                .strong(),
                        );
                    }
                });
            });
    }

    fn render_new_project_modal(&mut self, ctx: &egui::Context) {
        if !self.show_new_project_modal {
            return;
        }
        egui::Window::new("Add Project")
            .collapsible(false)
            .resizable(false)
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                egui::Grid::new("new_project_form_grid")
                    .num_columns(2)
                    .spacing([12.0, 10.0])
                    .show(ui, |ui| {
                        ui.label("Project Name:");
                        ui.add_sized(
                            [320.0, 28.0],
                            egui::TextEdit::singleline(&mut self.new_project_name),
                        );
                        ui.end_row();

                        ui.label("Local Path:");
                        ui.add_sized(
                            [320.0, 28.0],
                            egui::TextEdit::singleline(&mut self.new_project_path)
                                .font(egui::TextStyle::Monospace),
                        );
                        ui.end_row();

                        ui.label("Description:");
                        ui.add_sized(
                            [320.0, 28.0],
                            egui::TextEdit::singleline(&mut self.new_project_desc),
                        );
                        ui.end_row();
                    });

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let create_btn = egui::Button::new(
                        RichText::new("Create Project").color(Color32::WHITE).strong(),
                    )
                    .fill(Color32::from_rgb(37, 99, 235));
                    if ui.add(create_btn).clicked()
                        && !self.new_project_name.trim().is_empty()
                        && !self.new_project_path.trim().is_empty()
                    {
                        let proj = create_custom_project(
                            &self.new_project_name,
                            &self.new_project_path,
                            &self.new_project_desc,
                        );
                        let id = proj.id.clone();
                        self.storage.projects.push(proj);
                        let _ = self.storage.save();
                        self.selection = SidebarSelection::Project(id);
                        self.refresh_selected_project_metadata(true);
                        self.new_project_name.clear();
                        self.new_project_path.clear();
                        self.new_project_desc.clear();
                        self.show_new_project_modal = false;
                    }
                    if ui.button("Cancel").clicked() {
                        self.show_new_project_modal = false;
                    }
                });
            });
    }

    fn render_new_environment_modal(&mut self, ctx: &egui::Context) {
        if !self.show_new_env_modal {
            return;
        }
        let SidebarSelection::Project(proj_id) = self.selection.clone() else {
            self.show_new_env_modal = false;
            return;
        };
        let Some(proj_idx) = self.storage.projects.iter().position(|p| p.id == proj_id) else {
            self.show_new_env_modal = false;
            return;
        };

        let existing_envs: Vec<(String, String)> = self.storage.projects[proj_idx]
            .environments
            .iter()
            .map(|e| (e.key.clone(), e.name.clone()))
            .collect();

        egui::Window::new("Add Environment Level")
            .collapsible(false)
            .resizable(false)
            .default_width(440.0)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(
                        "Create a new environment level (e.g. UAT, QA, Canary, Sandbox, Pre-Prod).",
                    )
                    .size(12.5)
                    .color(Color32::from_rgb(150, 160, 180)),
                );
                ui.add_space(8.0);

                egui::Grid::new("new_environment_form_grid")
                    .num_columns(2)
                    .spacing([12.0, 10.0])
                    .show(ui, |ui| {
                        ui.label("Environment Name:");
                        ui.add_sized(
                            [280.0, 28.0],
                            egui::TextEdit::singleline(&mut self.new_env_name)
                                .hint_text("e.g. UAT, QA, Canary"),
                        );
                        ui.end_row();

                        ui.label("Clone Template From:");
                        ui.horizontal_wrapped(|ui| {
                            for (key, name) in &existing_envs {
                                if ui
                                    .selectable_label(self.new_env_clone_from == *key, name)
                                    .clicked()
                                {
                                    self.new_env_clone_from = key.clone();
                                }
                            }
                        });
                        ui.end_row();
                    });

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let create_btn = egui::Button::new(
                        RichText::new("Add Environment")
                            .color(Color32::WHITE)
                            .strong(),
                    )
                    .fill(Color32::from_rgb(37, 99, 235));

                    if ui.add(create_btn).clicked() {
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
                                self.refresh_selected_project_metadata(false);
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

                    if ui.button("Cancel").clicked() {
                        self.show_new_env_modal = false;
                    }
                });
            });
    }

    fn render_all_deployments_overview(&mut self, ui: &mut egui::Ui) {
        ui.heading(
            RichText::new("All Deployments")
                .size(22.0)
                .strong()
                .color(Color32::WHITE),
        );
        ui.label(
            RichText::new("Overview of deployments and active versions across all projects.")
                .color(Color32::from_rgb(145, 155, 175)),
        );
        ui.add_space(12.0);

        ui.columns(self.storage.projects.len().max(1), |cols| {
            for (idx, project) in self.storage.projects.iter().enumerate() {
                egui::Frame::new()
                    .fill(Color32::from_rgb(19, 23, 32))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(36, 42, 56)))
                    .corner_radius(8.0)
                    .inner_margin(egui::Margin::symmetric(14, 12))
                    .show(&mut cols[idx], |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            RichText::new(&project.name)
                                .size(15.5)
                                .strong()
                                .color(Color32::WHITE),
                        );
                        ui.label(
                            RichText::new(&project.local_path)
                                .size(12.0)
                                .monospace()
                                .color(Color32::from_rgb(130, 140, 160)),
                        );
                        ui.add_space(8.0);
                        ui.horizontal_wrapped(|ui| {
                            for env in &project.environments {
                                let badge = env.badge_label();
                                let ver_label = self
                                    .storage
                                    .last_succeeded_deployment(&project.id, &env.key)
                                    .map(|d| format!("{badge}: v{}", d.version))
                                    .unwrap_or_else(|| format!("{badge}: —"));
                                render_badge(ui, &ver_label, env_badge_color(&env.key));
                            }
                        });
                    });
            }
        });

        ui.add_space(14.0);
        self.render_history_and_logs_view(ui, None);
    }

    fn render_project_workspace(&mut self, ui: &mut egui::Ui, project_id: &str) {
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

        // Ensure selected_env_key is valid for this project
        if self.storage.projects[proj_idx]
            .get_env(&self.selected_env_key)
            .is_none()
        {
            if let Some(first) = self.storage.projects[proj_idx].environments.first() {
                self.selected_env_key = first.key.clone();
            }
        }

        let proj_name = self.storage.projects[proj_idx].name.clone();
        let proj_desc = self.storage.projects[proj_idx].description.clone();
        let proj_path = self.storage.projects[proj_idx].local_path.clone();
        let path_exists = PathBuf::from(&proj_path).exists();

        // 1. Clean Project Header
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.heading(
                        RichText::new(&proj_name)
                            .size(22.0)
                            .strong()
                            .color(Color32::WHITE),
                    );
                    if path_exists {
                        render_badge(ui, "Ready", Color32::from_rgb(20, 105, 65));
                    } else {
                        render_badge(ui, "Path Missing", Color32::from_rgb(145, 38, 45));
                    }
                });
                ui.label(
                    RichText::new(format!("{}  •  {}", proj_desc, proj_path))
                        .size(12.5)
                        .color(Color32::from_rgb(145, 155, 175)),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("↻ Refresh").clicked() {
                    self.refresh_selected_project_metadata(false);
                }
                if let Some(pkg_v) = &self.cached_pkg_version {
                    render_badge(
                        ui,
                        &format!("package.json v{pkg_v}"),
                        Color32::from_rgb(40, 60, 95),
                    );
                }
                if let Some(commit) = &self.cached_git_meta.commit_short {
                    let branch = self.cached_git_meta.branch.as_deref().unwrap_or("HEAD");
                    render_badge(
                        ui,
                        &format!("⎇ {branch} @ {commit}"),
                        Color32::from_rgb(52, 42, 90),
                    );
                }
            });
        });

        ui.add_space(10.0);

        // 2. Dynamic Environment Selector Cards + "+ Add Environment" Button
        let env_summaries: Vec<(String, String, String)> = self.storage.projects[proj_idx]
            .environments
            .iter()
            .map(|e| (e.key.clone(), e.name.clone(), e.badge_label()))
            .collect();

        let mut new_selected_env_key = None;
        ui.horizontal_wrapped(|ui| {
            for (env_key, env_name, badge_lbl) in &env_summaries {
                let is_active = &self.selected_env_key == env_key;
                let last_ok = self.storage.last_succeeded_deployment(project_id, env_key);
                let latest_any = self.storage.latest_deployment(project_id, env_key);

                let fill = if is_active {
                    Color32::from_rgb(25, 34, 52)
                } else {
                    Color32::from_rgb(18, 21, 29)
                };
                let stroke = if is_active {
                    Stroke::new(1.5_f32, env_badge_color(env_key))
                } else {
                    Stroke::new(1.0_f32, Color32::from_rgb(34, 40, 54))
                };

                let resp = egui::Frame::new()
                    .fill(fill)
                    .stroke(stroke)
                    .corner_radius(8.0)
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.set_min_width(200.0);
                        ui.horizontal(|ui| {
                            render_badge(ui, badge_lbl, env_badge_color(env_key));
                            ui.label(
                                RichText::new(env_name)
                                    .size(13.5)
                                    .strong()
                                    .color(Color32::WHITE),
                            );
                            if let Some(latest) = latest_any {
                                render_status_badge(ui, latest.status);
                            }
                        });
                        ui.add_space(2.0);
                        let active_ver_text = match last_ok {
                            Some(ok_dep) => format!("Active: v{}", ok_dep.version),
                            None => "Active: Not deployed".to_string(),
                        };
                        ui.label(
                            RichText::new(active_ver_text).size(12.0).color(
                                if last_ok.is_some() {
                                    Color32::from_rgb(95, 220, 150)
                                } else {
                                    Color32::from_rgb(130, 140, 160)
                                },
                            ),
                        );
                    })
                    .response
                    .interact(egui::Sense::click());

                if resp.clicked() {
                    new_selected_env_key = Some(env_key.clone());
                }
            }

            let add_env_btn = egui::Button::new(
                RichText::new("+ Add Environment")
                    .size(13.0)
                    .strong()
                    .color(Color32::from_rgb(190, 210, 245)),
            )
            .fill(Color32::from_rgb(22, 28, 42))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(55, 85, 145)))
            .min_size(Vec2::new(145.0, 52.0));

            if ui
                .add(add_env_btn)
                .on_hover_text("Add a new custom environment (e.g. UAT, QA, Canary, Sandbox)")
                .clicked()
            {
                self.new_env_clone_from = self.selected_env_key.clone();
                self.show_new_env_modal = true;
            }
        });

        if let Some(env_key) = new_selected_env_key {
            self.selected_env_key = env_key;
            self.refresh_selected_project_metadata(false);
        }

        ui.add_space(10.0);

        // 3. Clean, Full-Width Deploy Action Bar
        let active_env_name = self.storage.projects[proj_idx]
            .get_env(&self.selected_env_key)
            .map(|e| e.name.clone())
            .unwrap_or_else(|| self.selected_env_key.clone());
        self.render_deploy_action_bar(ui, &active_env_name);

        ui.add_space(12.0);

        // 4. Clean Section Tabs (Pipeline Steps | History & Logs | Server & SSH Settings)
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
            .filter_deployments(Some(project_id), None, None)
            .len();

        ui.horizontal(|ui| {
            for (tab, label) in [
                (
                    WorkspaceTab::PipelineSteps,
                    format!("🛠 Pipeline Steps ({enabled_count}/{total_steps})"),
                ),
                (
                    WorkspaceTab::HistoryAndLogs,
                    format!("📜 Deployments & Logs ({history_count})"),
                ),
                (
                    WorkspaceTab::ServerSettings,
                    "⚙ Environment & SSH Settings".to_string(),
                ),
            ] {
                let is_active = self.active_tab == tab;
                let btn = egui::Button::new(
                    RichText::new(label).size(13.5).strong().color(if is_active {
                        Color32::WHITE
                    } else {
                        Color32::from_rgb(165, 175, 195)
                    }),
                )
                .fill(if is_active {
                    Color32::from_rgb(34, 45, 68)
                } else {
                    Color32::from_rgb(19, 23, 32)
                })
                .stroke(if is_active {
                    Stroke::new(1.0_f32, Color32::from_rgb(75, 130, 235))
                } else {
                    Stroke::new(1.0_f32, Color32::from_rgb(34, 40, 54))
                })
                .min_size(Vec2::new(180.0, 34.0));

                if ui.add(btn).clicked() {
                    self.active_tab = tab;
                }
            }
        });

        ui.add_space(8.0);

        // 5. Render Active Full-Width Tab Content
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
    }

    fn render_deploy_action_bar(&mut self, ui: &mut egui::Ui, env_display_name: &str) {
        let mut trigger_deploy = false;

        egui::Frame::new()
            .fill(Color32::from_rgb(19, 23, 33))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(40, 48, 66)))
            .corner_radius(8.0)
            .inner_margin(egui::Margin::symmetric(14, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Version:")
                            .size(13.5)
                            .strong()
                            .color(Color32::WHITE),
                    );
                    ui.add_sized(
                        [115.0, 28.0],
                        egui::TextEdit::singleline(&mut self.deploy_version_input)
                            .font(egui::TextStyle::Monospace),
                    );

                    if ui.button("+Patch").clicked() {
                        self.deploy_version_input =
                            bump_semver(&self.deploy_version_input, VersionBump::Patch);
                    }
                    if ui.button("+Minor").clicked() {
                        self.deploy_version_input =
                            bump_semver(&self.deploy_version_input, VersionBump::Minor);
                    }
                    if ui.button("+Major").clicked() {
                        self.deploy_version_input =
                            bump_semver(&self.deploy_version_input, VersionBump::Major);
                    }

                    ui.separator();

                    ui.checkbox(&mut self.bump_package_json, "Bump package.json");
                    ui.checkbox(&mut self.create_git_tag, "Git Tag");

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let is_busy = self.active_deployment_id.is_some();
                        let label = if is_busy {
                            "⏳ Deploying...".to_string()
                        } else {
                            format!(
                                "▶ Deploy to {} (v{})",
                                env_display_name,
                                self.deploy_version_input.trim()
                            )
                        };
                        let btn = egui::Button::new(
                            RichText::new(label)
                                .size(14.0)
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(if is_busy {
                            Color32::from_rgb(65, 70, 85)
                        } else {
                            Color32::from_rgb(37, 99, 235)
                        })
                        .min_size(Vec2::new(210.0, 34.0));

                        if ui.add_enabled(!is_busy, btn).clicked() {
                            trigger_deploy = true;
                        }
                    });
                });
            });

        if trigger_deploy {
            self.trigger_current_environment_deploy();
        }
    }

    fn render_pipeline_steps_tab(&mut self, ui: &mut egui::Ui, proj_idx: usize) {
        let env_key = self.selected_env_key.clone();
        let mut config_changed = false;

        egui::Frame::new()
            .fill(Color32::from_rgb(18, 22, 31))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(36, 42, 56)))
            .corner_radius(8.0)
            .inner_margin(egui::Margin::symmetric(16, 14))
            .show(ui, |ui| {
                let project = &mut self.storage.projects[proj_idx];
                let Some(env_cfg) = project.get_env_mut(&env_key) else {
                    return;
                };

                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(format!("{} Pipeline Steps", env_cfg.name))
                                .size(15.5)
                                .strong()
                                .color(Color32::WHITE),
                        );
                        ui.label(
                            RichText::new(
                                "Check/uncheck steps to customize what runs. Placeholders: {remote_dir}, {remote_user}, {app_name}, {http_proxy}, {version}",
                            )
                            .size(12.0)
                            .color(Color32::from_rgb(140, 150, 170)),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("+ Add Remote SSH Step").clicked() {
                            env_cfg.steps.push(PipelineStep::new(
                                "Custom Remote Step",
                                StepKind::RemoteCommand {
                                    command: "cd {remote_dir} && ls -la".to_string(),
                                },
                            ));
                            config_changed = true;
                        }
                        if ui.button("+ Add Local Step").clicked() {
                            env_cfg.steps.push(PipelineStep::new(
                                "Custom Local Step",
                                StepKind::LocalCommand {
                                    command: "npm run lint".to_string(),
                                    allow_failure: false,
                                },
                            ));
                            config_changed = true;
                        }
                    });
                });

                ui.add_space(10.0);

                ScrollArea::vertical()
                    .id_salt("pipeline_steps_fullwidth_scroll")
                    .show(ui, |ui| {
                        let mut move_up_idx = None;
                        let mut move_down_idx = None;
                        let mut delete_idx = None;
                        let total_steps = env_cfg.steps.len();

                        for (idx, step) in env_cfg.steps.iter_mut().enumerate() {
                            let row_bg = if step.enabled {
                                Color32::from_rgb(23, 28, 40)
                            } else {
                                Color32::from_rgb(15, 18, 25)
                            };

                            egui::Frame::new()
                                .fill(row_bg)
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(38, 45, 62)))
                                .corner_radius(7.0)
                                .inner_margin(egui::Margin::symmetric(12, 8))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        if ui.checkbox(&mut step.enabled, "").changed() {
                                            config_changed = true;
                                        }
                                        ui.label(
                                            RichText::new(format!("{:02}", idx + 1))
                                                .monospace()
                                                .color(Color32::from_rgb(125, 140, 165)),
                                        );

                                        let (type_label, type_color) = match &step.kind {
                                            StepKind::LocalCommand { .. } => {
                                                ("Local Cmd", Color32::from_rgb(40, 80, 130))
                                            }
                                            StepKind::VerifyLocalPath { .. } => {
                                                ("Verify Path", Color32::from_rgb(68, 72, 110))
                                            }
                                            StepKind::TransferFiles { .. } => {
                                                ("SCP Upload", Color32::from_rgb(98, 52, 128))
                                            }
                                            StepKind::RemoteCommand { .. } => {
                                                ("Remote SSH", Color32::from_rgb(24, 98, 82))
                                            }
                                        };
                                        ui.add_sized([92.0, 22.0], |ui: &mut egui::Ui| {
                                            render_badge(ui, type_label, type_color);
                                            ui.allocate_response(Vec2::ZERO, egui::Sense::hover())
                                        });

                                        if ui
                                            .add_sized(
                                                [210.0, 26.0],
                                                egui::TextEdit::singleline(&mut step.name),
                                            )
                                            .changed()
                                        {
                                            config_changed = true;
                                        }

                                        let reserved_right = match &step.kind {
                                            StepKind::LocalCommand { .. }
                                            | StepKind::TransferFiles { .. } => 195.0,
                                            _ => 105.0,
                                        };
                                        let cmd_width =
                                            (ui.available_width() - reserved_right).max(180.0);

                                        match &mut step.kind {
                                            StepKind::LocalCommand {
                                                command,
                                                allow_failure,
                                            } => {
                                                if ui
                                                    .add_sized(
                                                        [cmd_width, 26.0],
                                                        egui::TextEdit::singleline(command)
                                                            .font(egui::TextStyle::Monospace),
                                                    )
                                                    .changed()
                                                {
                                                    config_changed = true;
                                                }
                                                if ui
                                                    .checkbox(allow_failure, "Allow fail")
                                                    .changed()
                                                {
                                                    config_changed = true;
                                                }
                                            }
                                            StepKind::VerifyLocalPath { path } => {
                                                if ui
                                                    .add_sized(
                                                        [cmd_width, 26.0],
                                                        egui::TextEdit::singleline(path)
                                                            .font(egui::TextStyle::Monospace),
                                                    )
                                                    .changed()
                                                {
                                                    config_changed = true;
                                                }
                                            }
                                            StepKind::TransferFiles {
                                                sources,
                                                stage_env_file,
                                            } => {
                                                let mut joined = sources.join(", ");
                                                if ui
                                                    .add_sized(
                                                        [cmd_width, 26.0],
                                                        egui::TextEdit::singleline(&mut joined)
                                                            .font(egui::TextStyle::Monospace),
                                                    )
                                                    .changed()
                                                {
                                                    *sources = joined
                                                        .split(',')
                                                        .map(|s| s.trim().to_string())
                                                        .filter(|s| !s.is_empty())
                                                        .collect();
                                                    config_changed = true;
                                                }
                                                if ui
                                                    .checkbox(stage_env_file, "Stage .env")
                                                    .changed()
                                                {
                                                    config_changed = true;
                                                }
                                            }
                                            StepKind::RemoteCommand { command } => {
                                                if ui
                                                    .add_sized(
                                                        [cmd_width, 26.0],
                                                        egui::TextEdit::singleline(command)
                                                            .font(egui::TextStyle::Monospace),
                                                    )
                                                    .changed()
                                                {
                                                    config_changed = true;
                                                }
                                            }
                                        }

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui
                                                    .small_button("✕")
                                                    .on_hover_text("Remove step")
                                                    .clicked()
                                                {
                                                    delete_idx = Some(idx);
                                                }
                                                if idx + 1 < total_steps
                                                    && ui
                                                        .small_button("↓")
                                                        .on_hover_text("Move down")
                                                        .clicked()
                                                {
                                                    move_down_idx = Some(idx);
                                                }
                                                if idx > 0
                                                    && ui
                                                        .small_button("↑")
                                                        .on_hover_text("Move up")
                                                        .clicked()
                                                {
                                                    move_up_idx = Some(idx);
                                                }
                                            },
                                        );
                                    });
                                });
                            ui.add_space(4.0);
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
                    });
            });

        if config_changed {
            let _ = self.storage.save();
        }
    }

    fn render_server_settings_tab(&mut self, ui: &mut egui::Ui, proj_idx: usize) {
        let env_key = self.selected_env_key.clone();
        let mut config_changed = false;
        let mut delete_current_env = false;
        let field_width = 480.0;

        let env_count = self.storage.projects[proj_idx].environments.len();
        let current_env_name = self.storage.projects[proj_idx]
            .get_env(&env_key)
            .map(|e| e.name.clone())
            .unwrap_or_else(|| env_key.clone());

        egui::Frame::new()
            .fill(Color32::from_rgb(18, 22, 31))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(36, 42, 56)))
            .corner_radius(8.0)
            .inner_margin(egui::Margin::symmetric(20, 16))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "{current_env_name} — Server & SSH Configuration"
                        ))
                        .size(16.0)
                        .strong()
                        .color(Color32::WHITE),
                    );
                    if self.cached_vault_found {
                        render_badge(
                            ui,
                            "OS Credential Vault Detected",
                            Color32::from_rgb(20, 105, 65),
                        );
                    } else {
                        render_badge(
                            ui,
                            "Using SSH Key / Manual Password",
                            Color32::from_rgb(95, 75, 25),
                        );
                    }

                    if env_count > 1 {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let del_btn = egui::Button::new(
                                RichText::new("🗑 Delete Environment")
                                    .color(Color32::from_rgb(245, 130, 130)),
                            );
                            if ui.add(del_btn).clicked() {
                                delete_current_env = true;
                            }
                        });
                    }
                });
                ui.add_space(12.0);

                let project = &mut self.storage.projects[proj_idx];
                egui::Grid::new("clean_server_settings_grid")
                    .num_columns(2)
                    .spacing([20.0, 12.0])
                    .show(ui, |ui| {
                        ui.label("Local Project Directory:");
                        if ui
                            .add_sized(
                                [field_width, 28.0],
                                egui::TextEdit::singleline(&mut project.local_path)
                                    .font(egui::TextStyle::Monospace),
                            )
                            .changed()
                        {
                            config_changed = true;
                        }
                        ui.end_row();

                        if let Some(env_cfg) = project.get_env_mut(&env_key) {
                            ui.label("Environment Display Name:");
                            if ui
                                .add_sized(
                                    [field_width, 28.0],
                                    egui::TextEdit::singleline(&mut env_cfg.name),
                                )
                                .changed()
                            {
                                config_changed = true;
                            }
                            ui.end_row();

                            ui.label("Application Name (PM2 / Dir):");
                            if ui
                                .add_sized(
                                    [field_width, 28.0],
                                    egui::TextEdit::singleline(&mut env_cfg.app_name)
                                        .font(egui::TextStyle::Monospace),
                                )
                                .changed()
                            {
                                config_changed = true;
                            }
                            ui.end_row();

                            ui.label("Target Servers (comma-separated):");
                            let mut servers_joined = env_cfg.servers.join(", ");
                            if ui
                                .add_sized(
                                    [field_width, 28.0],
                                    egui::TextEdit::singleline(&mut servers_joined)
                                        .hint_text("10.62.70.80, 10.62.70.81")
                                        .font(egui::TextStyle::Monospace),
                                )
                                .changed()
                            {
                                env_cfg.servers = servers_joined
                                    .split(',')
                                    .map(|s| s.trim().to_string())
                                    .filter(|s| !s.is_empty())
                                    .collect();
                                config_changed = true;
                            }
                            ui.end_row();

                            ui.label("Remote Directory Path:");
                            if ui
                                .add_sized(
                                    [field_width, 28.0],
                                    egui::TextEdit::singleline(&mut env_cfg.remote_dir)
                                        .font(egui::TextStyle::Monospace),
                                )
                                .changed()
                            {
                                config_changed = true;
                            }
                            ui.end_row();

                            ui.label("Remote SSH Username:");
                            if ui
                                .add_sized(
                                    [field_width, 28.0],
                                    egui::TextEdit::singleline(&mut env_cfg.remote_user),
                                )
                                .changed()
                            {
                                config_changed = true;
                            }
                            ui.end_row();

                            ui.label("OS Credential Vault Target:");
                            if ui
                                .add_sized(
                                    [field_width, 28.0],
                                    egui::TextEdit::singleline(&mut env_cfg.vault_target)
                                        .hint_text("indyserver"),
                                )
                                .changed()
                            {
                                config_changed = true;
                            }
                            ui.end_row();

                            ui.label("SSH Password Override:");
                            ui.add_sized(
                                [field_width, 28.0],
                                egui::TextEdit::singleline(&mut self.manual_ssh_password)
                                    .password(true)
                                    .hint_text(
                                        "Optional — leave blank to use OS Vault ('indyserver') or SSH Key",
                                    ),
                            );
                            ui.end_row();

                            ui.label("Environment File Source (.env):");
                            let mut env_src = env_cfg.env_source.clone().unwrap_or_default();
                            if ui
                                .add_sized(
                                    [field_width, 28.0],
                                    egui::TextEdit::singleline(&mut env_src)
                                        .hint_text("Optional, e.g. ./.env.dev or ./.env.prod")
                                        .font(egui::TextStyle::Monospace),
                                )
                                .changed()
                            {
                                env_cfg.env_source = if env_src.trim().is_empty() {
                                    None
                                } else {
                                    Some(env_src.trim().to_string())
                                };
                                config_changed = true;
                            }
                            ui.end_row();

                            ui.label("Outbound HTTP Proxy:");
                            let mut proxy = env_cfg.http_proxy.clone().unwrap_or_default();
                            if ui
                                .add_sized(
                                    [field_width, 28.0],
                                    egui::TextEdit::singleline(&mut proxy)
                                        .hint_text("Optional, e.g. http://10.54.18.42:8080")
                                        .font(egui::TextStyle::Monospace),
                                )
                                .changed()
                            {
                                env_cfg.http_proxy = if proxy.trim().is_empty() {
                                    None
                                } else {
                                    Some(proxy.trim().to_string())
                                };
                                config_changed = true;
                            }
                            ui.end_row();
                        }
                    });
            });

        if delete_current_env {
            if let Err(err) = self.storage.projects[proj_idx].remove_environment(&env_key) {
                self.status_banner = Some((err, false));
            } else {
                let _ = self.storage.save();
                if let Some(first) = self.storage.projects[proj_idx].environments.first() {
                    self.selected_env_key = first.key.clone();
                }
                self.refresh_selected_project_metadata(false);
                self.status_banner = Some((
                    format!("Removed environment '{current_env_name}'."),
                    true,
                ));
            }
        } else if config_changed {
            let _ = self.storage.save();
            self.refresh_selected_project_metadata(false);
        }
    }

    fn render_history_and_logs_view(&mut self, ui: &mut egui::Ui, project_filter: Option<&str>) {
        // Collect available environment filter options dynamically
        let mut filter_options: Vec<(String, Option<String>)> =
            vec![("All Environments".to_string(), None)];
        if let Some(pid) = project_filter {
            if let Some(proj) = self.storage.projects.iter().find(|p| p.id == pid) {
                for env in &proj.environments {
                    filter_options.push((env.name.clone(), Some(env.key.clone())));
                }
            }
        } else {
            for (k, label) in [
                ("dev", "Development"),
                ("staging", "Staging"),
                ("production", "Production"),
            ] {
                filter_options.push((label.to_string(), Some(k.to_string())));
            }
        }

        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Deployment History")
                    .size(15.5)
                    .strong()
                    .color(Color32::WHITE),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                for (label, filter_val) in filter_options.into_iter().rev() {
                    if ui
                        .selectable_label(self.history_env_filter == filter_val, label)
                        .clicked()
                    {
                        self.history_env_filter = filter_val;
                    }
                }
            });
        });
        ui.add_space(6.0);

        let filtered: Vec<DeploymentRecord> = self
            .storage
            .filter_deployments(project_filter, self.history_env_filter.as_deref(), None)
            .into_iter()
            .cloned()
            .collect();

        let mut redeploy_target: Option<DeploymentRecord> = None;

        egui::Frame::new()
            .fill(Color32::from_rgb(17, 20, 28))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(35, 42, 56)))
            .corner_radius(8.0)
            .inner_margin(egui::Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ScrollArea::vertical()
                    .id_salt("deployment_history_scroll")
                    .max_height(185.0)
                    .show(ui, |ui| {
                        if filtered.is_empty() {
                            ui.vertical_centered(|ui| {
                                ui.add_space(20.0);
                                ui.label(
                                    RichText::new("No deployments recorded yet.")
                                        .color(Color32::from_rgb(135, 145, 165)),
                                );
                                ui.add_space(20.0);
                            });
                        }

                        for dep in &filtered {
                            let is_selected =
                                self.selected_deployment_id.as_deref() == Some(&dep.id);
                            let card_fill = if is_selected {
                                Color32::from_rgb(27, 35, 52)
                            } else {
                                Color32::from_rgb(21, 25, 35)
                            };
                            let card_stroke = if is_selected {
                                Stroke::new(1.0_f32, Color32::from_rgb(75, 130, 235))
                            } else {
                                Stroke::new(1.0_f32, Color32::from_rgb(34, 40, 54))
                            };

                            egui::Frame::new()
                                .fill(card_fill)
                                .stroke(card_stroke)
                                .corner_radius(6.0)
                                .inner_margin(egui::Margin::symmetric(12, 8))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        render_status_badge(ui, dep.status);
                                        render_badge(
                                            ui,
                                            &format_env_badge(&dep.environment),
                                            env_badge_color(&dep.environment),
                                        );
                                        render_badge(
                                            ui,
                                            &format!("v{}", dep.version),
                                            Color32::from_rgb(44, 54, 76),
                                        );
                                        ui.label(
                                            RichText::new(&dep.project_name)
                                                .size(14.0)
                                                .strong()
                                                .color(Color32::WHITE),
                                        );

                                        let dur_str = dep
                                            .duration_ms
                                            .map(|ms| format!("{:.1}s", ms as f64 / 1000.0))
                                            .unwrap_or_else(|| "running...".to_string());
                                        let git_str = match (&dep.git_branch, &dep.git_commit) {
                                            (Some(b), Some(c)) => format!("•  {b}@{c}  "),
                                            _ => String::new(),
                                        };
                                        ui.label(
                                            RichText::new(format!(
                                                "•  {}  •  {}  {}{}",
                                                dep.started_at.format("%Y-%m-%d %H:%M:%S"),
                                                dur_str,
                                                git_str,
                                                dep.servers.join(", ")
                                            ))
                                            .size(12.0)
                                            .color(Color32::from_rgb(145, 155, 175)),
                                        );

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                let can_redeploy =
                                                    self.active_deployment_id.is_none();
                                                if ui
                                                    .add_enabled(
                                                        can_redeploy,
                                                        egui::Button::new("↻ Redeploy"),
                                                    )
                                                    .on_hover_text(
                                                        "Re-run this deployment version and pipeline snapshot",
                                                    )
                                                    .clicked()
                                                {
                                                    redeploy_target = Some(dep.clone());
                                                }
                                                if ui
                                                    .button(if is_selected {
                                                        "Viewing Logs"
                                                    } else {
                                                        "View Logs"
                                                    })
                                                    .clicked()
                                                {
                                                    self.selected_deployment_id =
                                                        Some(dep.id.clone());
                                                }
                                            },
                                        );
                                    });
                                });
                            ui.add_space(4.0);
                        }
                    });
            });

        if let Some(target) = redeploy_target {
            self.trigger_redeploy_from_record(&target);
        }

        ui.add_space(10.0);

        let selected_dep = self
            .selected_deployment_id
            .as_ref()
            .and_then(|id| self.storage.deployments.iter().find(|d| &d.id == id))
            .or_else(|| filtered.first());

        egui::Frame::new()
            .fill(Color32::from_rgb(10, 12, 18))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(36, 42, 56)))
            .corner_radius(8.0)
            .inner_margin(egui::Margin::symmetric(14, 12))
            .show(ui, |ui| {
                ui.set_min_size(Vec2::new(ui.available_width(), ui.available_height()));
                if let Some(dep) = selected_dep {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "Console Output — {} ({}) v{}",
                                dep.project_name,
                                format_env_badge(&dep.environment),
                                dep.version
                            ))
                            .size(14.5)
                            .strong()
                            .color(Color32::WHITE),
                        );
                        render_status_badge(ui, dep.status);
                    });
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    ScrollArea::vertical()
                        .id_salt("step_logs_terminal_scroll")
                        .show(ui, |ui| {
                            if dep.step_logs.is_empty() {
                                ui.label(
                                    RichText::new("Initializing deployment pipeline...")
                                        .monospace()
                                        .color(Color32::from_rgb(170, 180, 200)),
                                );
                            }
                            for (i, step_log) in dep.step_logs.iter().enumerate() {
                                let (icon, color) = match step_log.status {
                                    DeploymentStatus::Succeeded => {
                                        ("✔", Color32::from_rgb(75, 215, 135))
                                    }
                                    DeploymentStatus::Failed => {
                                        ("✖", Color32::from_rgb(245, 95, 95))
                                    }
                                    DeploymentStatus::Running => {
                                        ("⏳", Color32::from_rgb(250, 200, 75))
                                    }
                                    DeploymentStatus::Cancelled => {
                                        ("⏹", Color32::from_rgb(150, 160, 175))
                                    }
                                };

                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(format!(
                                            "{icon} [{}/{}] {}",
                                            i + 1,
                                            dep.step_logs.len(),
                                            step_log.step_name
                                        ))
                                        .strong()
                                        .color(color),
                                    );
                                    ui.label(
                                        RichText::new(format!("({} ms)", step_log.duration_ms))
                                            .small()
                                            .monospace()
                                            .color(Color32::from_rgb(130, 140, 160)),
                                    );
                                });
                                ui.label(
                                    RichText::new(format!("  $ {}", step_log.command_summary))
                                        .small()
                                        .monospace()
                                        .color(Color32::from_rgb(135, 165, 215)),
                                );
                                if !step_log.output.is_empty() {
                                    egui::Frame::new()
                                        .fill(Color32::from_rgb(7, 9, 13))
                                        .corner_radius(5.0)
                                        .inner_margin(egui::Margin::symmetric(10, 8))
                                        .show(ui, |ui| {
                                            ui.set_width(ui.available_width());
                                            ui.label(
                                                RichText::new(&step_log.output)
                                                    .small()
                                                    .monospace()
                                                    .color(Color32::from_rgb(205, 214, 230)),
                                            );
                                        });
                                }
                                ui.add_space(6.0);
                            }
                        });
                } else {
                    ui.label(
                        RichText::new("Select a deployment above to view step-by-step logs.")
                            .color(Color32::from_rgb(140, 150, 170)),
                    );
                }
            });
    }
}

fn render_badge(ui: &mut egui::Ui, text: &str, bg: Color32) {
    egui::Frame::new()
        .fill(bg)
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(7, 3))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.5).strong().color(Color32::WHITE));
        });
}

fn render_status_badge(ui: &mut egui::Ui, status: DeploymentStatus) {
    let bg = match status {
        DeploymentStatus::Succeeded => Color32::from_rgb(20, 115, 65),
        DeploymentStatus::Failed => Color32::from_rgb(155, 35, 42),
        DeploymentStatus::Running => Color32::from_rgb(165, 110, 20),
        DeploymentStatus::Cancelled => Color32::from_rgb(75, 80, 95),
    };
    render_badge(ui, status.label(), bg);
}

fn env_badge_color(env_key: &str) -> Color32 {
    match env_key.to_lowercase().as_str() {
        "dev" | "development" => Color32::from_rgb(35, 95, 165),
        "staging" | "stage" | "stg" => Color32::from_rgb(145, 95, 25),
        "prod" | "production" => Color32::from_rgb(125, 38, 155),
        "uat" => Color32::from_rgb(24, 115, 105),
        "qa" | "test" => Color32::from_rgb(65, 105, 45),
        _ => Color32::from_rgb(60, 80, 125),
    }
}
