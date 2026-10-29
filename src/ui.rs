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
use crate::storage::{AppStorage, ProjectSortMode};
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
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

// =============================================================================
// DESIGN TOKENS (Single Source of Truth for Sidebar & Modals)
// =============================================================================

// Typography: ONLY these 5 sizes allowed in sidebar & modals!
pub const FS_TITLE: f32 = 18.0; // Modal title (18px / 24px, weight 600)
pub const FS_BRAND: f32 = 16.0; // "Shiply" brand name (16px / 24px, weight 600)
pub const FS_BASE: f32 = 14.0;  // Nav items, project names, input text, buttons (14px / 20px, weight 500)
pub const FS_SMALL: f32 = 13.0; // Footer actions, modal description, helper text (13px / 20px, weight 400)
pub const FS_MICRO: f32 = 11.0; // "PROJECTS" label, env badges, version badge, status text (11px / 16px, weight 600)

// Spacing scale: ONLY 4, 8, 12, 16, 20, 24 px
pub const SP_4: f32 = 4.0;
pub const SP_8: f32 = 8.0;
pub const SP_12: f32 = 12.0;
pub const SP_16: f32 = 16.0;
pub const SP_20: f32 = 20.0;
pub const SP_24: f32 = 24.0;

// Radius scale
pub const R_SM: f32 = 6.0;  // badges, small buttons
pub const R_MD: f32 = 8.0;  // nav, inputs, buttons, cards
pub const R_LG: f32 = 12.0; // inner panels
pub const R_XL: f32 = 16.0; // modal

// Sidebar colors (dark)
pub const SB_BG: Color32 = Color32::from_rgb(11, 18, 32);                          // #0B1220
pub const SB_SURFACE: Color32 = Color32::from_rgb(17, 26, 46);                     // #111A2E
pub const SB_SURFACE_HOVER: Color32 = Color32::from_rgb(22, 33, 58);               // #16213A
pub const SB_BORDER: Color32 = Color32::from_rgba_premultiplied(15, 15, 15, 15);    // rgba(255,255,255,0.06)
pub const SB_TEXT: Color32 = Color32::from_rgb(230, 234, 242);                     // #E6EAF2
pub const SB_TEXT_MUTED: Color32 = Color32::from_rgb(139, 150, 173);               // #8B96AD
pub const SB_TEXT_FAINT: Color32 = Color32::from_rgb(100, 112, 138);               // #64708A
pub const ACCENT: Color32 = Color32::from_rgb(59, 130, 246);                       // #3B82F6
pub const ACCENT_SOFT: Color32 = Color32::from_rgba_premultiplied(8, 18, 35, 36);   // rgba(59,130,246,0.14)
pub const SUCCESS: Color32 = Color32::from_rgb(34, 197, 94);                       // #22C55E

pub const BADGE_DEV_BG: Color32 = Color32::from_rgba_premultiplied(9, 21, 40, 41);  // rgba(59,130,246,0.16)
pub const BADGE_DEV_TEXT: Color32 = Color32::from_rgb(125, 178, 255);              // #7DB2FF
pub const BADGE_PROD_BG: Color32 = Color32::from_rgba_premultiplied(27, 14, 40, 41); // rgba(168,85,247,0.16)
pub const BADGE_PROD_TEXT: Color32 = Color32::from_rgb(196, 161, 255);             // #C4A1FF

pub const DESTRUCTIVE_TEXT: Color32 = Color32::from_rgb(248, 113, 113);             // #F87171 (red-400)
pub const DESTRUCTIVE_HOVER_BG: Color32 = Color32::from_rgba_premultiplied(29, 8, 8, 31); // rgba(239, 68, 68, 0.12)
pub const DESTRUCTIVE_ACTIVE_BG: Color32 = Color32::from_rgba_premultiplied(48, 14, 14, 51); // rgba(239, 68, 68, 0.20)

// Modal colors (light)
pub const MD_OVERLAY: Color32 = Color32::from_rgba_premultiplied(1, 3, 13, 140);    // rgba(2,6,23,0.55)
pub const MD_BG: Color32 = Color32::WHITE;                                         // #FFFFFF
pub const MD_BORDER: Color32 = Color32::from_rgb(226, 232, 240);                   // #E2E8F0
pub const MD_TEXT: Color32 = Color32::from_rgb(15, 23, 42);                       // #0F172A
pub const MD_TEXT_MUTED: Color32 = Color32::from_rgb(100, 116, 139);               // #64748B
pub const MD_INPUT_BG: Color32 = Color32::from_rgb(248, 250, 252);                 // #F8FAFC
pub const MD_PANEL_BG: Color32 = Color32::from_rgb(241, 245, 249);                 // #F1F5F9
pub const MD_FOCUS_RING_COLOR: Color32 = Color32::from_rgba_premultiplied(18, 39, 73, 76); // rgba(59,130,246,0.30)

// =============================================================================
// Structured Typography Scale (Preserved for Main Content Area)
// =============================================================================
const FONT_XL: f32 = 20.0; // Page & Project Headers
const FONT_LG: f32 = 15.0; // Card Primary Values
const FONT_MD: f32 = 13.0; // Standard UI Text, Inputs, Buttons, Table Values
const FONT_SM: f32 = 11.5; // Secondary Metadata, Subtitles, Compact Buttons
const FONT_XS: f32 = 10.5; // Section Labels, Table Headers, Pill Badges
const POPUP_FONT_SIZE: f32 = 13.0; // Options popup menu typography (FS_SMALL)

// =============================================================================
// High-Contrast Professional SaaS Color Palette
// =============================================================================
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

#[derive(Debug, Clone)]
pub struct SidebarContextMenuState {
    pub project_id: String,
    pub click_pos: Pos2,
    pub submenu_open: bool,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncVersionCandidate {
    pub source_name: String,
    pub env_key: Option<String>,
    pub version: String,
    pub is_latest: bool,
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

    // Project Sorting & Grouping state
    project_sort_mode: ProjectSortMode,
    collapsed_project_groups: HashSet<String>,
    dragged_project_idx: Option<usize>,
    sidebar_sort_dropdown_open: bool,
    sidebar_context_menu: Option<SidebarContextMenuState>,
    sync_version_dropdown_open: bool,

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
    pub active_deployments: HashMap<(String, String), String>,
    pub active_deployment_id: Option<String>,

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
    new_project_group: String,
    new_project_imported_envs: Vec<EnvironmentConfig>,
    new_project_import_notice: Option<String>,

    // Edit Project Modal state
    show_edit_project_modal: bool,
    edit_project_id: String,
    edit_project_name: String,
    edit_project_path: String,
    edit_project_desc: String,
    edit_project_group: String,
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
    pub pending_update_restart: Option<(String, PathBuf)>,

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
    sidebar_refreshing_until: Option<Instant>,
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
            project_sort_mode: ProjectSortMode::Custom,
            collapsed_project_groups: HashSet::new(),
            dragged_project_idx: None,
            sidebar_sort_dropdown_open: false,
            sidebar_context_menu: None,
            sync_version_dropdown_open: false,
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
            active_deployments: HashMap::new(),
            active_deployment_id: None,
            show_new_project_modal: false,
            new_project_ps1_path: String::new(),
            new_project_import_siblings: true,
            new_project_name: String::new(),
            new_project_path: String::new(),
            new_project_desc: String::new(),
            new_project_group: String::new(),
            new_project_imported_envs: Vec::new(),
            new_project_import_notice: None,
            show_edit_project_modal: false,
            edit_project_id: String::new(),
            edit_project_name: String::new(),
            edit_project_path: String::new(),
            edit_project_desc: String::new(),
            edit_project_group: String::new(),
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
            pending_update_restart: None,
            last_sync_check: Instant::now(),
            status_banner: None,
            sidebar_collapsed: false,
            window_maximized: false,
            pending_delete_confirm: None,
            fonts_initialized: false,
            logo_texture: None,
            project_switch_loading_id: None,
            project_switch_loading_start: None,
            sidebar_refreshing_until: None,
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

    pub fn is_env_deploying(&self, project_id: &str, env_key: &str) -> bool {
        self.active_deployments
            .keys()
            .any(|(p, e)| p == project_id && e == env_key)
            || self
                .storage
                .deployments
                .iter()
                .any(|d| d.project_id == project_id && d.environment == env_key && d.status == DeploymentStatus::Running)
    }

    pub fn is_project_deploying(&self, project_id: &str) -> bool {
        self.active_deployments
            .keys()
            .any(|(p, _)| p == project_id)
            || self.active_deployment_id.as_ref().is_some_and(|dep_id| {
                self.storage
                    .deployments
                    .iter()
                    .any(|d| &d.id == dep_id && d.project_id == project_id)
            })
            || self
                .storage
                .deployments
                .iter()
                .any(|d| d.project_id == project_id && d.status == DeploymentStatus::Running)
    }

    pub fn active_deployment_for_env(&self, project_id: &str, env_key: &str) -> Option<String> {
        self.active_deployments
            .iter()
            .find(|((p, e), _)| p == project_id && e == env_key)
            .map(|(_, id)| id.clone())
            .or_else(|| {
                self.storage
                    .deployments
                    .iter()
                    .find(|d| d.project_id == project_id && d.environment == env_key && d.status == DeploymentStatus::Running)
                    .map(|d| d.id.clone())
            })
    }

    pub fn active_deployment_for_project(&self, project_id: &str) -> Option<String> {
        self.active_deployments
            .iter()
            .find(|((p, _), _)| p == project_id)
            .map(|(_, id)| id.clone())
            .or_else(|| {
                self.storage
                    .deployments
                    .iter()
                    .find(|d| d.project_id == project_id && d.status == DeploymentStatus::Running)
                    .map(|d| d.id.clone())
            })
    }

    pub fn is_deployment_active(&self, deployment_id: &str) -> bool {
        self.active_deployments.values().any(|id| id == deployment_id)
            || self.active_deployment_id.as_deref() == Some(deployment_id)
            || self
                .storage
                .deployments
                .iter()
                .any(|d| d.id == deployment_id && d.status == DeploymentStatus::Running)
    }

    pub fn has_active_deployments(&self) -> bool {
        !self.active_deployments.is_empty()
            || self.active_deployment_id.is_some()
            || self
                .storage
                .deployments
                .iter()
                .any(|d| d.status == DeploymentStatus::Running)
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

        // Ensure selected_deployment_id belongs strictly to this active project,
        // prioritizing any active/running deployment for this project & selected environment
        if let Some(active_dep_id) = self
            .active_deployment_for_env(proj_id, &self.selected_env_key)
            .or_else(|| self.active_deployment_for_project(proj_id))
        {
            self.selected_deployment_id = Some(active_dep_id);
        } else {
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
                    .find(|d| &d.project_id == proj_id && d.environment == self.selected_env_key)
                    .or_else(|| self.storage.deployments.iter().find(|d| &d.project_id == proj_id))
                    .map(|d| d.id.clone());
            }
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
                        SidebarSelection::Project(pid) => {
                            if pid != &record.project_id {
                                false
                            } else if let Some(sel_id) = &self.selected_deployment_id {
                                sel_id == &record.id
                            } else {
                                self.selected_env_key == record.environment
                            }
                        }
                        SidebarSelection::AllDeployments => {
                            if let Some(sel_id) = &self.selected_deployment_id {
                                sel_id == &record.id
                            } else {
                                true
                            }
                        }
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
                        SidebarSelection::Project(pid) => {
                            if pid != &record.project_id {
                                false
                            } else if let Some(sel_id) = &self.selected_deployment_id {
                                sel_id == &record.id
                            } else {
                                self.selected_env_key == record.environment
                            }
                        }
                        SidebarSelection::AllDeployments => {
                            if let Some(sel_id) = &self.selected_deployment_id {
                                sel_id == &record.id
                            } else {
                                true
                            }
                        }
                    };
                    if is_current {
                        self.selected_deployment_id = Some(record.id.clone());
                    }
                    self.active_deployments.retain(|_, id| id != &record.id);
                    if self.active_deployment_id.as_deref() == Some(&record.id) {
                        self.active_deployment_id = self.active_deployments.values().next().cloned();
                    }
                    self.storage.upsert_deployment(record);
                    if is_current {
                        self.refresh_selected_project_metadata(true);
                    }

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

                    // If all active deployments and queued bulk items are finished, apply any deferred update restart
                    if !self.has_active_deployments() && self.bulk_deploy_queue.is_empty() {
                        if let Some((new_ver, exe_path)) = self.pending_update_restart.take() {
                            if let Err(err) = restart_application(&exe_path) {
                                self.update_error = Some(err);
                            } else {
                                self.status_banner = Some((
                                    format!("All deployments completed! Updated to Shiply v{new_ver}. Restarting application..."),
                                    true,
                                ));
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        }
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
                    if self.has_active_deployments() {
                        self.pending_update_restart = Some((new_version.clone(), exe_path));
                        self.show_update_modal = false;
                        self.status_banner = Some((
                            format!("Shiply v{new_version} update is downloaded! Active deployments will continue running uninterrupted before restart."),
                            true,
                        ));
                    } else if let Err(err) = restart_application(&exe_path) {
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
        if !self.has_active_deployments()
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
            || self.has_active_deployments()
            || self.update_checking
            || self.update_downloading
            || self.project_switch_loading_id.is_some()
        {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }

    fn start_deployment(&mut self, req: DeploymentRequest) {
        self.flush_pending_save();
        if self.is_env_deploying(&req.project_id, &req.env_config.key) {
            self.status_banner = Some((
                format!(
                    "A deployment for '{}' ({}) is already running. Please wait for it to finish.",
                    req.project_name,
                    format_env_badge(&req.env_config.key)
                ),
                false,
            ));
            return;
        }

        let dep_id = req.deployment_id.clone();
        let key = (req.project_id.clone(), req.env_config.key.clone());
        self.active_deployments.insert(key, dep_id.clone());
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
        if self.has_active_deployments() {
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

                                    if let Some((_, exe_path)) = self.pending_update_restart.clone() {
                                        let restart_rect = Rect::from_center_size(
                                            Pos2::new(b_rect.right() - 118.0, b_rect.center().y),
                                            Vec2::new(96.0, 24.0),
                                        );
                                        let restart_resp = ui.interact(
                                            restart_rect,
                                            egui::Id::new("banner_restart_now_btn"),
                                            Sense::click(),
                                        )
                                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                                        ui.painter().rect_filled(
                                            restart_rect,
                                            CornerRadius::same(5),
                                            PRIMARY_BLUE,
                                        );
                                        ui.painter().text(
                                            restart_rect.center(),
                                            Align2::CENTER_CENTER,
                                            "Restart Now",
                                            FontId::proportional(FONT_SM),
                                            Color32::WHITE,
                                        );
                                        if restart_resp.clicked() {
                                            if let Err(err) = restart_application(&exe_path) {
                                                self.update_error = Some(err);
                                            } else {
                                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                            }
                                        }
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
        let width = 288.0 * anim_t;
        let pad_x = ((16.0 * anim_t).round() as i8).max(0);
        let mut sort_trigger_rect_out = None;
        egui::SidePanel::left("shiply_sidebar")
            .resizable(false)
            .exact_width(width)
            .frame(
                egui::Frame::new()
                    .fill(SB_BG)
                    .inner_margin(Margin::symmetric(pad_x, 0))
                    .stroke(Stroke::new(1.0_f32, SB_BORDER)),
            )
            .show(ctx, |ui| {
                if anim_t < 0.25 {
                    return;
                }
                ui.visuals_mut().override_text_color = Some(SB_TEXT);
                let inner_rect = ui.available_rect_before_wrap();
                let full_w = inner_rect.width();

                // 1. Brand Header (Pinned top, height 64px, bottom border 1px SB_BORDER)
                let brand_rect = Rect::from_min_size(inner_rect.min, Vec2::new(full_w, 64.0));
                let brand_ui = ui.new_child(egui::UiBuilder::new().max_rect(brand_rect));
                {
                    let logo_rect = Rect::from_min_size(
                        Pos2::new(brand_rect.left(), brand_rect.center().y - 14.0),
                        Vec2::new(28.0, 28.0),
                    );
                    draw_shiply_logo(
                        brand_ui.painter(),
                        logo_rect,
                        6,
                        FS_TITLE,
                        self.logo_texture.as_ref(),
                    );
                    brand_ui.painter().text(
                        Pos2::new(logo_rect.right() + 10.0, brand_rect.center().y),
                        Align2::LEFT_CENTER,
                        "Shiply",
                        FontId::proportional(FS_BRAND),
                        SB_TEXT,
                    );

                    // Version badge (--fs-micro, bg --sb-surface, radius --r-sm, padding 2px 8px, color --sb-text-muted), aligned right
                    let ver_text = format!("v{APP_VERSION}");
                    let ver_galley = brand_ui.painter().layout_no_wrap(
                        ver_text,
                        FontId::proportional(FS_MICRO),
                        SB_TEXT_MUTED,
                    );
                    let badge_pad_x = 8.0_f32;
                    let badge_h = 20.0_f32;
                    let badge_w = ver_galley.size().x + badge_pad_x * 2.0;
                    let badge_rect = Rect::from_min_size(
                        Pos2::new(brand_rect.right() - badge_w, brand_rect.center().y - badge_h / 2.0),
                        Vec2::new(badge_w, badge_h),
                    );
                    brand_ui.painter().rect_filled(badge_rect, CornerRadius::same(R_SM as u8), SB_SURFACE);
                    brand_ui.painter().galley(
                        Pos2::new(badge_rect.left() + badge_pad_x, badge_rect.center().y - ver_galley.size().y / 2.0),
                        ver_galley,
                        SB_TEXT_MUTED,
                    );

                    // Bottom border 1px SB_BORDER
                    brand_ui.painter().line_segment(
                        [
                            Pos2::new(brand_rect.left() - 16.0, brand_rect.bottom()),
                            Pos2::new(brand_rect.right() + 16.0, brand_rect.bottom()),
                        ],
                        Stroke::new(1.0_f32, SB_BORDER),
                    );
                }

                // 2. Footer (Pinned bottom, top border 1px SB_BORDER, padding-top 12px)
                let footer_bottom_clearance = 8.0_f32;
                let footer_h = 172.0_f32;
                let footer_rect = Rect::from_min_size(
                    Pos2::new(inner_rect.left(), inner_rect.bottom() - footer_bottom_clearance - footer_h),
                    Vec2::new(full_w, footer_h),
                );
                let mut footer_ui = ui.new_child(egui::UiBuilder::new().max_rect(footer_rect));
                {
                    // Top border 1px SB_BORDER
                    footer_ui.painter().line_segment(
                        [
                            Pos2::new(footer_rect.left() - 16.0, footer_rect.top()),
                            Pos2::new(footer_rect.right() + 16.0, footer_rect.top()),
                        ],
                        Stroke::new(1.0_f32, SB_BORDER),
                    );

                    footer_ui.add_space(SP_12);

                    // Actions: Sync, Open App Folder, Check for Updates
                    // Action 1: Sync App Directory Files
                    let (sync_rect, sync_resp) = footer_ui.allocate_exact_size(Vec2::new(full_w, 32.0), Sense::click());
                    let sync_resp = sync_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                    let sync_bg = if sync_resp.hovered() { SB_SURFACE } else { Color32::TRANSPARENT };
                    let sync_fg = if sync_resp.hovered() { SB_TEXT } else { SB_TEXT_MUTED };
                    footer_ui.painter().rect_filled(sync_rect, CornerRadius::same(R_MD as u8), sync_bg);
                    draw_icon_refresh(footer_ui.painter(), Pos2::new(sync_rect.left() + 16.0, sync_rect.center().y), sync_fg);
                    footer_ui.painter().text(
                        Pos2::new(sync_rect.left() + 32.0, sync_rect.center().y),
                        Align2::LEFT_CENTER,
                        "Sync App Directory Files",
                        FontId::proportional(FS_SMALL),
                        sync_fg,
                    );
                    if sync_resp.clicked() {
                        self.sidebar_refreshing_until = Some(Instant::now() + Duration::from_millis(400));
                        self.storage.reset_sync_cache();
                        if let Some(msg) = self.storage.sync_from_app_directory() {
                            self.refresh_selected_project_metadata(true);
                            self.status_banner = Some((msg, true));
                        } else {
                            self.status_banner = Some((
                                format!("Synced app directory ({})", self.storage.app_data_dir().display()),
                                true,
                            ));
                        }
                    }

                    footer_ui.add_space(SP_4);

                    // Action 2: Open App Folder (JSON / .ps1)
                    let (dir_rect, dir_resp) = footer_ui.allocate_exact_size(Vec2::new(full_w, 32.0), Sense::click());
                    let dir_resp = dir_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                    let dir_bg = if dir_resp.hovered() { SB_SURFACE } else { Color32::TRANSPARENT };
                    let dir_fg = if dir_resp.hovered() { SB_TEXT } else { SB_TEXT_MUTED };
                    footer_ui.painter().rect_filled(dir_rect, CornerRadius::same(R_MD as u8), dir_bg);
                    draw_icon_folder(footer_ui.painter(), Pos2::new(dir_rect.left() + 16.0, dir_rect.center().y), dir_fg);
                    footer_ui.painter().text(
                        Pos2::new(dir_rect.left() + 32.0, dir_rect.center().y),
                        Align2::LEFT_CENTER,
                        "Open App Folder (JSON / .ps1)",
                        FontId::proportional(FS_SMALL),
                        dir_fg,
                    );
                    if dir_resp.on_hover_text("Open the app folder where you can paste shiply_state.json or deploy-*.ps1 files").clicked() {
                        let dir = self.storage.app_data_dir();
                        let _ = fs::create_dir_all(&dir);
                        open_directory_in_os(&dir);
                    }

                    footer_ui.add_space(SP_4);

                    // Action 3: Check for Updates
                    let (upd_rect, upd_resp) = footer_ui.allocate_exact_size(Vec2::new(full_w, 32.0), Sense::click());
                    let upd_resp = upd_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                    let (upd_label, upd_fg) = if let Some(rel) = &self.available_update {
                        (format!("Update Available: {}!", rel.tag_name), Color32::from_rgb(250, 204, 21))
                    } else if self.update_checking {
                        ("Checking GitHub Releases...".to_string(), ACCENT)
                    } else if upd_resp.hovered() {
                        ("Check for Updates (GitHub)".to_string(), SB_TEXT)
                    } else {
                        ("Check for Updates (GitHub)".to_string(), SB_TEXT_MUTED)
                    };
                    let upd_bg = if upd_resp.hovered() { SB_SURFACE } else { Color32::TRANSPARENT };
                    footer_ui.painter().rect_filled(upd_rect, CornerRadius::same(R_MD as u8), upd_bg);
                    draw_icon_refresh(footer_ui.painter(), Pos2::new(upd_rect.left() + 16.0, upd_rect.center().y), upd_fg);
                    footer_ui.painter().text(
                        Pos2::new(upd_rect.left() + 32.0, upd_rect.center().y),
                        Align2::LEFT_CENTER,
                        upd_label,
                        FontId::proportional(FS_SMALL),
                        upd_fg,
                    );
                    if upd_resp.on_hover_text(format!("Check GitHub Releases ({GITHUB_REPO}) for a newer version of Shiply")).clicked() {
                        if self.available_update.is_some() {
                            self.show_update_modal = true;
                        } else if !self.update_checking {
                            spawn_update_check(self.updater_tx.clone(), true);
                        }
                    }

                    footer_ui.add_space(SP_12);

                    // Installed status row: dot 8px --success + "Installed v0.10.4" (--fs-micro, --sb-text-muted) + "Reinstall" as a small text link (--sb-text-faint, hover --accent). Keep it on one line.
                    let (stat_rect, _) = footer_ui.allocate_exact_size(Vec2::new(full_w, 24.0), Sense::hover());
                    let install_state = check_install_state(&default_install_dir(), APP_VERSION);
                    let (dot_color, status_text, action_text) = match &install_state {
                        InstallState::NotInstalled => (ACCENT, "Not installed".to_string(), "Install (.exe)"),
                        InstallState::InstalledCurrent { version, .. } => (SUCCESS, format!("Installed v{version}"), "Reinstall"),
                        InstallState::UpdateReady { running_version, .. } => (Color32::from_rgb(250, 204, 21), format!("Update to v{running_version}"), "Apply"),
                    };

                    // Dot 8px
                    footer_ui.painter().circle_filled(
                        Pos2::new(stat_rect.left() + 12.0, stat_rect.center().y),
                        4.0,
                        dot_color,
                    );

                    // Status text
                    let st_galley = footer_ui.painter().layout_no_wrap(
                        status_text,
                        FontId::proportional(FS_MICRO),
                        SB_TEXT_MUTED,
                    );
                    footer_ui.painter().galley(
                        Pos2::new(stat_rect.left() + 22.0, stat_rect.center().y - st_galley.size().y / 2.0),
                        st_galley.clone(),
                        SB_TEXT_MUTED,
                    );

                    // Action link on the same line
                    let link_x = stat_rect.left() + 22.0 + st_galley.size().x + 8.0;
                    let link_galley = footer_ui.painter().layout_no_wrap(
                        action_text.to_string(),
                        FontId::proportional(FS_MICRO),
                        SB_TEXT_FAINT,
                    );
                    let link_rect = Rect::from_min_size(
                        Pos2::new(link_x, stat_rect.center().y - link_galley.size().y / 2.0),
                        link_galley.size(),
                    );
                    let link_resp = footer_ui.interact(link_rect, egui::Id::new("sidebar_reinstall_link"), Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    let link_color = if link_resp.hovered() { ACCENT } else { SB_TEXT_FAINT };
                    footer_ui.painter().galley(link_rect.min, link_galley, link_color);

                    if link_resp.clicked() {
                        match install_or_update_self() {
                            Ok(msg) => self.status_banner = Some((msg, true)),
                            Err(err) => self.status_banner = Some((err, false)),
                        }
                    }

                    footer_ui.add_space(SP_16);
                }

                // 3. Middle Section: Scrollable (nav items + projects header + projects list)
                let middle_rect = Rect::from_min_max(
                    Pos2::new(inner_rect.left(), brand_rect.bottom() + SP_16),
                    Pos2::new(inner_rect.right(), footer_rect.top() - SP_8),
                );
                let mut middle_ui = ui.new_child(egui::UiBuilder::new().max_rect(middle_rect));
                middle_ui.spacing_mut().scroll.bar_width = 6.0;
                middle_ui.spacing_mut().scroll.bar_inner_margin = 2.0;

                ScrollArea::vertical()
                    .id_salt("sidebar_scrollable_middle")
                    .auto_shrink([false, false])
                    .show(&mut middle_ui, |ui| {
                        let full_w = ui.available_width();

                        // Nav items: All Deployments (height 40px, padding 0 12px, icon 18px, gap 12px, --fs-base, radius --r-md)
                        let is_overview = matches!(self.selection, SidebarSelection::AllDeployments);
                        let (nav_rect, nav_resp) = ui.allocate_exact_size(Vec2::new(full_w, 40.0), Sense::click());
                        let nav_resp = nav_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                        let nav_bg = if is_overview {
                            ACCENT_SOFT
                        } else if nav_resp.hovered() {
                            SB_SURFACE_HOVER
                        } else {
                            Color32::TRANSPARENT
                        };
                        let nav_fg = if is_overview || nav_resp.hovered() {
                            SB_TEXT
                        } else {
                            SB_TEXT_MUTED
                        };
                        ui.painter().rect_filled(nav_rect, CornerRadius::same(R_MD as u8), nav_bg);
                        if is_overview {
                            ui.painter().rect_filled(
                                Rect::from_min_max(
                                    Pos2::new(nav_rect.left(), nav_rect.top() + 8.0),
                                    Pos2::new(nav_rect.left() + 3.0, nav_rect.bottom() - 8.0),
                                ),
                                CornerRadius { nw: 0, ne: 3, se: 3, sw: 0 },
                                ACCENT,
                            );
                        }
                        draw_icon_layers(
                            ui.painter(),
                            Pos2::new(nav_rect.left() + 18.0, nav_rect.center().y),
                            if is_overview { ACCENT } else { nav_fg },
                        );
                        ui.painter().text(
                            Pos2::new(nav_rect.left() + 38.0, nav_rect.center().y),
                            Align2::LEFT_CENTER,
                            "All Deployments",
                            FontId::proportional(FS_BASE),
                            nav_fg,
                        );
                        if nav_resp.clicked() {
                            self.selection = SidebarSelection::AllDeployments;
                        }

                        // Bulk Deploy button (if projects exist)
                        if !self.storage.projects.is_empty() {
                            ui.add_space(SP_4);
                            let is_bulk_active = self.show_bulk_deploy_modal;
                            let (bulk_rect, bulk_resp) = ui.allocate_exact_size(Vec2::new(full_w, 40.0), Sense::click());
                            let bulk_resp = bulk_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                            let bulk_bg = if is_bulk_active {
                                ACCENT_SOFT
                            } else if bulk_resp.hovered() {
                                SB_SURFACE_HOVER
                            } else {
                                Color32::TRANSPARENT
                            };
                            let bulk_fg = if is_bulk_active || bulk_resp.hovered() {
                                SB_TEXT
                            } else {
                                SB_TEXT_MUTED
                            };
                            ui.painter().rect_filled(bulk_rect, CornerRadius::same(R_MD as u8), bulk_bg);
                            if is_bulk_active {
                                ui.painter().rect_filled(
                                    Rect::from_min_max(
                                        Pos2::new(bulk_rect.left(), bulk_rect.top() + 8.0),
                                        Pos2::new(bulk_rect.left() + 3.0, bulk_rect.bottom() - 8.0),
                                    ),
                                    CornerRadius { nw: 0, ne: 3, se: 3, sw: 0 },
                                    ACCENT,
                                );
                            }
                            draw_icon_send(
                                ui.painter(),
                                Pos2::new(bulk_rect.left() + 18.0, bulk_rect.center().y),
                                if is_bulk_active { ACCENT } else { bulk_fg },
                            );
                            ui.painter().text(
                                Pos2::new(bulk_rect.left() + 38.0, bulk_rect.center().y),
                                Align2::LEFT_CENTER,
                                "Bulk Deploy...",
                                FontId::proportional(FS_BASE),
                                bulk_fg,
                            );
                            if bulk_resp.on_hover_text("Select multiple projects to deploy sequentially").clicked() {
                                self.open_bulk_deploy_modal();
                            }
                        }

                        ui.add_space(SP_16);

                        // Projects header row: row height 32px, gap 8px
                        let (hdr_rect, _) = ui.allocate_exact_size(Vec2::new(full_w, 32.0), Sense::hover());
                        ui.painter().text(
                            Pos2::new(hdr_rect.left() + 4.0, hdr_rect.center().y),
                            Align2::LEFT_CENTER,
                            "PROJECTS",
                            FontId::proportional(FS_MICRO),
                            SB_TEXT_FAINT,
                        );

                        let add_w = 54.0_f32;
                        let sort_w = 96.0_f32;

                        // "+ ADD" button: height 28px, padding 0 10px, --fs-micro, bg --sb-surface, border 1px --sb-border, radius --r-sm, hover bg --sb-surface-hover
                        let add_btn_rect = Rect::from_min_size(
                            Pos2::new(hdr_rect.right() - add_w, hdr_rect.center().y - 14.0),
                            Vec2::new(add_w, 28.0),
                        );
                        let add_btn_resp = ui.interact(add_btn_rect, egui::Id::new("sidebar_add_proj_btn"), Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        let add_bg = if add_btn_resp.hovered() { SB_SURFACE_HOVER } else { SB_SURFACE };
                        ui.painter().rect_filled(add_btn_rect, CornerRadius::same(R_SM as u8), add_bg);
                        ui.painter().rect_stroke(add_btn_rect, CornerRadius::same(R_SM as u8), Stroke::new(1.0_f32, SB_BORDER), StrokeKind::Inside);
                        ui.painter().text(
                            add_btn_rect.center(),
                            Align2::CENTER_CENTER,
                            "+ ADD",
                            FontId::proportional(FS_MICRO),
                            SB_TEXT,
                        );
                        if add_btn_resp.clicked() {
                            self.new_project_group.clear();
                            self.show_new_project_modal = true;
                        }

                        // Sort control: height 28px, --fs-micro style but weight 500, bg --sb-surface, border 1px --sb-border, radius --r-sm, text color --sb-text
                        let sort_area_rect = Rect::from_min_size(
                            Pos2::new(hdr_rect.right() - add_w - 8.0 - sort_w, hdr_rect.center().y - 14.0),
                            Vec2::new(sort_w, 28.0),
                        );
                        sort_trigger_rect_out = Some(sort_area_rect);

                        let sort_btn_resp = ui.interact(sort_area_rect, egui::Id::new("sidebar_project_sort_trigger"), Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        if sort_btn_resp.clicked() {
                            self.sidebar_sort_dropdown_open = !self.sidebar_sort_dropdown_open;
                        }

                        let is_sort_open = self.sidebar_sort_dropdown_open;
                        let sort_bg = if is_sort_open || sort_btn_resp.hovered() { SB_SURFACE_HOVER } else { SB_SURFACE };
                        let sort_stroke = if is_sort_open {
                            Stroke::new(1.5_f32, ACCENT)
                        } else {
                            Stroke::new(1.0_f32, SB_BORDER)
                        };

                        ui.painter().rect_filled(sort_area_rect, CornerRadius::same(R_SM as u8), sort_bg);
                        ui.painter().rect_stroke(sort_area_rect, CornerRadius::same(R_SM as u8), sort_stroke, StrokeKind::Inside);

                        let sort_label = match self.project_sort_mode {
                            ProjectSortMode::Custom => "Custom",
                            ProjectSortMode::NameAsc => "A-Z",
                            ProjectSortMode::NameDesc => "Z-A",
                            ProjectSortMode::RecentDeploy => "Recent",
                        };

                        ui.painter().text(
                            Pos2::new(sort_area_rect.left() + 10.0, sort_area_rect.center().y),
                            Align2::LEFT_CENTER,
                            sort_label,
                            FontId::proportional(FS_MICRO),
                            SB_TEXT,
                        );

                        draw_outline_icon(
                            ui.painter(),
                            Pos2::new(sort_area_rect.right() - 12.0, sort_area_rect.center().y),
                            if is_sort_open { OutlineIcon::ChevronUp } else { OutlineIcon::ChevronDown },
                            SB_TEXT,
                        );

                        ui.add_space(SP_8);

                        // Skeleton loading on initial load or refresh:
                        let is_refreshing = self.sidebar_refreshing_until.as_ref().is_some_and(|t| Instant::now() < *t);
                        if is_refreshing && self.storage.projects.is_empty() {
                            ui.ctx().request_repaint();
                            let time = ui.input(|i| i.time);
                            for _ in 0..3 {
                                let (skel_rect, _) = ui.allocate_exact_size(Vec2::new(full_w, 64.0), Sense::hover());
                                ui.painter().rect_filled(skel_rect, CornerRadius::same(R_MD as u8), SB_SURFACE);
                                ui.painter().rect_stroke(skel_rect, CornerRadius::same(R_MD as u8), Stroke::new(1.0_f32, SB_BORDER), StrokeKind::Inside);

                                // Name bar: height 14px, width 60%, radius 4px
                                let name_w = (skel_rect.width() - 34.0) * 0.6;
                                let name_rect = Rect::from_min_size(Pos2::new(skel_rect.left() + 22.0, skel_rect.top() + 14.0), Vec2::new(name_w, 14.0));
                                paint_shimmer_rect(ui.painter(), name_rect, 4.0, time);

                                // Two badge pills: 44px x 20px and 52px x 20px, radius --r-sm, gap 6px
                                let pill1 = Rect::from_min_size(Pos2::new(skel_rect.left() + 22.0, skel_rect.top() + 36.0), Vec2::new(44.0, 20.0));
                                paint_shimmer_rect(ui.painter(), pill1, R_SM, time);
                                let pill2 = Rect::from_min_size(Pos2::new(skel_rect.left() + 22.0 + 44.0 + 6.0, skel_rect.top() + 36.0), Vec2::new(52.0, 20.0));
                                paint_shimmer_rect(ui.painter(), pill2, R_SM, time);

                                ui.add_space(SP_8);
                            }
                        } else if self.storage.projects.is_empty() {
                            let (empty_rect, _) = ui.allocate_exact_size(Vec2::new(full_w, 64.0), Sense::hover());
                            ui.painter().rect_stroke(
                                empty_rect,
                                CornerRadius::same(R_MD as u8),
                                Stroke::new(1.0_f32, SB_BORDER),
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

                            let has_groups = self.storage.projects.iter().any(|p| {
                                p.group.as_ref().is_some_and(|g| !g.trim().is_empty())
                            });

                            let mut group_sections: Vec<(Option<String>, Vec<usize>)> = Vec::new();
                            if has_groups {
                                for (idx, p) in self.storage.projects.iter().enumerate() {
                                    let grp = p.group.as_ref().map(|g| g.trim().to_string()).filter(|g| !g.is_empty());
                                    if let Some(pos) = group_sections.iter().position(|(g, _)| g.as_ref() == grp.as_ref()) {
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
                                group_sections.push((None, (0..self.storage.projects.len()).collect()));
                            }

                            let mut clicked_project_id = None;
                            let mut drag_reorder = None;

                            for (grp_opt, proj_indices) in group_sections {
                                if has_groups {
                                    let group_title = grp_opt.as_deref().unwrap_or("General");
                                    let group_key = grp_opt.clone().unwrap_or_else(|| "_general".to_string());
                                    let is_collapsed = self.collapsed_project_groups.contains(&group_key);

                                    let (grp_rect, grp_resp) = ui.allocate_exact_size(Vec2::new(full_w, 28.0), Sense::click());
                                    let grp_resp = grp_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                                    if grp_resp.hovered() {
                                        ui.painter().rect_filled(grp_rect, CornerRadius::same(R_SM as u8), SB_SURFACE);
                                    }

                                    let chev = if is_collapsed {
                                        OutlineIcon::ChevronRight
                                    } else {
                                        OutlineIcon::ChevronDown
                                    };
                                    draw_outline_icon(
                                        ui.painter(),
                                        Pos2::new(grp_rect.left() + 10.0, grp_rect.center().y),
                                        chev,
                                        SB_TEXT_FAINT,
                                    );
                                    ui.painter().text(
                                        Pos2::new(grp_rect.left() + 22.0, grp_rect.center().y),
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

                                    ui.add_space(SP_4);

                                    if is_collapsed {
                                        continue;
                                    }
                                }

                                for proj_idx in proj_indices {
                                    let project = &self.storage.projects[proj_idx];
                                    let is_selected = matches!(&self.selection, SidebarSelection::Project(id) if id == &project.id);
                                    let is_being_dragged = self.dragged_project_idx == Some(proj_idx);
                                    let is_switching = self.project_switch_loading_id.as_deref() == Some(&project.id);

                                    // Project Card: min-height 64px, padding 12px 12px 12px 8px, radius --r-md, bg transparent, border 1px transparent. Gap between cards: 8px.
                                    let (p_rect, p_resp) = ui.allocate_exact_size(Vec2::new(full_w, 64.0), Sense::click());
                                    let p_resp = p_resp.on_hover_cursor(egui::CursorIcon::PointingHand);

                                    let fill = if is_selected || is_switching {
                                        ACCENT_SOFT
                                    } else if p_resp.hovered() {
                                        SB_SURFACE
                                    } else {
                                        Color32::TRANSPARENT
                                    };
                                    ui.painter().rect_filled(p_rect, CornerRadius::same(R_MD as u8), fill);

                                    // Left vertical grip handle for dragging & reordering (16px slot always reserved)
                                    let grip_rect = Rect::from_min_size(
                                        Pos2::new(p_rect.left() + 2.0, p_rect.top() + 7.0),
                                        Vec2::new(16.0, 50.0),
                                    );
                                    let grip_resp = ui.interact(
                                        grip_rect,
                                        egui::Id::new(("sidebar_proj_grip", &project.id)),
                                        Sense::drag(),
                                    ).on_hover_cursor(egui::CursorIcon::Grab);

                                    if grip_resp.drag_started() || grip_resp.dragged() {
                                        self.dragged_project_idx = Some(proj_idx);
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                                    }

                                    // Drag handle opacity: 0 by default, 0.5 on card hover, 1 on handle hover
                                    let grip_color = if is_being_dragged || grip_resp.hovered() {
                                        ACCENT
                                    } else if p_resp.hovered() {
                                        Color32::from_rgba_premultiplied(45, 50, 65, 128)
                                    } else {
                                        Color32::TRANSPARENT
                                    };
                                    draw_outline_icon(
                                        ui.painter(),
                                        grip_rect.center(),
                                        OutlineIcon::GripVertical,
                                        grip_color,
                                    );

                                    // Drop target hover highlight
                                    if let Some(from_idx) = self.dragged_project_idx {
                                        if from_idx != proj_idx {
                                            if let Some(pos) = pointer_pos {
                                                if p_rect.contains(pos) {
                                                    ui.painter().rect_stroke(
                                                        p_rect,
                                                        CornerRadius::same(R_MD as u8),
                                                        Stroke::new(2.0_f32, ACCENT),
                                                        StrokeKind::Inside,
                                                    );
                                                    if is_pointer_released {
                                                        drag_reorder = Some((from_idx, proj_idx));
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // Active (selected): bg --accent-soft, border 1px rgba(59,130,246,0.35), plus a 3px accent bar on the left (radius 0 3px 3px 0, vertically inset 12px)
                                    if is_selected || is_switching {
                                        let border_color = Color32::from_rgba_premultiplied(20, 45, 86, 89);
                                        ui.painter().rect_stroke(
                                            p_rect,
                                            CornerRadius::same(R_MD as u8),
                                            Stroke::new(1.0_f32, border_color),
                                            StrokeKind::Inside,
                                        );
                                        let bar_rect = Rect::from_min_max(
                                            Pos2::new(p_rect.left(), p_rect.top() + 12.0),
                                            Pos2::new(p_rect.left() + 3.0, p_rect.bottom() - 12.0),
                                        );
                                        ui.painter().rect_filled(
                                            bar_rect,
                                            CornerRadius { nw: 0, ne: 3, se: 3, sw: 0 },
                                            ACCENT,
                                        );
                                    }

                                    // Content layout: [drag handle 16px wide] [content]. Content = row 1: project name, row 2: env badges. Gap between rows: 6px.
                                    let content_x = p_rect.left() + 22.0;

                                    // Check if this project is currently running an active deployment process
                                    let is_deploying = self.is_project_deploying(&project.id);

                                    // Row 1: Project Name (--fs-base, color --sb-text, SINGLE LINE ONLY, never wraps)
                                    let name_color = if is_selected || is_switching {
                                        Color32::WHITE
                                    } else {
                                        SB_TEXT
                                    };
                                    let avail_name_w = (p_rect.right() - 28.0 - content_x).max(20.0);
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
                                        Pos2::new(content_x, p_rect.top() + 12.0),
                                        name_galley,
                                        name_color,
                                    );

                                    // Spinner slot absolutely positioned (right 12px, vertically centered, 16px) taking zero layout space
                                    if is_deploying || is_switching {
                                        ui.ctx().request_repaint();
                                        let spinner_center = Pos2::new(p_rect.right() - 18.0, p_rect.center().y);
                                        let spinner_color = if is_deploying {
                                            ACCENT
                                        } else {
                                            Color32::from_rgb(148, 163, 184)
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
                                        p_resp.on_hover_text(format!("{}\n(A deployment is currently running)", project.name))
                                    } else {
                                        p_resp.on_hover_text(&project.name)
                                    };

                                    // Right-click context popup menu for Project options (custom cohesive dark menu)
                                    if p_resp.secondary_clicked() {
                                        if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
                                            self.sidebar_context_menu = Some(SidebarContextMenuState {
                                                project_id: project.id.clone(),
                                                click_pos: pos,
                                                submenu_open: false,
                                            });
                                        }
                                    }

                                    // Row 2: Env Badges (height 20px, padding 0 8px, --fs-micro, radius --r-sm, gap 6px)
                                    let row2_y = p_rect.top() + 36.0;

                                    if is_switching {
                                        // While switching: the env badge row of that card shows shimmer skeleton pills
                                        ui.ctx().request_repaint();
                                        let time = ui.input(|i| i.time);
                                        let pill1 = Rect::from_min_size(Pos2::new(content_x, row2_y), Vec2::new(44.0, 20.0));
                                        paint_shimmer_rect(ui.painter(), pill1, R_SM, time);
                                        let pill2 = Rect::from_min_size(Pos2::new(content_x + 50.0, row2_y), Vec2::new(52.0, 20.0));
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
                                            } else if let Some(ok_dep) = self.storage.last_succeeded_deployment(&project.id, &env.key) {
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

                                    ui.add_space(SP_8);
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
            });

        if let Some(r) = sort_trigger_rect_out {
            self.render_sidebar_sort_dropdown(ctx, r);
        }
        self.render_sidebar_custom_context_menu(ctx);
    }

    fn render_sidebar_sort_dropdown(&mut self, ctx: &egui::Context, trigger_rect: Rect) {
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
                    let bg = if item_resp.hovered() {
                        SB_SURFACE_HOVER
                    } else if is_selected {
                        ACCENT_SOFT
                    } else {
                        Color32::TRANSPARENT
                    };

                    ui.painter().rect_filled(item_rect, CornerRadius::same(R_SM as u8), bg);

                    // Text: --fs-small, color --sb-text
                    ui.painter().text(
                        Pos2::new(item_rect.left() + 10.0, item_rect.center().y),
                        Align2::LEFT_CENTER,
                        *label,
                        FontId::proportional(FS_SMALL),
                        SB_TEXT,
                    );

                    // Selected item: 14px check icon (SVG) on the right
                    if is_selected {
                        let check_pos = Pos2::new(item_rect.right() - 14.0, item_rect.center().y);
                        draw_outline_icon(ui.painter(), check_pos, OutlineIcon::Check, SB_TEXT);
                    }

                    if item_resp.clicked() {
                        self.project_sort_mode = *mode;
                        self.storage.sort_projects(*mode);
                        self.sidebar_sort_dropdown_open = false;
                    }
                }
            });
    }

    fn render_sidebar_custom_context_menu(&mut self, ctx: &egui::Context) {
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

        // Close on Escape key
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.show_new_project_modal = false;
            return;
        }

        // Dimmed backdrop with click-to-dismiss
        egui::Area::new(egui::Id::new("modal_backdrop_new_project"))
            .order(egui::Order::Middle)
            .fixed_pos(Pos2::ZERO)
            .show(ctx, |ui| {
                let screen_rect = ctx.screen_rect();
                let resp = ui.allocate_rect(screen_rect, Sense::click());
                ui.painter().rect_filled(screen_rect, CornerRadius::ZERO, MD_OVERLAY);
                if resp.clicked() {
                    self.show_new_project_modal = false;
                }
            });

        egui::Window::new("Add Project")
            .order(egui::Order::Foreground)
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .fixed_size(Vec2::new(560.0, 0.0))
            .frame(modal_dialog_frame())
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
                            if ui.add_enabled(can_autofill, parse_btn).clicked() && can_autofill {
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
                        if (ui.add_enabled(can_create, create_btn).clicked() || enter_submitted) && can_create {
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

    fn open_edit_project_modal(&mut self, proj_idx: usize) {
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

    fn render_edit_project_modal(&mut self, ctx: &egui::Context) {
        if !self.show_edit_project_modal {
            return;
        }

        // Close on Escape key
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.show_edit_project_modal = false;
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

        // Dimmed backdrop with click-to-dismiss
        egui::Area::new(egui::Id::new("modal_backdrop_edit_project"))
            .order(egui::Order::Middle)
            .fixed_pos(Pos2::ZERO)
            .show(ctx, |ui| {
                let screen_rect = ctx.screen_rect();
                let resp = ui.allocate_rect(screen_rect, Sense::click());
                ui.painter().rect_filled(screen_rect, CornerRadius::ZERO, MD_OVERLAY);
                if resp.clicked() {
                    self.show_edit_project_modal = false;
                }
            });

        egui::Window::new("Edit Project")
            .order(egui::Order::Foreground)
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .fixed_size(Vec2::new(560.0, 0.0))
            .frame(modal_dialog_frame())
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

    fn render_new_environment_modal(&mut self, ctx: &egui::Context) {
        if !self.show_new_env_modal {
            return;
        }

        // Close on Escape key
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.show_new_env_modal = false;
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

        // Dimmed backdrop with click-to-dismiss
        egui::Area::new(egui::Id::new("modal_backdrop_new_env"))
            .order(egui::Order::Middle)
            .fixed_pos(Pos2::ZERO)
            .show(ctx, |ui| {
                let screen_rect = ctx.screen_rect();
                let resp = ui.allocate_rect(screen_rect, Sense::click());
                ui.painter().rect_filled(screen_rect, CornerRadius::ZERO, MD_OVERLAY);
                if resp.clicked() {
                    self.show_new_env_modal = false;
                }
            });

        egui::Window::new("Add Environment Level")
            .order(egui::Order::Foreground)
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .fixed_size(Vec2::new(560.0, 0.0))
            .frame(modal_dialog_frame())
            .show(ctx, |ui| {
                ui.set_width(512.0);
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

    fn render_github_update_modal(&mut self, ctx: &egui::Context) {
        if !self.show_update_modal {
            return;
        }

        // Close on Escape key (allows checking deployments while downloading or deferred)
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.show_update_modal = false;
            return;
        }

        // Dimmed backdrop with click-to-dismiss
        egui::Area::new(egui::Id::new("modal_backdrop_github_update"))
            .order(egui::Order::Middle)
            .fixed_pos(Pos2::ZERO)
            .show(ctx, |ui| {
                let screen_rect = ctx.screen_rect();
                let resp = ui.allocate_rect(screen_rect, Sense::click());
                ui.painter().rect_filled(screen_rect, CornerRadius::ZERO, MD_OVERLAY);
                if resp.clicked() {
                    self.show_update_modal = false;
                }
            });

        if let Some(release) = self.available_update.clone() {
            egui::Window::new("Software Update Available")
                .order(egui::Order::Foreground)
                .title_bar(false)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .fixed_size(Vec2::new(560.0, 0.0))
                .frame(modal_dialog_frame())
                .show(ctx, |ui| {
                    ui.set_width(512.0);
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
            egui::Window::new("Update Information")
                .order(egui::Order::Foreground)
                .title_bar(false)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .fixed_size(Vec2::new(560.0, 0.0))
                .frame(modal_dialog_frame())
                .show(ctx, |ui| {
                    ui.set_width(512.0);
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
    fn render_delete_confirmation_modal(&mut self, ctx: &egui::Context) {
        let Some(action) = self.pending_delete_confirm.clone() else {
            return;
        };

        let mut execute_delete = false;
        let mut dismiss = false;

        // Close on Escape key
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            dismiss = true;
        }

        // Dimmed backdrop with click-to-dismiss
        egui::Area::new(egui::Id::new("modal_backdrop_delete_confirm"))
            .order(egui::Order::Middle)
            .fixed_pos(Pos2::ZERO)
            .show(ctx, |ui| {
                let screen_rect = ctx.screen_rect();
                let resp = ui.allocate_rect(screen_rect, Sense::click());
                ui.painter().rect_filled(screen_rect, CornerRadius::ZERO, MD_OVERLAY);
                if resp.clicked() {
                    dismiss = true;
                }
            });

        egui::Window::new("Confirm Deletion")
            .order(egui::Order::Foreground)
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .fixed_size(Vec2::new(560.0, 0.0))
            .frame(
                modal_dialog_frame()
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(252, 165, 165)))
            )
            .show(ctx, |ui| {
                ui.set_width(512.0);

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

        // Close on Escape key
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.show_bulk_deploy_modal = false;
            return;
        }

        // Dimmed backdrop with click-to-dismiss
        egui::Area::new(egui::Id::new("modal_backdrop_bulk_deploy"))
            .order(egui::Order::Middle)
            .fixed_pos(Pos2::ZERO)
            .show(ctx, |ui| {
                let screen_rect = ctx.screen_rect();
                let resp = ui.allocate_rect(screen_rect, Sense::click());
                ui.painter().rect_filled(screen_rect, CornerRadius::ZERO, MD_OVERLAY);
                if resp.clicked() {
                    self.show_bulk_deploy_modal = false;
                }
            });

        egui::Window::new("Bulk Deployment")
            .order(egui::Order::Foreground)
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .fixed_size(Vec2::new(560.0, 0.0))
            .frame(modal_dialog_frame())
            .show(ctx, |ui| {
                ui.set_width(512.0);

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
                        ("hdr_dup_proj", project_id),
                        OutlineIcon::Copy,
                        Some("Duplicate"),
                        TEXT_PRIMARY,
                        CARD_BG,
                        SUBTLE_BG,
                        CARD_BORDER,
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

    fn render_sync_version_dropdown(
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

    fn render_deploy_action_bar(
        &mut self,
        ui: &mut egui::Ui,
        env_display_name: &str,
        rollback_candidate: Option<DeploymentRecord>,
        sync_trigger_rect_out: &mut Option<Rect>,
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

        child.add_space(4.0);
        let project_id_owned = match &self.selection {
            SidebarSelection::Project(pid) => pid.clone(),
            SidebarSelection::AllDeployments => String::new(),
        };
        let project_id = project_id_owned.as_str();
        let sync_candidates = self.sync_version_candidates(project_id);
        let sync_btn_text = if self.sync_version_dropdown_open {
            "Sync ▴"
        } else {
            "Sync ▾"
        };
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
                PRIMARY_BLUE
            } else {
                TEXT_SECONDARY
            },
            if self.sync_version_dropdown_open {
                Color32::from_rgb(239, 246, 255)
            } else {
                SUBTLE_BG
            },
            CARD_BG,
            if self.sync_version_dropdown_open {
                PRIMARY_BLUE
            } else {
                CARD_BORDER
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
        horizontal_row(ui, 34.0, |ui| {
            ui.label(
                RichText::new("DEPLOYMENT HISTORY")
                    .size(FONT_MD)
                    .strong()
                    .color(TEXT_PRIMARY),
            );

            if is_project_view {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                    for (env_name, env_key) in project_envs.into_iter().rev() {
                        let is_active = self.selected_env_key == env_key;
                        let btn = egui::Button::new(
                            RichText::new(env_name).size(FONT_SM).strong().color(if is_active {
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
                            let is_busy = self.is_env_deploying(&dep.project_id, &dep.environment);

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

/// Renders a modern modal header according to design tokens:
/// Title (--fs-title 18px, weight 600, --md-text) + description (--fs-small 13px, --md-text-muted, margin-top 4px)
/// + anchored 32x32 close button top-right (radius --r-md 8px, hover bg --md-panel-bg).
fn render_modal_header(
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

/// Standardized dialog window frame adhering to design tokens:
/// width 560px, bg --md-bg, border 1px --md-border, radius --r-xl (16px), padding 24px, shadow 0 24px 60px rgba(2,6,23,0.35)
fn modal_dialog_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(MD_BG)
        .stroke(Stroke::new(1.0_f32, MD_BORDER))
        .corner_radius(R_XL)
        .shadow(Shadow {
            offset: [0, 24],
            blur: 60,
            spread: 0,
            color: Color32::from_rgba_unmultiplied(2, 6, 23, 89),
        })
        .inner_margin(Margin::same(24))
}

/// Unified modal text input (height 40px, font --fs-base 14px, bg --md-input-bg, border 1px --md-border, radius --r-md 8px).
fn modal_text_input(
    ui: &mut egui::Ui,
    value: &mut String,
    width: f32,
    placeholder: Option<&str>,
) -> egui::Response {
    let hint_str = placeholder.unwrap_or("Enter value...");
    let edit = egui::TextEdit::singleline(value)
        .font(FontId::proportional(FS_BASE))
        .background_color(MD_INPUT_BG)
        .desired_width(width)
        .min_size(Vec2::new(width, 40.0))
        .margin(Margin::symmetric(12, 10))
        .hint_text(
            RichText::new(hint_str)
                .size(FS_BASE)
                .color(MD_TEXT_MUTED.gamma_multiply(0.7)),
        );
    ui.add(edit)
}

/// Unified modal primary action button (height 40px, padding 0 16px, font --fs-base 14px, radius --r-md 8px, bg --accent).
fn modal_primary_button(text: &str, enabled: bool) -> egui::Button<'static> {
    egui::Button::new(
        RichText::new(text.to_string())
            .size(FS_BASE)
            .strong()
            .color(if enabled {
                Color32::WHITE
            } else {
                Color32::from_rgba_unmultiplied(255, 255, 255, 128)
            }),
    )
    .fill(if enabled {
        ACCENT
    } else {
        ACCENT.gamma_multiply(0.5)
    })
    .corner_radius(R_MD)
    .min_size(Vec2::new(0.0, 40.0))
}

/// Unified modal secondary button (height 40px, padding 0 16px, font --fs-base 14px, radius --r-md 8px, white bg, border 1px --md-border, hover --md-panel-bg).
fn modal_secondary_button(text: &str) -> egui::Button<'static> {
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

#[allow(dead_code)]
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

/// Paints a shimmering skeleton placeholder with a smooth 1.4s infinite wave.
fn paint_shimmer_rect(painter: &egui::Painter, rect: Rect, radius: f32, time: f64) {
    let phase = ((time % 1.4) / 1.4) as f32;
    let alpha_f = 0.06 + 0.06 * (0.5 + 0.5 * (phase * std::f32::consts::TAU).sin());
    let a = (alpha_f * 255.0).clamp(15.0, 31.0) as u8;
    let fill = Color32::from_rgba_premultiplied(a, a, a, a);
    painter.rect_filled(rect, CornerRadius::same(radius as u8), fill);
}

/// Paints a 20px environment badge adhering to sidebar tokens (--fs-micro, --r-sm, padding 0 8px).
fn paint_sidebar_env_badge(
    painter: &egui::Painter,
    pos: Pos2,
    text: &str,
    bg: Color32,
    fg: Color32,
) -> f32 {
    let font_id = FontId::proportional(FS_MICRO);
    let galley = painter.layout_no_wrap(text.to_string(), font_id, fg);
    let pad_x = 8.0_f32;
    let h = 20.0_f32;
    let w = galley.size().x + pad_x * 2.0;
    let rect = Rect::from_min_size(pos, Vec2::new(w, h));
    painter.rect_filled(rect, CornerRadius::same(R_SM as u8), bg);
    painter.galley(
        Pos2::new(rect.left() + pad_x, rect.center().y - galley.size().y / 2.0),
        galley,
        fg,
    );
    w
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

/// Renders a modern, realistic pulsating loading skeleton while switching active project.
fn render_project_workspace_skeleton(ui: &mut egui::Ui, proj_name: &str) {
    ui.ctx().request_repaint();
    let time = ui.input(|i| i.time);
    let full_w = ui.available_width();

    // Subtle pulsating shimmer between light slate tones
    let pulse = ((time * 4.0).sin() * 0.5 + 0.5) as f32;
    let r = (226.0 + (242.0 - 226.0) * pulse) as u8;
    let g = (232.0 + (246.0 - 232.0) * pulse) as u8;
    let b = (240.0 + (250.0 - 240.0) * pulse) as u8;
    let skel_fill = Color32::from_rgb(r, g, b);
    let skel_border = Color32::from_rgb(
        (215.0 + 12.0 * pulse) as u8,
        (223.0 + 10.0 * pulse) as u8,
        (234.0 + 8.0 * pulse) as u8,
    );

    // 1. Top Project Header Bar Skeleton
    horizontal_row(ui, 36.0, |ui| {
        let (title_rect, _) = ui.allocate_exact_size(Vec2::new(190.0, 28.0), Sense::hover());
        ui.painter().rect_filled(title_rect, CornerRadius::same(6), skel_fill);
        ui.painter().text(
            Pos2::new(title_rect.left() + 10.0, title_rect.center().y),
            Align2::LEFT_CENTER,
            proj_name,
            FontId::proportional(FONT_MD),
            Color32::from_rgb(148, 163, 184),
        );

        ui.add_space(8.0);
        let (badge_rect, _) = ui.allocate_exact_size(Vec2::new(56.0, 20.0), Sense::hover());
        ui.painter().rect_filled(badge_rect, CornerRadius::same(4), skel_fill);

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // Action button skeletons
            for _ in 0..3 {
                let (btn_rect, _) = ui.allocate_exact_size(Vec2::new(32.0, 32.0), Sense::hover());
                ui.painter().rect_filled(btn_rect, CornerRadius::same(6), skel_fill);
                ui.add_space(4.0);
            }
            // Branch selector skeleton
            let (branch_rect, _) = ui.allocate_exact_size(Vec2::new(160.0, 32.0), Sense::hover());
            ui.painter().rect_filled(branch_rect, CornerRadius::same(6), skel_fill);
        });
    });

    ui.add_space(4.0);

    // Subtitle / directory path skeleton line
    let (sub_rect, _) = ui.allocate_exact_size(Vec2::new(320.0, 14.0), Sense::hover());
    ui.painter().rect_filled(sub_rect, CornerRadius::same(4), skel_fill);

    ui.add_space(16.0);

    // 2. Environment Cards Row Skeleton
    let card_w = ((full_w - 24.0) / 3.0).clamp(160.0, 240.0);
    horizontal_row(ui, ENV_CARD_HEIGHT, |ui| {
        for i in 0..3 {
            let (card_rect, _) = ui.allocate_exact_size(Vec2::new(card_w, ENV_CARD_HEIGHT), Sense::hover());
            ui.painter().rect_filled(card_rect, CornerRadius::same(8), CARD_BG);
            ui.painter().rect_stroke(
                card_rect,
                CornerRadius::same(8),
                Stroke::new(1.0_f32, CARD_BORDER),
                StrokeKind::Inside,
            );

            // Card title skeleton badge
            let badge_w = if i == 0 { 80.0 } else if i == 1 { 64.0 } else { 92.0 };
            let env_badge_rect = Rect::from_min_size(
                Pos2::new(card_rect.left() + 12.0, card_rect.top() + 12.0),
                Vec2::new(badge_w, 20.0),
            );
            ui.painter().rect_filled(env_badge_rect, CornerRadius::same(4), skel_fill);

            // Version line skeleton
            let ver_rect = Rect::from_min_size(
                Pos2::new(card_rect.left() + 12.0, card_rect.top() + 42.0),
                Vec2::new(card_w * 0.65, 16.0),
            );
            ui.painter().rect_filled(ver_rect, CornerRadius::same(4), skel_fill);

            // Host / server line skeleton
            let host_rect = Rect::from_min_size(
                Pos2::new(card_rect.left() + 12.0, card_rect.top() + 68.0),
                Vec2::new(card_w * 0.45, 12.0),
            );
            ui.painter().rect_filled(host_rect, CornerRadius::same(3), skel_fill);

            ui.add_space(10.0);
        }
    });

    ui.add_space(16.0);

    // 3. Navigation Tabs Bar Skeleton
    horizontal_row(ui, 34.0, |ui| {
        for w in [130.0, 140.0, 120.0] {
            let (tab_rect, _) = ui.allocate_exact_size(Vec2::new(w, 32.0), Sense::hover());
            ui.painter().rect_filled(tab_rect, CornerRadius::same(6), skel_fill);
            ui.add_space(8.0);
        }
    });

    ui.add_space(12.0);

    // 4. Large Main Workspace Content Skeleton Card (Pipeline steps preview)
    egui::Frame::new()
        .fill(CARD_BG)
        .stroke(Stroke::new(1.0_f32, CARD_BORDER))
        .corner_radius(10.0)
        .inner_margin(Margin::same(18))
        .show(ui, |ui| {
            // Header inside card
            horizontal_row(ui, 28.0, |ui| {
                let (ch_rect, _) = ui.allocate_exact_size(Vec2::new(200.0, 20.0), Sense::hover());
                ui.painter().rect_filled(ch_rect, CornerRadius::same(4), skel_fill);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let (act_rect, _) = ui.allocate_exact_size(Vec2::new(110.0, 28.0), Sense::hover());
                    ui.painter().rect_filled(act_rect, CornerRadius::same(6), skel_fill);
                });
            });

            ui.add_space(12.0);

            // 3 Pipeline Step Skeletons
            for (step_title_w, step_cmd_w) in [(180.0, 0.75), (240.0, 0.9), (140.0, 0.6)] {
                let step_card_h = 76.0;
                let (step_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), step_card_h), Sense::hover());
                ui.painter().rect_filled(step_rect, CornerRadius::same(8), SUBTLE_BG);
                ui.painter().rect_stroke(
                    step_rect,
                    CornerRadius::same(8),
                    Stroke::new(1.0_f32, skel_border),
                    StrokeKind::Inside,
                );

                // Step number circle skeleton
                let num_pos = Pos2::new(step_rect.left() + 22.0, step_rect.top() + 22.0);
                ui.painter().circle_filled(num_pos, 10.0, skel_fill);

                // Step title skeleton
                let st_rect = Rect::from_min_size(
                    Pos2::new(step_rect.left() + 42.0, step_rect.top() + 14.0),
                    Vec2::new(step_title_w, 16.0),
                );
                ui.painter().rect_filled(st_rect, CornerRadius::same(4), skel_fill);

                // Step command skeleton bar
                let cmd_w = (step_rect.width() - 56.0) * step_cmd_w;
                let sc_rect = Rect::from_min_size(
                    Pos2::new(step_rect.left() + 42.0, step_rect.top() + 42.0),
                    Vec2::new(cmd_w, 20.0),
                );
                ui.painter().rect_filled(sc_rect, CornerRadius::same(4), skel_fill);

                ui.add_space(10.0);
            }
        });
}

/// Backward-compatible wrapper for render_project_loading_indicator
#[allow(dead_code)]
fn render_project_loading_indicator(ui: &mut egui::Ui, proj_name: &str) {
    render_project_workspace_skeleton(ui, proj_name);
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
    let s = Stroke::new(1.75_f32, color);
    painter.line_segment([Pos2::new(c.x - 6.0, c.y - 3.0), Pos2::new(c.x, c.y - 6.0)], s);
    painter.line_segment([Pos2::new(c.x, c.y - 6.0), Pos2::new(c.x + 6.0, c.y - 3.0)], s);
    painter.line_segment([Pos2::new(c.x + 6.0, c.y - 3.0), Pos2::new(c.x, c.y)], s);
    painter.line_segment([Pos2::new(c.x, c.y), Pos2::new(c.x - 6.0, c.y - 3.0)], s);

    painter.line_segment([Pos2::new(c.x - 6.0, c.y + 1.0), Pos2::new(c.x, c.y + 4.0)], s);
    painter.line_segment([Pos2::new(c.x, c.y + 4.0), Pos2::new(c.x + 6.0, c.y + 1.0)], s);
}

fn draw_icon_git_branch(painter: &egui::Painter, c: Pos2, color: Color32) {
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

fn draw_icon_server(painter: &egui::Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(1.75_f32, color);
    let top_r = Rect::from_center_size(Pos2::new(c.x, c.y - 2.5), Vec2::new(11.0, 4.0));
    let bot_r = Rect::from_center_size(Pos2::new(c.x, c.y + 2.5), Vec2::new(11.0, 4.0));
    painter.rect_stroke(top_r, CornerRadius::same(1), s, StrokeKind::Inside);
    painter.rect_stroke(bot_r, CornerRadius::same(1), s, StrokeKind::Inside);
}

fn draw_icon_folder(painter: &egui::Painter, c: Pos2, color: Color32) {
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

fn draw_icon_refresh(painter: &egui::Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(1.75_f32, color);
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
    let s = Stroke::new(1.75_f32, color);
    painter.line_segment([Pos2::new(c.x - 6.0, c.y), Pos2::new(c.x + 6.0, c.y)], s);
    painter.line_segment([Pos2::new(c.x, c.y - 6.0), Pos2::new(c.x, c.y + 6.0)], s);
}

fn draw_icon_send(painter: &egui::Painter, c: Pos2, color: Color32) {
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

fn draw_icon_chevron(painter: &egui::Painter, c: Pos2, expanded: bool, color: Color32) {
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

fn draw_icon_check(painter: &egui::Painter, c: Pos2, color: Color32) {
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
    Copy,
    Folder,
    ChevronRight,
    ChevronDown,
    ChevronUp,
    Check,
    X,
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
            draw_icon_refresh(painter, c, color);
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
            group: None,
            sort_order: None,
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
            group: None,
            sort_order: None,
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
                let mut trigger_rect = None;
                app.render_deploy_action_bar(
                    ui,
                    "Development Customer Data Ops",
                    Some(rollback_dep.clone()),
                    &mut trigger_rect,
                );
            });
        });

        // Also test with empty version string to ensure clean fallback text
        app.deploy_version_input = "   ".to_string();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.set_min_width(1000.0);
                let mut trigger_rect = None;
                app.render_deploy_action_bar(ui, "Production", None, &mut trigger_rect);
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
            group: None,
            sort_order: None,
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
            group: None,
            sort_order: None,
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
            group: None,
            sort_order: None,
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
            group: None,
            sort_order: None,
        };
        let proj2 = Project {
            id: "proj-idle".to_string(),
            name: "Admin Dashboard".to_string(),
            description: "Idle".to_string(),
            local_path: ".".to_string(),
            environments: vec![],
            group: None,
            sort_order: None,
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

    #[test]
    fn test_project_context_menu_and_duplication_ui() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        let proj = Project {
            id: "proj-orig".to_string(),
            name: "Backend Service".to_string(),
            description: "Production API".to_string(),
            local_path: ".".to_string(),
            environments: vec![EnvironmentConfig {
                key: "prod".to_string(),
                name: "Production".to_string(),
                app_name: "backend-prod".to_string(),
                servers: vec!["10.0.0.1".to_string()],
                remote_user: "deploy".to_string(),
                vault_target: "v1".to_string(),
                remote_dir: "/app".to_string(),
                env_source: None,
                http_proxy: None,
                steps: vec![PipelineStep::new("Build", StepKind::LocalCommand {
                    command: "cargo build".to_string(),
                    allow_failure: false,
                })],
            }],
            group: None,
            sort_order: None,
        };
        storage.projects.push(proj);

        let mut app = ShiplyApp::new(storage);
        app.selection = SidebarSelection::Project("proj-orig".to_string());

        // Render sidebar and project workspace to verify options and context menus execute without panic
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_sidebar(ui.ctx(), 1.0);
                app.render_project_workspace(ui, "proj-orig");
            });
        });

        // Test duplication action via storage
        let dup = app.storage.duplicate_project("proj-orig").expect("duplicate project");
        assert_eq!(dup.name, "Backend Service (Copy)");
        assert_eq!(dup.id, "proj-orig-copy");
        assert_eq!(app.storage.projects.len(), 2);
    }

    #[test]
    fn test_project_sorting_and_drag_reordering_ui() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        let p_beta = Project {
            id: "p-beta".to_string(),
            name: "Beta API".to_string(),
            description: "".to_string(),
            local_path: ".".to_string(),
            environments: vec![],
            group: None,
            sort_order: Some(0),
        };
        let p_alpha = Project {
            id: "p-alpha".to_string(),
            name: "Alpha Web".to_string(),
            description: "".to_string(),
            local_path: ".".to_string(),
            environments: vec![],
            group: None,
            sort_order: Some(1),
        };
        let p_gamma = Project {
            id: "p-gamma".to_string(),
            name: "Gamma Service".to_string(),
            description: "".to_string(),
            local_path: ".".to_string(),
            environments: vec![],
            group: None,
            sort_order: Some(2),
        };
        storage.projects = vec![p_beta, p_alpha, p_gamma];

        let mut app = ShiplyApp::new(storage);

        // 1. Render sidebar initial custom order
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_sidebar(ui.ctx(), 1.0);
            });
        });
        assert_eq!(app.storage.projects[0].name, "Beta API");
        assert_eq!(app.storage.projects[1].name, "Alpha Web");
        assert_eq!(app.storage.projects[2].name, "Gamma Service");

        // 2. Sort by NameAsc
        app.project_sort_mode = ProjectSortMode::NameAsc;
        app.storage.sort_projects(ProjectSortMode::NameAsc);
        assert_eq!(app.storage.projects[0].name, "Alpha Web");
        assert_eq!(app.storage.projects[1].name, "Beta API");
        assert_eq!(app.storage.projects[2].name, "Gamma Service");

        // 3. Sort by NameDesc
        app.project_sort_mode = ProjectSortMode::NameDesc;
        app.storage.sort_projects(ProjectSortMode::NameDesc);
        assert_eq!(app.storage.projects[0].name, "Gamma Service");
        assert_eq!(app.storage.projects[1].name, "Beta API");
        assert_eq!(app.storage.projects[2].name, "Alpha Web");

        // 4. Test reorder simulation (dragging first item to last position)
        app.dragged_project_idx = Some(0);
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_sidebar(ui.ctx(), 1.0);
            });
        });
        app.storage.reorder_project(0, 2);
        app.dragged_project_idx = None;
        app.project_sort_mode = ProjectSortMode::Custom;

        assert_eq!(app.storage.projects[0].name, "Beta API");
        assert_eq!(app.storage.projects[1].name, "Alpha Web");
        assert_eq!(app.storage.projects[2].name, "Gamma Service");

        // 5. Test move down and move up
        app.storage.move_project_down(0);
        assert_eq!(app.storage.projects[0].name, "Alpha Web");
        assert_eq!(app.storage.projects[1].name, "Beta API");

        app.storage.move_project_up(1);
        assert_eq!(app.storage.projects[0].name, "Beta API");
        assert_eq!(app.storage.projects[1].name, "Alpha Web");
    }

    #[test]
    fn test_project_grouping_and_collapsible_sections_ui() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        let p_backend = Project {
            id: "p-backend".to_string(),
            name: "Backend Service".to_string(),
            description: "".to_string(),
            local_path: ".".to_string(),
            environments: vec![],
            group: Some("Backend".to_string()),
            sort_order: Some(0),
        };
        let p_frontend = Project {
            id: "p-frontend".to_string(),
            name: "Frontend App".to_string(),
            description: "".to_string(),
            local_path: ".".to_string(),
            environments: vec![],
            group: Some("Frontend".to_string()),
            sort_order: Some(1),
        };
        let p_ungrouped = Project {
            id: "p-ungrouped".to_string(),
            name: "Ungrouped Service".to_string(),
            description: "".to_string(),
            local_path: ".".to_string(),
            environments: vec![],
            group: None,
            sort_order: Some(2),
        };
        storage.projects = vec![p_backend, p_frontend, p_ungrouped];

        let mut app = ShiplyApp::new(storage);

        // Verify existing project groups
        let groups = app.storage.existing_project_groups();
        assert_eq!(groups, vec!["Backend", "Frontend"]);

        // Render sidebar with groups expanded
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_sidebar(ui.ctx(), 1.0);
            });
        });

        // Collapse "Backend" group and render again
        app.collapsed_project_groups.insert("Backend".to_string());
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_sidebar(ui.ctx(), 1.0);
            });
        });
        assert!(app.collapsed_project_groups.contains("Backend"));

        // Change group of p-ungrouped to Backend
        assert!(app.storage.set_project_group("p-ungrouped", Some("Backend".to_string())));
        assert_eq!(
            app.storage.projects.iter().find(|p| p.id == "p-ungrouped").unwrap().group.as_deref(),
            Some("Backend")
        );

        // Remove group from p-frontend
        assert!(app.storage.set_project_group("p-frontend", None));
        assert_eq!(
            app.storage.projects.iter().find(|p| p.id == "p-frontend").unwrap().group,
            None
        );
    }

    #[test]
    fn test_sidebar_design_tokens_skeleton_loading_and_modals() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        // 1. Verify typography tokens
        assert_eq!(FS_TITLE, 18.0);
        assert_eq!(FS_BRAND, 16.0);
        assert_eq!(FS_BASE, 14.0);
        assert_eq!(FS_SMALL, 13.0);
        assert_eq!(FS_MICRO, 11.0);

        // 2. Verify spacing and radius scales
        assert_eq!(SP_4, 4.0);
        assert_eq!(SP_8, 8.0);
        assert_eq!(SP_12, 12.0);
        assert_eq!(SP_16, 16.0);
        assert_eq!(SP_20, 20.0);
        assert_eq!(SP_24, 24.0);
        assert_eq!(R_SM, 6.0);
        assert_eq!(R_MD, 8.0);
        assert_eq!(R_LG, 12.0);
        assert_eq!(R_XL, 16.0);

        // 3. Verify color tokens
        assert_eq!(SB_BG, Color32::from_rgb(11, 18, 32));
        assert_eq!(SB_SURFACE, Color32::from_rgb(17, 26, 46));
        assert_eq!(SB_SURFACE_HOVER, Color32::from_rgb(22, 33, 58));
        assert_eq!(MD_BG, Color32::WHITE);
        assert_eq!(MD_BORDER, Color32::from_rgb(226, 232, 240));

        let mut storage = crate::storage::AppStorage::default();
        storage.projects = vec![
            Project {
                id: "p-test-long".to_string(),
                name: "A Super Long Project Name That Should Not Wrap Onto Second Line".to_string(),
                description: "Test".to_string(),
                local_path: ".".to_string(),
                environments: vec![
                    EnvironmentConfig {
                        key: "dev".to_string(),
                        name: "Development".to_string(),
                        app_name: "test-dev".to_string(),
                        servers: vec!["127.0.0.1".to_string()],
                        remote_user: "dev".to_string(),
                        vault_target: "v1".to_string(),
                        remote_dir: "/app".to_string(),
                        env_source: None,
                        http_proxy: None,
                        steps: vec![],
                    },
                    EnvironmentConfig {
                        key: "prod".to_string(),
                        name: "Production".to_string(),
                        app_name: "test-prod".to_string(),
                        servers: vec!["127.0.0.2".to_string()],
                        remote_user: "prod".to_string(),
                        vault_target: "v1".to_string(),
                        remote_dir: "/app".to_string(),
                        env_source: None,
                        http_proxy: None,
                        steps: vec![],
                    },
                ],
                group: None,
                sort_order: Some(0),
            },
        ];

        let mut app = ShiplyApp::new(storage);

        // Test rendering with project switch loading (shimmer skeleton pills on card)
        app.project_switch_loading_id = Some("p-test-long".to_string());
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_sidebar(ui.ctx(), 1.0);
            });
        });

        // Test rendering empty state with refresh shimmer skeletons
        let mut empty_app = ShiplyApp::new(crate::storage::AppStorage::default());
        empty_app.sidebar_refreshing_until = Some(Instant::now() + Duration::from_secs(5));
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                empty_app.render_sidebar(ui.ctx(), 1.0);
            });
        });

        // Test rendering modals with design tokens
        app.show_new_project_modal = true;
        let _ = ctx.run(Default::default(), |ctx| {
            app.render_new_project_modal(ctx);
        });
        app.show_new_project_modal = false;

        app.open_edit_project_modal(0);
        let _ = ctx.run(Default::default(), |ctx| {
            app.render_edit_project_modal(ctx);
        });
        app.show_edit_project_modal = false;

        app.show_new_env_modal = true;
        let _ = ctx.run(Default::default(), |ctx| {
            app.render_new_environment_modal(ctx);
        });
        app.show_new_env_modal = false;

        app.pending_delete_confirm = Some(ConfirmDeleteAction::Project {
            project_id: "p-test-long".to_string(),
            project_name: "Test".to_string(),
        });
        let _ = ctx.run(Default::default(), |ctx| {
            app.render_delete_confirmation_modal(ctx);
        });
        app.pending_delete_confirm = None;

        app.show_bulk_deploy_modal = true;
        let _ = ctx.run(Default::default(), |ctx| {
            app.render_bulk_deploy_modal(ctx);
        });
        app.show_bulk_deploy_modal = false;

        app.show_update_modal = true;
        let _ = ctx.run(Default::default(), |ctx| {
            app.render_github_update_modal(ctx);
        });
        app.show_update_modal = false;
    }

    #[test]
    fn test_sidebar_sort_dropdown_menu_interaction() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        storage.projects = vec![
            Project {
                id: "p1".to_string(),
                name: "Alpha".to_string(),
                description: "".to_string(),
                local_path: ".".to_string(),
                environments: vec![],
                group: None,
                sort_order: Some(0),
            },
            Project {
                id: "p2".to_string(),
                name: "Beta".to_string(),
                description: "".to_string(),
                local_path: ".".to_string(),
                environments: vec![],
                group: None,
                sort_order: Some(1),
            },
        ];

        let mut app = ShiplyApp::new(storage);
        assert!(!app.sidebar_sort_dropdown_open);

        // 1. Open dropdown
        app.sidebar_sort_dropdown_open = true;

        // 2. Render sidebar and dropdown
        let _ = ctx.run(Default::default(), |ctx| {
            app.render_sidebar(ctx, 1.0);
        });
        assert!(app.sidebar_sort_dropdown_open);

        // 3. Test keyboard navigation cycling through sort modes
        let modes = [
            ProjectSortMode::Custom,
            ProjectSortMode::NameAsc,
            ProjectSortMode::NameDesc,
            ProjectSortMode::RecentDeploy,
        ];
        for expected_mode in modes {
            app.project_sort_mode = expected_mode;
            app.storage.sort_projects(expected_mode);
            assert_eq!(app.project_sort_mode, expected_mode);
        }

        // 4. Closing dropdown
        app.sidebar_sort_dropdown_open = false;
        assert!(!app.sidebar_sort_dropdown_open);
    }

    #[test]
    fn test_sidebar_custom_context_menu_viewport_flip_and_actions() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        storage.projects = vec![
            Project {
                id: "p-first".to_string(),
                name: "First Project".to_string(),
                description: "".to_string(),
                local_path: ".".to_string(),
                environments: vec![],
                group: Some("Services".to_string()),
                sort_order: Some(0),
            },
            Project {
                id: "p-second".to_string(),
                name: "Second Project".to_string(),
                description: "".to_string(),
                local_path: ".".to_string(),
                environments: vec![],
                group: None,
                sort_order: Some(1),
            },
        ];

        let mut app = ShiplyApp::new(storage);

        // 1. Trigger context menu for first project near top-left
        app.sidebar_context_menu = Some(SidebarContextMenuState {
            project_id: "p-first".to_string(),
            click_pos: Pos2::new(100.0, 100.0),
            submenu_open: false,
        });

        let _ = ctx.run(Default::default(), |ctx| {
            app.render_sidebar_custom_context_menu(ctx);
        });
        assert!(app.sidebar_context_menu.is_some());

        // 2. Trigger context menu near bottom-right to test viewport flip math
        let screen = ctx.screen_rect();
        let near_bottom_right = Pos2::new(screen.right() - 10.0, screen.bottom() - 10.0);
        app.sidebar_context_menu = Some(SidebarContextMenuState {
            project_id: "p-second".to_string(),
            click_pos: near_bottom_right,
            submenu_open: true,
        });

        let _ = ctx.run(Default::default(), |ctx| {
            app.render_sidebar_custom_context_menu(ctx);
        });
        assert!(app.sidebar_context_menu.is_some());

        // 3. Test destructive styling tokens
        assert_eq!(DESTRUCTIVE_TEXT, Color32::from_rgb(248, 113, 113));
        assert_eq!(DESTRUCTIVE_HOVER_BG, Color32::from_rgba_premultiplied(29, 8, 8, 31));
        assert_eq!(DESTRUCTIVE_ACTIVE_BG, Color32::from_rgba_premultiplied(48, 14, 14, 51));

        // 4. Test outline icons rendering without panic
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let icons = [
                    OutlineIcon::Trash,
                    OutlineIcon::Refresh,
                    OutlineIcon::ArrowUp,
                    OutlineIcon::ArrowDown,
                    OutlineIcon::Edit,
                    OutlineIcon::RotateCcw,
                    OutlineIcon::Key,
                    OutlineIcon::GripVertical,
                    OutlineIcon::Copy,
                    OutlineIcon::Folder,
                    OutlineIcon::ChevronRight,
                    OutlineIcon::ChevronDown,
                    OutlineIcon::ChevronUp,
                    OutlineIcon::Check,
                    OutlineIcon::X,
                ];
                for (i, icon) in icons.iter().enumerate() {
                    let c = Pos2::new(20.0 + i as f32 * 20.0, 20.0);
                    draw_outline_icon(ui.painter(), c, *icon, SB_TEXT);
                }
            });
        });
    }

    #[test]
    fn test_deployment_history_filters_by_active_environment_for_selected_project() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        let proj = Project {
            id: "proj-backend".to_string(),
            name: "Backend Service".to_string(),
            description: "API".to_string(),
            local_path: ".".to_string(),
            environments: vec![
                EnvironmentConfig {
                    key: "dev".to_string(),
                    name: "Development".to_string(),
                    app_name: "backend-dev".to_string(),
                    servers: vec!["10.0.0.1".to_string()],
                    remote_user: "deploy".to_string(),
                    vault_target: "v1".to_string(),
                    remote_dir: "/apps/dev".to_string(),
                    env_source: None,
                    http_proxy: None,
                    steps: Vec::new(),
                },
                EnvironmentConfig {
                    key: "prod".to_string(),
                    name: "Production".to_string(),
                    app_name: "backend-prod".to_string(),
                    servers: vec!["10.0.0.2".to_string()],
                    remote_user: "deploy".to_string(),
                    vault_target: "v2".to_string(),
                    remote_dir: "/apps/prod".to_string(),
                    env_source: None,
                    http_proxy: None,
                    steps: Vec::new(),
                },
            ],
            group: None,
            sort_order: None,
        };
        storage.projects = vec![proj];

        let dep_dev1 = DeploymentRecord {
            id: "dep-dev-1".to_string(),
            project_id: "proj-backend".to_string(),
            project_name: "Backend Service".to_string(),
            environment: "dev".to_string(),
            app_name: "backend-dev".to_string(),
            version: "1.0.0".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("commit1".to_string()),
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
        let dep_dev2 = DeploymentRecord {
            id: "dep-dev-2".to_string(),
            project_id: "proj-backend".to_string(),
            project_name: "Backend Service".to_string(),
            environment: "dev".to_string(),
            app_name: "backend-dev".to_string(),
            version: "1.0.1".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("commit2".to_string()),
            git_changes: vec![],
            is_rollback: false,
            servers: vec!["10.0.0.1".to_string()],
            status: DeploymentStatus::Succeeded,
            started_at: chrono::Utc::now(),
            finished_at: Some(chrono::Utc::now()),
            duration_ms: Some(1500),
            step_logs: vec![],
            executed_steps: vec![],
        };
        let dep_prod1 = DeploymentRecord {
            id: "dep-prod-1".to_string(),
            project_id: "proj-backend".to_string(),
            project_name: "Backend Service".to_string(),
            environment: "prod".to_string(),
            app_name: "backend-prod".to_string(),
            version: "1.0.0".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("commit1".to_string()),
            git_changes: vec![],
            is_rollback: false,
            servers: vec!["10.0.0.2".to_string()],
            status: DeploymentStatus::Succeeded,
            started_at: chrono::Utc::now(),
            finished_at: Some(chrono::Utc::now()),
            duration_ms: Some(2000),
            step_logs: vec![],
            executed_steps: vec![],
        };
        storage.deployments = vec![dep_dev1, dep_dev2, dep_prod1];

        let mut app = ShiplyApp::new(storage);
        app.selection = SidebarSelection::Project("proj-backend".to_string());
        app.selected_env_key = "dev".to_string();

        // 1. In project workspace with "dev" active: filtered deployments should only show 2 dev records
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_history_and_logs_view(ui, Some("proj-backend"));
            });
        });
        let dev_records = app.storage.filter_deployments(Some("proj-backend"), Some(&app.selected_env_key), None);
        assert_eq!(dev_records.len(), 2);
        assert!(dev_records.iter().all(|d| d.environment == "dev"));

        // 2. Switch to "prod" active: filtered deployments should only show 1 prod record
        app.selected_env_key = "prod".to_string();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_history_and_logs_view(ui, Some("proj-backend"));
            });
        });
        let prod_records = app.storage.filter_deployments(Some("proj-backend"), Some(&app.selected_env_key), None);
        assert_eq!(prod_records.len(), 1);
        assert_eq!(prod_records[0].environment, "prod");

        // 3. Global AllDeployments view: shows all 3 deployments across environments
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_history_and_logs_view(ui, None);
            });
        });
        let all_records = app.storage.filter_deployments(None, None, None);
        assert_eq!(all_records.len(), 3);
    }

    #[test]
    fn test_sync_version_candidates_and_application() {
        let mut storage = crate::storage::AppStorage::default();
        let proj = Project {
            id: "proj-sync".to_string(),
            name: "Frontend App".to_string(),
            description: "SPA".to_string(),
            local_path: ".".to_string(),
            environments: vec![
                EnvironmentConfig {
                    key: "dev".to_string(),
                    name: "Development".to_string(),
                    app_name: "front-dev".to_string(),
                    servers: vec![],
                    remote_user: "u".to_string(),
                    vault_target: "v".to_string(),
                    remote_dir: "/apps".to_string(),
                    env_source: None,
                    http_proxy: None,
                    steps: Vec::new(),
                },
                EnvironmentConfig {
                    key: "staging".to_string(),
                    name: "Staging".to_string(),
                    app_name: "front-staging".to_string(),
                    servers: vec![],
                    remote_user: "u".to_string(),
                    vault_target: "v".to_string(),
                    remote_dir: "/apps".to_string(),
                    env_source: None,
                    http_proxy: None,
                    steps: Vec::new(),
                },
                EnvironmentConfig {
                    key: "prod".to_string(),
                    name: "Production".to_string(),
                    app_name: "front-prod".to_string(),
                    servers: vec![],
                    remote_user: "u".to_string(),
                    vault_target: "v".to_string(),
                    remote_dir: "/apps".to_string(),
                    env_source: None,
                    http_proxy: None,
                    steps: Vec::new(),
                },
            ],
            group: None,
            sort_order: None,
        };
        storage.projects = vec![proj];

        let dep_dev = DeploymentRecord {
            id: "dep-dev-sync".to_string(),
            project_id: "proj-sync".to_string(),
            project_name: "Frontend App".to_string(),
            environment: "dev".to_string(),
            app_name: "front-dev".to_string(),
            version: "1.3.5".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("c1".to_string()),
            git_changes: vec![],
            is_rollback: false,
            servers: vec![],
            status: DeploymentStatus::Succeeded,
            started_at: chrono::Utc::now(),
            finished_at: Some(chrono::Utc::now()),
            duration_ms: Some(1000),
            step_logs: vec![],
            executed_steps: vec![],
        };
        let dep_staging = DeploymentRecord {
            id: "dep-staging-sync".to_string(),
            project_id: "proj-sync".to_string(),
            project_name: "Frontend App".to_string(),
            environment: "staging".to_string(),
            app_name: "front-staging".to_string(),
            version: "1.2.0".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("c2".to_string()),
            git_changes: vec![],
            is_rollback: false,
            servers: vec![],
            status: DeploymentStatus::Succeeded,
            started_at: chrono::Utc::now() - chrono::Duration::hours(2),
            finished_at: Some(chrono::Utc::now() - chrono::Duration::hours(2)),
            duration_ms: Some(900),
            step_logs: vec![],
            executed_steps: vec![],
        };
        storage.deployments = vec![dep_dev, dep_staging];

        let mut app = ShiplyApp::new(storage);
        app.selection = SidebarSelection::Project("proj-sync".to_string());
        app.selected_env_key = "prod".to_string();
        app.cached_pkg_version = Some("1.3.5".to_string());

        // While in "prod", candidates should come from dev and staging
        let candidates = app.sync_version_candidates("proj-sync");
        assert_eq!(candidates.len(), 2);
        let dev_cand = candidates.iter().find(|c| c.source_name == "Development").unwrap();
        assert_eq!(dev_cand.version, "1.3.5");
        assert!(dev_cand.is_latest);

        let staging_cand = candidates.iter().find(|c| c.source_name == "Staging").unwrap();
        assert_eq!(staging_cand.version, "1.2.0");
        assert!(!staging_cand.is_latest);

        // Apply synced version from Development
        app.apply_synced_version("1.3.5", "Development");
        assert_eq!(app.deploy_version_input, "1.3.5");
        assert!(!app.sync_version_dropdown_open);
        let banner = app.status_banner.as_ref().unwrap();
        assert!(banner.0.contains("Synced version to v1.3.5 from Development"));
        assert!(banner.1); // is_success
    }

    #[test]
    fn test_console_loading_indicator_on_running_pipeline() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        let proj = Project {
            id: "proj-running-console".to_string(),
            name: "Realtime API".to_string(),
            description: "Console Test".to_string(),
            local_path: ".".to_string(),
            environments: vec![EnvironmentConfig {
                key: "prod".to_string(),
                name: "Production".to_string(),
                app_name: "realtime-prod".to_string(),
                servers: vec!["10.0.0.1".to_string()],
                remote_user: "deploy".to_string(),
                vault_target: "v".to_string(),
                remote_dir: "/apps".to_string(),
                env_source: None,
                http_proxy: None,
                steps: Vec::new(),
            }],
            group: None,
            sort_order: None,
        };
        storage.projects = vec![proj];

        let running_dep = DeploymentRecord {
            id: "dep-console-run".to_string(),
            project_id: "proj-running-console".to_string(),
            project_name: "Realtime API".to_string(),
            environment: "prod".to_string(),
            app_name: "realtime-prod".to_string(),
            version: "2.0.0".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("abc".to_string()),
            git_changes: vec![],
            is_rollback: false,
            servers: vec!["10.0.0.1".to_string()],
            status: DeploymentStatus::Running,
            started_at: chrono::Utc::now(),
            finished_at: None,
            duration_ms: None,
            step_logs: vec![crate::domain::StepExecutionLog {
                step_name: "Building Docker container".to_string(),
                status: DeploymentStatus::Running,
                duration_ms: 3200,
                output: "Step 1/5: RUN cargo build\n[OK] Intermediate image cached\n".to_string(),
                command_summary: "docker build -t app:2.0.0 .".to_string(),
            }],
            executed_steps: vec![],
        };
        storage.deployments = vec![running_dep];

        let mut app = ShiplyApp::new(storage);
        app.selection = SidebarSelection::Project("proj-running-console".to_string());
        app.selected_env_key = "prod".to_string();
        app.active_deployment_id = Some("dep-console-run".to_string());
        app.selected_deployment_id = Some("dep-console-run".to_string());
        app.console_collapsed = false;

        // Render history and terminal window with running deployment
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_history_and_logs_view(ui, Some("proj-running-console"));
            });
        });

        assert_eq!(app.active_deployment_id.as_deref(), Some("dep-console-run"));
    }

    #[test]
    fn test_concurrent_deployments_across_different_projects_prevent_same_project_only() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        let proj_backend = Project {
            id: "proj-backend".to_string(),
            name: "Backend Service".to_string(),
            description: "API".to_string(),
            local_path: ".".to_string(),
            environments: vec![EnvironmentConfig {
                key: "prod".to_string(),
                name: "Production".to_string(),
                app_name: "backend-prod".to_string(),
                servers: vec!["10.0.0.1".to_string()],
                remote_user: "deploy".to_string(),
                vault_target: "v1".to_string(),
                remote_dir: "/apps/backend".to_string(),
                env_source: None,
                http_proxy: None,
                steps: Vec::new(),
            }],
            group: None,
            sort_order: None,
        };
        let proj_frontend = Project {
            id: "proj-frontend".to_string(),
            name: "Frontend Service".to_string(),
            description: "SPA".to_string(),
            local_path: ".".to_string(),
            environments: vec![EnvironmentConfig {
                key: "prod".to_string(),
                name: "Production".to_string(),
                app_name: "frontend-prod".to_string(),
                servers: vec!["10.0.0.2".to_string()],
                remote_user: "deploy".to_string(),
                vault_target: "v2".to_string(),
                remote_dir: "/apps/frontend".to_string(),
                env_source: None,
                http_proxy: None,
                steps: Vec::new(),
            }],
            group: None,
            sort_order: None,
        };
        storage.projects = vec![proj_backend.clone(), proj_frontend.clone()];

        let mut app = ShiplyApp::new(storage);

        // 1. Initially neither project is deploying
        assert!(!app.is_project_deploying("proj-backend"));
        assert!(!app.is_project_deploying("proj-frontend"));
        assert!(!app.has_active_deployments());

        // 2. Start deployment for backend
        let req_backend = crate::executor::DeploymentRequest {
            deployment_id: "dep-backend-1".to_string(),
            project_id: "proj-backend".to_string(),
            project_name: "Backend Service".to_string(),
            local_path: PathBuf::from("."),
            env_config: proj_backend.environments[0].clone(),
            version: "1.0.0".to_string(),
            is_rollback: false,
            previous_commit: None,
            manual_password: None,
            target_git_branch: None,
            bump_package_json: false,
            create_git_tag_on_success: false,
            skip_ssh_preflight: false,
            rollback_commit: None,
        };
        app.start_deployment(req_backend);

        // Backend is now deploying
        assert!(app.is_project_deploying("proj-backend"));
        assert!(!app.is_project_deploying("proj-frontend")); // Frontend is free!
        assert!(app.has_active_deployments());

        // 3. Attempting to deploy backend AGAIN should be prevented
        let req_backend_again = crate::executor::DeploymentRequest {
            deployment_id: "dep-backend-2".to_string(),
            project_id: "proj-backend".to_string(),
            project_name: "Backend Service".to_string(),
            local_path: PathBuf::from("."),
            env_config: proj_backend.environments[0].clone(),
            version: "1.0.1".to_string(),
            is_rollback: false,
            previous_commit: None,
            manual_password: None,
            target_git_branch: None,
            bump_package_json: false,
            create_git_tag_on_success: false,
            skip_ssh_preflight: false,
            rollback_commit: None,
        };
        app.start_deployment(req_backend_again);
        let banner = app.status_banner.as_ref().unwrap();
        assert!(banner.0.contains("A deployment for 'Backend Service'"));
        assert!(banner.0.contains("is already running"));
        assert!(!banner.1); // false = error banner

        // 4. Deploying frontend AT THE SAME TIME is allowed and succeeds!
        let req_frontend = crate::executor::DeploymentRequest {
            deployment_id: "dep-frontend-1".to_string(),
            project_id: "proj-frontend".to_string(),
            project_name: "Frontend Service".to_string(),
            local_path: PathBuf::from("."),
            env_config: proj_frontend.environments[0].clone(),
            version: "2.0.0".to_string(),
            is_rollback: false,
            previous_commit: None,
            manual_password: None,
            target_git_branch: None,
            bump_package_json: false,
            create_git_tag_on_success: false,
            skip_ssh_preflight: false,
            rollback_commit: None,
        };
        app.start_deployment(req_frontend);

        // BOTH projects are now deploying concurrently!
        assert!(app.is_project_deploying("proj-backend"));
        assert!(app.is_project_deploying("proj-frontend"));
        assert_eq!(app.active_deployments.len(), 2);

        // 5. Simulate finish event for backend
        let finished_backend = DeploymentRecord {
            id: "dep-backend-1".to_string(),
            project_id: "proj-backend".to_string(),
            project_name: "Backend Service".to_string(),
            environment: "prod".to_string(),
            app_name: "backend-prod".to_string(),
            version: "1.0.0".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("c1".to_string()),
            git_changes: vec![],
            is_rollback: false,
            servers: vec!["10.0.0.1".to_string()],
            status: DeploymentStatus::Succeeded,
            started_at: chrono::Utc::now(),
            finished_at: Some(chrono::Utc::now()),
            duration_ms: Some(1500),
            step_logs: vec![],
            executed_steps: vec![],
        };
        let (tx, rx) = mpsc::channel();
        tx.send(ExecutorEvent::Finished(finished_backend)).unwrap();
        app.event_rx = rx;
        app.poll_executor_events(&ctx);

        // Backend is no longer deploying, but Frontend is STILL deploying!
        assert!(!app.is_project_deploying("proj-backend"));
        assert!(app.is_project_deploying("proj-frontend"));
        assert_eq!(app.active_deployments.len(), 1);

        // Backend can now be deployed again
        let req_backend_new = crate::executor::DeploymentRequest {
            deployment_id: "dep-backend-3".to_string(),
            project_id: "proj-backend".to_string(),
            project_name: "Backend Service".to_string(),
            local_path: PathBuf::from("."),
            env_config: proj_backend.environments[0].clone(),
            version: "1.0.2".to_string(),
            is_rollback: false,
            previous_commit: None,
            manual_password: None,
            target_git_branch: None,
            bump_package_json: false,
            create_git_tag_on_success: false,
            skip_ssh_preflight: false,
            rollback_commit: None,
        };
        app.start_deployment(req_backend_new);
        assert!(app.is_project_deploying("proj-backend"));
    }

    #[test]
    fn test_concurrent_deployments_for_multiple_environments_on_same_project() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        let proj_multi = Project {
            id: "proj-api".to_string(),
            name: "API Service".to_string(),
            description: "API".to_string(),
            local_path: ".".to_string(),
            environments: vec![
                EnvironmentConfig {
                    key: "dev".to_string(),
                    name: "Development".to_string(),
                    app_name: "api-dev".to_string(),
                    servers: vec!["10.0.0.10".to_string()],
                    remote_user: "deploy".to_string(),
                    vault_target: "v-dev".to_string(),
                    remote_dir: "/apps/api-dev".to_string(),
                    env_source: None,
                    http_proxy: None,
                    steps: Vec::new(),
                },
                EnvironmentConfig {
                    key: "prod".to_string(),
                    name: "Production".to_string(),
                    app_name: "api-prod".to_string(),
                    servers: vec!["10.0.0.20".to_string()],
                    remote_user: "deploy".to_string(),
                    vault_target: "v-prod".to_string(),
                    remote_dir: "/apps/api-prod".to_string(),
                    env_source: None,
                    http_proxy: None,
                    steps: Vec::new(),
                },
            ],
            group: None,
            sort_order: None,
        };
        storage.projects = vec![proj_multi.clone()];

        let mut app = ShiplyApp::new(storage);

        // 1. Initially neither environment is deploying
        assert!(!app.is_env_deploying("proj-api", "dev"));
        assert!(!app.is_env_deploying("proj-api", "prod"));
        assert!(!app.is_project_deploying("proj-api"));

        // 2. Start deployment for Development environment
        let req_dev = crate::executor::DeploymentRequest {
            deployment_id: "dep-api-dev-1".to_string(),
            project_id: "proj-api".to_string(),
            project_name: "API Service".to_string(),
            local_path: PathBuf::from("."),
            env_config: proj_multi.environments[0].clone(),
            version: "1.0.0".to_string(),
            is_rollback: false,
            previous_commit: None,
            manual_password: None,
            target_git_branch: None,
            bump_package_json: false,
            create_git_tag_on_success: false,
            skip_ssh_preflight: false,
            rollback_commit: None,
        };
        app.start_deployment(req_dev);

        // Development is deploying; Production is free
        assert!(app.is_env_deploying("proj-api", "dev"));
        assert!(!app.is_env_deploying("proj-api", "prod"));
        assert!(app.is_project_deploying("proj-api"));

        // 3. Attempting to deploy Development AGAIN on the same project is prevented
        let req_dev_again = crate::executor::DeploymentRequest {
            deployment_id: "dep-api-dev-2".to_string(),
            project_id: "proj-api".to_string(),
            project_name: "API Service".to_string(),
            local_path: PathBuf::from("."),
            env_config: proj_multi.environments[0].clone(),
            version: "1.0.1".to_string(),
            is_rollback: false,
            previous_commit: None,
            manual_password: None,
            target_git_branch: None,
            bump_package_json: false,
            create_git_tag_on_success: false,
            skip_ssh_preflight: false,
            rollback_commit: None,
        };
        app.start_deployment(req_dev_again);
        let banner = app.status_banner.as_ref().unwrap();
        assert!(banner.0.contains("A deployment for 'API Service' (DEV) is already running"));
        assert!(!banner.1); // error banner

        // 4. Deploying Production AT THE SAME TIME on the SAME PROJECT is allowed!
        let req_prod = crate::executor::DeploymentRequest {
            deployment_id: "dep-api-prod-1".to_string(),
            project_id: "proj-api".to_string(),
            project_name: "API Service".to_string(),
            local_path: PathBuf::from("."),
            env_config: proj_multi.environments[1].clone(),
            version: "1.0.0".to_string(),
            is_rollback: false,
            previous_commit: None,
            manual_password: None,
            target_git_branch: None,
            bump_package_json: false,
            create_git_tag_on_success: false,
            skip_ssh_preflight: false,
            rollback_commit: None,
        };
        app.start_deployment(req_prod);

        // BOTH Development and Production are now deploying simultaneously on the SAME project!
        assert!(app.is_env_deploying("proj-api", "dev"));
        assert!(app.is_env_deploying("proj-api", "prod"));
        assert_eq!(app.active_deployments.len(), 2);
        assert!(app.is_project_deploying("proj-api"));

        // 5. Attempting to deploy Production AGAIN while running is prevented
        let req_prod_again = crate::executor::DeploymentRequest {
            deployment_id: "dep-api-prod-2".to_string(),
            project_id: "proj-api".to_string(),
            project_name: "API Service".to_string(),
            local_path: PathBuf::from("."),
            env_config: proj_multi.environments[1].clone(),
            version: "1.0.1".to_string(),
            is_rollback: false,
            previous_commit: None,
            manual_password: None,
            target_git_branch: None,
            bump_package_json: false,
            create_git_tag_on_success: false,
            skip_ssh_preflight: false,
            rollback_commit: None,
        };
        app.start_deployment(req_prod_again);
        let banner_prod = app.status_banner.as_ref().unwrap();
        assert!(banner_prod.0.contains("A deployment for 'API Service' (PROD) is already running"));
        assert!(!banner_prod.1);

        // 6. Development deployment completes
        let finished_dev = DeploymentRecord {
            id: "dep-api-dev-1".to_string(),
            project_id: "proj-api".to_string(),
            project_name: "API Service".to_string(),
            environment: "dev".to_string(),
            app_name: "api-dev".to_string(),
            version: "1.0.0".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("c1".to_string()),
            git_changes: vec![],
            is_rollback: false,
            servers: vec!["10.0.0.10".to_string()],
            status: DeploymentStatus::Succeeded,
            started_at: chrono::Utc::now(),
            finished_at: Some(chrono::Utc::now()),
            duration_ms: Some(800),
            step_logs: vec![],
            executed_steps: vec![],
        };
        let (tx, rx) = mpsc::channel();
        tx.send(ExecutorEvent::Finished(finished_dev)).unwrap();
        app.event_rx = rx;
        app.poll_executor_events(&ctx);

        // Development is done, but Production is STILL running!
        assert!(!app.is_env_deploying("proj-api", "dev"));
        assert!(app.is_env_deploying("proj-api", "prod"));
        assert_eq!(app.active_deployments.len(), 1);
        assert!(app.is_project_deploying("proj-api"));

        // 7. Development can now be deployed again while Production is still active
        let req_dev_new = crate::executor::DeploymentRequest {
            deployment_id: "dep-api-dev-3".to_string(),
            project_id: "proj-api".to_string(),
            project_name: "API Service".to_string(),
            local_path: PathBuf::from("."),
            env_config: proj_multi.environments[0].clone(),
            version: "1.0.2".to_string(),
            is_rollback: false,
            previous_commit: None,
            manual_password: None,
            target_git_branch: None,
            bump_package_json: false,
            create_git_tag_on_success: false,
            skip_ssh_preflight: false,
            rollback_commit: None,
        };
        app.start_deployment(req_dev_new);
        assert!(app.is_env_deploying("proj-api", "dev"));
        assert!(app.is_env_deploying("proj-api", "prod"));
        assert_eq!(app.active_deployments.len(), 2);
    }

    #[test]
    fn test_deployment_keeps_running_on_update_and_restart_deferred_until_finished() {
        let storage = crate::storage::AppStorage::default();
        let mut app = ShiplyApp::new(storage);
        let ctx = egui::Context::default();

        // 1. Mark an active deployment in progress
        app.active_deployments
            .insert(("proj-app".to_string(), "prod".to_string()), "dep-99".to_string());
        assert!(app.has_active_deployments());
        assert!(app.is_project_deploying("proj-app"));
        assert!(app.is_env_deploying("proj-app", "prod"));

        // 2. An update download completes while deployment is still running
        let (updater_tx, updater_rx) = mpsc::channel();
        updater_tx
            .send(UpdaterEvent::ReadyToRestart {
                new_version: "0.10.7".to_string(),
                exe_path: PathBuf::from("mock/path/shiply.exe"),
            })
            .unwrap();
        app.updater_rx = updater_rx;
        app.poll_executor_events(&ctx);

        // 3. Deployment was NOT interrupted, restart is deferred
        assert!(app.has_active_deployments());
        assert!(app.is_project_deploying("proj-app"));
        assert_eq!(
            app.pending_update_restart,
            Some(("0.10.7".to_string(), PathBuf::from("mock/path/shiply.exe")))
        );
        let banner = app.status_banner.as_ref().unwrap();
        assert!(banner.0.contains("Active deployments will continue running uninterrupted before restart."));

        // 4. When the deployment finishes, the deferred restart is automatically triggered
        let finished_record = DeploymentRecord {
            id: "dep-99".to_string(),
            project_id: "proj-app".to_string(),
            project_name: "App Service".to_string(),
            environment: "prod".to_string(),
            app_name: "app-prod".to_string(),
            version: "1.0.0".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("c1".to_string()),
            git_changes: vec![],
            is_rollback: false,
            servers: vec!["10.0.0.1".to_string()],
            status: DeploymentStatus::Succeeded,
            started_at: chrono::Utc::now(),
            finished_at: Some(chrono::Utc::now()),
            duration_ms: Some(1000),
            step_logs: vec![],
            executed_steps: vec![],
        };
        let (exec_tx, exec_rx) = mpsc::channel();
        exec_tx.send(ExecutorEvent::Finished(finished_record)).unwrap();
        app.event_rx = exec_rx;
        app.poll_executor_events(&ctx);

        // 5. Active deployments is now empty and pending_update_restart has been consumed
        assert!(!app.has_active_deployments());
        assert!(app.pending_update_restart.is_none());
        // Since mock exe does not exist, update_error safely records the error without crash
        assert!(app.update_error.is_some());
    }
}
