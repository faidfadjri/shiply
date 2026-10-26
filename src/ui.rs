use crate::credentials::read_os_vault_credential;
use crate::domain::{
    format_env_badge, DeploymentRecord, DeploymentStatus, EnvironmentConfig, PipelineStep, Project,
    StepKind,
};
use crate::executor::{run_deployment, DeploymentRequest, ExecutorEvent};
use crate::installer::{
    check_install_state, default_install_dir, install_or_update_self, InstallState, APP_VERSION,
};
use crate::ps1_importer::{import_project_from_path, parse_ps1_file};
use crate::storage::AppStorage;
use crate::version::{
    bump_semver, checkout_git_branch, configure_background_command, list_git_branches,
    read_git_metadata, read_package_json_version, GitMetadata, VersionBump,
};
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId,
    Layout, Margin, Pos2, Rect, RichText, ScrollArea, Sense, Shadow, Stroke, StrokeKind, TextStyle,
    Vec2,
};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

// =============================================================================
// Structured Typography Scale (Single Unified Proportional Font Family)
// =============================================================================
const FONT_XL: f32 = 20.0; // Page & Project Headers
const FONT_LG: f32 = 15.0; // Card Primary Values & Modal Titles
const FONT_MD: f32 = 13.0; // Standard UI Text, Inputs, Buttons, Table Values
const FONT_SM: f32 = 11.5; // Secondary Metadata, Subtitles, Compact Buttons
const FONT_XS: f32 = 10.5; // Section Labels, Table Headers, Pill Badges

// =============================================================================
// High-Contrast Professional SaaS Color Palette
// =============================================================================
const SIDEBAR_BG: Color32 = Color32::from_rgb(15, 23, 42); // #0f172a
const SIDEBAR_CARD_ACTIVE: Color32 = Color32::from_rgb(20, 33, 68);
const SIDEBAR_CARD_HOVER: Color32 = Color32::from_rgb(30, 41, 59); // #1e293b
const SIDEBAR_BORDER_ACTIVE: Color32 = Color32::from_rgb(59, 130, 246); // #3b82f6
const SIDEBAR_PILL_BG: Color32 = Color32::from_rgb(30, 41, 59); // #1e293b
const SIDEBAR_DIVIDER: Color32 = Color32::from_rgb(30, 41, 59); // #1e293b
const SIDEBAR_TEXT_MUTED: Color32 = Color32::from_rgb(148, 163, 184); // #94a3b8

const CANVAS_BG: Color32 = Color32::from_rgb(241, 245, 249); // #f1f5f9
const CARD_BG: Color32 = Color32::from_rgb(255, 255, 255); // #ffffff
const CARD_BORDER: Color32 = Color32::from_rgb(203, 213, 225); // #cbd5e1
const SUBTLE_BG: Color32 = Color32::from_rgb(248, 250, 252); // #f8fafc
const PILL_CONTAINER_BG: Color32 = Color32::from_rgb(226, 232, 240); // #e2e8f0

// Grey input field fill so inputs clearly contrast against white (#FFFFFF) cards
const INPUT_BG: Color32 = Color32::from_rgb(241, 245, 249); // #f1f5f9
const INPUT_BG_HOVER: Color32 = Color32::from_rgb(235, 240, 247); // #ebf0f7
const INPUT_BORDER: Color32 = Color32::from_rgb(186, 198, 214); // #bac6d6
const INPUT_BORDER_HOVER: Color32 = Color32::from_rgb(148, 163, 184); // #94a3b8

const TEXT_PRIMARY: Color32 = Color32::from_rgb(15, 23, 42); // #0f172a
const TEXT_SECONDARY: Color32 = Color32::from_rgb(51, 65, 85); // #334155
const TEXT_MUTED: Color32 = Color32::from_rgb(100, 116, 139); // #64748b
// Distinct medium-grey placeholder text
const TEXT_PLACEHOLDER: Color32 = Color32::from_rgb(142, 155, 175); // #8e9baf

const PRIMARY_BLUE: Color32 = Color32::from_rgb(37, 99, 235); // #2563eb
const PRIMARY_BLUE_HOVER: Color32 = Color32::from_rgb(29, 78, 216); // #1d4ed8
const BLUE_PILL_BG: Color32 = Color32::from_rgb(219, 234, 254); // #dbeafe
const BLUE_PILL_TEXT: Color32 = Color32::from_rgb(29, 78, 216); // #1d4ed8

const SUCCESS_PILL_BG: Color32 = Color32::from_rgb(220, 252, 231); // #dcfce7
const SUCCESS_PILL_TEXT: Color32 = Color32::from_rgb(21, 128, 61); // #15803d

const WARN_PILL_BG: Color32 = Color32::from_rgb(254, 243, 199); // #fef3c7
const WARN_PILL_TEXT: Color32 = Color32::from_rgb(180, 83, 9); // #b45309

const FAIL_PILL_BG: Color32 = Color32::from_rgb(254, 226, 226); // #fee2e2
const FAIL_PILL_TEXT: Color32 = Color32::from_rgb(185, 28, 28); // #b91c1c

const PURPLE_PILL_BG: Color32 = Color32::from_rgb(243, 232, 255); // #f3e8ff
const PURPLE_PILL_TEXT: Color32 = Color32::from_rgb(126, 34, 206); // #7e22ce

const TEAL_PILL_BG: Color32 = Color32::from_rgb(204, 251, 241); // #ccfbf1
const TEAL_PILL_TEXT: Color32 = Color32::from_rgb(15, 118, 110); // #0f766e

const TERM_HEADER_BG: Color32 = Color32::from_rgb(22, 31, 48); // #161f30
const TERM_BODY_BG: Color32 = Color32::from_rgb(11, 15, 25); // #0b0f19
const TERM_GREEN: Color32 = Color32::from_rgb(16, 185, 129); // #10b981
const TERM_RED: Color32 = Color32::from_rgb(239, 68, 68); // #ef4444
const TERM_YELLOW: Color32 = Color32::from_rgb(245, 158, 11); // #f59e0b

const ENV_CARD_HEIGHT: f32 = 96.0;

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

    // Collapsible Console Output & Code Changes state
    console_collapsed: bool,
    changes_collapsed: bool,
    collapsed_step_indices: HashSet<usize>,

    // Deployment version, branch & release controls
    deploy_version_input: String,
    selected_git_branch: String,
    bump_package_json: bool,
    create_git_tag: bool,
    manual_ssh_password: String,

    // Cached project metadata (Git + branches + package.json + OS Vault check)
    cached_project_id: Option<String>,
    cached_pkg_version: Option<String>,
    cached_git_meta: GitMetadata,
    cached_git_branches: Vec<String>,
    cached_vault_found: bool,

    // Background execution channel
    event_tx: Sender<ExecutorEvent>,
    event_rx: Receiver<ExecutorEvent>,
    active_deployment_id: Option<String>,

    // Add Project & Auto-Import .ps1 Modal state
    show_new_project_modal: bool,
    new_project_ps1_path: String,
    new_project_import_siblings: bool,
    new_project_name: String,
    new_project_path: String,
    new_project_desc: String,
    new_project_imported_envs: Vec<EnvironmentConfig>,
    new_project_import_notice: Option<String>,

    // Add Environment Modal state
    show_new_env_modal: bool,
    new_env_name: String,
    new_env_clone_from: String,
    new_env_ps1_path: String,

    // Throttled external file sync timer (Requirement #8 & #9)
    last_sync_check: Instant,

    // Status banner
    status_banner: Option<(String, bool)>,
    fonts_initialized: bool,
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
            .unwrap_or_default();
        let selection = match &initial_project_id {
            Some(id) => SidebarSelection::Project(id.clone()),
            None => SidebarSelection::AllDeployments,
        };
        let initial_selected_dep = storage.deployments.first().map(|d| d.id.clone());

        let mut app = Self {
            storage,
            selection,
            selected_env_key: initial_env_key.clone(),
            active_tab: WorkspaceTab::HistoryAndLogs,
            history_env_filter: None,
            selected_deployment_id: initial_selected_dep,
            console_collapsed: false,
            changes_collapsed: false,
            collapsed_step_indices: HashSet::new(),
            deploy_version_input: "1.0.0".to_string(),
            selected_git_branch: String::new(),
            bump_package_json: true,
            create_git_tag: false,
            manual_ssh_password: String::new(),
            cached_project_id: None,
            cached_pkg_version: None,
            cached_git_meta: GitMetadata::default(),
            cached_git_branches: Vec::new(),
            cached_vault_found: false,
            event_tx,
            event_rx,
            active_deployment_id: None,
            show_new_project_modal: false,
            new_project_ps1_path: String::new(),
            new_project_import_siblings: true,
            new_project_name: String::new(),
            new_project_path: String::new(),
            new_project_desc: String::new(),
            new_project_imported_envs: Vec::new(),
            new_project_import_notice: None,
            show_new_env_modal: false,
            new_env_name: String::new(),
            new_env_clone_from: initial_env_key,
            new_env_ps1_path: String::new(),
            last_sync_check: Instant::now(),
            status_banner: None,
            fonts_initialized: false,
        };
        app.refresh_selected_project_metadata(true);
        app
    }

    pub fn set_status_banner(&mut self, message: String, is_success: bool) {
        self.status_banner = Some((message, is_success));
    }

    /// Configures a single unified font family (`Segoe UI` on Windows) so all text shares a consistent structure.
    fn install_system_fonts(ctx: &egui::Context) {
        let mut fonts = FontDefinitions::default();

        #[cfg(windows)]
        {
            if let Ok(win_dir) = std::env::var("SystemRoot") {
                let fonts_dir = PathBuf::from(win_dir).join("Fonts");
                if let Ok(bytes) = fs::read(fonts_dir.join("segoeui.ttf")) {
                    fonts.font_data.insert(
                        "segoeui".to_owned(),
                        Arc::new(FontData::from_owned(bytes)),
                    );
                    if let Some(prop) = fonts.families.get_mut(&FontFamily::Proportional) {
                        prop.insert(0, "segoeui".to_owned());
                    }
                    if let Some(mono) = fonts.families.get_mut(&FontFamily::Monospace) {
                        mono.insert(0, "segoeui".to_owned());
                    }
                }
            }
        }

        ctx.set_fonts(fonts);
    }

    fn refresh_selected_project_metadata(&mut self, update_version_input: bool) {
        let SidebarSelection::Project(proj_id) = &self.selection else {
            return;
        };
        let Some(proj) = self.storage.projects.iter().find(|p| &p.id == proj_id) else {
            return;
        };

        if proj.get_env(&self.selected_env_key).is_none() {
            self.selected_env_key = proj
                .environments
                .first()
                .map(|e| e.key.clone())
                .unwrap_or_default();
        }

        let path = PathBuf::from(&proj.local_path);
        self.cached_project_id = Some(proj.id.clone());
        self.cached_pkg_version = read_package_json_version(&path);
        self.cached_git_meta = read_git_metadata(&path);
        self.cached_git_branches = list_git_branches(&path);

        if let Some(current_branch) = &self.cached_git_meta.branch {
            self.selected_git_branch = current_branch.clone();
            if !self.cached_git_branches.contains(current_branch) {
                self.cached_git_branches.insert(0, current_branch.clone());
            }
        } else if let Some(first_b) = self.cached_git_branches.first() {
            self.selected_git_branch = first_b.clone();
        } else {
            self.selected_git_branch.clear();
        }

        if let Some(env_cfg) = proj.get_env(&self.selected_env_key) {
            self.cached_vault_found = read_os_vault_credential(&env_cfg.vault_target).is_some();
        } else {
            self.cached_vault_found = false;
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
                    let action_prefix = if record.is_rollback {
                        "Rollback"
                    } else {
                        "Deployment"
                    };
                    let msg = format!(
                        "{action_prefix} of {} ({}) v{} {}",
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

        // Throttled check (once every 2s) for files pasted into the app directory
        if self.active_deployment_id.is_none()
            && self.last_sync_check.elapsed() >= Duration::from_secs(2)
        {
            self.last_sync_check = Instant::now();
            if let Some(sync_msg) = self.storage.sync_from_app_directory() {
                if matches!(self.selection, SidebarSelection::AllDeployments) {
                    if let Some(first_proj) = self.storage.projects.first() {
                        self.selection = SidebarSelection::Project(first_proj.id.clone());
                    }
                }
                self.refresh_selected_project_metadata(true);
                self.status_banner = Some((sync_msg, true));
            }
        }

        if received || self.active_deployment_id.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
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
        self.console_collapsed = false;
        self.collapsed_step_indices.clear();
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

        let prev_commit = self
            .storage
            .last_succeeded_deployment(&proj.id, &env_cfg.key)
            .and_then(|d| d.git_commit.clone());

        let target_branch = if self.selected_git_branch.trim().is_empty() {
            None
        } else {
            Some(self.selected_git_branch.trim().to_string())
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
            target_git_branch: target_branch,
            rollback_commit: None,
            previous_commit: prev_commit,
            is_rollback: false,
        };

        let _ = self.storage.save();
        self.start_deployment(req);
    }

    fn trigger_redeploy_from_record(&mut self, record: &DeploymentRecord, is_rollback: bool) {
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
            self.status_banner = Some((
                format!("Environment '{}' not found.", record.environment),
                false,
            ));
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

        let prev_commit = self
            .storage
            .last_succeeded_deployment(&proj.id, &env_cfg.key)
            .and_then(|d| d.git_commit.clone());

        let req = DeploymentRequest {
            deployment_id: Uuid::new_v4().to_string(),
            project_id: proj.id.clone(),
            project_name: proj.name.clone(),
            local_path: PathBuf::from(&proj.local_path),
            env_config: env_cfg,
            version: record.version.clone(),
            bump_package_json: true,
            create_git_tag_on_success: false,
            manual_password: manual_pw,
            skip_ssh_preflight: false,
            target_git_branch: record.git_branch.clone(),
            rollback_commit: if is_rollback {
                record.git_commit.clone()
            } else {
                None
            },
            previous_commit: prev_commit,
            is_rollback,
        };

        self.deploy_version_input = record.version.clone();
        self.start_deployment(req);
    }

    fn apply_ps1_auto_import_to_modal(&mut self, raw_path: &str) {
        let path = PathBuf::from(raw_path.trim().trim_matches('"'));
        match import_project_from_path(&path, self.new_project_import_siblings) {
            Ok(imported) => {
                self.new_project_name = imported.name;
                self.new_project_path = imported.local_path;
                self.new_project_desc = imported.description;
                let env_names: Vec<String> = imported
                    .environments
                    .iter()
                    .map(|e| format!("{} ({})", e.name, e.app_name))
                    .collect();
                self.new_project_imported_envs = imported.environments;
                self.new_project_import_notice = Some(format!(
                    "Auto-imported {} environment(s): {}",
                    env_names.len(),
                    env_names.join(", ")
                ));
            }
            Err(err) => {
                self.new_project_import_notice = Some(format!("Import error: {err}"));
            }
        }
    }

    fn configure_light_workspace_style(ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = Vec2::new(10.0, 8.0);
        style.spacing.button_padding = Vec2::new(12.0, 6.0);
        style.spacing.indent = 16.0;

        // Structured Typography Hierarchy — all using the same Proportional font family
        let mut text_styles = BTreeMap::new();
        text_styles.insert(
            TextStyle::Heading,
            FontId::new(FONT_XL, FontFamily::Proportional),
        );
        text_styles.insert(
            TextStyle::Body,
            FontId::new(FONT_MD, FontFamily::Proportional),
        );
        text_styles.insert(
            TextStyle::Button,
            FontId::new(FONT_MD, FontFamily::Proportional),
        );
        text_styles.insert(
            TextStyle::Small,
            FontId::new(FONT_SM, FontFamily::Proportional),
        );
        text_styles.insert(
            TextStyle::Monospace,
            FontId::new(FONT_MD, FontFamily::Proportional),
        );
        style.text_styles = text_styles;

        let mut visuals = egui::Visuals::light();
        visuals.panel_fill = CANVAS_BG;
        visuals.window_fill = CARD_BG;
        visuals.faint_bg_color = SUBTLE_BG;
        // Grey field fill (#F1F5F9) so input boxes stand out clearly on white (#FFFFFF) cards
        visuals.extreme_bg_color = INPUT_BG;
        visuals.override_text_color = Some(TEXT_PRIMARY);

        // Focused input border & text selection highlight:
        // In egui, TextEdit draws its focused border using `visuals.selection.stroke`!
        visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(37, 99, 235, 45);
        visuals.selection.stroke = Stroke::new(1.5_f32, PRIMARY_BLUE);

        visuals.widgets.noninteractive.bg_fill = CARD_BG;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, CARD_BORDER);
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_SECONDARY);

        visuals.widgets.inactive.bg_fill = INPUT_BG;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, INPUT_BORDER);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
        visuals.widgets.inactive.corner_radius = 6.0.into();

        visuals.widgets.hovered.bg_fill = INPUT_BG_HOVER;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.5_f32, INPUT_BORDER_HOVER);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
        visuals.widgets.hovered.corner_radius = 6.0.into();

        visuals.widgets.active.bg_fill = INPUT_BG;
        visuals.widgets.active.bg_stroke = Stroke::new(1.5_f32, PRIMARY_BLUE);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, PRIMARY_BLUE);
        visuals.widgets.active.corner_radius = 6.0.into();

        visuals.widgets.open.bg_fill = INPUT_BG;
        visuals.widgets.open.bg_stroke = Stroke::new(1.5_f32, PRIMARY_BLUE);
        visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
        visuals.widgets.open.corner_radius = 6.0.into();

        style.visuals = visuals;
        ctx.set_style(style);
    }
}

impl eframe::App for ShiplyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.fonts_initialized {
            Self::install_system_fonts(ctx);
            self.fonts_initialized = true;
        }
        self.poll_executor_events(ctx);
        Self::configure_light_workspace_style(ctx);

        self.render_sidebar(ctx);
        self.render_new_project_modal(ctx);
        self.render_new_environment_modal(ctx);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(CANVAS_BG)
                    .inner_margin(Margin::symmetric(28, 22)),
            )
            .show(ctx, |ui| {
                ScrollArea::vertical()
                    .id_salt("main_workspace_vertical_scroll")
                    .show(ui, |ui| {
                        if let Some((msg, is_ok)) = &self.status_banner.clone() {
                            let (bg, fg, border) = if *is_ok {
                                (
                                    SUCCESS_PILL_BG,
                                    SUCCESS_PILL_TEXT,
                                    Color32::from_rgb(74, 222, 128),
                                )
                            } else {
                                (
                                    FAIL_PILL_BG,
                                    FAIL_PILL_TEXT,
                                    Color32::from_rgb(248, 113, 113),
                                )
                            };
                            let banner_w = ui.available_width();
                            let (b_rect, _) =
                                ui.allocate_exact_size(Vec2::new(banner_w, 40.0), Sense::hover());
                            ui.painter()
                                .rect_filled(b_rect, CornerRadius::same(8), bg);
                            ui.painter().rect_stroke(
                                b_rect,
                                CornerRadius::same(8),
                                Stroke::new(1.0_f32, border),
                                StrokeKind::Inside,
                            );
                            ui.painter().text(
                                Pos2::new(b_rect.left() + 14.0, b_rect.center().y),
                                Align2::LEFT_CENTER,
                                msg,
                                FontId::proportional(FONT_MD),
                                fg,
                            );
                            let close_rect = Rect::from_center_size(
                                Pos2::new(b_rect.right() - 36.0, b_rect.center().y),
                                Vec2::new(48.0, 24.0),
                            );
                            let close_resp = ui.interact(
                                close_rect,
                                egui::Id::new("close_status_banner"),
                                Sense::click(),
                            );
                            ui.painter().text(
                                close_rect.center(),
                                Align2::CENTER_CENTER,
                                "Close",
                                FontId::proportional(FONT_SM),
                                fg,
                            );
                            if close_resp.clicked() {
                                self.status_banner = None;
                            }
                            ui.add_space(12.0);
                        }

                        if self.storage.projects.is_empty() {
                            self.render_first_time_empty_state(ui);
                        } else {
                            match self.selection.clone() {
                                SidebarSelection::AllDeployments => {
                                    self.render_all_deployments_overview(ui);
                                }
                                SidebarSelection::Project(proj_id) => {
                                    self.render_project_workspace(ui, &proj_id);
                                }
                            }
                        }
                    });
            });
    }
}

impl ShiplyApp {
    fn render_sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("shiply_sidebar")
            .resizable(false)
            .exact_width(248.0)
            .frame(
                egui::Frame::new()
                    .fill(SIDEBAR_BG)
                    .inner_margin(Margin::symmetric(16, 18))
                    .stroke(Stroke::new(1.0_f32, SIDEBAR_DIVIDER)),
            )
            .show(ctx, |ui| {
                ui.visuals_mut().override_text_color = Some(Color32::from_rgb(241, 245, 249));
                let full_w = ui.available_width();

                // 1. Top Brand Bar (Fixed 32px height)
                let (brand_rect, _) =
                    ui.allocate_exact_size(Vec2::new(full_w, 32.0), Sense::hover());
                let logo_rect = Rect::from_min_size(
                    Pos2::new(brand_rect.left(), brand_rect.center().y - 14.0),
                    Vec2::new(28.0, 28.0),
                );
                ui.painter()
                    .rect_filled(logo_rect, CornerRadius::same(6), PRIMARY_BLUE);
                ui.painter().text(
                    logo_rect.center(),
                    Align2::CENTER_CENTER,
                    "S",
                    FontId::proportional(FONT_LG),
                    Color32::WHITE,
                );
                ui.painter().text(
                    Pos2::new(logo_rect.right() + 10.0, brand_rect.center().y),
                    Align2::LEFT_CENTER,
                    "Shiply",
                    FontId::proportional(FONT_XL),
                    Color32::WHITE,
                );
                paint_pill_right_aligned(
                    ui.painter(),
                    Pos2::new(brand_rect.right(), brand_rect.center().y),
                    &format!("V{APP_VERSION}"),
                    SIDEBAR_PILL_BG,
                    SIDEBAR_TEXT_MUTED,
                );

                ui.add_space(12.0);
                draw_divider(ui, SIDEBAR_DIVIDER);
                ui.add_space(10.0);

                // 2. All Deployments Navigation Button (Fixed 38px height)
                let is_overview = matches!(self.selection, SidebarSelection::AllDeployments);
                let (nav_rect, nav_resp) =
                    ui.allocate_exact_size(Vec2::new(full_w, 38.0), Sense::click());
                let nav_bg = if is_overview {
                    SIDEBAR_CARD_ACTIVE
                } else if nav_resp.hovered() {
                    SIDEBAR_CARD_HOVER
                } else {
                    Color32::TRANSPARENT
                };
                ui.painter()
                    .rect_filled(nav_rect, CornerRadius::same(7), nav_bg);
                if is_overview {
                    ui.painter().rect_stroke(
                        nav_rect,
                        CornerRadius::same(7),
                        Stroke::new(1.0_f32, SIDEBAR_BORDER_ACTIVE),
                        StrokeKind::Inside,
                    );
                }
                draw_icon_layers(
                    ui.painter(),
                    Pos2::new(nav_rect.left() + 16.0, nav_rect.center().y),
                    if is_overview {
                        Color32::from_rgb(96, 165, 250)
                    } else {
                        SIDEBAR_TEXT_MUTED
                    },
                );
                ui.painter().text(
                    Pos2::new(nav_rect.left() + 34.0, nav_rect.center().y),
                    Align2::LEFT_CENTER,
                    "All Deployments",
                    FontId::proportional(FONT_MD),
                    if is_overview {
                        Color32::WHITE
                    } else {
                        Color32::from_rgb(226, 232, 240)
                    },
                );
                if nav_resp.clicked() {
                    self.selection = SidebarSelection::AllDeployments;
                }

                ui.add_space(18.0);

                // 3. PROJECTS Section Header + "+ ADD" button (Fixed 26px height)
                let (hdr_rect, _) =
                    ui.allocate_exact_size(Vec2::new(full_w, 26.0), Sense::hover());
                ui.painter().text(
                    Pos2::new(hdr_rect.left() + 2.0, hdr_rect.center().y),
                    Align2::LEFT_CENTER,
                    "PROJECTS",
                    FontId::proportional(FONT_XS),
                    SIDEBAR_TEXT_MUTED,
                );
                let add_btn_rect = Rect::from_min_size(
                    Pos2::new(hdr_rect.right() - 52.0, hdr_rect.center().y - 11.0),
                    Vec2::new(52.0, 22.0),
                );
                let add_btn_resp = ui.interact(
                    add_btn_rect,
                    egui::Id::new("sidebar_add_proj_btn"),
                    Sense::click(),
                );
                ui.painter().rect_filled(
                    add_btn_rect,
                    CornerRadius::same(5),
                    if add_btn_resp.hovered() {
                        PRIMARY_BLUE
                    } else {
                        SIDEBAR_PILL_BG
                    },
                );
                ui.painter().rect_stroke(
                    add_btn_rect,
                    CornerRadius::same(5),
                    Stroke::new(1.0_f32, Color32::from_rgb(51, 65, 85)),
                    StrokeKind::Inside,
                );
                ui.painter().text(
                    add_btn_rect.center(),
                    Align2::CENTER_CENTER,
                    "+ ADD",
                    FontId::proportional(FONT_XS),
                    Color32::WHITE,
                );
                if add_btn_resp.clicked() {
                    self.show_new_project_modal = true;
                }

                ui.add_space(8.0);

                // 4. Project Cards List
                if self.storage.projects.is_empty() {
                    let (empty_rect, _) =
                        ui.allocate_exact_size(Vec2::new(full_w, 60.0), Sense::hover());
                    ui.painter().rect_stroke(
                        empty_rect,
                        CornerRadius::same(8),
                        Stroke::new(1.0_f32, SIDEBAR_DIVIDER),
                        StrokeKind::Inside,
                    );
                    ui.painter().text(
                        empty_rect.center(),
                        Align2::CENTER_CENTER,
                        "No projects yet.\nClick '+ ADD' or import .ps1",
                        FontId::proportional(FONT_SM),
                        SIDEBAR_TEXT_MUTED,
                    );
                } else {
                    let mut clicked_project_id = None;
                    for project in &self.storage.projects {
                        let is_selected = matches!(
                            &self.selection,
                            SidebarSelection::Project(id) if id == &project.id
                        );
                        let (p_rect, p_resp) =
                            ui.allocate_exact_size(Vec2::new(full_w, 66.0), Sense::click());

                        let fill = if is_selected {
                            SIDEBAR_CARD_ACTIVE
                        } else if p_resp.hovered() {
                            Color32::from_rgb(22, 32, 52)
                        } else {
                            Color32::TRANSPARENT
                        };
                        ui.painter()
                            .rect_filled(p_rect, CornerRadius::same(8), fill);
                        if is_selected {
                            ui.painter().rect_stroke(
                                p_rect,
                                CornerRadius::same(8),
                                Stroke::new(1.5_f32, SIDEBAR_BORDER_ACTIVE),
                                StrokeKind::Inside,
                            );
                        }

                        // Project Title
                        ui.painter().text(
                            Pos2::new(p_rect.left() + 12.0, p_rect.top() + 20.0),
                            Align2::LEFT_CENTER,
                            &project.name,
                            FontId::proportional(FONT_MD),
                            if is_selected {
                                Color32::from_rgb(96, 165, 250)
                            } else {
                                Color32::WHITE
                            },
                        );

                        // Environment version pills row inside project card
                        let mut pill_x = p_rect.left() + 12.0;
                        let pill_y = p_rect.top() + 45.0;
                        if project.environments.is_empty() {
                            paint_small_sidebar_pill(
                                ui.painter(),
                                Pos2::new(pill_x, pill_y),
                                "NO ENVIRONMENTS",
                                SIDEBAR_PILL_BG,
                                SIDEBAR_TEXT_MUTED,
                            );
                        } else {
                            for env in project.environments.iter().take(3) {
                                let badge = env.badge_label();
                                let (label, bg, fg) = if let Some(ok_dep) =
                                    self.storage.last_succeeded_deployment(&project.id, &env.key)
                                {
                                    (
                                        format!("{badge} v{}", ok_dep.version),
                                        Color32::from_rgb(23, 37, 84),
                                        Color32::from_rgb(96, 165, 250),
                                    )
                                } else {
                                    (
                                        format!("{badge} —"),
                                        SIDEBAR_PILL_BG,
                                        SIDEBAR_TEXT_MUTED,
                                    )
                                };
                                let w = paint_small_sidebar_pill(
                                    ui.painter(),
                                    Pos2::new(pill_x, pill_y),
                                    &label,
                                    bg,
                                    fg,
                                );
                                pill_x += w + 5.0;
                            }
                        }

                        if p_resp.clicked() {
                            clicked_project_id = Some(project.id.clone());
                        }
                        ui.add_space(4.0);
                    }

                    if let Some(pid) = clicked_project_id {
                        self.selection = SidebarSelection::Project(pid);
                        self.refresh_selected_project_metadata(true);
                    }
                }

                // 5. Bottom Sidebar Footer (App Install + Open Data Dir + Sync)
                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    let install_state = check_install_state(&default_install_dir(), APP_VERSION);
                    let (install_label, install_color) = match &install_state {
                        InstallState::NotInstalled => (
                            "Install Desktop App (.exe)".to_string(),
                            Color32::from_rgb(96, 165, 250),
                        ),
                        InstallState::InstalledCurrent { version, .. } => (
                            format!("Installed v{version} (Reinstall)"),
                            TERM_GREEN,
                        ),
                        InstallState::UpdateReady {
                            installed_version,
                            running_version,
                            ..
                        } => (
                            format!("Update App (v{installed_version} -> v{running_version})"),
                            TERM_YELLOW,
                        ),
                    };

                    let (inst_rect, inst_resp) =
                        ui.allocate_exact_size(Vec2::new(full_w, 24.0), Sense::click());
                    ui.painter().circle_filled(
                        Pos2::new(inst_rect.left() + 6.0, inst_rect.center().y),
                        4.0,
                        install_color,
                    );
                    ui.painter().text(
                        Pos2::new(inst_rect.left() + 16.0, inst_rect.center().y),
                        Align2::LEFT_CENTER,
                        install_label,
                        FontId::proportional(FONT_SM),
                        install_color,
                    );
                    if inst_resp.clicked() {
                        match install_or_update_self() {
                            Ok(msg) => self.status_banner = Some((msg, true)),
                            Err(err) => self.status_banner = Some((err, false)),
                        }
                    }

                    ui.add_space(4.0);
                    let (dir_rect, dir_resp) =
                        ui.allocate_exact_size(Vec2::new(full_w, 24.0), Sense::click());
                    draw_icon_folder(
                        ui.painter(),
                        Pos2::new(dir_rect.left() + 6.0, dir_rect.center().y),
                        SIDEBAR_TEXT_MUTED,
                    );
                    ui.painter().text(
                        Pos2::new(dir_rect.left() + 18.0, dir_rect.center().y),
                        Align2::LEFT_CENTER,
                        "Open App Folder (JSON / .ps1)",
                        FontId::proportional(FONT_SM),
                        if dir_resp.hovered() {
                            Color32::WHITE
                        } else {
                            SIDEBAR_TEXT_MUTED
                        },
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

                    ui.add_space(4.0);
                    let (sync_rect, sync_resp) =
                        ui.allocate_exact_size(Vec2::new(full_w, 24.0), Sense::click());
                    draw_icon_refresh(
                        ui.painter(),
                        Pos2::new(sync_rect.left() + 6.0, sync_rect.center().y),
                        SIDEBAR_TEXT_MUTED,
                    );
                    ui.painter().text(
                        Pos2::new(sync_rect.left() + 18.0, sync_rect.center().y),
                        Align2::LEFT_CENTER,
                        "Sync App Directory Files",
                        FontId::proportional(FONT_SM),
                        if sync_resp.hovered() {
                            Color32::WHITE
                        } else {
                            SIDEBAR_TEXT_MUTED
                        },
                    );
                    if sync_resp.clicked() {
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

                    ui.add_space(6.0);
                    draw_divider(ui, SIDEBAR_DIVIDER);
                });
            });
    }

    fn render_first_time_empty_state(&mut self, ui: &mut egui::Ui) {
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
                    ui.painter()
                        .rect_filled(icon_rect, CornerRadius::same(12), BLUE_PILL_BG);
                    draw_icon_layers(ui.painter(), icon_rect.center(), PRIMARY_BLUE);

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
                            if let Some(picked) = pick_ps1_file_dialog() {
                                self.new_project_ps1_path = picked.clone();
                                self.apply_ps1_auto_import_to_modal(&picked);
                            }
                            self.show_new_project_modal = true;
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
                            self.show_new_project_modal = true;
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
    }

    fn render_new_project_modal(&mut self, ctx: &egui::Context) {
        if !self.show_new_project_modal {
            return;
        }

        // Dimmed backdrop
        egui::Area::new(egui::Id::new("modal_backdrop_new_project"))
            .order(egui::Order::Middle)
            .fixed_pos(Pos2::ZERO)
            .show(ctx, |ui| {
                let screen_rect = ctx.screen_rect();
                ui.painter().rect_filled(
                    screen_rect,
                    CornerRadius::ZERO,
                    Color32::from_black_alpha(95),
                );
            });

        egui::Window::new("Add Project")
            .order(egui::Order::Foreground)
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .fixed_size(Vec2::new(560.0, 0.0))
            .frame(
                egui::Frame::new()
                    .fill(CARD_BG)
                    .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                    .corner_radius(12.0)
                    .shadow(Shadow {
                        offset: [0, 8],
                        blur: 24,
                        spread: 0,
                        color: Color32::from_black_alpha(45),
                    })
                    .inner_margin(Margin::symmetric(24, 22)),
            )
            .show(ctx, |ui| {
                ui.set_width(512.0);

                // Modal Header
                horizontal_row(ui, 28.0, |ui| {
                    ui.label(
                        RichText::new("Add New Project")
                            .size(FONT_LG)
                            .strong()
                            .color(TEXT_PRIMARY),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let close_btn = egui::Button::new(
                            RichText::new("×").size(FONT_LG).strong().color(TEXT_MUTED),
                        )
                        .fill(SUBTLE_BG)
                        .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                        .corner_radius(6.0)
                        .min_size(Vec2::new(28.0, 28.0));
                        if ui.add(close_btn).clicked() {
                            self.show_new_project_modal = false;
                        }
                    });
                });
                ui.label(
                    RichText::new(
                        "Auto-import a PowerShell deployment script (.ps1) to populate environments and steps, or configure manually.",
                    )
                    .size(FONT_SM)
                    .color(TEXT_MUTED),
                );

                ui.add_space(12.0);

                // Auto-Import .ps1 Card Box
                egui::Frame::new()
                    .fill(SUBTLE_BG)
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(191, 219, 254)))
                    .corner_radius(8.0)
                    .inner_margin(Margin::symmetric(14, 12))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("AUTO-IMPORT FROM POWERSHELL SCRIPT (.PS1)")
                                .size(FONT_XS)
                                .strong()
                                .color(PRIMARY_BLUE),
                        );
                        ui.add_space(6.0);
                        horizontal_row(ui, 34.0, |ui| {
                            styled_text_edit(
                                ui,
                                &mut self.new_project_ps1_path,
                                285.0,
                                Some(r"D:\NTI\Projects\backend\ndm-dq-subscription\deploy-dev.ps1"),
                            );
                            let browse_btn = egui::Button::new(
                                RichText::new("Browse .ps1...")
                                    .size(FONT_SM)
                                    .strong()
                                    .color(TEXT_PRIMARY),
                            )
                            .fill(CARD_BG)
                            .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                            .min_size(Vec2::new(95.0, 32.0));
                            if ui.add(browse_btn).clicked() {
                                if let Some(picked) = pick_ps1_file_dialog() {
                                    self.new_project_ps1_path = picked.clone();
                                    self.apply_ps1_auto_import_to_modal(&picked);
                                }
                            }

                            let parse_btn = egui::Button::new(
                                RichText::new("Auto-Fill")
                                    .size(FONT_SM)
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .fill(PRIMARY_BLUE)
                            .min_size(Vec2::new(80.0, 32.0));
                            if ui.add(parse_btn).clicked()
                                && !self.new_project_ps1_path.trim().is_empty()
                            {
                                let path_copy = self.new_project_ps1_path.clone();
                                self.apply_ps1_auto_import_to_modal(&path_copy);
                            }
                        });

                        ui.add_space(4.0);
                        if ui
                            .checkbox(
                                &mut self.new_project_import_siblings,
                                RichText::new(
                                    "Also auto-import sibling deploy-*.ps1 scripts in the same project folder",
                                )
                                .size(FONT_SM)
                                .color(TEXT_SECONDARY),
                            )
                            .changed()
                            && !self.new_project_ps1_path.trim().is_empty()
                        {
                            let path_copy = self.new_project_ps1_path.clone();
                            self.apply_ps1_auto_import_to_modal(&path_copy);
                        }

                        if let Some(notice) = &self.new_project_import_notice {
                            ui.add_space(4.0);
                            let is_err = notice.starts_with("Import error:");
                            ui.label(
                                RichText::new(notice)
                                    .size(FONT_SM)
                                    .strong()
                                    .color(if is_err {
                                        FAIL_PILL_TEXT
                                    } else {
                                        SUCCESS_PILL_TEXT
                                    }),
                            );
                        }
                    });

                ui.add_space(14.0);

                // Project Form Fields
                ui.label(
                    RichText::new("Project Name")
                        .size(FONT_SM)
                        .strong()
                        .color(TEXT_SECONDARY),
                );
                ui.add_space(3.0);
                styled_text_edit(
                    ui,
                    &mut self.new_project_name,
                    492.0,
                    Some("e.g. ndm-dq-subscription"),
                );

                ui.add_space(10.0);
                ui.label(
                    RichText::new("Local Project Directory")
                        .size(FONT_SM)
                        .strong()
                        .color(TEXT_SECONDARY),
                );
                ui.add_space(3.0);
                horizontal_row(ui, 34.0, |ui| {
                    styled_text_edit(
                        ui,
                        &mut self.new_project_path,
                        380.0,
                        Some(r"e.g. D:\NTI\Projects\backend\ndm-dq-subscription"),
                    );
                    let folder_btn = egui::Button::new(
                        RichText::new("Browse Folder...")
                            .size(FONT_SM)
                            .strong()
                            .color(TEXT_PRIMARY),
                    )
                    .fill(SUBTLE_BG)
                    .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                    .min_size(Vec2::new(108.0, 32.0));
                    if ui.add(folder_btn).clicked() {
                        if let Some(folder) = pick_folder_dialog() {
                            self.new_project_path = folder.clone();
                            if self.new_project_name.trim().is_empty() {
                                self.apply_ps1_auto_import_to_modal(&folder);
                            }
                        }
                    }
                });

                ui.add_space(10.0);
                ui.label(
                    RichText::new("Description")
                        .size(FONT_SM)
                        .strong()
                        .color(TEXT_SECONDARY),
                );
                ui.add_space(3.0);
                styled_text_edit(
                    ui,
                    &mut self.new_project_desc,
                    492.0,
                    Some("e.g. Backend service (Node.js / TypeScript / PM2)"),
                );

                ui.add_space(18.0);
                draw_divider(ui, CARD_BORDER);
                ui.add_space(12.0);

                horizontal_row(ui, 36.0, |ui| {
                    let env_count = self.new_project_imported_envs.len();
                    ui.label(
                        RichText::new(if env_count > 0 {
                            format!("{env_count} environment(s) ready to import")
                        } else {
                            "Starts empty (you can add or import environments anytime)".to_string()
                        })
                        .size(FONT_SM)
                        .color(TEXT_MUTED),
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let can_create = !self.new_project_name.trim().is_empty()
                            && !self.new_project_path.trim().is_empty();
                        let create_btn = egui::Button::new(
                            RichText::new("Create Project")
                                .size(FONT_MD)
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(if can_create { PRIMARY_BLUE } else { TEXT_MUTED })
                        .corner_radius(7.0)
                        .min_size(Vec2::new(135.0, 34.0));

                        if ui.add(create_btn).clicked() && can_create {
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
                            self.new_project_imported_envs.clear();
                            self.new_project_import_notice = None;
                            self.show_new_project_modal = false;
                        }

                        let cancel_btn = egui::Button::new(
                            RichText::new("Cancel")
                                .size(FONT_MD)
                                .strong()
                                .color(TEXT_SECONDARY),
                        )
                        .fill(SUBTLE_BG)
                        .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                        .corner_radius(7.0)
                        .min_size(Vec2::new(84.0, 34.0));
                        if ui.add(cancel_btn).clicked() {
                            self.show_new_project_modal = false;
                        }
                    });
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

        egui::Area::new(egui::Id::new("modal_backdrop_new_env"))
            .order(egui::Order::Middle)
            .fixed_pos(Pos2::ZERO)
            .show(ctx, |ui| {
                ui.painter().rect_filled(
                    ctx.screen_rect(),
                    CornerRadius::ZERO,
                    Color32::from_black_alpha(95),
                );
            });

        egui::Window::new("Add Environment Level")
            .order(egui::Order::Foreground)
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .fixed_size(Vec2::new(500.0, 0.0))
            .frame(
                egui::Frame::new()
                    .fill(CARD_BG)
                    .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                    .corner_radius(12.0)
                    .shadow(Shadow {
                        offset: [0, 8],
                        blur: 24,
                        spread: 0,
                        color: Color32::from_black_alpha(45),
                    })
                    .inner_margin(Margin::symmetric(24, 22)),
            )
            .show(ctx, |ui| {
                ui.set_width(452.0);
                horizontal_row(ui, 28.0, |ui| {
                    ui.label(
                        RichText::new("Add Environment Level")
                            .size(FONT_LG)
                            .strong()
                            .color(TEXT_PRIMARY),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let close_btn = egui::Button::new(
                            RichText::new("×").size(FONT_LG).strong().color(TEXT_MUTED),
                        )
                        .fill(SUBTLE_BG)
                        .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                        .corner_radius(6.0)
                        .min_size(Vec2::new(28.0, 28.0));
                        if ui.add(close_btn).clicked() {
                            self.show_new_env_modal = false;
                        }
                    });
                });

                ui.label(
                    RichText::new(
                        "Import an environment from a .ps1 script or create a custom level (dev, staging, production, UAT, QA).",
                    )
                    .size(FONT_SM)
                    .color(TEXT_MUTED),
                );
                ui.add_space(12.0);

                // Option 1: Import from .ps1 script
                egui::Frame::new()
                    .fill(SUBTLE_BG)
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(191, 219, 254)))
                    .corner_radius(8.0)
                    .inner_margin(Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("IMPORT ENVIRONMENT FROM .PS1 SCRIPT")
                                .size(FONT_XS)
                                .strong()
                                .color(PRIMARY_BLUE),
                        );
                        ui.add_space(4.0);
                        horizontal_row(ui, 34.0, |ui| {
                            styled_text_edit(
                                ui,
                                &mut self.new_env_ps1_path,
                                235.0,
                                Some(r"Path to deploy-dev.ps1 or deploy-prod.ps1"),
                            );
                            let browse_btn = egui::Button::new(
                                RichText::new("Browse .ps1...")
                                    .size(FONT_SM)
                                    .strong()
                                    .color(TEXT_PRIMARY),
                            )
                            .fill(CARD_BG)
                            .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                            .min_size(Vec2::new(92.0, 32.0));
                            if ui.add(browse_btn).clicked() {
                                if let Some(picked) = pick_ps1_file_dialog() {
                                    self.new_env_ps1_path = picked;
                                }
                            }
                            let imp_btn = egui::Button::new(
                                RichText::new("Import")
                                    .size(FONT_SM)
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .fill(PRIMARY_BLUE)
                            .min_size(Vec2::new(72.0, 32.0));
                            if ui.add(imp_btn).clicked()
                                && !self.new_env_ps1_path.trim().is_empty()
                            {
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
                                        self.refresh_selected_project_metadata(false);
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

                ui.add_space(12.0);

                // Option 2: Create by Name
                ui.label(
                    RichText::new("Or Create Custom Environment Name")
                        .size(FONT_SM)
                        .strong()
                        .color(TEXT_SECONDARY),
                );
                ui.add_space(4.0);
                styled_text_edit(
                    ui,
                    &mut self.new_env_name,
                    432.0,
                    Some("e.g. Development, Staging, Production, UAT, QA"),
                );

                if !existing_envs.is_empty() {
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new("Clone Pipeline Template From:")
                            .size(FONT_SM)
                            .strong()
                            .color(TEXT_SECONDARY),
                    );
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        for (key, name) in &existing_envs {
                            let selected = self.new_env_clone_from == *key;
                            let btn = egui::Button::new(
                                RichText::new(name).size(FONT_SM).strong().color(if selected {
                                    BLUE_PILL_TEXT
                                } else {
                                    TEXT_SECONDARY
                                }),
                            )
                            .fill(if selected { BLUE_PILL_BG } else { SUBTLE_BG })
                            .stroke(Stroke::new(
                                1.0_f32,
                                if selected { PRIMARY_BLUE } else { CARD_BORDER },
                            ));
                            if ui.add(btn).clicked() {
                                self.new_env_clone_from = key.clone();
                            }
                        }
                    });
                }

                ui.add_space(16.0);
                draw_divider(ui, CARD_BORDER);
                ui.add_space(12.0);

                horizontal_row(ui, 34.0, |ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let create_btn = egui::Button::new(
                            RichText::new("Add Environment")
                                .size(FONT_MD)
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(PRIMARY_BLUE)
                        .corner_radius(7.0)
                        .min_size(Vec2::new(135.0, 34.0));

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

                        let cancel_btn = egui::Button::new(
                            RichText::new("Cancel")
                                .size(FONT_MD)
                                .strong()
                                .color(TEXT_SECONDARY),
                        )
                        .fill(SUBTLE_BG)
                        .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                        .corner_radius(7.0)
                        .min_size(Vec2::new(84.0, 34.0));
                        if ui.add(cancel_btn).clicked() {
                            self.show_new_env_modal = false;
                        }
                    });
                });
            });
    }

    fn render_all_deployments_overview(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("All Deployments")
                .size(FONT_XL)
                .strong()
                .color(TEXT_PRIMARY),
        );
        ui.label(
            RichText::new("Overview of deployments and active versions across all projects.")
                .size(FONT_MD)
                .color(TEXT_SECONDARY),
        );
        ui.add_space(16.0);

        let proj_count = self.storage.projects.len().max(1);
        let spacing = 14.0;
        let card_w = ((ui.available_width() - spacing * (proj_count.saturating_sub(1) as f32))
            / (proj_count as f32))
            .max(240.0);

        let mut clicked_proj = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(spacing, 14.0);
            for project in &self.storage.projects {
                let (rect, resp) =
                    ui.allocate_exact_size(Vec2::new(card_w, 108.0), Sense::click());
                draw_elevated_card(
                    ui.painter(),
                    rect,
                    CARD_BG,
                    Stroke::new(
                        if resp.hovered() { 1.5_f32 } else { 1.0_f32 },
                        if resp.hovered() {
                            PRIMARY_BLUE
                        } else {
                            CARD_BORDER
                        },
                    ),
                );

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
                    let (bg, fg) = env_pastel_pill_colors(&env.key);
                    let ver_label = self
                        .storage
                        .last_succeeded_deployment(&project.id, &env.key)
                        .map(|d| format!("{badge}: v{}", d.version))
                        .unwrap_or_else(|| format!("{badge}: —"));
                    let w = paint_pill_left_aligned(
                        ui.painter(),
                        Pos2::new(px, py),
                        &ver_label,
                        bg,
                        fg,
                    );
                    px += w + 6.0;
                }
                if resp.clicked() {
                    clicked_proj = Some(project.id.clone());
                }
            }
        });

        if let Some(pid) = clicked_proj {
            self.selection = SidebarSelection::Project(pid);
            self.refresh_selected_project_metadata(true);
            return;
        }

        ui.add_space(20.0);
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

        let proj_name = self.storage.projects[proj_idx].name.clone();
        let proj_desc = self.storage.projects[proj_idx].description.clone();
        let proj_path = self.storage.projects[proj_idx].local_path.clone();
        let path_exists = PathBuf::from(&proj_path).exists();
        let avail_w = ui.available_width();

        // 1. Top Project Header Bar (Fixed 36px height)
        let mut delete_project_clicked = false;
        let mut selected_branch_changed = None;

        horizontal_row(ui, 36.0, |ui| {
            ui.label(
                RichText::new(&proj_name)
                    .size(FONT_XL)
                    .strong()
                    .color(TEXT_PRIMARY),
            );
            let (r_txt, r_bg, r_fg) = if path_exists {
                ("READY", SUCCESS_PILL_BG, SUCCESS_PILL_TEXT)
            } else {
                ("PATH MISSING", FAIL_PILL_BG, FAIL_PILL_TEXT)
            };
            render_pill(ui, r_txt, r_bg, r_fg);

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let del_proj_btn = egui::Button::new(
                    RichText::new("Delete Project")
                        .size(FONT_SM)
                        .strong()
                        .color(FAIL_PILL_TEXT),
                )
                .fill(FAIL_PILL_BG)
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(252, 165, 165)))
                .corner_radius(7.0)
                .min_size(Vec2::new(100.0, 32.0));
                if ui.add(del_proj_btn).clicked() {
                    delete_project_clicked = true;
                }

                let ref_btn = egui::Button::new(
                    RichText::new("Refresh")
                        .size(FONT_SM)
                        .strong()
                        .color(TEXT_SECONDARY),
                )
                .fill(CARD_BG)
                .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                .corner_radius(7.0)
                .min_size(Vec2::new(68.0, 32.0));
                if ui
                    .add(ref_btn)
                    .on_hover_text("Refresh Git branches, commits, and package.json version")
                    .clicked()
                {
                    self.refresh_selected_project_metadata(false);
                }

                // Git Branch Selector Dropdown (Feature #2)
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

                    egui::ComboBox::from_id_salt("header_git_branch_selector")
                        .selected_text(
                            RichText::new(format!("Branch: {current_b}{commit_suffix}"))
                                .size(FONT_SM)
                                .strong()
                                .color(TEXT_PRIMARY),
                        )
                        .width(185.0)
                        .show_ui(ui, |ui| {
                            for branch in self.cached_git_branches.clone() {
                                let is_sel = self.selected_git_branch == branch;
                                if ui.selectable_label(is_sel, &branch).clicked() && !is_sel {
                                    selected_branch_changed = Some(branch);
                                }
                            }
                        });
                }
            });
        });

        if delete_project_clicked {
            self.storage.projects.remove(proj_idx);
            let _ = self.storage.save();
            self.selection = self
                .storage
                .projects
                .first()
                .map(|p| SidebarSelection::Project(p.id.clone()))
                .unwrap_or(SidebarSelection::AllDeployments);
            self.refresh_selected_project_metadata(true);
            self.status_banner = Some((format!("Removed project '{proj_name}'."), true));
            return;
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

        // 3. Environment Cards Grid — Exact 96px height + direct Delete (`×`) button on each card (Feature #5)
        let env_summaries: Vec<(String, String)> = self.storage.projects[proj_idx]
            .environments
            .iter()
            .map(|e| (e.key.clone(), e.name.clone()))
            .collect();

        let mut new_selected_env_key = None;
        let mut delete_env_key = None;
        let card_spacing = 14.0;
        let card_w = ((avail_w - card_spacing * 2.0) / 3.0).max(210.0);

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

                // Delete (`×`) button rect at top-right of the card
                let del_rect = Rect::from_center_size(
                    Pos2::new(card_rect.right() - 18.0, card_rect.top() + 24.0),
                    Vec2::new(20.0, 20.0),
                );
                let del_resp = ui.interact(
                    del_rect,
                    egui::Id::new(("del_env_card_btn", project_id, env_key)),
                    Sense::click(),
                );

                let stroke = if is_active {
                    Stroke::new(2.0_f32, PRIMARY_BLUE)
                } else if card_resp.hovered() {
                    Stroke::new(1.5_f32, INPUT_BORDER_HOVER)
                } else {
                    Stroke::new(1.0_f32, CARD_BORDER)
                };

                draw_elevated_card(ui.painter(), card_rect, CARD_BG, stroke);

                // Top-left: Environment pastel pill
                let (pill_bg, pill_fg) = env_pastel_pill_colors(env_key);
                paint_pill_left_aligned(
                    ui.painter(),
                    Pos2::new(card_rect.left() + 16.0, card_rect.top() + 24.0),
                    &env_name.to_uppercase(),
                    pill_bg,
                    pill_fg,
                );

                // Top-right (left of delete `×` button): Status pill or "NOT DEPLOYED"
                if let Some(latest) = latest_any {
                    let (s_bg, s_fg) = status_pill_colors(latest.status);
                    paint_pill_right_aligned(
                        ui.painter(),
                        Pos2::new(del_rect.left() - 6.0, card_rect.top() + 24.0),
                        &latest.status.label().to_uppercase(),
                        s_bg,
                        s_fg,
                    );
                } else {
                    ui.painter().text(
                        Pos2::new(del_rect.left() - 8.0, card_rect.top() + 24.0),
                        Align2::RIGHT_CENTER,
                        "NOT DEPLOYED",
                        FontId::proportional(FONT_XS),
                        TEXT_MUTED,
                    );
                }

                // Paint the `×` delete environment button
                if del_resp.hovered() {
                    ui.painter()
                        .rect_filled(del_rect, CornerRadius::same(4), FAIL_PILL_BG);
                }
                ui.painter().text(
                    del_rect.center(),
                    Align2::CENTER_CENTER,
                    "×",
                    FontId::proportional(FONT_MD),
                    if del_resp.hovered() {
                        FAIL_PILL_TEXT
                    } else {
                        TEXT_MUTED
                    },
                );

                // Bottom area of card: Active version or "No Active Build"
                if let Some(ok_dep) = last_ok {
                    ui.painter().text(
                        Pos2::new(card_rect.left() + 16.0, card_rect.top() + 58.0),
                        Align2::LEFT_CENTER,
                        format!("Active: v{}", ok_dep.version),
                        FontId::proportional(FONT_LG),
                        TEXT_PRIMARY,
                    );
                    ui.painter().text(
                        Pos2::new(card_rect.left() + 16.0, card_rect.top() + 78.0),
                        Align2::LEFT_CENTER,
                        "CURRENT VERSION",
                        FontId::proportional(FONT_XS),
                        TEXT_MUTED,
                    );
                } else {
                    ui.painter().text(
                        Pos2::new(card_rect.left() + 16.0, card_rect.top() + 62.0),
                        Align2::LEFT_CENTER,
                        "No Active Build",
                        FontId::proportional(FONT_LG),
                        TEXT_PLACEHOLDER,
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
            let add_bg = if add_resp.hovered() {
                CARD_BG
            } else {
                SUBTLE_BG
            };
            let add_stroke = if add_resp.hovered() {
                Stroke::new(1.5_f32, PRIMARY_BLUE)
            } else {
                Stroke::new(1.0_f32, INPUT_BORDER)
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
            draw_icon_plus(ui.painter(), plus_center, TEXT_MUTED);
            ui.painter().text(
                Pos2::new(add_rect.center().x, add_rect.center().y + 14.0),
                Align2::CENTER_CENTER,
                "ADD / IMPORT ENVIRONMENT",
                FontId::proportional(FONT_SM),
                TEXT_SECONDARY,
            );

            if add_resp.clicked() {
                self.new_env_clone_from = self.selected_env_key.clone();
                self.show_new_env_modal = true;
            }
        });

        if let Some((del_key, del_name)) = delete_env_key {
            if let Err(err) = self.storage.projects[proj_idx].remove_environment(&del_key) {
                self.status_banner = Some((err, false));
            } else {
                let _ = self.storage.save();
                self.selected_env_key = self.storage.projects[proj_idx]
                    .environments
                    .first()
                    .map(|e| e.key.clone())
                    .unwrap_or_default();
                self.refresh_selected_project_metadata(false);
                self.status_banner =
                    Some((format!("Removed environment '{del_name}'."), true));
            }
        } else if let Some(env_key) = new_selected_env_key {
            self.selected_env_key = env_key;
            self.refresh_selected_project_metadata(false);
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
        self.render_deploy_action_bar(ui, &active_env_name, rollback_candidate);

        ui.add_space(18.0);

        // 5. Clean Section Tabs (Fixed 38px height)
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

        horizontal_row(ui, 38.0, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(10.0, 0.0);
            for (tab, label) in [
                (
                    WorkspaceTab::PipelineSteps,
                    format!("Pipeline Steps ({enabled_count}/{total_steps})"),
                ),
                (
                    WorkspaceTab::HistoryAndLogs,
                    format!(">_  Deployments & Logs ({history_count})"),
                ),
                (
                    WorkspaceTab::ServerSettings,
                    "Environment & SSH Settings".to_string(),
                ),
            ] {
                let is_active = self.active_tab == tab;
                let btn = egui::Button::new(
                    RichText::new(label).size(FONT_MD).strong().color(if is_active {
                        PRIMARY_BLUE
                    } else {
                        TEXT_SECONDARY
                    }),
                )
                .fill(if is_active {
                    CARD_BG
                } else {
                    Color32::TRANSPARENT
                })
                .stroke(if is_active {
                    Stroke::new(1.5_f32, Color32::from_rgb(147, 197, 253))
                } else {
                    Stroke::NONE
                })
                .corner_radius(8.0)
                .min_size(Vec2::new(175.0, 36.0));

                if ui.add(btn).clicked() {
                    self.active_tab = tab;
                }
            }
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
    }

    fn render_deploy_action_bar(
        &mut self,
        ui: &mut egui::Ui,
        env_display_name: &str,
        rollback_candidate: Option<DeploymentRecord>,
    ) {
        let mut trigger_deploy = false;
        let mut trigger_rollback = None;
        let avail_w = ui.available_width();

        let (bar_rect, _) = ui.allocate_exact_size(Vec2::new(avail_w, 78.0), Sense::hover());
        draw_elevated_card(
            ui.painter(),
            bar_rect,
            CARD_BG,
            Stroke::new(1.0_f32, CARD_BORDER),
        );

        ui.painter().text(
            Pos2::new(bar_rect.left() + 20.0, bar_rect.top() + 18.0),
            Align2::LEFT_CENTER,
            "VERSION CONTROL & RELEASE",
            FontId::proportional(FONT_XS),
            TEXT_MUTED,
        );

        let controls_rect = Rect::from_min_max(
            Pos2::new(bar_rect.left() + 20.0, bar_rect.top() + 30.0),
            Pos2::new(bar_rect.right() - 20.0, bar_rect.bottom() - 12.0),
        );

        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(controls_rect)
                .layout(Layout::left_to_right(Align::Center)),
        );

        styled_text_edit(
            &mut child,
            &mut self.deploy_version_input,
            92.0,
            Some("1.0.0"),
        );
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
                    .color(TEXT_SECONDARY),
            )
            .fill(SUBTLE_BG)
            .stroke(Stroke::new(1.0_f32, CARD_BORDER))
            .corner_radius(6.0)
            .min_size(Vec2::new(56.0, 32.0));
            if child.add(b).clicked() {
                self.deploy_version_input = bump_semver(&self.deploy_version_input, bump);
            }
        }

        child.add_space(12.0);
        let sep_x = child.cursor().min.x;
        child.painter().line_segment(
            [
                Pos2::new(sep_x, bar_rect.top() + 18.0),
                Pos2::new(sep_x, bar_rect.bottom() - 18.0),
            ],
            Stroke::new(1.0_f32, CARD_BORDER),
        );
        child.add_space(14.0);

        child.checkbox(
            &mut self.bump_package_json,
            RichText::new("Bump package.json")
                .size(FONT_MD)
                .strong()
                .color(TEXT_PRIMARY),
        );
        child.add_space(10.0);
        child.checkbox(
            &mut self.create_git_tag,
            RichText::new("Git Tag")
                .size(FONT_MD)
                .strong()
                .color(TEXT_SECONDARY),
        );

        // Right-aligned Deploy Button + Rollback Button (Feature #3)
        let is_busy = self.active_deployment_id.is_some();
        let btn_w = 236.0;
        let btn_h = 42.0;
        let btn_rect = Rect::from_min_size(
            Pos2::new(
                bar_rect.right() - 20.0 - btn_w,
                bar_rect.center().y - btn_h / 2.0,
            ),
            Vec2::new(btn_w, btn_h),
        );
        let btn_resp = ui.interact(btn_rect, egui::Id::new("primary_deploy_btn"), Sense::click());
        let btn_bg = if is_busy {
            TEXT_MUTED
        } else if btn_resp.hovered() {
            PRIMARY_BLUE_HOVER
        } else {
            PRIMARY_BLUE
        };
        ui.painter()
            .rect_filled(btn_rect, CornerRadius::same(8), btn_bg);

        draw_icon_send(
            ui.painter(),
            Pos2::new(btn_rect.left() + 24.0, btn_rect.center().y),
            Color32::WHITE,
        );
        let btn_text = if is_busy {
            "Deploying...".to_string()
        } else {
            format!(
                "Deploy to {} (v{})",
                env_display_name,
                self.deploy_version_input.trim()
            )
        };
        ui.painter().text(
            Pos2::new(btn_rect.center().x + 10.0, btn_rect.center().y),
            Align2::CENTER_CENTER,
            btn_text,
            FontId::proportional(FONT_MD),
            Color32::WHITE,
        );

        if !is_busy && btn_resp.clicked() {
            trigger_deploy = true;
        }

        // Quick Rollback Button if a previous succeeded deployment exists
        if let Some(prev_dep) = rollback_candidate {
            let rb_w = 148.0;
            let rb_rect = Rect::from_min_size(
                Pos2::new(
                    btn_rect.left() - 10.0 - rb_w,
                    bar_rect.center().y - btn_h / 2.0,
                ),
                Vec2::new(rb_w, btn_h),
            );
            let rb_resp = ui.interact(
                rb_rect,
                egui::Id::new("quick_rollback_btn"),
                Sense::click(),
            );
            ui.painter().rect_filled(
                rb_rect,
                CornerRadius::same(8),
                if rb_resp.hovered() && !is_busy {
                    WARN_PILL_BG
                } else {
                    SUBTLE_BG
                },
            );
            ui.painter().rect_stroke(
                rb_rect,
                CornerRadius::same(8),
                Stroke::new(1.0_f32, WARN_PILL_TEXT),
                StrokeKind::Inside,
            );
            ui.painter().text(
                rb_rect.center(),
                Align2::CENTER_CENTER,
                format!("Rollback v{}", prev_dep.version),
                FontId::proportional(FONT_SM),
                WARN_PILL_TEXT,
            );
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
    fn render_pipeline_steps_tab(&mut self, ui: &mut egui::Ui, proj_idx: usize) {
        let env_key = self.selected_env_key.clone();
        let mut config_changed = false;

        let env_name = self.storage.projects[proj_idx]
            .get_env(&env_key)
            .map(|e| e.name.clone())
            .unwrap_or_else(|| env_key.clone());

        horizontal_row(ui, 34.0, |ui| {
            ui.label(
                RichText::new(format!("{} PIPELINE STEPS", env_name.to_uppercase()))
                    .size(FONT_MD)
                    .strong()
                    .color(TEXT_PRIMARY),
            );
            ui.label(
                RichText::new(
                    "— Variables: {remote_dir}, {remote_user}, {app_name}, {http_proxy}, {version}",
                )
                .size(FONT_SM)
                .color(TEXT_MUTED),
            );

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let project = &mut self.storage.projects[proj_idx];
                if let Some(env_cfg) = project.get_env_mut(&env_key) {
                    let remote_btn = egui::Button::new(
                        RichText::new("+ Add Remote SSH")
                            .size(FONT_SM)
                            .strong()
                            .color(PRIMARY_BLUE),
                    )
                    .fill(CARD_BG)
                    .stroke(Stroke::new(1.0_f32, CARD_BORDER));
                    if ui.add(remote_btn).clicked() {
                        env_cfg.steps.push(PipelineStep::new(
                            "Custom Remote Step",
                            StepKind::RemoteCommand {
                                command: "cd {remote_dir} && ls -la".to_string(),
                            },
                        ));
                        config_changed = true;
                    }

                    let local_btn = egui::Button::new(
                        RichText::new("+ Add Local Step")
                            .size(FONT_SM)
                            .strong()
                            .color(TEXT_SECONDARY),
                    )
                    .fill(CARD_BG)
                    .stroke(Stroke::new(1.0_f32, CARD_BORDER));
                    if ui.add(local_btn).clicked() {
                        env_cfg.steps.push(PipelineStep::new(
                            "Custom Local Step",
                            StepKind::LocalCommand {
                                command: "npm run lint".to_string(),
                                allow_failure: false,
                            },
                        ));
                        config_changed = true;
                    }
                }
            });
        });

        ui.add_space(10.0);

        let project = &mut self.storage.projects[proj_idx];
        let Some(env_cfg) = project.get_env_mut(&env_key) else {
            return;
        };

        let mut move_up_idx = None;
        let mut move_down_idx = None;
        let mut delete_idx = None;
        let total_steps = env_cfg.steps.len();

        for (idx, step) in env_cfg.steps.iter_mut().enumerate() {
            let card_fill = if step.enabled { CARD_BG } else { SUBTLE_BG };
            egui::Frame::new()
                .fill(card_fill)
                .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                .corner_radius(10.0)
                .inner_margin(Margin::symmetric(16, 12))
                .show(ui, |ui| {
                    // Row 1: Enable Checkbox + Step # + Type Pill + Step Name + Option Checkbox | Up / Down / Delete Buttons
                    horizontal_row(ui, 32.0, |ui| {
                        if ui.checkbox(&mut step.enabled, "").changed() {
                            config_changed = true;
                        }
                        ui.label(
                            RichText::new(format!("#{:02}", idx + 1))
                                .size(FONT_SM)
                                .strong()
                                .color(TEXT_MUTED),
                        );

                        let (type_label, bg, fg) = match &step.kind {
                            StepKind::LocalCommand { .. } => {
                                ("LOCAL", BLUE_PILL_BG, BLUE_PILL_TEXT)
                            }
                            StepKind::VerifyLocalPath { .. } => {
                                ("VERIFY", WARN_PILL_BG, WARN_PILL_TEXT)
                            }
                            StepKind::TransferFiles { .. } => {
                                ("SCP UPLOAD", PURPLE_PILL_BG, PURPLE_PILL_TEXT)
                            }
                            StepKind::RemoteCommand { .. } => {
                                ("REMOTE SSH", TEAL_PILL_BG, TEAL_PILL_TEXT)
                            }
                        };
                        render_pill(ui, type_label, bg, fg);

                        if styled_text_edit(
                            ui,
                            &mut step.name,
                            230.0,
                            Some("Step name (e.g. Build Application)"),
                        ) {
                            config_changed = true;
                        }

                        // Step-type specific toggle placed cleanly in Row 1 with plenty of space
                        match &mut step.kind {
                            StepKind::LocalCommand { allow_failure, .. } => {
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

                        // Right-aligned Reorder & Delete actions (never overlaps!)
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                            let del_btn = egui::Button::new(
                                RichText::new("Delete")
                                    .size(FONT_SM)
                                    .strong()
                                    .color(FAIL_PILL_TEXT),
                            )
                            .fill(FAIL_PILL_BG)
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(252, 165, 165)))
                            .corner_radius(6.0)
                            .min_size(Vec2::new(58.0, 28.0));
                            if ui.add(del_btn).on_hover_text("Delete step").clicked() {
                                delete_idx = Some(idx);
                            }

                            let can_down = idx + 1 < total_steps;
                            let down_btn = egui::Button::new(
                                RichText::new("Down")
                                    .size(FONT_SM)
                                    .strong()
                                    .color(if can_down {
                                        TEXT_SECONDARY
                                    } else {
                                        TEXT_PLACEHOLDER
                                    }),
                            )
                            .fill(SUBTLE_BG)
                            .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                            .corner_radius(6.0)
                            .min_size(Vec2::new(50.0, 28.0));
                            if ui.add(down_btn).on_hover_text("Move step down").clicked()
                                && can_down
                            {
                                move_down_idx = Some(idx);
                            }

                            let can_up = idx > 0;
                            let up_btn = egui::Button::new(
                                RichText::new("Up")
                                    .size(FONT_SM)
                                    .strong()
                                    .color(if can_up {
                                        TEXT_SECONDARY
                                    } else {
                                        TEXT_PLACEHOLDER
                                    }),
                            )
                            .fill(SUBTLE_BG)
                            .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                            .corner_radius(6.0)
                            .min_size(Vec2::new(42.0, 28.0));
                            if ui.add(up_btn).on_hover_text("Move step up").clicked() && can_up {
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
                                "e.g. ./dist/server.js",
                            ),
                            StepKind::TransferFiles { .. } => (
                                "SCP Sources:",
                                "e.g. ./dist, ./package.json, ./package-lock.json",
                            ),
                            StepKind::RemoteCommand { .. } => (
                                "Remote SSH:",
                                "e.g. /data/{remote_user}/nodejs/bin/pm2 restart {app_name}",
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
                                if styled_text_edit(ui, command, input_w, Some(placeholder)) {
                                    config_changed = true;
                                }
                            }
                            StepKind::VerifyLocalPath { path } => {
                                if styled_text_edit(ui, path, input_w, Some(placeholder)) {
                                    config_changed = true;
                                }
                            }
                            StepKind::TransferFiles { sources, .. } => {
                                let mut joined = sources.join(", ");
                                if styled_text_edit(ui, &mut joined, input_w, Some(placeholder)) {
                                    *sources = joined
                                        .split(',')
                                        .map(|s| s.trim().to_string())
                                        .filter(|s| !s.is_empty())
                                        .collect();
                                    config_changed = true;
                                }
                            }
                            StepKind::RemoteCommand { command } => {
                                if styled_text_edit(ui, command, input_w, Some(placeholder)) {
                                    config_changed = true;
                                }
                            }
                        }
                    });
                });
            ui.add_space(8.0);
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
            let _ = self.storage.save();
        }
    }

    fn render_server_settings_tab(&mut self, ui: &mut egui::Ui, proj_idx: usize) {
        let env_key = self.selected_env_key.clone();
        let mut config_changed = false;
        let mut delete_current_env = false;
        let field_width = 480.0;

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

            // Feature #5: Always allow deleting the environment (even if only 1 environment exists)
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let del_btn = egui::Button::new(
                    RichText::new("Delete Environment")
                        .size(FONT_SM)
                        .strong()
                        .color(FAIL_PILL_TEXT),
                )
                .fill(FAIL_PILL_BG)
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(252, 165, 165)))
                .corner_radius(6.0);
                if ui.add(del_btn).clicked() {
                    delete_current_env = true;
                }
            });
        });

        ui.add_space(10.0);

        egui::Frame::new()
            .fill(CARD_BG)
            .stroke(Stroke::new(1.0_f32, CARD_BORDER))
            .corner_radius(10.0)
            .inner_margin(Margin::symmetric(22, 20))
            .show(ui, |ui| {
                let project = &mut self.storage.projects[proj_idx];
                egui::Grid::new("clean_server_settings_grid")
                    .num_columns(2)
                    .spacing([28.0, 14.0])
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("Local Project Directory:")
                                .size(FONT_MD)
                                .color(TEXT_SECONDARY)
                                .strong(),
                        );
                        if styled_text_edit(
                            ui,
                            &mut project.local_path,
                            field_width,
                            Some(r"e.g. D:\NTI\Projects\backend\ndm-dq-subscription"),
                        ) {
                            config_changed = true;
                        }
                        ui.end_row();

                        if let Some(env_cfg) = project.get_env_mut(&env_key) {
                            ui.label(
                                RichText::new("Environment Display Name:")
                                    .size(FONT_MD)
                                    .color(TEXT_SECONDARY)
                                    .strong(),
                            );
                            if styled_text_edit(
                                ui,
                                &mut env_cfg.name,
                                field_width,
                                Some("e.g. Development, Staging, Production"),
                            ) {
                                config_changed = true;
                            }
                            ui.end_row();

                            ui.label(
                                RichText::new("Application Name (PM2 / Dir):")
                                    .size(FONT_MD)
                                    .color(TEXT_SECONDARY)
                                    .strong(),
                            );
                            if styled_text_edit(
                                ui,
                                &mut env_cfg.app_name,
                                field_width,
                                Some("e.g. ndm-dq-subscription-dev"),
                            ) {
                                config_changed = true;
                            }
                            ui.end_row();

                            ui.label(
                                RichText::new("Target Servers (comma-separated):")
                                    .size(FONT_MD)
                                    .color(TEXT_SECONDARY)
                                    .strong(),
                            );
                            let mut servers_joined = env_cfg.servers.join(", ");
                            if styled_text_edit(
                                ui,
                                &mut servers_joined,
                                field_width,
                                Some("e.g. 10.62.70.80, 10.62.70.81"),
                            ) {
                                env_cfg.servers = servers_joined
                                    .split(',')
                                    .map(|s| s.trim().to_string())
                                    .filter(|s| !s.is_empty())
                                    .collect();
                                config_changed = true;
                            }
                            ui.end_row();

                            ui.label(
                                RichText::new("Remote Directory Path:")
                                    .size(FONT_MD)
                                    .color(TEXT_SECONDARY)
                                    .strong(),
                            );
                            if styled_text_edit(
                                ui,
                                &mut env_cfg.remote_dir,
                                field_width,
                                Some("e.g. /data/indy-service/apps/ndm-dq-subscription-dev"),
                            ) {
                                config_changed = true;
                            }
                            ui.end_row();

                            ui.label(
                                RichText::new("Remote SSH Username:")
                                    .size(FONT_MD)
                                    .color(TEXT_SECONDARY)
                                    .strong(),
                            );
                            if styled_text_edit(
                                ui,
                                &mut env_cfg.remote_user,
                                field_width,
                                Some("e.g. appndq"),
                            ) {
                                config_changed = true;
                            }
                            ui.end_row();

                            ui.label(
                                RichText::new("OS Credential Vault Target:")
                                    .size(FONT_MD)
                                    .color(TEXT_SECONDARY)
                                    .strong(),
                            );
                            if styled_text_edit(
                                ui,
                                &mut env_cfg.vault_target,
                                field_width,
                                Some("e.g. indyserver"),
                            ) {
                                config_changed = true;
                            }
                            ui.end_row();

                            ui.label(
                                RichText::new("SSH Password Override:")
                                    .size(FONT_MD)
                                    .color(TEXT_SECONDARY)
                                    .strong(),
                            );
                            ui.add(
                                egui::TextEdit::singleline(&mut self.manual_ssh_password)
                                    .password(true)
                                    .desired_width(field_width)
                                    .margin(Margin::symmetric(10, 7))
                                    .hint_text(
                                        RichText::new(
                                            "Optional — leave blank to use OS Vault ('indyserver') or SSH Key",
                                        )
                                        .size(FONT_MD)
                                        .color(TEXT_PLACEHOLDER),
                                    ),
                            );
                            ui.end_row();

                            ui.label(
                                RichText::new("Environment File Source (.env):")
                                    .size(FONT_MD)
                                    .color(TEXT_SECONDARY)
                                    .strong(),
                            );
                            let mut env_src = env_cfg.env_source.clone().unwrap_or_default();
                            if styled_text_edit(
                                ui,
                                &mut env_src,
                                field_width,
                                Some("Optional — e.g. ./.env.dev or ./.env.prod"),
                            ) {
                                env_cfg.env_source = if env_src.trim().is_empty() {
                                    None
                                } else {
                                    Some(env_src.trim().to_string())
                                };
                                config_changed = true;
                            }
                            ui.end_row();

                            ui.label(
                                RichText::new("Outbound HTTP Proxy:")
                                    .size(FONT_MD)
                                    .color(TEXT_SECONDARY)
                                    .strong(),
                            );
                            let mut proxy = env_cfg.http_proxy.clone().unwrap_or_default();
                            if styled_text_edit(
                                ui,
                                &mut proxy,
                                field_width,
                                Some("Optional — e.g. http://10.54.18.42:8080"),
                            ) {
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
                self.selected_env_key = self.storage.projects[proj_idx]
                    .environments
                    .first()
                    .map(|e| e.key.clone())
                    .unwrap_or_default();
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

        // Section Header + Filter Pills (Fixed 34px height)
        horizontal_row(ui, 34.0, |ui| {
            ui.label(
                RichText::new("DEPLOYMENT HISTORY")
                    .size(FONT_MD)
                    .strong()
                    .color(TEXT_PRIMARY),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                for (label, filter_val) in filter_options.into_iter().rev() {
                    let is_active = self.history_env_filter == filter_val;
                    let btn = egui::Button::new(
                        RichText::new(label).size(FONT_SM).strong().color(if is_active {
                            Color32::WHITE
                        } else {
                            TEXT_SECONDARY
                        }),
                    )
                    .fill(if is_active { TEXT_PRIMARY } else { CARD_BG })
                    .stroke(Stroke::new(
                        1.0_f32,
                        if is_active { TEXT_PRIMARY } else { CARD_BORDER },
                    ))
                    .corner_radius(6.0);
                    if ui.add(btn).clicked() {
                        self.history_env_filter = filter_val;
                    }
                }
            });
        });

        ui.add_space(10.0);

        let filtered: Vec<DeploymentRecord> = self
            .storage
            .filter_deployments(project_filter, self.history_env_filter.as_deref(), None)
            .into_iter()
            .cloned()
            .collect();

        let mut redeploy_target: Option<(DeploymentRecord, bool)> = None;

        // Structured Deployment History Table Card
        egui::Frame::new()
            .fill(CARD_BG)
            .stroke(Stroke::new(1.0_f32, CARD_BORDER))
            .corner_radius(10.0)
            .inner_margin(Margin::symmetric(20, 14))
            .show(ui, |ui| {
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

                if filtered.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        ui.label(
                            RichText::new("No deployments recorded yet.")
                                .size(FONT_MD)
                                .color(TEXT_MUTED),
                        );
                        ui.add_space(20.0);
                    });
                }

                for (idx, dep) in filtered.iter().enumerate() {
                    if idx > 0 {
                        draw_divider(ui, SUBTLE_BG);
                    }
                    ui.add_space(6.0);
                    let is_selected = self.selected_deployment_id.as_deref() == Some(&dep.id);
                    let row_h = 44.0;

                    horizontal_row(ui, row_h, |ui| {
                        // STATUS column
                        let (s_rect, _) =
                            ui.allocate_exact_size(Vec2::new(110.0, row_h), Sense::hover());
                        let (s_bg, s_fg) = status_pill_colors(dep.status);
                        paint_pill_left_aligned(
                            ui.painter(),
                            Pos2::new(s_rect.left(), s_rect.center().y),
                            &dep.status.label().to_uppercase(),
                            s_bg,
                            s_fg,
                        );

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
                                    v_rect.top() + 13.0
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
                                Pos2::new(v_rect.left(), v_rect.top() + 31.0),
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
                            Pos2::new(d_rect.left() + 6.0, d_rect.top() + 12.0),
                            TEXT_MUTED,
                        );
                        let git_line = match (&dep.git_branch, &dep.git_commit) {
                            (Some(b), Some(c)) => format!("{b} @ {c}"),
                            _ => dep.project_name.clone(),
                        };
                        ui.painter().text(
                            Pos2::new(d_rect.left() + 18.0, d_rect.top() + 12.0),
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
                            Pos2::new(d_rect.left() + 6.0, d_rect.top() + 31.0),
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
                            Pos2::new(t_rect.left(), t_rect.top() + 12.0),
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
                            Pos2::new(t_rect.left(), t_rect.top() + 31.0),
                            Align2::LEFT_CENTER,
                            dur_str,
                            FontId::proportional(FONT_SM),
                            TEXT_MUTED,
                        );

                        // ACTIONS column (Rollback + Re-run + View Logs)
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                            let is_busy = self.active_deployment_id.is_some();

                            let (red_rect, red_resp) =
                                ui.allocate_exact_size(Vec2::new(28.0, 28.0), Sense::click());
                            if red_resp.hovered() && !is_busy {
                                ui.painter().rect_filled(
                                    red_rect,
                                    CornerRadius::same(6),
                                    PILL_CONTAINER_BG,
                                );
                            }
                            draw_icon_refresh(ui.painter(), red_rect.center(), TEXT_SECONDARY);
                            if !is_busy
                                && red_resp
                                    .on_hover_text("Re-run this deployment version and pipeline")
                                    .clicked()
                            {
                                redeploy_target = Some((dep.clone(), false));
                            }

                            if dep.status == DeploymentStatus::Succeeded {
                                let rb_btn = egui::Button::new(
                                    RichText::new("Rollback")
                                        .size(FONT_SM)
                                        .strong()
                                        .color(WARN_PILL_TEXT),
                                )
                                .fill(WARN_PILL_BG)
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(251, 191, 36)))
                                .corner_radius(6.0);
                                if !is_busy
                                    && ui
                                        .add(rb_btn)
                                        .on_hover_text(format!(
                                            "Rollback {} to v{}",
                                            dep.environment, dep.version
                                        ))
                                        .clicked()
                                {
                                    redeploy_target = Some((dep.clone(), true));
                                }
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
            });

        if let Some((target, is_rb)) = redeploy_target {
            self.trigger_redeploy_from_record(&target, is_rb);
        }

        ui.add_space(18.0);

        let selected_dep = self
            .selected_deployment_id
            .as_ref()
            .and_then(|id| self.storage.deployments.iter().find(|d| &d.id == id))
            .or_else(|| filtered.first())
            .cloned();

        if let Some(dep) = selected_dep {
            // Feature #10: Collapsible CODE CHANGES (GIT COMMITS) Panel
            if !dep.git_changes.is_empty() {
                egui::Frame::new()
                    .fill(CARD_BG)
                    .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                    .corner_radius(10.0)
                    .inner_margin(Margin::symmetric(18, 12))
                    .show(ui, |ui| {
                        horizontal_row(ui, 26.0, |ui| {
                            let (chev_r, chev_resp) =
                                ui.allocate_exact_size(Vec2::new(18.0, 18.0), Sense::click());
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
                                RichText::new(format!(
                                    "CODE CHANGES IN v{} ({} entry/entries)",
                                    dep.version,
                                    dep.git_changes.len()
                                ))
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
                            for change_line in &dep.git_changes {
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

            // Collapsible CONSOLE OUTPUT Section (Fixed 32px header row)
            horizontal_row(ui, 32.0, |ui| {
                let (chev_r, chev_resp) =
                    ui.allocate_exact_size(Vec2::new(18.0, 18.0), Sense::click());
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

    fn render_terminal_window(&mut self, ui: &mut egui::Ui, dep: &DeploymentRecord) {
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
                            .interact(Sense::click());

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
                    });
            });
    }
}

// =============================================================================
// Native OS File / Folder Dialogs & Helpers
// =============================================================================

fn pick_ps1_file_dialog() -> Option<String> {
    #[cfg(windows)]
    {
        let script = r#"
Add-Type -AssemblyName System.Windows.Forms
$dlg = New-Object System.Windows.Forms.OpenFileDialog
$dlg.Filter = "PowerShell Scripts (*.ps1)|*.ps1|All Files (*.*)|*.*"
$dlg.Title = "Select PowerShell Deployment Script (.ps1)"
if ($dlg.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
    Write-Output $dlg.FileName
}
"#;
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-Sta", "-Command", script]);
        configure_background_command(&mut cmd);
        let out = cmd.output().ok()?;
        let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if path.is_empty() {
            None
        } else {
            Some(path)
        }
    }
    #[cfg(not(windows))]
    {
        None
    }
}

fn pick_folder_dialog() -> Option<String> {
    #[cfg(windows)]
    {
        let script = r#"
Add-Type -AssemblyName System.Windows.Forms
$dlg = New-Object System.Windows.Forms.FolderBrowserDialog
$dlg.Description = "Select Local Project Directory"
if ($dlg.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
    Write-Output $dlg.SelectedPath
}
"#;
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-Sta", "-Command", script]);
        configure_background_command(&mut cmd);
        let out = cmd.output().ok()?;
        let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if path.is_empty() {
            None
        } else {
            Some(path)
        }
    }
    #[cfg(not(windows))]
    {
        None
    }
}

fn open_directory_in_os(dir: &Path) {
    #[cfg(windows)]
    {
        let _ = Command::new("explorer").arg(dir).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(dir).spawn();
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let _ = Command::new("xdg-open").arg(dir).spawn();
    }
}

// =============================================================================
// Bounded Layout Helpers & Fixed-Height Pill/Icon Painters (Never Stretches!)
// =============================================================================

fn horizontal_row<R>(
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

/// Consistent text input field with grey fill (`INPUT_BG`), visible border on focus,
/// structured proportional font (`FONT_MD`), and grey placeholder (`TEXT_PLACEHOLDER`).
fn styled_text_edit(
    ui: &mut egui::Ui,
    value: &mut String,
    width: f32,
    hint: Option<&str>,
) -> bool {
    let placeholder_str = hint.unwrap_or("Enter value...");
    let edit = egui::TextEdit::singleline(value)
        .font(FontId::proportional(FONT_MD))
        .background_color(INPUT_BG)
        .desired_width(width)
        .margin(Margin::symmetric(10, 7))
        .hint_text(
            RichText::new(placeholder_str)
                .size(FONT_MD)
                .color(TEXT_PLACEHOLDER),
        );
    ui.add(edit).changed()
}

fn draw_elevated_card(painter: &egui::Painter, rect: Rect, fill: Color32, stroke: Stroke) {
    let shadow_rect = rect.translate(Vec2::new(0.0, 2.0));
    painter.rect_filled(
        shadow_rect,
        CornerRadius::same(10),
        Color32::from_black_alpha(8),
    );
    painter.rect_filled(rect, CornerRadius::same(10), fill);
    painter.rect_stroke(rect, CornerRadius::same(10), stroke, StrokeKind::Inside);
}

fn draw_divider(ui: &mut egui::Ui, color: Color32) {
    let avail_w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(avail_w, 1.0), Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(1.0_f32, color),
    );
}

fn render_pill(ui: &mut egui::Ui, text: &str, bg: Color32, fg: Color32) {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), FontId::proportional(FONT_XS), fg);
    let pill_w = galley.size().x + 16.0;
    let pill_h = 22.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(pill_w, pill_h), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(5), bg);
    ui.painter().galley(
        Pos2::new(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        fg,
    );
}

fn paint_pill_left_aligned(
    painter: &egui::Painter,
    left_center: Pos2,
    text: &str,
    bg: Color32,
    fg: Color32,
) -> f32 {
    let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(FONT_XS), fg);
    let w = galley.size().x + 16.0;
    let h = 22.0;
    let rect = Rect::from_min_size(
        Pos2::new(left_center.x, left_center.y - h / 2.0),
        Vec2::new(w, h),
    );
    painter.rect_filled(rect, CornerRadius::same(5), bg);
    painter.galley(
        Pos2::new(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        fg,
    );
    w
}

fn paint_pill_right_aligned(
    painter: &egui::Painter,
    right_center: Pos2,
    text: &str,
    bg: Color32,
    fg: Color32,
) -> f32 {
    let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(FONT_XS), fg);
    let w = galley.size().x + 16.0;
    let h = 22.0;
    let rect = Rect::from_min_size(
        Pos2::new(right_center.x - w, right_center.y - h / 2.0),
        Vec2::new(w, h),
    );
    painter.rect_filled(rect, CornerRadius::same(5), bg);
    painter.galley(
        Pos2::new(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        fg,
    );
    w
}

fn paint_small_sidebar_pill(
    painter: &egui::Painter,
    left_center: Pos2,
    text: &str,
    bg: Color32,
    fg: Color32,
) -> f32 {
    let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(FONT_XS), fg);
    let w = galley.size().x + 12.0;
    let h = 19.0;
    let rect = Rect::from_min_size(
        Pos2::new(left_center.x, left_center.y - h / 2.0),
        Vec2::new(w, h),
    );
    painter.rect_filled(rect, CornerRadius::same(4), bg);
    painter.galley(
        Pos2::new(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        fg,
    );
    w
}

fn status_pill_colors(status: DeploymentStatus) -> (Color32, Color32) {
    match status {
        DeploymentStatus::Succeeded => (SUCCESS_PILL_BG, SUCCESS_PILL_TEXT),
        DeploymentStatus::Failed => (FAIL_PILL_BG, FAIL_PILL_TEXT),
        DeploymentStatus::Running => (WARN_PILL_BG, WARN_PILL_TEXT),
        DeploymentStatus::Cancelled => (PILL_CONTAINER_BG, TEXT_SECONDARY),
    }
}

fn render_light_status_badge(ui: &mut egui::Ui, status: DeploymentStatus) {
    let (bg, fg) = status_pill_colors(status);
    render_pill(ui, &status.label().to_uppercase(), bg, fg);
}

fn env_pastel_pill_colors(env_key: &str) -> (Color32, Color32) {
    match env_key.to_lowercase().as_str() {
        "dev" | "development" => (BLUE_PILL_BG, BLUE_PILL_TEXT),
        "staging" | "stage" | "stg" => (WARN_PILL_BG, WARN_PILL_TEXT),
        "prod" | "production" => (PURPLE_PILL_BG, PURPLE_PILL_TEXT),
        _ => (TEAL_PILL_BG, TEAL_PILL_TEXT),
    }
}

// Vector Icon Painters
fn draw_icon_layers(painter: &egui::Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(1.5_f32, color);
    painter.line_segment([Pos2::new(c.x - 6.0, c.y - 3.0), Pos2::new(c.x, c.y - 6.0)], s);
    painter.line_segment([Pos2::new(c.x, c.y - 6.0), Pos2::new(c.x + 6.0, c.y - 3.0)], s);
    painter.line_segment([Pos2::new(c.x + 6.0, c.y - 3.0), Pos2::new(c.x, c.y)], s);
    painter.line_segment([Pos2::new(c.x, c.y), Pos2::new(c.x - 6.0, c.y - 3.0)], s);

    painter.line_segment([Pos2::new(c.x - 6.0, c.y + 1.0), Pos2::new(c.x, c.y + 4.0)], s);
    painter.line_segment([Pos2::new(c.x, c.y + 4.0), Pos2::new(c.x + 6.0, c.y + 1.0)], s);
}

fn draw_icon_git_branch(painter: &egui::Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(1.4_f32, color);
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

fn draw_icon_server(painter: &egui::Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(1.3_f32, color);
    let top_r = Rect::from_center_size(Pos2::new(c.x, c.y - 2.5), Vec2::new(11.0, 4.0));
    let bot_r = Rect::from_center_size(Pos2::new(c.x, c.y + 2.5), Vec2::new(11.0, 4.0));
    painter.rect_stroke(top_r, CornerRadius::same(1), s, StrokeKind::Inside);
    painter.rect_stroke(bot_r, CornerRadius::same(1), s, StrokeKind::Inside);
}

fn draw_icon_folder(painter: &egui::Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(1.3_f32, color);
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

fn draw_icon_refresh(painter: &egui::Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(1.5_f32, color);
    painter.circle_stroke(c, 5.0, s);
    painter.line_segment(
        [Pos2::new(c.x + 2.5, c.y - 5.0), Pos2::new(c.x + 5.5, c.y - 4.5)],
        s,
    );
    painter.line_segment(
        [Pos2::new(c.x + 5.5, c.y - 4.5), Pos2::new(c.x + 4.5, c.y - 1.5)],
        s,
    );
}

fn draw_icon_plus(painter: &egui::Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(1.8_f32, color);
    painter.line_segment([Pos2::new(c.x - 6.0, c.y), Pos2::new(c.x + 6.0, c.y)], s);
    painter.line_segment([Pos2::new(c.x, c.y - 6.0), Pos2::new(c.x, c.y + 6.0)], s);
}

fn draw_icon_send(painter: &egui::Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(1.6_f32, color);
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

fn draw_icon_chevron(painter: &egui::Painter, c: Pos2, expanded: bool, color: Color32) {
    let s = Stroke::new(1.6_f32, color);
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

fn draw_icon_check(painter: &egui::Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(1.8_f32, color);
    painter.line_segment(
        [Pos2::new(c.x - 4.0, c.y + 0.5), Pos2::new(c.x - 1.0, c.y + 3.5)],
        s,
    );
    painter.line_segment(
        [Pos2::new(c.x - 1.0, c.y + 3.5), Pos2::new(c.x + 4.5, c.y - 3.0)],
        s,
    );
}
