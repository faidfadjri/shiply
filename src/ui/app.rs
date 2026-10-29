use super::*;

pub struct ShiplyApp {
    pub(crate) storage: AppStorage,
    pub(crate) selection: SidebarSelection,
    pub(crate) selected_env_key: String,
    pub(crate) active_tab: WorkspaceTab,
    pub(crate) history_env_filter: Option<String>,
    pub(crate) history_visible_limit: usize,
    pub(crate) selected_deployment_id: Option<String>,

    // Debounced disk save & field editing buffers
    pub(crate) dirty_save_pending: bool,
    pub(crate) last_edit_time: Option<Instant>,
    pub(crate) servers_edit_buffer: Option<(String, String)>,

    // Collapsible Console Output & Code Changes state
    pub(crate) console_collapsed: bool,
    pub(crate) changes_collapsed: bool,
    pub(crate) collapsed_step_indices: HashSet<usize>,
    pub(crate) dragged_step_idx: Option<usize>,

    // Project Sorting & Grouping state
    pub(crate) project_sort_mode: ProjectSortMode,
    pub(crate) collapsed_project_groups: HashSet<String>,
    pub(crate) dragged_project_idx: Option<usize>,
    pub(crate) sidebar_sort_dropdown_open: bool,
    pub project_search_query: String,
    pub(crate) sidebar_context_menu: Option<SidebarContextMenuState>,
    pub(crate) sync_version_dropdown_open: bool,

    // Deployment version, branch & release controls
    pub(crate) deploy_version_input: String,
    pub(crate) selected_git_branch: String,
    pub(crate) bump_package_json: bool,
    pub(crate) create_git_tag: bool,
    pub(crate) manual_ssh_password: String,

    // Cached project metadata (Git + branches + package.json + OS Vault check)
    pub(crate) cached_project_id: Option<String>,
    pub(crate) cached_pkg_version: Option<String>,
    pub(crate) cached_git_meta: GitMetadata,
    pub(crate) cached_git_branches: Vec<String>,
    pub(crate) cached_vault_found: bool,
    pub(crate) cached_git_changes: Vec<String>,

    // Background execution channel
    pub(crate) event_tx: Sender<ExecutorEvent>,
    pub(crate) event_rx: Receiver<ExecutorEvent>,
    pub active_deployments: HashMap<(String, String), String>,
    pub active_deployment_id: Option<String>,

    // Async Git inspection channel (instant project switching without UI freezing)
    pub(crate) git_inspect_tx: Sender<GitInspectionResult>,
    pub(crate) git_inspect_rx: Receiver<GitInspectionResult>,

    // Add Project & Auto-Import .ps1 Modal state
    pub(crate) show_new_project_modal: bool,
    pub(crate) new_project_ps1_path: String,
    pub(crate) new_project_import_siblings: bool,
    pub(crate) new_project_name: String,
    pub(crate) new_project_path: String,
    pub(crate) new_project_desc: String,
    pub(crate) new_project_group: String,
    pub(crate) new_project_imported_envs: Vec<EnvironmentConfig>,
    pub(crate) new_project_import_notice: Option<String>,

    // Edit Project Modal state
    pub(crate) show_edit_project_modal: bool,
    pub(crate) edit_project_id: String,
    pub(crate) edit_project_name: String,
    pub(crate) edit_project_path: String,
    pub(crate) edit_project_desc: String,
    pub(crate) edit_project_group: String,
    pub(crate) edit_project_ps1_path: String,
    pub(crate) edit_project_import_notice: Option<String>,

    // Add Environment Modal state
    pub(crate) show_new_env_modal: bool,
    pub(crate) new_env_name: String,
    pub(crate) new_env_clone_from: String,
    pub(crate) new_env_ps1_path: String,

    // Bulk Deployment Modal & Queue state
    pub(crate) show_bulk_deploy_modal: bool,
    pub(crate) bulk_selected_projects: HashSet<String>,
    pub(crate) bulk_target_env_key: String,
    pub(crate) bulk_project_versions: BTreeMap<String, String>,
    pub(crate) bulk_bump_pkg: bool,
    pub(crate) bulk_create_tag: bool,
    pub(crate) bulk_deploy_queue: VecDeque<DeploymentRequest>,
    pub(crate) bulk_total_count: usize,

    // Onboarding & Multi-Platform OS Credential Setup Guide state
    pub(crate) show_onboarding_guide: bool,
    pub(crate) show_credential_guide: bool,
    pub(crate) cred_guide_os_tab: CredentialOsTab,
    pub(crate) cred_guide_target_input: String,
    pub(crate) cred_guide_user_input: String,

    // In-App GitHub Releases Auto-Updater state
    pub(crate) updater_tx: Sender<UpdaterEvent>,
    pub(crate) updater_rx: Receiver<UpdaterEvent>,
    pub(crate) show_update_modal: bool,
    pub(crate) available_update: Option<ReleaseInfo>,
    pub(crate) update_checking: bool,
    pub(crate) update_downloading: bool,
    pub(crate) update_progress: f32,
    pub(crate) update_status_text: String,
    pub(crate) update_error: Option<String>,
    pub pending_update_restart: Option<(String, PathBuf)>,

    // Throttled external file sync timer (Requirement #8 & #9)
    pub(crate) last_sync_check: Instant,

    // Status banner & Window/Sidebar state
    pub(crate) status_banner: Option<(String, bool)>,
    pub(crate) sidebar_collapsed: bool,
    pub(crate) window_maximized: bool,
    pub(crate) pending_delete_confirm: Option<ConfirmDeleteAction>,
    pub(crate) fonts_initialized: bool,
    pub(crate) logo_texture: Option<egui::TextureHandle>,
    pub(crate) project_switch_loading_id: Option<String>,
    pub(crate) project_switch_loading_start: Option<Instant>,
    pub(crate) sidebar_refreshing_until: Option<Instant>,
    pub(crate) dark_mode: bool,
    pub(crate) sidebar_drawer_open: bool,
    pub(crate) is_version_input_focused: bool,
    pub(crate) step_scp_sources_buffer: Option<(String, String)>,
    pub(crate) server_env_source_buffer: Option<(String, String)>,
    pub(crate) server_http_proxy_buffer: Option<(String, String)>,
    pub refresh_anim_start: Option<Instant>,
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
            project_search_query: String::new(),
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
            dark_mode: false,
            sidebar_drawer_open: false,
            is_version_input_focused: false,
            step_scp_sources_buffer: None,
            server_env_source_buffer: None,
            server_http_proxy_buffer: None,
            refresh_anim_start: None,
            git_inspect_tx,
            git_inspect_rx,
        };
        if let Some(notice) = migration_notice {
            app.status_banner = Some((notice, true));
        }
        app.refresh_selected_project_metadata(true);
        app
    }

    pub fn theme(&self) -> Theme {
        if self.dark_mode {
            Theme::dark()
        } else {
            Theme::light()
        }
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

    /// Configures system fonts: proportional UI text (`Segoe UI`) and monospace (`Consolas`) on Windows.
    pub(crate) fn install_system_fonts(ctx: &egui::Context) {
        #[allow(unused_mut)]
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
                }
                if let Ok(bytes) = fs::read(fonts_dir.join("consola.ttf")) {
                    fonts.font_data.insert(
                        "consola".to_owned(),
                        Arc::new(FontData::from_owned(bytes)),
                    );
                    if let Some(mono) = fonts.families.get_mut(&FontFamily::Monospace) {
                        mono.insert(0, "consola".to_owned());
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

    pub(crate) fn refresh_selected_project_metadata(&mut self, update_version_input: bool) {
        self.refresh_anim_start = Some(Instant::now());
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

    pub(crate) fn poll_executor_events(&mut self, ctx: &egui::Context) {
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

    pub(crate) fn start_deployment(&mut self, req: DeploymentRequest) {
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

    pub(crate) fn trigger_current_environment_deploy(&mut self) {
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

    pub(crate) fn open_bulk_deploy_modal(&mut self) {
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

    pub(crate) fn sync_bulk_project_versions(&mut self) {
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

    pub(crate) fn trigger_bulk_deployment(&mut self) {
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

    pub(crate) fn trigger_redeploy_from_record(&mut self, record: &DeploymentRecord, is_rollback: bool) {
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

    pub(crate) fn apply_ps1_auto_import_to_modal(&mut self, raw_path: &str) {
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

    pub fn configure_workspace_theme(ctx: &egui::Context, dark: bool) {
        let theme = if dark { Theme::dark() } else { Theme::light() };
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = Vec2::new(10.0, 8.0);
        style.spacing.button_padding = Vec2::new(12.0, 6.0);
        style.spacing.indent = 16.0;
        style.spacing.scroll.bar_outer_margin = 0.0;
        style.spacing.scroll.bar_inner_margin = 4.0;
        style.spacing.scroll.bar_width = 8.0;

        // Structured Typography Hierarchy
        let mut text_styles = BTreeMap::new();
        text_styles.insert(
            TextStyle::Heading,
            FontId::new(FS_TITLE, FontFamily::Proportional),
        );
        text_styles.insert(
            TextStyle::Body,
            FontId::new(FS_BASE, FontFamily::Proportional),
        );
        text_styles.insert(
            TextStyle::Button,
            FontId::new(FS_BASE, FontFamily::Proportional),
        );
        text_styles.insert(
            TextStyle::Small,
            FontId::new(FS_SMALL, FontFamily::Proportional),
        );
        text_styles.insert(
            TextStyle::Monospace,
            FontId::new(FS_BASE, FontFamily::Monospace),
        );
        style.text_styles = text_styles;

        let mut visuals = if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        visuals.panel_fill = theme.bg;
        visuals.window_fill = theme.surface;
        visuals.faint_bg_color = theme.surface_2;
        visuals.extreme_bg_color = theme.surface;
        visuals.override_text_color = Some(theme.text);

        visuals.selection.bg_fill = theme.primary_soft;
        visuals.selection.stroke = Stroke::new(1.5_f32, theme.primary);

        visuals.widgets.noninteractive.bg_fill = theme.surface;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, theme.border);
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, theme.text_muted);

        visuals.widgets.inactive.bg_fill = theme.surface;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, theme.border_strong);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, theme.text);
        visuals.widgets.inactive.corner_radius = R_MD.into();

        visuals.widgets.hovered.bg_fill = theme.surface_2;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, theme.primary);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, theme.text);
        visuals.widgets.hovered.corner_radius = R_MD.into();

        visuals.widgets.active.bg_fill = theme.surface_2;
        visuals.widgets.active.bg_stroke = Stroke::new(1.5_f32, theme.primary_active);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, theme.primary_active);
        visuals.widgets.active.corner_radius = R_MD.into();

        visuals.widgets.open.bg_fill = theme.surface;
        visuals.widgets.open.bg_stroke = Stroke::new(1.5_f32, theme.primary);
        visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, theme.text);
        visuals.widgets.open.corner_radius = R_MD.into();

        visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);

        style.visuals = visuals;
        ctx.set_style(style);
    }

    pub fn configure_light_workspace_style(ctx: &egui::Context) {
        Self::configure_workspace_theme(ctx, false);
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

        let refresh_angle = if let Some(start) = self.refresh_anim_start {
            let elapsed = start.elapsed().as_secs_f32();
            if elapsed < 0.65 {
                ctx.request_repaint();
                (elapsed / 0.65) * std::f32::consts::TAU
            } else {
                self.refresh_anim_start = None;
                0.0
            }
        } else {
            0.0
        };
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("refresh_anim_angle"), refresh_angle));

        if let Some(m) = ctx.input(|i| i.viewport().maximized) {
            self.window_maximized = m;
        }

        self.render_termius_topbar(ctx);
        let screen_width = ctx.screen_rect().width();
        let is_narrow = screen_width < 900.0;
        let is_rail_breakpoint = (900.0..1200.0).contains(&screen_width);

        if !is_narrow {
            self.sidebar_drawer_open = false;
            let sidebar_anim_t =
                ctx.animate_bool(egui::Id::new("sidebar_smooth_anim"), !self.sidebar_collapsed && !is_rail_breakpoint);
            self.render_sidebar(
                ctx,
                if is_rail_breakpoint || self.sidebar_collapsed {
                    0.25
                } else {
                    sidebar_anim_t.max(0.25)
                },
            );
        } else if self.sidebar_drawer_open {
            self.render_sidebar_drawer(ctx);
        }

        self.render_new_project_modal(ctx);
        self.render_edit_project_modal(ctx);
        self.render_new_environment_modal(ctx);
        self.render_bulk_deploy_modal(ctx);
        self.render_github_update_modal(ctx);
        self.render_delete_confirmation_modal(ctx);
        self.render_window_resize_handles(ctx);

        let theme = self.theme();
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme.bg)
                    .inner_margin(Margin::ZERO),
            )
            .show(ctx, |ui| {
                // Background linear gradient: linear-gradient(180deg, #FFFFFF 0%, #F5F7FF 100%)
                let c_rect = ui.max_rect();
                let mut mesh = egui::Mesh::default();
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: c_rect.left_top(),
                    uv: Pos2::ZERO,
                    color: theme.bg_gradient_top,
                });
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: c_rect.right_top(),
                    uv: Pos2::ZERO,
                    color: theme.bg_gradient_top,
                });
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: c_rect.right_bottom(),
                    uv: Pos2::ZERO,
                    color: theme.bg_gradient_bottom,
                });
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: c_rect.left_bottom(),
                    uv: Pos2::ZERO,
                    color: theme.bg_gradient_bottom,
                });
                mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
                ui.painter().add(egui::Shape::mesh(mesh));

                ScrollArea::vertical()
                    .id_salt("main_workspace_vertical_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let avail_w = ui.available_width();
                        let page_pad = (avail_w * 0.03).clamp(16.0, 32.0);
                        let content_w = avail_w.min(1200.0) - (page_pad * 2.0);
                        let margin_x = ((avail_w - content_w) / 2.0).max(page_pad) as i8;
                        let pad_i = page_pad as i8;

                        egui::Frame::new()
                            .inner_margin(Margin {
                                left: margin_x,
                                right: margin_x,
                                top: pad_i,
                                bottom: pad_i,
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
