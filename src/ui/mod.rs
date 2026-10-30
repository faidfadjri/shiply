pub mod theme;
pub mod types;
pub mod icons;
pub mod helpers;
pub mod components;
pub mod modals;
pub mod topbar;
pub mod sidebar;
pub mod views;
pub mod app;

#[cfg(test)]
mod tests;

pub use app::ShiplyApp;
pub use theme::*;
pub use types::*;
pub(crate) use icons::*;
pub(crate) use helpers::*;
pub(crate) use components::*;
#[cfg(test)]
pub(crate) use modals::*;

// Common imports used across UI submodules
use crate::credentials::read_os_vault_credential;
use crate::domain::{
    format_env_badge, DeploymentRecord, DeploymentStatus, EnvironmentConfig, LocalShell, PipelineStep, Project,
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
