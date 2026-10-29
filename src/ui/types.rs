use eframe::egui::Pos2;
use crate::version::GitMetadata;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SidebarSelection {
    AllDeployments,
    Project(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkspaceTab {
    PipelineSteps,
    HistoryAndLogs,
    ServerSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CredentialOsTab {
    Windows,
    Linux,
    Mac,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConfirmDeleteAction {
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
