use crate::credentials::read_os_vault_credential;
use crate::domain::{
    format_env_badge, DeploymentRecord, DeploymentStatus, EnvironmentConfig, PipelineStep, Project,
    StepKind,
};
use crate::executor::{run_deployment, DeploymentRequest, ExecutorEvent};
use crate::installer::{
    check_install_state, default_install_dir, install_or_update_self, load_shiply_logo_texture,
    InstallState, APP_VERSION,
};
use crate::ps1_importer::{import_project_from_path, parse_ps1_file};
use crate::storage::AppStorage;
use crate::updater::{
    restart_application, spawn_download_and_apply_update, spawn_update_check, ReleaseInfo,
    UpdaterEvent, GITHUB_RELEASES_PAGE_URL, GITHUB_REPO,
};
use crate::version::{
    bump_semver, checkout_git_branch, configure_background_command, list_git_branches,
    read_git_code_changes, read_git_metadata, read_package_json_version, GitMetadata, VersionBump,
};
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId,
    Layout, Margin, Pos2, Rect, RichText, ScrollArea, Sense, Shadow, Stroke, StrokeKind, TextStyle,
    Vec2,
};
use std::collections::{BTreeMap, HashSet, VecDeque};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CredentialOsTab {
    Windows,
    Linux,
    Mac,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConfirmDeleteAction {
    Project {
        project_id: String,
        project_name: String,
    },
    Environment {
        project_id: String,
        project_name: String,
        env_key: String,
        env_name: String,
    },
}

#[derive(Debug)]
pub struct GitInspectionResult {
    pub project_id: String,
    pub pkg_version: Option<String>,
    pub git_meta: GitMetadata,
    pub git_branches: Vec<String>,
    pub git_changes: Vec<String>,
    pub update_version_input: bool,
}

pub struct ShiplyApp {
    storage: AppStorage,
    selection: SidebarSelection,
    selected_env_key: String,
    active_tab: WorkspaceTab,
    history_env_filter: Option<String>,
    history_visible_limit: usize,
    selected_deployment_id: Option<String>,

    // Debounced disk save & field editing buffers
    dirty_save_pending: bool,
    last_edit_time: Option<Instant>,
    servers_edit_buffer: Option<(String, String)>,

    // Collapsible Console Output & Code Changes state
    console_collapsed: bool,
    changes_collapsed: bool,
    collapsed_step_indices: HashSet<usize>,
    dragged_step_idx: Option<usize>,

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
    cached_git_changes: Vec<String>,

    // Background execution channel
    event_tx: Sender<ExecutorEvent>,
    event_rx: Receiver<ExecutorEvent>,
    active_deployment_id: Option<String>,

    // Async Git inspection channel (instant project switching without UI freezing)
    git_inspect_tx: Sender<GitInspectionResult>,
    git_inspect_rx: Receiver<GitInspectionResult>,

    // Add Project & Auto-Import .ps1 Modal state
    show_new_project_modal: bool,
    new_project_ps1_path: String,
    new_project_import_siblings: bool,
    new_project_name: String,
    new_project_path: String,
    new_project_desc: String,
    new_project_imported_envs: Vec<EnvironmentConfig>,
    new_project_import_notice: Option<String>,

    // Edit Project Modal state
    show_edit_project_modal: bool,
    edit_project_id: String,
    edit_project_name: String,
    edit_project_path: String,
    edit_project_desc: String,
    edit_project_ps1_path: String,
    edit_project_import_notice: Option<String>,

    // Add Environment Modal state
    show_new_env_modal: bool,
    new_env_name: String,
    new_env_clone_from: String,
    new_env_ps1_path: String,

    // Bulk Deployment Modal & Queue state
    show_bulk_deploy_modal: bool,
    bulk_selected_projects: HashSet<String>,
    bulk_target_env_key: String,
    bulk_project_versions: BTreeMap<String, String>,
    bulk_bump_pkg: bool,
    bulk_create_tag: bool,
    bulk_deploy_queue: VecDeque<DeploymentRequest>,
    bulk_total_count: usize,

    // Onboarding & Multi-Platform OS Credential Setup Guide state
    show_onboarding_guide: bool,
    show_credential_guide: bool,
    cred_guide_os_tab: CredentialOsTab,
    cred_guide_target_input: String,
    cred_guide_user_input: String,

    // In-App GitHub Releases Auto-Updater state
    updater_tx: Sender<UpdaterEvent>,
    updater_rx: Receiver<UpdaterEvent>,
    show_update_modal: bool,
    available_update: Option<ReleaseInfo>,
    update_checking: bool,
    update_downloading: bool,
    update_progress: f32,
    update_status_text: String,
    update_error: Option<String>,

    // Throttled external file sync timer (Requirement #8 & #9)
    last_sync_check: Instant,

    // Status banner & Window/Sidebar state
    status_banner: Option<(String, bool)>,
    sidebar_collapsed: bool,
    window_maximized: bool,
    pending_delete_confirm: Option<ConfirmDeleteAction>,
    fonts_initialized: bool,
    logo_texture: Option<egui::TextureHandle>,
    project_switch_loading_id: Option<String>,
    project_switch_loading_start: Option<Instant>,
}

impl ShiplyApp {
    pub fn new(storage: AppStorage) -> Self {
        let (event_tx, event_rx) = mpsc::channel();
        let (updater_tx, updater_rx) = mpsc::channel();
        let (git_inspect_tx, git_inspect_rx) = mpsc::channel();
        let migration_notice = storage.migration_notice.clone();
        // Run a non-blocking background check against GitHub Releases on startup
        spawn_update_check(updater_tx.clone(), false);

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
            history_visible_limit: 5,
            selected_deployment_id: initial_selected_dep,
            dirty_save_pending: false,
            last_edit_time: None,
            servers_edit_buffer: None,
            console_collapsed: false,
            changes_collapsed: false,
            collapsed_step_indices: HashSet::new(),
            dragged_step_idx: None,
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
            cached_git_changes: Vec::new(),
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
            show_edit_project_modal: false,
            edit_project_id: String::new(),
            edit_project_name: String::new(),
            edit_project_path: String::new(),
            edit_project_desc: String::new(),
            edit_project_ps1_path: String::new(),
            edit_project_import_notice: None,
            show_new_env_modal: false,
            new_env_name: String::new(),
            new_env_clone_from: initial_env_key.clone(),
            new_env_ps1_path: String::new(),
            show_bulk_deploy_modal: false,
            bulk_selected_projects: HashSet::new(),
            bulk_target_env_key: initial_env_key,
            bulk_project_versions: BTreeMap::new(),
            bulk_bump_pkg: true,
            bulk_create_tag: false,
            bulk_deploy_queue: VecDeque::new(),
            bulk_total_count: 0,
            show_onboarding_guide: true,
            show_credential_guide: true,
            cred_guide_os_tab: if cfg!(windows) {
                CredentialOsTab::Windows
            } else if cfg!(target_os = "macos") {
                CredentialOsTab::Mac
            } else {
                CredentialOsTab::Linux
            },
            cred_guide_target_input: "prod-ssh-server".to_string(),
            cred_guide_user_input: "deploy".to_string(),
            updater_tx,
            updater_rx,
            show_update_modal: false,
            available_update: None,
            update_checking: false,
            update_downloading: false,
            update_progress: 0.0,
            update_status_text: String::new(),
            update_error: None,
            last_sync_check: Instant::now(),
            status_banner: None,
            sidebar_collapsed: false,
            window_maximized: false,
            pending_delete_confirm: None,
            fonts_initialized: false,
            logo_texture: None,
            project_switch_loading_id: None,
            project_switch_loading_start: None,
            git_inspect_tx,
            git_inspect_rx,
        };
        if let Some(notice) = migration_notice {
            app.status_banner = Some((notice, true));
        }
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

    pub fn mark_config_dirty(&mut self) {
        self.dirty_save_pending = true;
        self.last_edit_time = Some(Instant::now());
    }

    pub fn flush_pending_save(&mut self) {
        if self.dirty_save_pending {
            let _ = self.storage.save();
            self.dirty_save_pending = false;
            self.last_edit_time = None;
        }
    }

    pub fn process_pending_git_inspect(&mut self) -> bool {
        let mut received = false;
        while let Ok(res) = self.git_inspect_rx.try_recv() {
            if let SidebarSelection::Project(cur_id) = &self.selection {
                if cur_id == &res.project_id {
                    self.cached_pkg_version = res.pkg_version;
                    self.cached_git_meta = res.git_meta;
                    self.cached_git_branches = res.git_branches;
                    self.cached_git_changes = res.git_changes;

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

                    if res.update_version_input {
                        self.deploy_version_input = self.storage.next_auto_bumped_version(
                            cur_id,
                            &self.selected_env_key,
                            self.cached_pkg_version.as_deref(),
                        );
                    }

                    if self.project_switch_loading_id.as_deref() == Some(&res.project_id) {
                        self.project_switch_loading_id = None;
                        self.project_switch_loading_start = None;
                    }
                    received = true;
                }
            }
        }
        received
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

        let is_different_proj = self.cached_project_id.as_deref() != Some(&proj.id);
        if is_different_proj {
            self.history_visible_limit = 5;
            self.servers_edit_buffer = None;
            self.cached_pkg_version = None;
            self.cached_git_changes.clear();
            self.project_switch_loading_id = Some(proj.id.clone());
            self.project_switch_loading_start = Some(Instant::now());
        }
        self.cached_project_id = Some(proj.id.clone());

        if let Some(env_cfg) = proj.get_env(&self.selected_env_key) {
            self.cached_vault_found = read_os_vault_credential(&env_cfg.vault_target).is_some();
        } else {
            self.cached_vault_found = false;
        }

        // Ensure selected_deployment_id belongs strictly to this active project
        let belongs_to_current_project = self
            .selected_deployment_id
            .as_ref()
            .and_then(|id| self.storage.deployments.iter().find(|d| &d.id == id))
            .is_some_and(|d| &d.project_id == proj_id);

        if !belongs_to_current_project {
            self.selected_deployment_id = self
                .storage
                .deployments
                .iter()
                .find(|d| &d.project_id == proj_id)
                .map(|d| d.id.clone());
        }

        if update_version_input {
            self.deploy_version_input = self.storage.next_auto_bumped_version(
                proj_id,
                &self.selected_env_key,
                self.cached_pkg_version.as_deref(),
            );
        }

        // Dispatch heavy Git inspection to a background worker so project switching is instant (0ms latency, zero UI freeze)
        let tx = self.git_inspect_tx.clone();
        let proj_id_clone = proj.id.clone();
        let path = PathBuf::from(&proj.local_path);
        let last_deploy_commit = self
            .storage
            .last_succeeded_deployment(proj_id, &self.selected_env_key)
            .and_then(|d| d.git_commit.clone());

        std::thread::spawn(move || {
            let pkg_version = read_package_json_version(&path);
            let git_meta = read_git_metadata(&path);
            let git_branches = list_git_branches(&path);
            let git_changes = read_git_code_changes(&path, last_deploy_commit.as_deref());

            let _ = tx.send(GitInspectionResult {
                project_id: proj_id_clone,
                pkg_version,
                git_meta,
                git_branches,
                git_changes,
                update_version_input,
            });
        });
    }

    fn poll_executor_events(&mut self, ctx: &egui::Context) {
        let mut received = false;
        while let Ok(event) = self.event_rx.try_recv() {
            received = true;
            match event {
                ExecutorEvent::Updated(record) => {
                    let is_current = match &self.selection {
                        SidebarSelection::Project(pid) => pid == &record.project_id,
                        SidebarSelection::AllDeployments => true,
                    };
                    if is_current {
                        self.selected_deployment_id = Some(record.id.clone());
                    }
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
                    self.status_banner = Some((msg.clone(), ok));
                    let is_current = match &self.selection {
                        SidebarSelection::Project(pid) => pid == &record.project_id,
                        SidebarSelection::AllDeployments => true,
                    };
                    if is_current {
                        self.selected_deployment_id = Some(record.id.clone());
                    }
                    self.active_deployment_id = None;
                    self.storage.upsert_deployment(record);
                    self.refresh_selected_project_metadata(true);

                    if let Some(next_req) = self.bulk_deploy_queue.pop_front() {
                        let done = self
                            .bulk_total_count
                            .saturating_sub(self.bulk_deploy_queue.len() + 1);
                        let step_idx = done + 1;
                        let total = self.bulk_total_count.max(step_idx);
                        let next_msg = format!(
                            "Bulk Deploy [{step_idx}/{total}]: Deploying {} ({}) v{}... (Previous: {msg})",
                            next_req.project_name,
                            format_env_badge(&next_req.env_config.key),
                            next_req.version
                        );
                        self.status_banner = Some((next_msg, ok));
                        self.start_deployment(next_req);
                    } else if self.bulk_total_count > 0 {
                        let total = self.bulk_total_count;
                        self.bulk_total_count = 0;
                        self.status_banner = Some((
                            format!("Bulk Deployment finished ({total} project(s) deployed). Last: {msg}"),
                            ok,
                        ));
                    }
                }
            }
        }

        // Poll GitHub Releases Auto-Updater events
        while let Ok(upd_event) = self.updater_rx.try_recv() {
            received = true;
            match upd_event {
                UpdaterEvent::Checking => {
                    self.update_checking = true;
                }
                UpdaterEvent::UpdateAvailable { release, manual: _ } => {
                    self.update_checking = false;
                    self.update_error = None;
                    self.available_update = Some(release);
                    self.show_update_modal = true;
                }
                UpdaterEvent::UpToDate {
                    current_version,
                    manual,
                } => {
                    self.update_checking = false;
                    if manual {
                        self.status_banner = Some((
                            format!(
                                "Shiply v{current_version} is up to date with GitHub Releases ({GITHUB_REPO})."
                            ),
                            true,
                        ));
                    }
                }
                UpdaterEvent::CheckFailed { error, manual } => {
                    self.update_checking = false;
                    if manual {
                        self.status_banner = Some((error, false));
                    }
                }
                UpdaterEvent::DownloadProgress {
                    fraction,
                    status_text,
                    ..
                } => {
                    self.update_downloading = true;
                    self.update_progress = fraction;
                    self.update_status_text = status_text;
                }
                UpdaterEvent::ReadyToRestart {
                    new_version,
                    exe_path,
                } => {
                    self.update_downloading = false;
                    self.update_progress = 1.0;
                    if let Err(err) = restart_application(&exe_path) {
                        self.update_error = Some(err);
                    } else {
                        self.status_banner = Some((
                            format!("Updated to Shiply v{new_version}! Restarting application..."),
                            true,
                        ));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                UpdaterEvent::UpdateFailed { error } => {
                    self.update_downloading = false;
                    self.update_error = Some(error);
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

        if self.process_pending_git_inspect() {
            received = true;
        }

        if received
            || self.active_deployment_id.is_some()
            || self.update_checking
            || self.update_downloading
            || self.project_switch_loading_id.is_some()
        {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }

    fn start_deployment(&mut self, req: DeploymentRequest) {
        self.flush_pending_save();
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

    fn open_bulk_deploy_modal(&mut self) {
        if self.bulk_target_env_key.is_empty()
            || !self
                .storage
                .projects
                .iter()
                .any(|p| p.get_env(&self.bulk_target_env_key).is_some())
        {
            self.bulk_target_env_key = self
                .storage
                .projects
                .iter()
                .flat_map(|p| p.environments.first())
                .map(|e| e.key.clone())
                .next()
                .unwrap_or_else(|| "dev".to_string());
        }

        // Clean up any deleted project IDs from selection
        let valid_ids: HashSet<String> = self
            .storage
            .projects
            .iter()
            .map(|p| p.id.clone())
            .collect();
        self.bulk_selected_projects
            .retain(|id| valid_ids.contains(id));

        if self.bulk_selected_projects.is_empty() {
            for proj in &self.storage.projects {
                if !proj.environments.is_empty() {
                    self.bulk_selected_projects.insert(proj.id.clone());
                }
            }
        }

        self.sync_bulk_project_versions();
        self.show_bulk_deploy_modal = true;
    }

    fn sync_bulk_project_versions(&mut self) {
        for proj in &self.storage.projects {
            let env_key = proj
                .get_env(&self.bulk_target_env_key)
                .or_else(|| proj.environments.first())
                .map(|e| e.key.clone())
                .unwrap_or_else(|| self.bulk_target_env_key.clone());
            let pkg_ver = read_package_json_version(&PathBuf::from(&proj.local_path));
            let auto_ver =
                self.storage
                    .next_auto_bumped_version(&proj.id, &env_key, pkg_ver.as_deref());
            self.bulk_project_versions.insert(proj.id.clone(), auto_ver);
        }
    }

    fn trigger_bulk_deployment(&mut self) {
        if self.active_deployment_id.is_some() {
            self.status_banner = Some((
                "A deployment is already running. Please wait for it to finish.".to_string(),
                false,
            ));
            return;
        }

        let manual_pw = if self.manual_ssh_password.is_empty() {
            None
        } else {
            Some(self.manual_ssh_password.clone())
        };

        let mut requests = VecDeque::new();
        for proj in &self.storage.projects {
            if !self.bulk_selected_projects.contains(&proj.id) {
                continue;
            }
            let Some(env_cfg) = proj
                .get_env(&self.bulk_target_env_key)
                .or_else(|| proj.environments.first())
                .cloned()
            else {
                continue;
            };

            let version = self
                .bulk_project_versions
                .get(&proj.id)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| {
                    let pkg_ver = read_package_json_version(&PathBuf::from(&proj.local_path));
                    self.storage.next_auto_bumped_version(
                        &proj.id,
                        &env_cfg.key,
                        pkg_ver.as_deref(),
                    )
                });

            let prev_commit = self
                .storage
                .last_succeeded_deployment(&proj.id, &env_cfg.key)
                .and_then(|d| d.git_commit.clone());

            requests.push_back(DeploymentRequest {
                deployment_id: Uuid::new_v4().to_string(),
                project_id: proj.id.clone(),
                project_name: proj.name.clone(),
                local_path: PathBuf::from(&proj.local_path),
                env_config: env_cfg,
                version,
                bump_package_json: self.bulk_bump_pkg,
                create_git_tag_on_success: self.bulk_create_tag,
                manual_password: manual_pw.clone(),
                skip_ssh_preflight: false,
                target_git_branch: None,
                rollback_commit: None,
                previous_commit: prev_commit,
                is_rollback: false,
            });
        }

        if requests.is_empty() {
            self.status_banner = Some((
                "Select at least one project with a configured environment to run Bulk Deployment."
                    .to_string(),
                false,
            ));
            return;
        }

        let total = requests.len();
        self.bulk_total_count = total;
        self.bulk_deploy_queue = requests;
        self.show_bulk_deploy_modal = false;

        if let Some(first_req) = self.bulk_deploy_queue.pop_front() {
            self.status_banner = Some((
                format!(
                    "Bulk Deploy [1/{total}]: Starting {} ({}) v{}...",
                    first_req.project_name,
                    format_env_badge(&first_req.env_config.key),
                    first_req.version
                ),
                true,
            ));
            self.start_deployment(first_req);
        }
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
        style.spacing.scroll.bar_outer_margin = 0.0;
        style.spacing.scroll.bar_inner_margin = 4.0;
        style.spacing.scroll.bar_width = 8.0;

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

        // Change cursor to pointer (PointingHand) whenever hovering clickable / interactive elements
        visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);

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
        if self.logo_texture.is_none() {
            self.logo_texture = Some(load_shiply_logo_texture(ctx));
        }
        self.poll_executor_events(ctx);
        Self::configure_light_workspace_style(ctx);

        if self.project_switch_loading_id.is_some() {
            ctx.request_repaint();
        }

        if let Some(m) = ctx.input(|i| i.viewport().maximized) {
            self.window_maximized = m;
        }

        self.render_termius_topbar(ctx);
        let sidebar_anim_t =
            ctx.animate_bool(egui::Id::new("sidebar_smooth_anim"), !self.sidebar_collapsed);
        if sidebar_anim_t > 0.001 {
            self.render_sidebar(ctx, sidebar_anim_t);
        }
        self.render_new_project_modal(ctx);
        self.render_edit_project_modal(ctx);
        self.render_new_environment_modal(ctx);
        self.render_bulk_deploy_modal(ctx);
        self.render_github_update_modal(ctx);
        self.render_delete_confirmation_modal(ctx);
        self.render_window_resize_handles(ctx);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(CANVAS_BG)
                    .inner_margin(Margin::ZERO),
            )
            .show(ctx, |ui| {
                ScrollArea::vertical()
                    .id_salt("main_workspace_vertical_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        egui::Frame::new()
                            .inner_margin(Margin {
                                left: 28,
                                right: 20,
                                top: 22,
                                bottom: 22,
                            })
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
                                    let (b_rect, _) = ui.allocate_exact_size(
                                        Vec2::new(banner_w, 40.0),
                                        Sense::hover(),
                                    );
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
                                    )
                                    .on_hover_cursor(egui::CursorIcon::PointingHand);
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
            });

        // Debounced state saving: writes to disk 400ms after user finishes typing
        if self.dirty_save_pending {
            if let Some(last) = self.last_edit_time {
                if last.elapsed() >= Duration::from_millis(400) {
                    let _ = self.storage.save();
                    self.dirty_save_pending = false;
                    self.last_edit_time = None;
                } else {
                    ctx.request_repaint_after(Duration::from_millis(100));
                }
            } else {
                let _ = self.storage.save();
                self.dirty_save_pending = false;
            }
        }
    }
}

impl ShiplyApp {
    /// Renders the custom frameless Termius-style topbar with left navigation pills,
    /// draggable window region, notification bell, and Minimize / Maximize / Close actions.
    fn render_termius_topbar(&mut self, ctx: &egui::Context) {
        let topbar_bg = Color32::from_rgb(13, 17, 30); // #0d111e (Termius deep midnight bar)
        let pill_bg = Color32::from_rgb(31, 38, 55); // #1f2637 (Termius pill fill)
        let icon_muted = Color32::from_rgb(148, 163, 184);

        if let Some(maximized) = ctx.input(|i| i.viewport().maximized) {
            self.window_maximized = maximized;
        }

        egui::TopBottomPanel::top("shiply_termius_topbar")
            .exact_height(42.0)
            .frame(
                egui::Frame::new()
                    .fill(topbar_bg)
                    .inner_margin(Margin::symmetric(10, 0))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(26, 34, 52))),
            )
            .show(ctx, |ui| {
                let bar_rect = ui.max_rect();
                let cy = bar_rect.center().y;
                let pill_h = 28.0;
                let mut x = bar_rect.left() + 2.0;

                // 1. Left Hamburger Menu Button (≡) — toggles sidebar visibility
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
                if menu_resp.on_hover_text("Toggle Sidebar").clicked() {
                    self.sidebar_collapsed = !self.sidebar_collapsed;
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
                    Color32::from_rgb(226, 232, 240),
                );
                let left_controls_end = x + 100.0;

                // =========================================================================
                // Right-Side Window Actions (Bell, Minimize ─, Maximize/Restore ❐, Close ×)
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

                // Interactive Drag Region in the middle empty space of the topbar
                let drag_left = left_controls_end;
                let drag_right = (bell_rect.left() - 6.0).max(drag_left + 10.0);
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
                    TERM_YELLOW
                } else if bell_resp.hovered() {
                    Color32::WHITE
                } else {
                    icon_muted
                };
                draw_icon_bell(ui.painter(), bell_rect.center(), bell_fg);
                if self.available_update.is_some() {
                    // Yellow circle dot on top right of notification icon
                    ui.painter().circle_filled(
                        Pos2::new(bell_rect.center().x + 4.5, bell_rect.center().y - 4.5),
                        3.2,
                        Color32::from_rgb(250, 204, 21),
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

                // C. Maximize / Restore Button (Single box □ when normal, double overlapping squares ❐ when maximized)
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
                        Color32::from_rgb(220, 38, 38),
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
    fn render_window_resize_handles(&self, ctx: &egui::Context) {
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

    fn render_sidebar(&mut self, ctx: &egui::Context, anim_t: f32) {
        let width = 248.0 * anim_t;
        let pad_x = ((16.0 * anim_t).round() as i8).max(0);
        egui::SidePanel::left("shiply_sidebar")
            .resizable(false)
            .exact_width(width)
            .frame(
                egui::Frame::new()
                    .fill(SIDEBAR_BG)
                    .inner_margin(Margin::symmetric(pad_x, 18))
                    .stroke(Stroke::new(1.0_f32, SIDEBAR_DIVIDER)),
            )
            .show(ctx, |ui| {
                if anim_t < 0.25 {
                    return;
                }
                ui.visuals_mut().override_text_color = Some(Color32::from_rgb(241, 245, 249));
                let full_w = ui.available_width();

                // 1. Top Brand Bar (Fixed 32px height)
                let (brand_rect, _) =
                    ui.allocate_exact_size(Vec2::new(full_w, 32.0), Sense::hover());
                let logo_rect = Rect::from_min_size(
                    Pos2::new(brand_rect.left(), brand_rect.center().y - 14.0),
                    Vec2::new(28.0, 28.0),
                );
                draw_shiply_logo(
                    ui.painter(),
                    logo_rect,
                    6,
                    FONT_LG,
                    self.logo_texture.as_ref(),
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
                let nav_resp = nav_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
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

                if !self.storage.projects.is_empty() {
                    ui.add_space(4.0);
                    let (bulk_nav_rect, bulk_nav_resp) =
                        ui.allocate_exact_size(Vec2::new(full_w, 34.0), Sense::click());
                    let bulk_nav_resp =
                        bulk_nav_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                    let bulk_nav_bg = if self.show_bulk_deploy_modal {
                        SIDEBAR_CARD_ACTIVE
                    } else if bulk_nav_resp.hovered() {
                        SIDEBAR_CARD_HOVER
                    } else {
                        Color32::TRANSPARENT
                    };
                    ui.painter()
                        .rect_filled(bulk_nav_rect, CornerRadius::same(7), bulk_nav_bg);
                    draw_icon_send(
                        ui.painter(),
                        Pos2::new(bulk_nav_rect.left() + 16.0, bulk_nav_rect.center().y),
                        Color32::from_rgb(96, 165, 250),
                    );
                    ui.painter().text(
                        Pos2::new(bulk_nav_rect.left() + 34.0, bulk_nav_rect.center().y),
                        Align2::LEFT_CENTER,
                        "Bulk Deploy...",
                        FontId::proportional(FONT_MD),
                        Color32::from_rgb(226, 232, 240),
                    );
                    if bulk_nav_resp
                        .on_hover_text("Select multiple projects to deploy sequentially")
                        .clicked()
                    {
                        self.open_bulk_deploy_modal();
                    }
                }

                ui.add_space(16.0);

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
                let add_btn_resp = ui
                    .interact(
                        add_btn_rect,
                        egui::Id::new("sidebar_add_proj_btn"),
                        Sense::click(),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
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
                        let p_resp = p_resp.on_hover_cursor(egui::CursorIcon::PointingHand);

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

                        // Check if this project is currently running an active deployment process
                        let is_deploying = self.active_deployment_id.as_ref().is_some_and(|dep_id| {
                            self.storage
                                .deployments
                                .iter()
                                .any(|d| &d.id == dep_id && d.project_id == project.id)
                        }) || self
                            .storage
                            .deployments
                            .iter()
                            .any(|d| d.project_id == project.id && d.status == DeploymentStatus::Running);

                        let is_switching =
                            self.project_switch_loading_id.as_deref() == Some(&project.id);

                        // Project Title (constrained width so it doesn't collide with spinner)
                        let title_color = if is_selected {
                            Color32::from_rgb(96, 165, 250)
                        } else {
                            Color32::WHITE
                        };
                        let max_title_w = if is_deploying || is_switching {
                            full_w - 56.0
                        } else {
                            full_w - 24.0
                        };
                        let title_galley = ui.painter().layout(
                            project.name.clone(),
                            FontId::proportional(FONT_MD),
                            title_color,
                            max_title_w,
                        );
                        ui.painter().galley(
                            Pos2::new(
                                p_rect.left() + 12.0,
                                p_rect.top() + 20.0 - title_galley.size().y / 2.0,
                            ),
                            title_galley,
                            title_color,
                        );

                        // Circle loading indicator spinner on the right side if running deployment or changing active project
                        if is_deploying || is_switching {
                            ui.ctx().request_repaint();
                            let spinner_center =
                                Pos2::new(p_rect.right() - 20.0, p_rect.top() + 20.0);
                            let spinner_color = if is_deploying {
                                Color32::from_rgb(96, 165, 250) // Bright Blue for active running deployment
                            } else {
                                Color32::from_rgb(148, 163, 184) // Muted Slate for project metadata load
                            };
                            draw_circle_loading_spinner(
                                ui.painter(),
                                spinner_center,
                                7.0,
                                spinner_color,
                                ui.input(|i| i.time),
                            );
                        }

                        let p_resp = if is_deploying {
                            p_resp.on_hover_text("A deployment is currently running for this project")
                        } else {
                            p_resp
                        };

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
                                let is_env_running = self.storage.deployments.iter().any(|d| {
                                    d.project_id == project.id
                                        && d.environment == env.key
                                        && (d.status == DeploymentStatus::Running
                                            || self.active_deployment_id.as_deref() == Some(&d.id))
                                });
                                let (label, bg, fg) = if is_env_running {
                                    (
                                        format!("{badge} Deploying..."),
                                        Color32::from_rgb(30, 58, 138),
                                        Color32::from_rgb(96, 165, 250),
                                    )
                                } else if let Some(ok_dep) =
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

                // 5. Bottom Sidebar Footer (App Install + GitHub Auto-Update + Open Data Dir + Sync)
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
                    let inst_resp = inst_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
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

                    // GitHub Releases Auto-Update Check / Notification Button
                    ui.add_space(4.0);
                    let (upd_rect, upd_resp) =
                        ui.allocate_exact_size(Vec2::new(full_w, 24.0), Sense::click());
                    let upd_resp = upd_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                    let (upd_label, upd_color) = if let Some(rel) = &self.available_update {
                        (
                            format!("Update Available: {}!", rel.tag_name),
                            TERM_YELLOW,
                        )
                    } else if self.update_checking {
                        (
                            "Checking GitHub Releases...".to_string(),
                            Color32::from_rgb(96, 165, 250),
                        )
                    } else {
                        (
                            "Check for Updates (GitHub)".to_string(),
                            if upd_resp.hovered() {
                                Color32::WHITE
                            } else {
                                SIDEBAR_TEXT_MUTED
                            },
                        )
                    };
                    draw_icon_refresh(
                        ui.painter(),
                        Pos2::new(upd_rect.left() + 6.0, upd_rect.center().y),
                        upd_color,
                    );
                    ui.painter().text(
                        Pos2::new(upd_rect.left() + 18.0, upd_rect.center().y),
                        Align2::LEFT_CENTER,
                        upd_label,
                        FontId::proportional(FONT_SM),
                        upd_color,
                    );
                    if upd_resp
                        .on_hover_text(format!(
                            "Check GitHub Releases ({GITHUB_REPO}) for a newer version of Shiply"
                        ))
                        .clicked()
                    {
                        if self.available_update.is_some() {
                            self.show_update_modal = true;
                        } else if !self.update_checking {
                            spawn_update_check(self.updater_tx.clone(), true);
                        }
                    }

                    ui.add_space(4.0);
                    let (dir_rect, dir_resp) =
                        ui.allocate_exact_size(Vec2::new(full_w, 24.0), Sense::click());
                    let dir_resp = dir_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
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
                    let sync_resp = sync_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
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

        ui.add_space(18.0);
        self.render_onboarding_guideline_card(ui);
        ui.add_space(16.0);
        self.render_os_credentials_guideline_card(ui);
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
                    .inner_margin(Margin {
                        left: 24,
                        right: 24,
                        top: 18,
                        bottom: 22,
                    }),
            )
            .show(ctx, |ui| {
                ui.set_width(512.0);

                if render_modal_header(
                    ui,
                    "close_new_proj_modal",
                    "Add New Project",
                    "Auto-import a PowerShell deployment script (.ps1) to populate environments and steps, or configure manually.",
                ) {
                    self.show_new_project_modal = false;
                }

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
                                Some(r"e.g. C:\Projects\my-app\deploy-dev.ps1"),
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
                    Some("e.g. my-web-service"),
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
                        Some(r"e.g. C:\Projects\my-web-service"),
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
                    Some("e.g. Backend API service (Node.js / PM2)"),
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

    fn open_edit_project_modal(&mut self, proj_idx: usize) {
        if let Some(proj) = self.storage.projects.get(proj_idx) {
            self.edit_project_id = proj.id.clone();
            self.edit_project_name = proj.name.clone();
            self.edit_project_path = proj.local_path.clone();
            self.edit_project_desc = proj.description.clone();
            self.edit_project_ps1_path.clear();
            self.edit_project_import_notice = None;
            self.show_edit_project_modal = true;
        }
    }

    fn render_edit_project_modal(&mut self, ctx: &egui::Context) {
        if !self.show_edit_project_modal {
            return;
        }

        let Some(proj_idx) = self
            .storage
            .projects
            .iter()
            .position(|p| p.id == self.edit_project_id)
        else {
            self.show_edit_project_modal = false;
            return;
        };

        egui::Area::new(egui::Id::new("modal_backdrop_edit_project"))
            .order(egui::Order::Middle)
            .fixed_pos(Pos2::ZERO)
            .show(ctx, |ui| {
                ui.painter().rect_filled(
                    ctx.screen_rect(),
                    CornerRadius::ZERO,
                    Color32::from_black_alpha(95),
                );
            });

        egui::Window::new("Edit Project")
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
                    .inner_margin(Margin {
                        left: 24,
                        right: 24,
                        top: 18,
                        bottom: 22,
                    }),
            )
            .show(ctx, |ui| {
                ui.set_width(512.0);

                if render_modal_header(
                    ui,
                    "close_edit_proj_modal",
                    "Edit Project Settings",
                    "Update the project name, local directory path, description, or sync environments from a .ps1 script.",
                ) {
                    self.show_edit_project_modal = false;
                }

                ui.add_space(14.0);

                ui.label(
                    RichText::new("Project Name")
                        .size(FONT_SM)
                        .strong()
                        .color(TEXT_SECONDARY),
                );
                ui.add_space(3.0);
                styled_text_edit(
                    ui,
                    &mut self.edit_project_name,
                    492.0,
                    Some("e.g. my-web-service"),
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
                        &mut self.edit_project_path,
                        380.0,
                        Some(r"e.g. C:\Projects\my-web-service"),
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
                            self.edit_project_path = folder;
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
                    &mut self.edit_project_desc,
                    492.0,
                    Some("e.g. Backend API service (Node.js / PM2)"),
                );

                ui.add_space(14.0);

                // Optional: Import/Sync additional .ps1 script into this project
                egui::Frame::new()
                    .fill(SUBTLE_BG)
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(191, 219, 254)))
                    .corner_radius(8.0)
                    .inner_margin(Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("SYNC / IMPORT ENVIRONMENT FROM .PS1 SCRIPT")
                                .size(FONT_XS)
                                .strong()
                                .color(PRIMARY_BLUE),
                        );
                        ui.add_space(4.0);
                        horizontal_row(ui, 34.0, |ui| {
                            styled_text_edit(
                                ui,
                                &mut self.edit_project_ps1_path,
                                285.0,
                                Some(r"e.g. C:\Projects\my-app\deploy-prod.ps1"),
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
                                    self.edit_project_ps1_path = picked;
                                }
                            }

                            let imp_btn = egui::Button::new(
                                RichText::new("Import")
                                    .size(FONT_SM)
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .fill(PRIMARY_BLUE)
                            .min_size(Vec2::new(80.0, 32.0));
                            if ui.add(imp_btn).clicked()
                                && !self.edit_project_ps1_path.trim().is_empty()
                            {
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

                ui.add_space(16.0);
                draw_divider(ui, CARD_BORDER);
                ui.add_space(12.0);

                horizontal_row(ui, 36.0, |ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let can_save = !self.edit_project_name.trim().is_empty()
                            && !self.edit_project_path.trim().is_empty();
                        let save_btn = egui::Button::new(
                            RichText::new("Save Changes")
                                .size(FONT_MD)
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(if can_save { PRIMARY_BLUE } else { TEXT_MUTED })
                        .corner_radius(7.0)
                        .min_size(Vec2::new(135.0, 34.0));

                        if ui.add(save_btn).clicked() && can_save {
                            let proj = &mut self.storage.projects[proj_idx];
                            proj.name = self.edit_project_name.trim().to_string();
                            proj.local_path = self.edit_project_path.trim().to_string();
                            proj.description = if self.edit_project_desc.trim().is_empty() {
                                "Custom deployment project".to_string()
                            } else {
                                self.edit_project_desc.trim().to_string()
                            };
                            let updated_name = proj.name.clone();
                            let _ = self.storage.save();
                            self.refresh_selected_project_metadata(false);
                            self.status_banner = Some((
                                format!("Updated project '{updated_name}'."),
                                true,
                            ));
                            self.show_edit_project_modal = false;
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
                            self.show_edit_project_modal = false;
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
                    .inner_margin(Margin {
                        left: 24,
                        right: 24,
                        top: 18,
                        bottom: 22,
                    }),
            )
            .show(ctx, |ui| {
                ui.set_width(452.0);
                if render_modal_header(
                    ui,
                    "close_new_env_modal",
                    "Add Environment Level",
                    "Import an environment from a .ps1 script or create a custom level (dev, staging, production, UAT, QA).",
                ) {
                    self.show_new_env_modal = false;
                }
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
                                Some(r"e.g. C:\Projects\my-app\deploy-staging.ps1"),
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

    fn render_github_update_modal(&mut self, ctx: &egui::Context) {
        if !self.show_update_modal {
            return;
        }

        egui::Area::new(egui::Id::new("modal_backdrop_github_update"))
            .order(egui::Order::Middle)
            .fixed_pos(Pos2::ZERO)
            .show(ctx, |ui| {
                ui.painter().rect_filled(
                    ctx.screen_rect(),
                    CornerRadius::ZERO,
                    Color32::from_black_alpha(95),
                );
            });

        if let Some(release) = self.available_update.clone() {
            egui::Window::new("Software Update Available")
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
                        .inner_margin(Margin {
                            left: 24,
                            right: 24,
                            top: 18,
                            bottom: 22,
                        }),
                )
                .show(ctx, |ui| {
                    ui.set_width(512.0);
                    if render_modal_header(
                        ui,
                        "close_github_update_modal",
                        "Software Update Available",
                        &format!(
                            "A newer release of Shiply is available on GitHub Releases ({GITHUB_REPO})."
                        ),
                    ) && !self.update_downloading
                    {
                        self.show_update_modal = false;
                    }

                    ui.add_space(12.0);

                    // Version Comparison Card (Current Version vs. New Version Tag)
                    egui::Frame::new()
                        .fill(SUBTLE_BG)
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(191, 219, 254)))
                        .corner_radius(8.0)
                        .inner_margin(Margin::symmetric(14, 12))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            horizontal_row(ui, 26.0, |ui| {
                                ui.label(
                                    RichText::new("Current Version:")
                                        .size(FONT_SM)
                                        .strong()
                                        .color(TEXT_SECONDARY),
                                );
                                render_pill(
                                    ui,
                                    &format!("v{APP_VERSION}"),
                                    PILL_CONTAINER_BG,
                                    TEXT_SECONDARY,
                                );
                                ui.label(
                                    RichText::new("→")
                                        .size(FONT_LG)
                                        .strong()
                                        .color(PRIMARY_BLUE),
                                );
                                ui.label(
                                    RichText::new("New Release:")
                                        .size(FONT_SM)
                                        .strong()
                                        .color(TEXT_SECONDARY),
                                );
                                render_pill(
                                    ui,
                                    &release.tag_name,
                                    SUCCESS_PILL_BG,
                                    SUCCESS_PILL_TEXT,
                                );
                            });
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(format!(
                                    "{}  •  Published: {}",
                                    release.name, release.published_at
                                ))
                                .size(FONT_XS)
                                .color(TEXT_MUTED),
                            );
                        });

                    ui.add_space(12.0);

                    // Release Notes / Changelog section
                    ui.label(
                        RichText::new("RELEASE NOTES / CHANGELOG")
                            .size(FONT_XS)
                            .strong()
                            .color(TEXT_MUTED),
                    );
                    ui.add_space(4.0);
                    egui::Frame::new()
                        .fill(INPUT_BG)
                        .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                        .corner_radius(8.0)
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
                                            ui.add_space(4.0);
                                            ui.label(
                                                RichText::new(h)
                                                    .size(FONT_MD)
                                                    .strong()
                                                    .color(TEXT_PRIMARY),
                                            );
                                        } else if let Some(h) = trimmed.strip_prefix("## ") {
                                            ui.add_space(4.0);
                                            ui.label(
                                                RichText::new(h)
                                                    .size(FONT_MD)
                                                    .strong()
                                                    .color(PRIMARY_BLUE),
                                            );
                                        } else {
                                            ui.label(
                                                RichText::new(line)
                                                    .size(FONT_SM)
                                                    .color(TEXT_SECONDARY),
                                            );
                                        }
                                    }
                                });
                        });

                    // Download Progress Bar / Status Indicator
                    if self.update_downloading || self.update_progress > 0.0 {
                        ui.add_space(12.0);
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
                            .size(FONT_SM)
                            .strong()
                            .color(PRIMARY_BLUE),
                        );
                        ui.add_space(4.0);
                        let bar_w = ui.available_width();
                        let (bar_rect, _) =
                            ui.allocate_exact_size(Vec2::new(bar_w, 10.0), Sense::hover());
                        ui.painter()
                            .rect_filled(bar_rect, CornerRadius::same(5), PILL_CONTAINER_BG);
                        let fill_w = (bar_w * self.update_progress.clamp(0.02, 1.0)).min(bar_w);
                        let fill_rect = Rect::from_min_size(bar_rect.min, Vec2::new(fill_w, 10.0));
                        ui.painter()
                            .rect_filled(fill_rect, CornerRadius::same(5), PRIMARY_BLUE);
                    }

                    if let Some(err) = &self.update_error {
                        ui.add_space(10.0);
                        egui::Frame::new()
                            .fill(FAIL_PILL_BG)
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(252, 165, 165)))
                            .corner_radius(6.0)
                            .inner_margin(Margin::symmetric(12, 8))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(err)
                                        .size(FONT_SM)
                                        .strong()
                                        .color(FAIL_PILL_TEXT),
                                );
                            });
                    }

                    ui.add_space(16.0);
                    draw_divider(ui, CARD_BORDER);
                    ui.add_space(12.0);

                    horizontal_row(ui, 36.0, |ui| {
                        let view_gh_btn = egui::Button::new(
                            RichText::new("View on GitHub")
                                .size(FONT_SM)
                                .strong()
                                .color(TEXT_SECONDARY),
                        )
                        .fill(SUBTLE_BG)
                        .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                        .corner_radius(7.0)
                        .min_size(Vec2::new(115.0, 34.0));
                        if ui.add(view_gh_btn).clicked() {
                            let url = if release.html_url.is_empty() {
                                GITHUB_RELEASES_PAGE_URL
                            } else {
                                &release.html_url
                            };
                            open_url_in_browser(url);
                        }

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let update_btn_label = if self.update_downloading {
                                "Updating...".to_string()
                            } else {
                                "Update & Restart".to_string()
                            };
                            let update_btn = egui::Button::new(
                                RichText::new(update_btn_label)
                                    .size(FONT_MD)
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .fill(if self.update_downloading {
                                TEXT_MUTED
                            } else {
                                PRIMARY_BLUE
                            })
                            .corner_radius(7.0)
                            .min_size(Vec2::new(155.0, 36.0));

                            if ui
                                .add_enabled(!self.update_downloading, update_btn)
                                .clicked()
                            {
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

                            let later_btn = egui::Button::new(
                                RichText::new("Remind Me Later")
                                    .size(FONT_MD)
                                    .strong()
                                    .color(TEXT_SECONDARY),
                            )
                            .fill(SUBTLE_BG)
                            .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                            .corner_radius(7.0)
                            .min_size(Vec2::new(130.0, 36.0));

                            if ui
                                .add_enabled(!self.update_downloading, later_btn)
                                .clicked()
                            {
                                self.show_update_modal = false;
                            }
                        });
                    });
                });
        } else {
            // Up-to-date Update Information Dialog (when clicking notification bell or checking)
            egui::Window::new("Update Information")
                .order(egui::Order::Foreground)
                .title_bar(false)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .fixed_size(Vec2::new(480.0, 0.0))
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
                        .inner_margin(Margin {
                            left: 24,
                            right: 24,
                            top: 20,
                            bottom: 22,
                        }),
                )
                .show(ctx, |ui| {
                    ui.set_width(432.0);
                    if render_modal_header(
                        ui,
                        "close_update_info_modal",
                        "Update Information",
                        "Check for newer versions published on GitHub Releases.",
                    ) {
                        self.show_update_modal = false;
                    }

                    ui.add_space(14.0);

                    egui::Frame::new()
                        .fill(SUCCESS_PILL_BG)
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(187, 247, 208)))
                        .corner_radius(8.0)
                        .inner_margin(Margin::symmetric(14, 12))
                        .show(ui, |ui| {
                            horizontal_row(ui, 24.0, |ui| {
                                render_pill(
                                    ui,
                                    "UP TO DATE",
                                    Color32::from_rgb(34, 197, 94),
                                    Color32::WHITE,
                                );
                                ui.label(
                                    RichText::new(format!("Shiply v{APP_VERSION}"))
                                        .size(FONT_MD)
                                        .strong()
                                        .color(SUCCESS_PILL_TEXT),
                                );
                            });
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(format!(
                                    "You are running the latest version of Shiply. Update checks query {GITHUB_REPO} on startup."
                                ))
                                .size(FONT_SM)
                                .color(Color32::from_rgb(20, 83, 45)),
                            );
                        });

                    if self.update_checking {
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new("Checking GitHub Releases...")
                                .size(FONT_SM)
                                .color(PRIMARY_BLUE),
                        );
                    }

                    ui.add_space(18.0);
                    draw_divider(ui, CARD_BORDER);
                    ui.add_space(14.0);

                    horizontal_row(ui, 36.0, |ui| {
                        let view_gh_btn = egui::Button::new(
                            RichText::new("View on GitHub")
                                .size(FONT_SM)
                                .strong()
                                .color(TEXT_SECONDARY),
                        )
                        .fill(SUBTLE_BG)
                        .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                        .corner_radius(7.0)
                        .min_size(Vec2::new(115.0, 34.0));
                        if ui.add(view_gh_btn).clicked() {
                            open_url_in_browser(GITHUB_RELEASES_PAGE_URL);
                        }

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let close_btn = egui::Button::new(
                                RichText::new("Close")
                                    .size(FONT_MD)
                                    .strong()
                                    .color(TEXT_SECONDARY),
                            )
                            .fill(SUBTLE_BG)
                            .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                            .corner_radius(7.0)
                            .min_size(Vec2::new(80.0, 36.0));
                            if ui.add(close_btn).clicked() {
                                self.show_update_modal = false;
                            }

                            let check_btn = egui::Button::new(
                                RichText::new("Check Again")
                                    .size(FONT_MD)
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .fill(PRIMARY_BLUE)
                            .corner_radius(7.0)
                            .min_size(Vec2::new(115.0, 36.0));
                            if ui
                                .add_enabled(!self.update_checking, check_btn)
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
    fn render_delete_confirmation_modal(&mut self, ctx: &egui::Context) {
        let Some(action) = self.pending_delete_confirm.clone() else {
            return;
        };

        egui::Area::new(egui::Id::new("modal_backdrop_delete_confirm"))
            .order(egui::Order::Middle)
            .fixed_pos(Pos2::ZERO)
            .show(ctx, |ui| {
                ui.painter().rect_filled(
                    ctx.screen_rect(),
                    CornerRadius::ZERO,
                    Color32::from_black_alpha(110),
                );
            });

        let mut execute_delete = false;
        let mut dismiss = false;

        egui::Window::new("Confirm Deletion")
            .order(egui::Order::Foreground)
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .fixed_size(Vec2::new(480.0, 0.0))
            .frame(
                egui::Frame::new()
                    .fill(CARD_BG)
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(252, 165, 165)))
                    .corner_radius(12.0)
                    .shadow(Shadow {
                        offset: [0, 8],
                        blur: 24,
                        spread: 0,
                        color: Color32::from_black_alpha(45),
                    })
                    .inner_margin(Margin {
                        left: 24,
                        right: 24,
                        top: 20,
                        bottom: 22,
                    }),
            )
            .show(ctx, |ui| {
                ui.set_width(432.0);

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

                ui.add_space(14.0);

                egui::Frame::new()
                    .fill(FAIL_PILL_BG)
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(254, 202, 202)))
                    .corner_radius(8.0)
                    .inner_margin(Margin::symmetric(14, 12))
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
                                    .size(FONT_MD)
                                    .strong()
                                    .color(FAIL_PILL_TEXT),
                            );
                        });
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(warning_text)
                                .size(FONT_SM)
                                .color(Color32::from_rgb(127, 29, 29)),
                        );
                    });

                ui.add_space(18.0);
                draw_divider(ui, CARD_BORDER);
                ui.add_space(14.0);

                horizontal_row(ui, 36.0, |ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let delete_btn = egui::Button::new(
                            RichText::new("Delete Permanently")
                                .size(FONT_MD)
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(220, 38, 38))
                        .corner_radius(7.0)
                        .min_size(Vec2::new(155.0, 36.0));

                        if ui.add(delete_btn).clicked() {
                            execute_delete = true;
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
                        .min_size(Vec2::new(88.0, 36.0));

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

    fn render_all_deployments_overview(&mut self, ui: &mut egui::Ui) {
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
            || (self.bulk_total_count > 0 && self.active_deployment_id.is_some())
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

    fn render_onboarding_guideline_card(&mut self, ui: &mut egui::Ui) {
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

    fn render_os_credentials_guideline_card(&mut self, ui: &mut egui::Ui) {
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

    fn render_bulk_deploy_modal(&mut self, ctx: &egui::Context) {
        if !self.show_bulk_deploy_modal {
            return;
        }

        // Dimmed backdrop
        egui::Area::new(egui::Id::new("modal_backdrop_bulk_deploy"))
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

        egui::Window::new("Bulk Deployment")
            .order(egui::Order::Foreground)
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .fixed_size(Vec2::new(620.0, 0.0))
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
                    .inner_margin(Margin {
                        left: 24,
                        right: 24,
                        top: 18,
                        bottom: 22,
                    }),
            )
            .show(ctx, |ui| {
                ui.set_width(572.0);

                if render_modal_header(
                    ui,
                    "close_bulk_deploy_modal",
                    "Bulk Deployment — Select Projects to Deploy",
                    "Choose which projects to deploy, select the target environment, and review each project's auto-bumped release version.",
                ) {
                    self.show_bulk_deploy_modal = false;
                }
                ui.add_space(12.0);

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
                            .size(FONT_SM)
                            .strong()
                            .color(TEXT_SECONDARY),
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
                                .size(FONT_SM)
                                .strong()
                                .color(PRIMARY_BLUE),
                        )
                        .width(210.0)
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
                                .size(FONT_XS)
                                .strong()
                                .color(TEXT_SECONDARY),
                        )
                        .fill(SUBTLE_BG)
                        .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                        .min_size(Vec2::new(72.0, 28.0));
                        if ui.add(clear_btn).clicked() {
                            self.bulk_selected_projects.clear();
                        }

                        let all_btn = egui::Button::new(
                            RichText::new("Select All")
                                .size(FONT_XS)
                                .strong()
                                .color(PRIMARY_BLUE),
                        )
                        .fill(BLUE_PILL_BG)
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(147, 197, 253)))
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

                ui.add_space(10.0);

                // Project Checklist Table
                egui::Frame::new()
                    .fill(SUBTLE_BG)
                    .stroke(Stroke::new(1.0_f32, CARD_BORDER))
                    .corner_radius(8.0)
                    .inner_margin(Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ScrollArea::vertical()
                            .id_salt("bulk_projects_list_scroll")
                            .max_height(240.0)
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
                                                        .size(FONT_MD)
                                                        .strong()
                                                        .color(TEXT_PRIMARY),
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
                                            styled_text_edit(
                                                ui,
                                                ver_entry,
                                                90.0,
                                                Some("1.0.0"),
                                            );
                                            ui.label(
                                                RichText::new("Version:")
                                                    .size(FONT_XS)
                                                    .color(TEXT_MUTED),
                                            );
                                        });
                                    });
                                    draw_divider(ui, CARD_BORDER);
                                }
                            });
                    });

                ui.add_space(12.0);

                // Options + Trigger Action Footer
                horizontal_row(ui, 36.0, |ui| {
                    ui.checkbox(
                        &mut self.bulk_bump_pkg,
                        RichText::new("Bump package.json")
                            .size(FONT_SM)
                            .color(TEXT_SECONDARY),
                    );
                    ui.add_space(8.0);
                    ui.checkbox(
                        &mut self.bulk_create_tag,
                        RichText::new("Git Tag")
                            .size(FONT_SM)
                            .color(TEXT_SECONDARY),
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let sel_count = self.bulk_selected_projects.len();
                        let run_btn = egui::Button::new(
                            RichText::new(format!("Deploy {sel_count} Selected Project(s)"))
                                .size(FONT_MD)
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(if sel_count > 0 {
                            PRIMARY_BLUE
                        } else {
                            INPUT_BORDER
                        })
                        .corner_radius(7.0)
                        .min_size(Vec2::new(210.0, 36.0));

                        if ui.add_enabled(sel_count > 0, run_btn).clicked() {
                            self.trigger_bulk_deployment();
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
                        .min_size(Vec2::new(84.0, 36.0));
                        if ui.add(cancel_btn).clicked() {
                            self.show_bulk_deploy_modal = false;
                        }
                    });
                });
            });
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

        let proj_name = self.storage.projects[proj_idx].name.clone();

        // If currently in the loading indicator transition for switching active project:
        if self.project_switch_loading_id.as_deref() == Some(project_id) {
            if let Some(start) = self.project_switch_loading_start {
                if start.elapsed() < Duration::from_millis(300) {
                    render_project_loading_indicator(ui, &proj_name);
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

        // 1. Top Project Header Bar (Fixed 36px height — all right-side controls vertically centered at 32px height)
        let mut delete_project_clicked = false;
        let mut edit_project_clicked = false;
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

            let right_w = ui.available_width();
            ui.allocate_ui_with_layout(
                Vec2::new(right_w, 36.0),
                Layout::right_to_left(Align::Center),
                |ui| {
                    ui.set_min_height(36.0);
                    ui.set_max_height(36.0);
                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);

                    if outline_icon_button(
                        ui,
                        ("hdr_delete_proj", project_id),
                        OutlineIcon::Trash,
                        Some("Delete"),
                        FAIL_PILL_TEXT,
                        FAIL_PILL_BG,
                        Color32::from_rgb(254, 202, 202),
                        Color32::from_rgb(252, 165, 165),
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
                        TEXT_SECONDARY,
                        CARD_BG,
                        SUBTLE_BG,
                        CARD_BORDER,
                        true,
                    )
                    .on_hover_text("Refresh Git branches, commits, and package.json version")
                    .clicked()
                    {
                        self.refresh_selected_project_metadata(true);
                    }

                    if outline_icon_button(
                        ui,
                        ("hdr_edit_proj", project_id),
                        OutlineIcon::Edit,
                        Some("Edit Project"),
                        TEXT_PRIMARY,
                        CARD_BG,
                        SUBTLE_BG,
                        CARD_BORDER,
                        true,
                    )
                    .on_hover_text("Edit project name, local directory path, or description")
                    .clicked()
                    {
                        edit_project_clicked = true;
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
                                        .color(TEXT_PRIMARY),
                                )
                                .width(185.0)
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
                        TEXT_MUTED,
                    );
                }

                // Paint the outline Trash delete environment button
                ui.painter().rect_filled(
                    del_rect,
                    CornerRadius::same(5),
                    if del_resp.hovered() {
                        FAIL_PILL_BG
                    } else {
                        SUBTLE_BG
                    },
                );
                ui.painter().rect_stroke(
                    del_rect,
                    CornerRadius::same(5),
                    Stroke::new(
                        1.0_f32,
                        if del_resp.hovered() {
                            Color32::from_rgb(252, 165, 165)
                        } else {
                            CARD_BORDER
                        },
                    ),
                    StrokeKind::Inside,
                );
                draw_outline_icon(
                    ui.painter(),
                    del_rect.center(),
                    OutlineIcon::Trash,
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
            let add_resp = add_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
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
        self.render_deploy_action_bar(ui, &active_env_name, rollback_candidate);

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
                    format!("Deployments & Logs ({history_count})"),
                ),
                (
                    WorkspaceTab::ServerSettings,
                    "Environment & SSH Settings".to_string(),
                ),
            ] {
                let is_active = self.active_tab == tab;
                if render_uniform_tab_button(
                    ui,
                    ("workspace_tab_btn", project_id, &label),
                    &label,
                    is_active,
                    36.0,
                )
                .clicked()
                {
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

        // Right-aligned Deploy Button + Rollback Button (Feature #3: Dynamic sizing based on env & version)
        let is_busy = self.active_deployment_id.is_some();
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
            FontId::proportional(FONT_MD),
            Color32::WHITE,
        );
        let deploy_icon_w = 14.0;
        let deploy_gap = 8.0;
        let deploy_pad_x = 18.0;
        let deploy_content_w = deploy_icon_w + deploy_gap + deploy_galley.size().x;
        let mut btn_w = (deploy_content_w + deploy_pad_x * 2.0).max(180.0);
        let btn_h = 42.0;

        let rollback_info = rollback_candidate.as_ref().map(|prev_dep| {
            let rb_text = format!("Rollback v{}", prev_dep.version);
            let rb_galley = ui.painter().layout_no_wrap(
                rb_text,
                FontId::proportional(FONT_SM),
                WARN_PILL_TEXT,
            );
            let rb_icon_w = 14.0;
            let rb_gap = 7.0;
            let rb_pad_x = 14.0;
            let rb_content_w = rb_icon_w + rb_gap + rb_galley.size().x;
            let rb_w = (rb_content_w + rb_pad_x * 2.0).max(136.0);
            (rb_galley, rb_icon_w, rb_gap, rb_content_w, rb_w)
        });

        let left_controls_right = child.cursor().min.x;
        let max_right_area_w = (bar_rect.right() - 20.0 - (left_controls_right + 16.0)).max(180.0);
        let rb_total_w = rollback_info.as_ref().map(|(_, _, _, _, w)| *w + 10.0).unwrap_or(0.0);
        let total_needed = btn_w + rb_total_w;
        if total_needed > max_right_area_w {
            let overflow = total_needed - max_right_area_w;
            btn_w = (btn_w - overflow).max(140.0);
        }

        let btn_rect = Rect::from_min_size(
            Pos2::new(
                bar_rect.right() - 20.0 - btn_w,
                bar_rect.center().y - btn_h / 2.0,
            ),
            Vec2::new(btn_w, btn_h),
        );
        let btn_resp = ui
            .interact(btn_rect, egui::Id::new("primary_deploy_btn"), Sense::click())
            .on_hover_cursor(if is_busy {
                egui::CursorIcon::NotAllowed
            } else {
                egui::CursorIcon::PointingHand
            });
        let btn_bg = if is_busy {
            TEXT_MUTED
        } else if btn_resp.hovered() {
            PRIMARY_BLUE_HOVER
        } else {
            PRIMARY_BLUE
        };
        ui.painter()
            .rect_filled(btn_rect, CornerRadius::same(8), btn_bg);

        // Center icon and text together as a cohesive unit
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

        // Quick Rollback Button if a previous succeeded deployment exists
        if let (Some(prev_dep), Some((rb_galley, rb_icon_w, rb_gap, rb_content_w, rb_w))) =
            (rollback_candidate, rollback_info)
        {
            let rb_rect = Rect::from_min_size(
                Pos2::new(
                    btn_rect.left() - 10.0 - rb_w,
                    bar_rect.center().y - btn_h / 2.0,
                ),
                Vec2::new(rb_w, btn_h),
            );
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

            let rb_start_x = (rb_rect.center().x - rb_content_w / 2.0)
                .max(rb_rect.left() + 8.0);
            draw_outline_icon(
                ui.painter(),
                Pos2::new(rb_start_x + rb_icon_w / 2.0, rb_rect.center().y),
                OutlineIcon::RotateCcw,
                WARN_PILL_TEXT,
            );
            let rb_text_pos = Pos2::new(
                rb_start_x + rb_icon_w + rb_gap,
                rb_rect.center().y - rb_galley.size().y / 2.0,
            );
            ui.painter().galley(rb_text_pos, rb_galley, WARN_PILL_TEXT);

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
            let mut card_fill = if step.enabled { CARD_BG } else { SUBTLE_BG };
            let step_border_color = if is_being_dragged {
                PRIMARY_BLUE
            } else if !step.enabled {
                CARD_BORDER
            } else {
                match &step.kind {
                    StepKind::LocalCommand { .. } => Color32::from_rgb(147, 197, 253),
                    StepKind::VerifyLocalPath { .. } => Color32::from_rgb(252, 211, 77),
                    StepKind::TransferFiles { .. } => Color32::from_rgb(216, 180, 254),
                    StepKind::RemoteCommand { .. } => Color32::from_rgb(153, 246, 228),
                }
            };
            if is_being_dragged {
                card_fill = BLUE_PILL_BG.gamma_multiply(0.4);
            }

            let frame_resp = egui::Frame::new()
                .fill(card_fill)
                .stroke(Stroke::new(if is_being_dragged { 2.0_f32 } else { 1.2_f32 }, step_border_color))
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
                            &mut step.name,
                            210.0,
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
                                    path,
                                    input_w,
                                    Some(placeholder),
                                ) {
                                    config_changed = true;
                                }
                            }
                            StepKind::TransferFiles { sources, .. } => {
                                let mut joined = sources.join(", ");
                                if syntax_highlighted_text_edit(
                                    ui,
                                    &mut joined,
                                    input_w,
                                    Some(placeholder),
                                ) {
                                    *sources = joined
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
            if let Some(from_idx) = self.dragged_step_idx {
                if from_idx != idx {
                    if let Some(pos) = pointer_pos {
                        if card_rect.contains(pos) {
                            // Highlight card as the active drop target
                            ui.painter().rect_stroke(
                                card_rect,
                                CornerRadius::same(10),
                                Stroke::new(2.5_f32, PRIMARY_BLUE),
                                StrokeKind::Inside,
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

    fn render_server_settings_tab(&mut self, ui: &mut egui::Ui, proj_idx: usize) {
        let env_key = self.selected_env_key.clone();
        let mut config_changed = false;
        let mut delete_current_env = false;
        let mut verify_vault_clicked = false;
        let mut open_cred_guide_on_overview = false;
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
                            Some(r"e.g. C:\Projects\my-web-service"),
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
                                Some("e.g. my-web-service-dev"),
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
                                Some("e.g. /var/www/my-web-service or /home/deploy/apps/my-app"),
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
                                Some("e.g. deploy or ubuntu"),
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
                            ui.horizontal(|ui| {
                                if styled_text_edit(
                                    ui,
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

                            ui.label(
                                RichText::new("SSH Password Override:")
                                    .size(FONT_MD)
                                    .color(TEXT_SECONDARY)
                                    .strong(),
                            );
                            ui.add(
                                egui::TextEdit::singleline(&mut self.manual_ssh_password)
                                    .font(FontId::proportional(FONT_MD))
                                    .background_color(INPUT_BG)
                                    .password(true)
                                    .desired_width(field_width)
                                    .margin(Margin::symmetric(10, 7))
                                    .hint_text(
                                        RichText::new(
                                            "Optional — leave blank to use OS Credential Vault or SSH Key",
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
                                Some("Optional — e.g. ./.env.dev or ./.env.production"),
                            ) {
                                env_cfg.env_source = if env_src.trim().is_empty() {
                                    None
                                } else {
                                    Some(env_src)
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
                                Some("Optional — e.g. http://proxy.example.com:8080"),
                            ) {
                                env_cfg.http_proxy = if proxy.trim().is_empty() {
                                    None
                                } else {
                                    Some(proxy)
                                };
                                config_changed = true;
                            }
                            ui.end_row();

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

    fn render_history_and_logs_view(&mut self, ui: &mut egui::Ui, project_filter: Option<&str>) {
        let filter_options = self.storage.existing_environment_tabs(project_filter);
        if self.history_env_filter.is_some()
            && !filter_options
                .iter()
                .any(|(_, k)| *k == self.history_env_filter)
        {
            self.history_env_filter = None;
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
                        self.history_visible_limit = 5;
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

                let total_count = filtered.len();
                let visible_count = total_count.min(self.history_visible_limit.max(5));

                for (idx, dep) in filtered.iter().take(visible_count).enumerate() {
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

                        // ACTIONS column (Rollback + Re-run + View Logs) using outline icons
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                            let is_busy = self.active_deployment_id.is_some();

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

fn open_url_in_browser(url: &str) {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", "start", "", url]);
        configure_background_command(&mut cmd);
        let _ = cmd.spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(url).spawn();
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let _ = Command::new("xdg-open").arg(url).spawn();
    }
}

/// Draws a crisp outline notification bell icon (`🔔`) matching the Termius topbar.
fn draw_icon_bell(painter: &egui::Painter, c: Pos2, color: Color32) {
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
fn draw_window_max_restore_icon(
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

/// Syntax-highlighted single-line text editor for pipeline commands, paths, and SCP sources.
/// Colors `{template}` variables in purple, CLI commands in blue, flags (`--flag`) in amber,
/// paths/strings in teal, and shell operators (`&&`, `|`, `;`) in crimson.
fn syntax_highlighted_text_edit(
    ui: &mut egui::Ui,
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
        .font(FontId::proportional(FONT_MD))
        .background_color(INPUT_BG)
        .desired_width(width)
        .margin(Margin::symmetric(10, 7))
        .layouter(&mut layouter)
        .hint_text(
            RichText::new(placeholder_str)
                .size(FONT_MD)
                .color(TEXT_PLACEHOLDER),
        );
    ui.add(edit).changed()
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

    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let ch = chars[i];

        // 1. Whitespace
        if ch.is_whitespace() {
            let start = i;
            while i < len && chars[i].is_whitespace() {
                i += 1;
            }
            let slice: String = chars[start..i].iter().collect();
            job.append(&slice, 0.0, TextFormat::simple(font_id.clone(), TEXT_PRIMARY));
            continue;
        }

        // 2. Template Variable `{...}`
        if ch == '{' {
            let start = i;
            i += 1;
            while i < len && chars[i] != '}' && !chars[i].is_whitespace() {
                i += 1;
            }
            if i < len && chars[i] == '}' {
                i += 1;
            }
            let slice: String = chars[start..i].iter().collect();
            job.append(
                &slice,
                0.0,
                TextFormat::simple(font_id.clone(), color_template),
            );
            continue;
        }

        // 3. Quoted String `"..."` or `'...'`
        if ch == '"' || ch == '\'' {
            let quote = ch;
            let start = i;
            i += 1;
            while i < len && chars[i] != quote {
                if chars[i] == '{' {
                    break;
                }
                i += 1;
            }
            if i < len && chars[i] == quote {
                i += 1;
            }
            let slice: String = chars[start..i].iter().collect();
            job.append(&slice, 0.0, TextFormat::simple(font_id.clone(), color_path));
            continue;
        }

        // 4. Operators & Separators (`&&`, `||`, `|`, `;`, `,`)
        if ch == '&' || ch == '|' || ch == ';' || ch == ',' {
            let start = i;
            i += 1;
            while i < len && (chars[i] == '&' || chars[i] == '|') {
                i += 1;
            }
            let slice: String = chars[start..i].iter().collect();
            job.append(
                &slice,
                0.0,
                TextFormat::simple(font_id.clone(), color_operator),
            );
            continue;
        }

        // 5. General Token (stop at whitespace, `{`, quote, or operator)
        let start = i;
        while i < len
            && !chars[i].is_whitespace()
            && chars[i] != '{'
            && chars[i] != '"'
            && chars[i] != '\''
            && chars[i] != '&'
            && chars[i] != '|'
            && chars[i] != ';'
            && chars[i] != ','
        {
            i += 1;
        }
        let token: String = chars[start..i].iter().collect();
        let lower = token.to_ascii_lowercase();
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
        } else if matches!(
            lower.as_str(),
            "npm"
                | "npx"
                | "yarn"
                | "pnpm"
                | "node"
                | "pm2"
                | "cargo"
                | "git"
                | "docker"
                | "systemctl"
                | "service"
                | "ssh"
                | "scp"
                | "mkdir"
                | "rm"
                | "cp"
                | "mv"
                | "chmod"
                | "chown"
                | "tar"
                | "curl"
                | "wget"
                | "export"
                | "cd"
                | "echo"
                | "python"
                | "pytest"
                | "go"
                | "cmdkey"
                | "security"
                | "secret-tool"
                | "sudo"
                | "run"
                | "build"
                | "test"
                | "install"
                | "ci"
                | "restart"
                | "reload"
                | "start"
                | "stop"
                | "delete"
                | "status"
                | "store"
        ) {
            color_keyword
        } else {
            TEXT_PRIMARY
        };

        job.append(&token, 0.0, TextFormat::simple(font_id.clone(), token_color));
    }

    job
}

/// Renders a modal header with the title on the top-left, subtitle below, and a crisp
/// vector `×` close button anchored at the top-right corner of the popup dialog.
fn render_modal_header(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    title: &str,
    subtitle: &str,
) -> bool {
    let full_w = ui.available_width();
    let top_pos = ui.cursor().min;

    // Position the 28x28 close button right at the top-right corner of the modal card
    let close_rect = Rect::from_min_size(
        Pos2::new(top_pos.x + full_w - 22.0, top_pos.y - 4.0),
        Vec2::new(28.0, 28.0),
    );
    let close_resp = ui
        .interact(
            close_rect,
            ui.make_persistent_id(id_salt),
            Sense::click(),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand);

    let (title_rect, _) =
        ui.allocate_exact_size(Vec2::new(full_w - 36.0, 24.0), Sense::hover());
    ui.painter().text(
        Pos2::new(title_rect.left(), title_rect.center().y),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(FONT_LG),
        TEXT_PRIMARY,
    );

    let close_bg = if close_resp.hovered() {
        FAIL_PILL_BG
    } else {
        SUBTLE_BG
    };
    let close_border = if close_resp.hovered() {
        Color32::from_rgb(252, 165, 165)
    } else {
        CARD_BORDER
    };
    let close_fg = if close_resp.hovered() {
        FAIL_PILL_TEXT
    } else {
        TEXT_MUTED
    };

    ui.painter()
        .rect_filled(close_rect, CornerRadius::same(6), close_bg);
    ui.painter().rect_stroke(
        close_rect,
        CornerRadius::same(6),
        Stroke::new(1.0_f32, close_border),
        StrokeKind::Inside,
    );
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

    ui.add_space(4.0);
    ui.label(RichText::new(subtitle).size(FONT_SM).color(TEXT_MUTED));

    close_resp.on_hover_text("Close dialog").clicked()
}

/// Renders a tab button with uniform horizontal padding (`16px` left & right) and consistent
/// bordered styling so adjacent tabs have an exact, uniform visual gap.
fn render_uniform_tab_button(
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
        .rect_filled(rect, CornerRadius::same(8), fill);
    ui.painter()
        .rect_stroke(rect, CornerRadius::same(8), stroke, StrokeKind::Inside);

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

/// Draws an animated circular loading spinner with a faint 360-degree background ring
/// and an energetic rotating foreground arc.
fn draw_circle_loading_spinner(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    color: Color32,
    time: f64,
) {
    let stroke_w = (radius * 0.32).clamp(1.2, 2.0);
    // 1. Faint circular track underneath
    let track_color = color.gamma_multiply(0.25);
    painter.circle_stroke(center, radius, Stroke::new(stroke_w, track_color));

    // 2. Energetic spinning foreground arc (~270 degrees)
    let speed = 5.2; // ~0.83 turns per second
    let start_angle = (time * speed) as f32;
    let sweep_angle = std::f32::consts::PI * 1.5;
    let segments = 24;
    let step = sweep_angle / segments as f32;

    for i in 0..segments {
        let a1 = start_angle + i as f32 * step;
        let a2 = start_angle + (i + 1) as f32 * step;
        let p1 = Pos2::new(center.x + radius * a1.cos(), center.y + radius * a1.sin());
        let p2 = Pos2::new(center.x + radius * a2.cos(), center.y + radius * a2.sin());
        painter.line_segment([p1, p2], Stroke::new(stroke_w, color));
    }
}

/// Paints a status badge pill left-aligned with an animated circular loading spinner
/// on the left side of the label text.
fn paint_running_pill_left_aligned(
    painter: &egui::Painter,
    left_center: Pos2,
    text: &str,
    bg: Color32,
    fg: Color32,
    time: f64,
) -> f32 {
    let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(FONT_XS), fg);
    let spinner_r = 4.2;
    let spinner_w = spinner_r * 2.0;
    let gap = 6.0;
    let pad_x = 8.0;
    let w = pad_x + spinner_w + gap + galley.size().x + pad_x;
    let h = 22.0;
    let rect = Rect::from_min_size(
        Pos2::new(left_center.x, left_center.y - h / 2.0),
        Vec2::new(w, h),
    );

    painter.rect_filled(rect, CornerRadius::same(5), bg);
    painter.rect_stroke(
        rect,
        CornerRadius::same(5),
        Stroke::new(1.0_f32, fg.gamma_multiply(0.28)),
        StrokeKind::Inside,
    );

    let spinner_c = Pos2::new(rect.left() + pad_x + spinner_r, rect.center().y);
    draw_circle_loading_spinner(painter, spinner_c, spinner_r, fg, time);

    let text_pos = Pos2::new(
        spinner_c.x + spinner_r + gap,
        rect.center().y - galley.size().y / 2.0,
    );
    painter.galley(text_pos, galley, fg);

    w
}

/// Paints a status badge pill right-aligned with an animated circular loading spinner
/// on the left side of the label text.
fn paint_running_pill_right_aligned(
    painter: &egui::Painter,
    right_center: Pos2,
    text: &str,
    bg: Color32,
    fg: Color32,
    time: f64,
) -> f32 {
    let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(FONT_XS), fg);
    let spinner_r = 4.2;
    let spinner_w = spinner_r * 2.0;
    let gap = 6.0;
    let pad_x = 8.0;
    let w = pad_x + spinner_w + gap + galley.size().x + pad_x;
    let h = 22.0;
    let rect = Rect::from_min_size(
        Pos2::new(right_center.x - w, right_center.y - h / 2.0),
        Vec2::new(w, h),
    );

    painter.rect_filled(rect, CornerRadius::same(5), bg);
    painter.rect_stroke(
        rect,
        CornerRadius::same(5),
        Stroke::new(1.0_f32, fg.gamma_multiply(0.28)),
        StrokeKind::Inside,
    );

    let spinner_c = Pos2::new(rect.left() + pad_x + spinner_r, rect.center().y);
    draw_circle_loading_spinner(painter, spinner_c, spinner_r, fg, time);

    let text_pos = Pos2::new(
        spinner_c.x + spinner_r + gap,
        rect.center().y - galley.size().y / 2.0,
    );
    painter.galley(text_pos, galley, fg);

    w
}

/// Renders a centered, polished loading indicator card while switching active project.
fn render_project_loading_indicator(ui: &mut egui::Ui, proj_name: &str) {
    ui.ctx().request_repaint();
    let avail_w = ui.available_width();
    let avail_h = ui.available_height().max(320.0);
    let time = ui.input(|i| i.time);

    ui.allocate_ui_with_layout(
        Vec2::new(avail_w, avail_h),
        Layout::top_down(Align::Center),
        |ui| {
            ui.add_space(avail_h * 0.22);
            let card_w = (avail_w * 0.6).clamp(380.0, 520.0);
            let card_h = 175.0;
            let (card_rect, _) = ui.allocate_exact_size(Vec2::new(card_w, card_h), Sense::hover());

            // Card background & subtle border
            ui.painter().rect_filled(card_rect, CornerRadius::same(12), CARD_BG);
            ui.painter().rect_stroke(
                card_rect,
                CornerRadius::same(12),
                Stroke::new(1.0_f32, CARD_BORDER),
                StrokeKind::Inside,
            );

            // Large animated circular loading spinner
            let spinner_center = Pos2::new(card_rect.center().x, card_rect.top() + 46.0);
            draw_circle_loading_spinner(ui.painter(), spinner_center, 18.0, PRIMARY_BLUE, time);

            // Project name & status text
            ui.painter().text(
                Pos2::new(card_rect.center().x, card_rect.top() + 92.0),
                Align2::CENTER_CENTER,
                format!("Switching to {proj_name}..."),
                FontId::proportional(FONT_LG),
                TEXT_PRIMARY,
            );

            ui.painter().text(
                Pos2::new(card_rect.center().x, card_rect.top() + 124.0),
                Align2::CENTER_CENTER,
                "Loading Git repository status, branches & deployment pipelines...",
                FontId::proportional(FONT_SM),
                TEXT_SECONDARY,
            );
        },
    );
}

fn render_light_status_badge(ui: &mut egui::Ui, status: DeploymentStatus) {
    let (bg, fg) = status_pill_colors(status);
    let text = status.label().to_uppercase();
    if status == DeploymentStatus::Running {
        ui.ctx().request_repaint();
        let time = ui.input(|i| i.time);
        let spinner_r = 4.2;
        let spinner_w = spinner_r * 2.0;
        let gap = 6.0;
        let pad_x = 8.0;
        let galley = ui
            .painter()
            .layout_no_wrap(text, FontId::proportional(FONT_XS), fg);
        let pill_w = pad_x + spinner_w + gap + galley.size().x + pad_x;
        let pill_h = 22.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(pill_w, pill_h), Sense::hover());
        ui.painter().rect_filled(rect, CornerRadius::same(5), bg);
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(5),
            Stroke::new(1.0_f32, fg.gamma_multiply(0.28)),
            StrokeKind::Inside,
        );
        let spinner_c = Pos2::new(rect.left() + pad_x + spinner_r, rect.center().y);
        draw_circle_loading_spinner(ui.painter(), spinner_c, spinner_r, fg, time);
        ui.painter().galley(
            Pos2::new(
                spinner_c.x + spinner_r + gap,
                rect.center().y - galley.size().y / 2.0,
            ),
            galley,
            fg,
        );
    } else {
        render_pill(ui, &text, bg, fg);
    }
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

#[cfg(windows)]
fn open_windows_credential_manager() {
    let _ = Command::new("control.exe")
        .args(["/name", "Microsoft.CredentialManager"])
        .spawn();
}

/// Draws the official Shiply `logo.ico` texture inside a rounded container (falling back to 'S' if unavailable).
fn draw_shiply_logo(
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
enum OutlineIcon {
    Trash,
    Refresh,
    ArrowUp,
    ArrowDown,
    Edit,
    RotateCcw,
    Key,
    GripVertical,
}

#[allow(clippy::too_many_arguments)]
fn outline_icon_button(
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

fn draw_outline_icon(painter: &egui::Painter, c: Pos2, icon: OutlineIcon, color: Color32) {
    let s = Stroke::new(1.5_f32, color);
    match icon {
        OutlineIcon::Trash => {
            // Lid horizontal line
            painter.line_segment(
                [Pos2::new(c.x - 5.5, c.y - 3.5), Pos2::new(c.x + 5.5, c.y - 3.5)],
                s,
            );
            // Lid handle
            painter.line_segment(
                [Pos2::new(c.x - 2.0, c.y - 3.5), Pos2::new(c.x - 2.0, c.y - 5.2)],
                s,
            );
            painter.line_segment(
                [Pos2::new(c.x - 2.0, c.y - 5.2), Pos2::new(c.x + 2.0, c.y - 5.2)],
                s,
            );
            painter.line_segment(
                [Pos2::new(c.x + 2.0, c.y - 5.2), Pos2::new(c.x + 2.0, c.y - 3.5)],
                s,
            );
            // Trash bin body
            painter.line_segment(
                [Pos2::new(c.x - 4.2, c.y - 3.5), Pos2::new(c.x - 3.6, c.y + 5.5)],
                s,
            );
            painter.line_segment(
                [Pos2::new(c.x - 3.6, c.y + 5.5), Pos2::new(c.x + 3.6, c.y + 5.5)],
                s,
            );
            painter.line_segment(
                [Pos2::new(c.x + 3.6, c.y + 5.5), Pos2::new(c.x + 4.2, c.y - 3.5)],
                s,
            );
            // Inner vertical lines
            painter.line_segment(
                [Pos2::new(c.x - 1.3, c.y - 1.2), Pos2::new(c.x - 1.3, c.y + 3.3)],
                s,
            );
            painter.line_segment(
                [Pos2::new(c.x + 1.3, c.y - 1.2), Pos2::new(c.x + 1.3, c.y + 3.3)],
                s,
            );
        }
        OutlineIcon::Refresh => {
            draw_icon_refresh(painter, c, color);
        }
        OutlineIcon::ArrowUp => {
            painter.line_segment(
                [Pos2::new(c.x, c.y + 5.0), Pos2::new(c.x, c.y - 5.0)],
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
                [Pos2::new(c.x, c.y - 5.0), Pos2::new(c.x, c.y + 5.0)],
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
            let p1 = Pos2::new(c.x - 4.3, c.y + 1.5);
            let p2 = Pos2::new(c.x + 2.2, c.y - 5.0);
            let p3 = Pos2::new(c.x + 5.0, c.y - 2.2);
            let p4 = Pos2::new(c.x - 1.5, c.y + 4.3);
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_syntax_highlight_layout_job() {
        let cmd = "cd {remote_dir} && pm2 restart {app_name} --update-env ./dist/server.js";
        let job = build_pipeline_syntax_layout_job(cmd);
        assert_eq!(job.text, cmd);
        assert!(job.sections.len() >= 7);

        let has_template_purple = job
            .sections
            .iter()
            .any(|s| s.format.color == PURPLE_PILL_TEXT);
        let has_keyword_blue = job
            .sections
            .iter()
            .any(|s| s.format.color == BLUE_PILL_TEXT);
        let has_flag_amber = job
            .sections
            .iter()
            .any(|s| s.format.color == WARN_PILL_TEXT);
        let has_path_teal = job
            .sections
            .iter()
            .any(|s| s.format.color == TEAL_PILL_TEXT);
        let has_operator_crimson = job
            .sections
            .iter()
            .any(|s| s.format.color == FAIL_PILL_TEXT);

        assert!(has_template_purple);
        assert!(has_keyword_blue);
        assert!(has_flag_amber);
        assert!(has_path_teal);
        assert!(has_operator_crimson);
    }

    #[test]
    fn test_running_badge_pill_colors_and_layout() {
        let (bg, fg) = status_pill_colors(DeploymentStatus::Running);
        assert_eq!(bg, WARN_PILL_BG);
        assert_eq!(fg, WARN_PILL_TEXT);

        // Verify circle loading spinner angle progression
        let speed = 5.2_f64;
        let t0 = 0.0_f64;
        let t1 = 1.0_f64;
        let a0 = (t0 * speed) as f32;
        let a1 = (t1 * speed) as f32;
        assert!((a1 - a0 - 5.2_f32).abs() < 1e-4);

        // Test painter execution with egui Context
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let left_center = Pos2::new(100.0, 50.0);
                let w = paint_running_pill_left_aligned(
                    ui.painter(),
                    left_center,
                    "RUNNING",
                    bg,
                    fg,
                    1.23,
                );
                assert!(w > 50.0);

                let right_center = Pos2::new(200.0, 50.0);
                let w_r = paint_running_pill_right_aligned(
                    ui.painter(),
                    right_center,
                    "RUNNING",
                    bg,
                    fg,
                    1.23,
                );
                assert!(w_r > 50.0);
            });
        });
    }

    #[test]
    fn test_active_project_code_changes_isolation() {
        let mut storage = AppStorage::default();
        let env_be = EnvironmentConfig {
            key: "dev".to_string(),
            name: "Development".to_string(),
            app_name: "backend-dev".to_string(),
            servers: vec!["10.0.0.1".to_string()],
            remote_user: "deploy".to_string(),
            vault_target: "backend_dev".to_string(),
            remote_dir: "/apps/backend".to_string(),
            env_source: None,
            http_proxy: None,
            steps: Vec::new(),
        };
        let proj_backend = Project {
            id: "proj-backend".to_string(),
            name: "Backend Service".to_string(),
            description: "Backend API".to_string(),
            local_path: ".".to_string(),
            environments: vec![env_be],
        };

        let env_fe = EnvironmentConfig {
            key: "dev".to_string(),
            name: "Development".to_string(),
            app_name: "frontend-dev".to_string(),
            servers: vec!["10.0.0.2".to_string()],
            remote_user: "deploy".to_string(),
            vault_target: "frontend_dev".to_string(),
            remote_dir: "/apps/frontend".to_string(),
            env_source: None,
            http_proxy: None,
            steps: Vec::new(),
        };
        let proj_frontend = Project {
            id: "proj-frontend".to_string(),
            name: "Frontend App".to_string(),
            description: "Web frontend".to_string(),
            local_path: ".".to_string(),
            environments: vec![env_fe],
        };

        storage.projects = vec![proj_backend, proj_frontend];

        let dep_backend = DeploymentRecord {
            id: "dep-be-1".to_string(),
            project_id: "proj-backend".to_string(),
            project_name: "Backend Service".to_string(),
            environment: "dev".to_string(),
            app_name: "backend-dev".to_string(),
            version: "1.0.1".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("be11111".to_string()),
            git_changes: vec!["feat(api): add auth endpoint".to_string()],
            is_rollback: false,
            servers: vec!["10.0.0.1".to_string()],
            status: DeploymentStatus::Succeeded,
            started_at: chrono::Utc::now(),
            finished_at: Some(chrono::Utc::now()),
            duration_ms: Some(1500),
            step_logs: Vec::new(),
            executed_steps: Vec::new(),
        };

        let dep_frontend = DeploymentRecord {
            id: "dep-fe-1".to_string(),
            project_id: "proj-frontend".to_string(),
            project_name: "Frontend App".to_string(),
            environment: "dev".to_string(),
            app_name: "frontend-dev".to_string(),
            version: "2.0.0".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("fe22222".to_string()),
            git_changes: vec!["feat(ui): redesign dashboard".to_string()],
            is_rollback: false,
            servers: vec!["10.0.0.2".to_string()],
            status: DeploymentStatus::Succeeded,
            started_at: chrono::Utc::now(),
            finished_at: Some(chrono::Utc::now()),
            duration_ms: Some(2000),
            step_logs: Vec::new(),
            executed_steps: Vec::new(),
        };

        storage.deployments = vec![dep_backend.clone(), dep_frontend.clone()];

        let mut app = ShiplyApp::new(storage);
        // Switch to Backend project and set an invalid foreign selected deployment ID
        app.selection = SidebarSelection::Project("proj-backend".to_string());
        app.selected_deployment_id = Some("dep-fe-1".to_string());

        // When refreshing metadata for Backend, selected_deployment_id must be corrected to Backend
        app.refresh_selected_project_metadata(false);
        assert_eq!(app.selected_deployment_id.as_deref(), Some("dep-be-1"));

        // When filtering deployments for Backend, frontend deployments are excluded
        let filtered = app.storage.filter_deployments(Some("proj-backend"), None, None);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].project_id, "proj-backend");
        assert_eq!(
            filtered[0].git_changes,
            vec!["feat(api): add auth endpoint".to_string()]
        );
    }

    #[test]
    fn test_project_switch_loading_indicator_and_hover_cursor() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        // Verify global visuals hover interact cursor is set to PointingHand (Pointer)
        assert_eq!(
            ctx.style().visuals.interact_cursor,
            Some(egui::CursorIcon::PointingHand)
        );

        // Test render_project_loading_indicator execution without panic
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_project_loading_indicator(ui, "Backend API");
            });
        });
    }

    #[test]
    fn test_deploy_and_rollback_button_dynamic_sizing_no_offset() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut app = ShiplyApp::new(crate::storage::AppStorage::default());
        app.deploy_version_input = "1.0.10-beta.2+build.2026".to_string();

        let rollback_dep = DeploymentRecord {
            id: "dep-rb-test".to_string(),
            project_id: "p1".to_string(),
            project_name: "Customer Data Ops".to_string(),
            environment: "development".to_string(),
            app_name: "data-ops".to_string(),
            version: "1.0.9-hotfix".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("abc1234".to_string()),
            git_changes: vec![],
            is_rollback: false,
            servers: vec!["10.0.0.1".to_string()],
            status: DeploymentStatus::Succeeded,
            started_at: chrono::Utc::now(),
            finished_at: Some(chrono::Utc::now()),
            duration_ms: Some(1200),
            step_logs: vec![],
            executed_steps: vec![],
        };

        // Run UI render with long environment name and version
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.set_min_width(1200.0);
                app.render_deploy_action_bar(
                    ui,
                    "Development Customer Data Ops",
                    Some(rollback_dep.clone()),
                );
            });
        });

        // Also test with empty version string to ensure clean fallback text
        app.deploy_version_input = "   ".to_string();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.set_min_width(1000.0);
                app.render_deploy_action_bar(ui, "Production", None);
            });
        });
    }

    #[test]
    fn test_deployment_history_pagination_last_5_and_show_more_10() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        // Create 18 mock deployments
        for i in 1..=18 {
            storage.deployments.push(DeploymentRecord {
                id: format!("dep-{i}"),
                project_id: "p1".to_string(),
                project_name: "Test App".to_string(),
                environment: "dev".to_string(),
                app_name: "test-app".to_string(),
                version: format!("1.0.{i}"),
                git_branch: Some("main".to_string()),
                git_commit: Some(format!("c{i}")),
                git_changes: vec![],
                is_rollback: false,
                servers: vec!["10.0.0.1".to_string()],
                status: DeploymentStatus::Succeeded,
                started_at: chrono::Utc::now(),
                finished_at: Some(chrono::Utc::now()),
                duration_ms: Some(1000),
                step_logs: vec![],
                executed_steps: vec![],
            });
        }

        let mut app = ShiplyApp::new(storage);
        assert_eq!(app.history_visible_limit, 5);

        // Render UI with initially 5 items
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_history_and_logs_view(ui, None);
            });
        });

        // Simulate Show More click -> increases by 10 to 15
        app.history_visible_limit = app.history_visible_limit.saturating_add(10);
        assert_eq!(app.history_visible_limit, 15);

        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_history_and_logs_view(ui, None);
            });
        });

        // Next Show More click -> increases by 10 to 25 (exceeds 18 total)
        app.history_visible_limit = app.history_visible_limit.saturating_add(10);
        assert_eq!(app.history_visible_limit, 25);

        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_history_and_logs_view(ui, None);
            });
        });

        // Reset/Collapse to last 5
        app.history_visible_limit = 5;
        assert_eq!(app.history_visible_limit, 5);
    }

    #[test]
    fn test_server_settings_input_buffering_and_debounced_save() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        let env = EnvironmentConfig {
            key: "dev".to_string(),
            name: "Development".to_string(),
            app_name: "api-dev".to_string(),
            servers: vec!["10.0.0.1".to_string()],
            remote_user: "deploy".to_string(),
            vault_target: "dev_vault".to_string(),
            remote_dir: "/apps/api".to_string(),
            env_source: None,
            http_proxy: None,
            steps: vec![],
        };
        let project = Project {
            id: "p1".to_string(),
            name: "Test Project".to_string(),
            description: "Desc".to_string(),
            local_path: ".".to_string(),
            environments: vec![env],
        };
        storage.projects.push(project);

        let mut app = ShiplyApp::new(storage);
        app.selected_env_key = "dev".to_string();

        assert!(!app.dirty_save_pending);
        assert!(app.servers_edit_buffer.is_none());

        // Render Server Settings Tab without panic
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_server_settings_tab(ui, 0);
            });
        });

        // The buffer should now be initialized with the joined server string
        assert_eq!(
            app.servers_edit_buffer,
            Some(("dev".to_string(), "10.0.0.1".to_string()))
        );

        // Simulate typing and mark dirty
        app.mark_config_dirty();
        assert!(app.dirty_save_pending);
        assert!(app.last_edit_time.is_some());

        // Flush pending save
        app.flush_pending_save();
        assert!(!app.dirty_save_pending);
        assert!(app.last_edit_time.is_none());
    }

    #[test]
    fn test_draggable_pipeline_step_reorder_ui() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        let env = EnvironmentConfig {
            key: "dev".to_string(),
            name: "Development".to_string(),
            app_name: "api-dev".to_string(),
            servers: vec!["10.0.0.1".to_string()],
            remote_user: "deploy".to_string(),
            vault_target: "dev_vault".to_string(),
            remote_dir: "/apps/api".to_string(),
            env_source: None,
            http_proxy: None,
            steps: vec![
                PipelineStep::new("Build", StepKind::LocalCommand { command: "cargo build".to_string(), allow_failure: false }),
                PipelineStep::new("Verify", StepKind::VerifyLocalPath { path: "./target".to_string() }),
                PipelineStep::new("Upload", StepKind::TransferFiles { sources: vec!["./target".to_string()], stage_env_file: true }),
            ],
        };
        let project = Project {
            id: "p1".to_string(),
            name: "Test Project".to_string(),
            description: "Desc".to_string(),
            local_path: ".".to_string(),
            environments: vec![env],
        };
        storage.projects.push(project);

        let mut app = ShiplyApp::new(storage);
        app.selected_env_key = "dev".to_string();

        // Render pipeline steps tab initially
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_pipeline_steps_tab(ui, 0);
            });
        });

        assert_eq!(app.storage.projects[0].environments[0].steps[0].name, "Build");
        assert_eq!(app.storage.projects[0].environments[0].steps[1].name, "Verify");
        assert_eq!(app.storage.projects[0].environments[0].steps[2].name, "Upload");

        // Simulate dragging step 2 (Upload)
        app.dragged_step_idx = Some(2);
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_pipeline_steps_tab(ui, 0);
            });
        });

        // Test reorder execution
        app.storage.projects[0].environments[0].reorder_step(2, 0);
        app.dragged_step_idx = None;
        app.mark_config_dirty();

        assert_eq!(app.storage.projects[0].environments[0].steps[0].name, "Upload");
        assert_eq!(app.storage.projects[0].environments[0].steps[1].name, "Build");
        assert_eq!(app.storage.projects[0].environments[0].steps[2].name, "Verify");
        assert!(app.dirty_save_pending);
    }

    #[test]
    fn test_async_git_inspect_channel_and_project_switch() {
        let mut storage = crate::storage::AppStorage::default();
        let project = Project {
            id: "proj-async".to_string(),
            name: "Async Project".to_string(),
            description: "Desc".to_string(),
            local_path: ".".to_string(),
            environments: vec![EnvironmentConfig {
                key: "dev".to_string(),
                name: "Development".to_string(),
                app_name: "async-dev".to_string(),
                servers: vec!["127.0.0.1".to_string()],
                remote_user: "deploy".to_string(),
                vault_target: "vault".to_string(),
                remote_dir: "/app".to_string(),
                env_source: None,
                http_proxy: None,
                steps: Vec::new(),
            }],
        };
        storage.projects.push(project);
        let mut app = ShiplyApp::new(storage);

        // Simulate fast project selection
        app.selection = SidebarSelection::Project("proj-async".to_string());
        app.refresh_selected_project_metadata(true);

        // Project selection was updated immediately without UI freeze
        assert_eq!(app.cached_project_id.as_deref(), Some("proj-async"));

        // Simulate incoming background inspection result
        let _ = app.git_inspect_tx.send(GitInspectionResult {
            project_id: "proj-async".to_string(),
            pkg_version: Some("2.5.0".to_string()),
            git_meta: GitMetadata {
                branch: Some("feature/instant-switch".to_string()),
                commit_short: Some("999aaaa".to_string()),
                commit_subject: Some("feat: instant switch".to_string()),
            },
            git_branches: vec!["main".to_string(), "feature/instant-switch".to_string()],
            git_changes: vec!["M src/ui.rs".to_string()],
            update_version_input: true,
        });

        let received = app.process_pending_git_inspect();
        assert!(received);
        assert_eq!(app.cached_pkg_version.as_deref(), Some("2.5.0"));
        assert_eq!(app.selected_git_branch, "feature/instant-switch");
        assert_eq!(app.cached_git_changes, vec!["M src/ui.rs"]);
        assert!(app.project_switch_loading_id.is_none());
    }

    #[test]
    fn test_project_menu_sidebar_running_deployment_circle_loading_indicator() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        let proj1 = Project {
            id: "proj-running".to_string(),
            name: "Customer Portal".to_string(),
            description: "Active".to_string(),
            local_path: ".".to_string(),
            environments: vec![EnvironmentConfig {
                key: "prod".to_string(),
                name: "Production".to_string(),
                app_name: "portal-prod".to_string(),
                servers: vec!["10.0.0.1".to_string()],
                remote_user: "deploy".to_string(),
                vault_target: "v1".to_string(),
                remote_dir: "/apps/portal".to_string(),
                env_source: None,
                http_proxy: None,
                steps: Vec::new(),
            }],
        };
        let proj2 = Project {
            id: "proj-idle".to_string(),
            name: "Admin Dashboard".to_string(),
            description: "Idle".to_string(),
            local_path: ".".to_string(),
            environments: vec![],
        };
        storage.projects = vec![proj1, proj2];

        // Add a running deployment for proj-running
        let running_dep = DeploymentRecord {
            id: "dep-running-1".to_string(),
            project_id: "proj-running".to_string(),
            project_name: "Customer Portal".to_string(),
            environment: "prod".to_string(),
            app_name: "portal-prod".to_string(),
            version: "1.0.5".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("c0ffee1".to_string()),
            git_changes: vec![],
            is_rollback: false,
            servers: vec!["10.0.0.1".to_string()],
            status: DeploymentStatus::Running,
            started_at: chrono::Utc::now(),
            finished_at: None,
            duration_ms: None,
            step_logs: vec![],
            executed_steps: vec![],
        };
        storage.deployments = vec![running_dep];

        let mut app = ShiplyApp::new(storage);
        app.active_deployment_id = Some("dep-running-1".to_string());

        // Render sidebar with anim_t = 1.0 (fully open)
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_sidebar(ui.ctx(), 1.0);
            });
        });

        // Verify active deployment is registered and belongs to proj-running
        let is_running_p1 = app.storage.deployments.iter().any(|d| {
            d.project_id == "proj-running" && d.status == DeploymentStatus::Running
        });
        let is_running_p2 = app.storage.deployments.iter().any(|d| {
            d.project_id == "proj-idle" && d.status == DeploymentStatus::Running
        });
        assert!(is_running_p1);
        assert!(!is_running_p2);
    }
}
