use crate::domain::{DeploymentRecord, DeploymentStatus, Project};
use crate::ps1_importer::import_project_from_path;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppStorage {
    #[serde(default)]
    pub projects: Vec<Project>,
    #[serde(default)]
    pub deployments: Vec<DeploymentRecord>,
    #[serde(skip)]
    storage_path: Option<PathBuf>,
    #[serde(skip)]
    last_state_mtime: Option<SystemTime>,
    #[serde(skip)]
    imported_files_mtime: HashMap<PathBuf, SystemTime>,
}

impl AppStorage {
    /// Resolves `shiply_state.json` directly inside the application executable's directory
    /// so users can easily paste/replace `shiply_state.json` (or drop `.json` / `.ps1` files)
    /// into the app folder and have Shiply automatically recognize all projects and history.
    pub fn default_storage_path() -> PathBuf {
        if let Ok(exe_path) = env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let candidate = exe_dir.join("shiply_state.json");
                if candidate.exists() || is_dir_writable(exe_dir) {
                    return candidate;
                }
            }
        }
        if let Some(proj_dirs) = dirs::data_local_dir() {
            proj_dirs.join("shiply").join("shiply_state.json")
        } else {
            PathBuf::from("shiply_state.json")
        }
    }

    /// Returns the directory where `shiply_state.json` (and any drop-in source files) live.
    pub fn app_data_dir(&self) -> PathBuf {
        self.storage_path
            .as_ref()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| {
                Self::default_storage_path()
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("."))
            })
    }

    pub fn storage_file_path(&self) -> Option<&Path> {
        self.storage_path.as_deref()
    }

    pub fn reset_sync_cache(&mut self) {
        self.last_state_mtime = None;
        self.imported_files_mtime.clear();
    }

    /// Loads state from `path` if it exists, or initializes an empty state (0 projects, 0 environments)
    /// on first-time installation. Also scans the app directory for any pasted `.json` or `.ps1` files.
    pub fn load_or_default(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let mut state = if let Ok(raw) = fs::read_to_string(&path) {
            serde_json::from_str::<AppStorage>(&raw).unwrap_or_default()
        } else {
            AppStorage::default()
        };

        state.storage_path = Some(path.clone());
        if !path.exists() {
            let _ = state.save();
        }
        state.last_state_mtime = file_mtime(&path);
        let _ = state.sync_from_app_directory();
        state
    }

    pub fn save(&mut self) -> Result<(), String> {
        let Some(path) = self.storage_path.clone() else {
            return Ok(());
        };
        self.save_to(&path)?;
        self.last_state_mtime = file_mtime(&path);
        Ok(())
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create storage directory: {e}"))?;
            }
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize state: {e}"))?;
        let tmp_path = path.with_extension("json.tmp");
        fs::write(&tmp_path, json)
            .map_err(|e| format!("Failed to write temporary state file: {e}"))?;
        fs::rename(&tmp_path, path)
            .or_else(|_| fs::copy(&tmp_path, path).map(|_| ()))
            .map_err(|e| format!("Failed to finalize state file: {e}"))?;
        let _ = fs::remove_file(&tmp_path);
        Ok(())
    }

    /// Synchronizes state with the app directory:
    /// 1. If `shiply_state.json` was replaced or edited externally, reloads projects & history.
    /// 2. If another `.json` file (containing `AppStorage`, `Project`, or `Vec<Project>`) or a `deploy*.ps1`
    ///    script was pasted into the app directory, merges it into `self` automatically.
    pub fn sync_from_app_directory(&mut self) -> Option<String> {
        let state_path = self.storage_path.clone()?;
        let mut notifications = Vec::new();

        // 1. Check if primary `shiply_state.json` was updated externally on disk
        if let Some(disk_mtime) = file_mtime(&state_path) {
            let is_changed = self
                .last_state_mtime
                .is_none_or(|prev| disk_mtime > prev);
            if is_changed {
                if let Ok(raw) = fs::read_to_string(&state_path) {
                    if let Ok(parsed) = serde_json::from_str::<AppStorage>(&raw) {
                        self.projects = parsed.projects;
                        self.deployments = parsed.deployments;
                        self.last_state_mtime = Some(disk_mtime);
                        notifications.push(format!(
                            "Loaded {} project(s) and {} deployment(s) from {}",
                            self.projects.len(),
                            self.deployments.len(),
                            state_path
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("shiply_state.json")
                        ));
                    }
                }
            }
        }

        // 2. Scan the app directory for any drop-in `.json` or `.ps1` files
        let Some(app_dir) = state_path.parent().map(|p| p.to_path_buf()) else {
            return if notifications.is_empty() {
                None
            } else {
                Some(notifications.join(" | "))
            };
        };

        let Ok(entries) = fs::read_dir(&app_dir) else {
            return if notifications.is_empty() {
                None
            } else {
                Some(notifications.join(" | "))
            };
        };

        let mut merged_any = false;
        for entry in entries.flatten() {
            let file_path = entry.path();
            if !file_path.is_file() || file_path == state_path {
                continue;
            }
            let Some(ext) = file_path.extension().and_then(|s| s.to_str()) else {
                continue;
            };
            let Some(mtime) = file_mtime(&file_path) else {
                continue;
            };
            if self
                .imported_files_mtime
                .get(&file_path)
                .is_some_and(|prev| *prev >= mtime)
            {
                continue;
            }

            if ext.eq_ignore_ascii_case("json") {
                if let Ok(raw) = fs::read_to_string(&file_path) {
                    if let Ok(single_proj) = serde_json::from_str::<Project>(&raw) {
                        if self.merge_projects(vec![single_proj]) > 0 {
                            merged_any = true;
                            notifications.push(format!(
                                "Auto-imported project from '{}'",
                                file_path.file_name().and_then(|s| s.to_str()).unwrap_or("project.json")
                            ));
                        }
                    } else if let Ok(proj_list) = serde_json::from_str::<Vec<Project>>(&raw) {
                        let added_p = self.merge_projects(proj_list);
                        if added_p > 0 {
                            merged_any = true;
                            notifications.push(format!(
                                "Auto-imported {added_p} project(s) from '{}'",
                                file_path.file_name().and_then(|s| s.to_str()).unwrap_or("projects.json")
                            ));
                        }
                    } else if let Ok(ext_storage) = serde_json::from_str::<AppStorage>(&raw) {
                        if !ext_storage.projects.is_empty() || !ext_storage.deployments.is_empty() {
                            let added_p = self.merge_projects(ext_storage.projects);
                            let added_d = self.merge_deployments(ext_storage.deployments);
                            if added_p > 0 || added_d > 0 {
                                merged_any = true;
                                notifications.push(format!(
                                    "Auto-imported {added_p} project(s), {added_d} deployment(s) from '{}'",
                                    file_path.file_name().and_then(|s| s.to_str()).unwrap_or("file.json")
                                ));
                            }
                        }
                    }
                }
                self.imported_files_mtime.insert(file_path, mtime);
            } else if ext.eq_ignore_ascii_case("ps1") {
                let fname = file_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if fname.starts_with("deploy") {
                    if let Ok(imported_proj) = import_project_from_path(&file_path, false) {
                        if self.merge_projects(vec![imported_proj]) > 0 {
                            merged_any = true;
                            notifications.push(format!(
                                "Auto-imported PowerShell script '{}'",
                                file_path.file_name().and_then(|s| s.to_str()).unwrap_or("deploy.ps1")
                            ));
                        }
                    }
                }
                self.imported_files_mtime.insert(file_path, mtime);
            }
        }

        if merged_any {
            let _ = self.save();
        }

        if notifications.is_empty() {
            None
        } else {
            Some(notifications.join(" | "))
        }
    }

    pub fn merge_projects(&mut self, incoming: Vec<Project>) -> usize {
        let mut count = 0;
        for proj in incoming {
            if let Some(existing) = self.projects.iter_mut().find(|p| p.id == proj.id) {
                for env in proj.environments {
                    if !existing.environments.iter().any(|e| e.key == env.key) {
                        existing.environments.push(env);
                        count += 1;
                    }
                }
            } else {
                self.projects.push(proj);
                count += 1;
            }
        }
        count
    }

    pub fn merge_deployments(&mut self, incoming: Vec<DeploymentRecord>) -> usize {
        let mut count = 0;
        for dep in incoming {
            if !self.deployments.iter().any(|d| d.id == dep.id) {
                self.deployments.push(dep);
                count += 1;
            }
        }
        self.deployments
            .sort_by_key(|a| std::cmp::Reverse(a.started_at));
        count
    }

    /// Inserts a new deployment record at the top (newest-first) or updates an existing record with the same ID.
    pub fn upsert_deployment(&mut self, record: DeploymentRecord) {
        if let Some(existing) = self.deployments.iter_mut().find(|d| d.id == record.id) {
            *existing = record;
        } else {
            self.deployments.insert(0, record);
        }
        let _ = self.save();
    }

    /// Returns the most recent successful deployment for a given project and environment key.
    pub fn last_succeeded_deployment(
        &self,
        project_id: &str,
        env_key: &str,
    ) -> Option<&DeploymentRecord> {
        self.deployments.iter().find(|d| {
            d.project_id == project_id
                && d.environment == env_key
                && d.status == DeploymentStatus::Succeeded
        })
    }

    /// Returns the previous successful deployment (prior to the currently active one) for Rollback.
    pub fn previous_rollback_candidate(
        &self,
        project_id: &str,
        env_key: &str,
    ) -> Option<&DeploymentRecord> {
        let mut succeeded_iter = self.deployments.iter().filter(|d| {
            d.project_id == project_id
                && d.environment == env_key
                && d.status == DeploymentStatus::Succeeded
        });
        let _current = succeeded_iter.next()?;
        succeeded_iter.next()
    }

    /// Returns the most recent deployment (regardless of status) for a given project and environment key.
    pub fn latest_deployment(
        &self,
        project_id: &str,
        env_key: &str,
    ) -> Option<&DeploymentRecord> {
        self.deployments
            .iter()
            .find(|d| d.project_id == project_id && d.environment == env_key)
    }

    /// Filters deployments by optional project ID, optional environment key, and optional status.
    pub fn filter_deployments(
        &self,
        project_id: Option<&str>,
        env_filter: Option<&str>,
        status_filter: Option<DeploymentStatus>,
    ) -> Vec<&DeploymentRecord> {
        self.deployments
            .iter()
            .filter(|d| {
                project_id.is_none_or(|pid| d.project_id == pid)
                    && env_filter.is_none_or(|env| d.environment == env)
                    && status_filter.is_none_or(|st| d.status == st)
            })
            .collect()
    }
}

fn file_mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn is_dir_writable(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    let probe = dir.join(".shiply_write_probe.tmp");
    if fs::write(&probe, b"ok").is_ok() {
        let _ = fs::remove_file(&probe);
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tempfile::TempDir;

    fn dummy_record(
        id: &str,
        project_id: &str,
        env_key: &str,
        version: &str,
        status: DeploymentStatus,
    ) -> DeploymentRecord {
        DeploymentRecord {
            id: id.to_string(),
            project_id: project_id.to_string(),
            project_name: project_id.to_string(),
            environment: env_key.to_string(),
            app_name: format!("{project_id}-{env_key}"),
            version: version.to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("a1b2c3d".to_string()),
            git_changes: vec!["a1b2c3d feat: initial release".to_string()],
            is_rollback: false,
            servers: vec!["10.62.70.80".to_string()],
            status,
            started_at: Utc::now(),
            finished_at: Some(Utc::now()),
            duration_ms: Some(1200),
            step_logs: Vec::new(),
            executed_steps: Vec::new(),
        }
    }

    #[test]
    fn test_first_time_install_is_empty_without_project_or_environment() {
        let tmp = TempDir::new().expect("tempdir");
        let file_path = tmp.path().join("shiply_state.json");

        let storage = AppStorage::load_or_default(&file_path);
        assert!(storage.projects.is_empty());
        assert!(storage.deployments.is_empty());
    }

    #[test]
    fn test_storage_persistence_and_auto_recognize_pasted_files() {
        let tmp = TempDir::new().expect("tempdir");
        let file_path = tmp.path().join("shiply_state.json");

        let mut storage = AppStorage::load_or_default(&file_path);
        storage.upsert_deployment(dummy_record(
            "dep-1",
            "ndm-dq-subscription",
            "dev",
            "1.0.0",
            DeploymentStatus::Succeeded,
        ));
        storage.upsert_deployment(dummy_record(
            "dep-2",
            "ndm-dq-subscription",
            "dev",
            "1.0.1",
            DeploymentStatus::Succeeded,
        ));

        let last_ok = storage
            .last_succeeded_deployment("ndm-dq-subscription", "dev")
            .expect("last succeeded");
        assert_eq!(last_ok.version, "1.0.1");

        let rollback_cand = storage
            .previous_rollback_candidate("ndm-dq-subscription", "dev")
            .expect("rollback candidate");
        assert_eq!(rollback_cand.version, "1.0.0");

        // Simulate user pasting a project JSON file into the app directory
        let drop_json = tmp.path().join("my_project.json");
        let sample_proj = Project {
            id: "ndm-dq-subscription".to_string(),
            name: "ndm-dq-subscription".to_string(),
            description: "Backend Service".to_string(),
            local_path: r"D:\NTI\Projects\backend\ndm-dq-subscription".to_string(),
            environments: Vec::new(),
        };
        fs::write(
            &drop_json,
            serde_json::to_string_pretty(&sample_proj).unwrap(),
        )
        .expect("write drop_json");

        let notice = storage.sync_from_app_directory();
        assert!(notice.is_some());
        assert_eq!(storage.projects.len(), 1);
        assert_eq!(storage.projects[0].id, "ndm-dq-subscription");
    }
}
