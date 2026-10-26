use crate::domain::{DeploymentRecord, DeploymentStatus, Project};
use crate::presets::default_projects;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStorage {
    pub projects: Vec<Project>,
    pub deployments: Vec<DeploymentRecord>,
    #[serde(skip)]
    storage_path: Option<PathBuf>,
}

impl Default for AppStorage {
    fn default() -> Self {
        Self {
            projects: default_projects(),
            deployments: Vec::new(),
            storage_path: None,
        }
    }
}

impl AppStorage {
    pub fn default_storage_path() -> PathBuf {
        if let Some(proj_dirs) = dirs::data_local_dir() {
            proj_dirs.join("shiply").join("shiply_state.json")
        } else {
            PathBuf::from(".shiply_state.json")
        }
    }

    pub fn load_or_default(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        if let Ok(raw) = fs::read_to_string(&path) {
            if let Ok(mut parsed) = serde_json::from_str::<AppStorage>(&raw) {
                if parsed.projects.is_empty() {
                    parsed.projects = default_projects();
                }
                parsed.storage_path = Some(path);
                return parsed;
            }
        }

        let state = Self {
            storage_path: Some(path),
            ..Self::default()
        };
        let _ = state.save();
        state
    }

    pub fn save(&self) -> Result<(), String> {
        let Some(path) = &self.storage_path else {
            return Ok(());
        };
        self.save_to(path)
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
    fn test_storage_persistence_and_queries() {
        let tmp = TempDir::new().expect("tempdir");
        let file_path = tmp.path().join("shiply_state.json");

        let mut storage = AppStorage::load_or_default(&file_path);
        assert_eq!(storage.projects.len(), 2);

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
            DeploymentStatus::Failed,
        ));

        let last_ok = storage
            .last_succeeded_deployment("ndm-dq-subscription", "dev")
            .expect("last succeeded");
        assert_eq!(last_ok.version, "1.0.0");

        let latest = storage
            .latest_deployment("ndm-dq-subscription", "dev")
            .expect("latest");
        assert_eq!(latest.version, "1.0.1");

        // Reload from disk and verify history is still there
        let reloaded = AppStorage::load_or_default(&file_path);
        assert_eq!(reloaded.deployments.len(), 2);
        assert_eq!(reloaded.deployments[0].id, "dep-2");
        assert_eq!(reloaded.deployments[1].id, "dep-1");
    }
}
