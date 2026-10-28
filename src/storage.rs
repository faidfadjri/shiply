use crate::domain::{DeploymentRecord, DeploymentStatus, Project};
use crate::ps1_importer::import_project_from_path;
use crate::version::{bump_semver, VersionBump};
use rusqlite::{params, Connection};
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
    #[serde(skip)]
    pub migration_notice: Option<String>,
}

#[derive(Deserialize)]
struct LegacyJsonState {
    #[serde(default)]
    projects: Vec<Project>,
    #[serde(default)]
    deployments: Vec<DeploymentRecord>,
}

/// SQLite database initialization and schema creation.
pub fn init_db(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;

         CREATE TABLE IF NOT EXISTS metadata (
             key TEXT PRIMARY KEY,
             value TEXT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS projects (
             id TEXT PRIMARY KEY,
             name TEXT NOT NULL,
             description TEXT NOT NULL,
             local_path TEXT NOT NULL,
             environments_json TEXT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS deployments (
             id TEXT PRIMARY KEY,
             project_id TEXT NOT NULL,
             project_name TEXT NOT NULL,
             environment TEXT NOT NULL,
             app_name TEXT NOT NULL,
             version TEXT NOT NULL,
             git_branch TEXT,
             git_commit TEXT,
             git_changes TEXT NOT NULL,
             is_rollback INTEGER NOT NULL,
             servers TEXT NOT NULL,
             status TEXT NOT NULL,
             started_at TEXT NOT NULL,
             finished_at TEXT,
             duration_ms INTEGER,
             step_logs TEXT NOT NULL,
             executed_steps TEXT NOT NULL
         );

         CREATE INDEX IF NOT EXISTS idx_deployments_project ON deployments (project_id);
         CREATE INDEX IF NOT EXISTS idx_deployments_env ON deployments (project_id, environment);
         CREATE INDEX IF NOT EXISTS idx_deployments_started_at ON deployments (started_at DESC);",
    )?;
    Ok(())
}

pub fn load_projects_from_db(conn: &Connection) -> Result<Vec<Project>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, local_path, environments_json FROM projects ORDER BY name ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let name: String = row.get(1)?;
        let description: String = row.get(2)?;
        let local_path: String = row.get(3)?;
        let environments_json: String = row.get(4)?;
        let environments = serde_json::from_str(&environments_json).unwrap_or_default();
        Ok(Project {
            id,
            name,
            description,
            local_path,
            environments,
        })
    })?;

    let mut projects = Vec::new();
    for p in rows {
        projects.push(p?);
    }
    Ok(projects)
}

pub fn load_deployments_from_db(
    conn: &Connection,
) -> Result<Vec<DeploymentRecord>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, project_name, environment, app_name, version, git_branch, git_commit, git_changes, is_rollback, servers, status, started_at, finished_at, duration_ms, step_logs, executed_steps
         FROM deployments
         ORDER BY started_at DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let project_id: String = row.get(1)?;
        let project_name: String = row.get(2)?;
        let environment: String = row.get(3)?;
        let app_name: String = row.get(4)?;
        let version: String = row.get(5)?;
        let git_branch: Option<String> = row.get(6)?;
        let git_commit: Option<String> = row.get(7)?;
        let git_changes_json: String = row.get(8)?;
        let is_rollback_int: i64 = row.get(9)?;
        let servers_json: String = row.get(10)?;
        let status_str: String = row.get(11)?;
        let started_at_str: String = row.get(12)?;
        let finished_at_str: Option<String> = row.get(13)?;
        let duration_ms_int: Option<i64> = row.get(14)?;
        let step_logs_json: String = row.get(15)?;
        let executed_steps_json: String = row.get(16)?;

        let git_changes = serde_json::from_str(&git_changes_json).unwrap_or_default();
        let servers = serde_json::from_str(&servers_json).unwrap_or_default();
        let step_logs = serde_json::from_str(&step_logs_json).unwrap_or_default();
        let executed_steps = serde_json::from_str(&executed_steps_json).unwrap_or_default();
        let is_rollback = is_rollback_int != 0;
        let status = match status_str.as_str() {
            "Running" => DeploymentStatus::Running,
            "Succeeded" => DeploymentStatus::Succeeded,
            "Cancelled" => DeploymentStatus::Cancelled,
            _ => DeploymentStatus::Failed,
        };
        let started_at = chrono::DateTime::parse_from_rfc3339(&started_at_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now());
        let finished_at = finished_at_str.and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(&s)
                .ok()
                .map(|dt| dt.with_timezone(&chrono::Utc))
        });
        let duration_ms = duration_ms_int.map(|v| v as u64);

        Ok(DeploymentRecord {
            id,
            project_id,
            project_name,
            environment,
            app_name,
            version,
            git_branch,
            git_commit,
            git_changes,
            is_rollback,
            servers,
            status,
            started_at,
            finished_at,
            duration_ms,
            step_logs,
            executed_steps,
        })
    })?;

    let mut deployments = Vec::new();
    for d in rows {
        deployments.push(d?);
    }
    Ok(deployments)
}

pub fn save_all_to_db(
    conn: &mut Connection,
    projects: &[Project],
    deployments: &[DeploymentRecord],
) -> Result<(), rusqlite::Error> {
    init_db(conn)?;
    let tx = conn.transaction()?;

    tx.execute("DELETE FROM projects", [])?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO projects (id, name, description, local_path, environments_json) VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for proj in projects {
            let envs_json = serde_json::to_string(&proj.environments).unwrap_or_default();
            stmt.execute(params![
                proj.id,
                proj.name,
                proj.description,
                proj.local_path,
                envs_json
            ])?;
        }
    }

    tx.execute("DELETE FROM deployments", [])?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO deployments (id, project_id, project_name, environment, app_name, version, git_branch, git_commit, git_changes, is_rollback, servers, status, started_at, finished_at, duration_ms, step_logs, executed_steps)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        )?;
        for dep in deployments {
            let git_changes_json = serde_json::to_string(&dep.git_changes).unwrap_or_default();
            let servers_json = serde_json::to_string(&dep.servers).unwrap_or_default();
            let step_logs_json = serde_json::to_string(&dep.step_logs).unwrap_or_default();
            let executed_steps_json =
                serde_json::to_string(&dep.executed_steps).unwrap_or_default();
            let status_str = match dep.status {
                DeploymentStatus::Running => "Running",
                DeploymentStatus::Succeeded => "Succeeded",
                DeploymentStatus::Cancelled => "Cancelled",
                DeploymentStatus::Failed => "Failed",
            };
            let started_at_str = dep.started_at.to_rfc3339();
            let finished_at_str = dep.finished_at.as_ref().map(|dt| dt.to_rfc3339());
            let is_rollback_int = if dep.is_rollback { 1i64 } else { 0i64 };
            let duration_ms_int = dep.duration_ms.map(|d| d as i64);

            stmt.execute(params![
                dep.id,
                dep.project_id,
                dep.project_name,
                dep.environment,
                dep.app_name,
                dep.version,
                dep.git_branch,
                dep.git_commit,
                git_changes_json,
                is_rollback_int,
                servers_json,
                status_str,
                started_at_str,
                finished_at_str,
                duration_ms_int,
                step_logs_json,
                executed_steps_json,
            ])?;
        }
    }

    tx.commit()?;
    Ok(())
}

pub fn upsert_deployment_to_db(
    conn: &Connection,
    dep: &DeploymentRecord,
) -> Result<(), rusqlite::Error> {
    init_db(conn)?;
    let git_changes_json = serde_json::to_string(&dep.git_changes).unwrap_or_default();
    let servers_json = serde_json::to_string(&dep.servers).unwrap_or_default();
    let step_logs_json = serde_json::to_string(&dep.step_logs).unwrap_or_default();
    let executed_steps_json = serde_json::to_string(&dep.executed_steps).unwrap_or_default();
    let status_str = match dep.status {
        DeploymentStatus::Running => "Running",
        DeploymentStatus::Succeeded => "Succeeded",
        DeploymentStatus::Cancelled => "Cancelled",
        DeploymentStatus::Failed => "Failed",
    };
    let started_at_str = dep.started_at.to_rfc3339();
    let finished_at_str = dep.finished_at.as_ref().map(|dt| dt.to_rfc3339());
    let is_rollback_int = if dep.is_rollback { 1i64 } else { 0i64 };
    let duration_ms_int = dep.duration_ms.map(|d| d as i64);

    conn.execute(
        "INSERT OR REPLACE INTO deployments (id, project_id, project_name, environment, app_name, version, git_branch, git_commit, git_changes, is_rollback, servers, status, started_at, finished_at, duration_ms, step_logs, executed_steps)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        params![
            dep.id,
            dep.project_id,
            dep.project_name,
            dep.environment,
            dep.app_name,
            dep.version,
            dep.git_branch,
            dep.git_commit,
            git_changes_json,
            is_rollback_int,
            servers_json,
            status_str,
            started_at_str,
            finished_at_str,
            duration_ms_int,
            step_logs_json,
            executed_steps_json,
        ],
    )?;
    Ok(())
}

impl AppStorage {
    /// Resolves `shiply_state.db` directly inside the application executable's directory
    /// so users can easily manage `shiply_state.db` (or drop `.json` / `.ps1` files)
    /// into the app folder and have Shiply automatically recognize all projects and history.
    pub fn default_storage_path() -> PathBuf {
        if let Ok(exe_path) = env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let candidate_db = exe_dir.join("shiply_state.db");
                let candidate_json = exe_dir.join("shiply_state.json");
                if candidate_db.exists() || candidate_json.exists() || is_dir_writable(exe_dir) {
                    return candidate_db;
                }
            }
        }
        if let Some(proj_dirs) = dirs::data_local_dir() {
            proj_dirs.join("shiply").join("shiply_state.db")
        } else {
            PathBuf::from("shiply_state.db")
        }
    }

    /// Returns the directory where `shiply_state.db` (and any drop-in source files) live.
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

    /// Loads state from SQLite at `path` if it exists. If the database does not exist or is empty,
    /// seamlessly detects and migrates legacy `shiply_state.json` without any data loss.
    /// Also scans the app directory for any pasted `.json` or `.ps1` files.
    pub fn load_or_default(path: impl Into<PathBuf>) -> Self {
        let input_path = path.into();
        let is_json_ext = input_path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("json"));
        let db_path = if is_json_ext {
            input_path.with_extension("db")
        } else {
            input_path.clone()
        };

        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = fs::create_dir_all(parent);
            }
        }

        let mut state = AppStorage {
            projects: Vec::new(),
            deployments: Vec::new(),
            storage_path: Some(db_path.clone()),
            last_state_mtime: None,
            imported_files_mtime: HashMap::new(),
            migration_notice: None,
        };

        let mut needs_save = false;

        if let Ok(mut conn) = Connection::open(&db_path) {
            let _ = init_db(&conn);
            let mut db_projects = load_projects_from_db(&conn).unwrap_or_default();
            let mut db_deployments = load_deployments_from_db(&conn).unwrap_or_default();

            // If SQLite is completely empty, check for legacy JSON state file to migrate seamlessly
            if db_projects.is_empty() && db_deployments.is_empty() {
                let mut legacy_candidates = Vec::new();
                if is_json_ext {
                    legacy_candidates.push(input_path.clone());
                } else {
                    legacy_candidates.push(input_path.with_extension("json"));
                }
                if let Some(p) = db_path.parent() {
                    let parent_json = p.join("shiply_state.json");
                    if !legacy_candidates.contains(&parent_json) {
                        legacy_candidates.push(parent_json);
                    }
                }
                // If using the default storage path, also check default global locations
                let is_default = db_path == Self::default_storage_path();
                if is_default {
                    if let Ok(exe) = env::current_exe() {
                        if let Some(exe_dir) = exe.parent() {
                            let exe_json = exe_dir.join("shiply_state.json");
                            if !legacy_candidates.contains(&exe_json) {
                                legacy_candidates.push(exe_json);
                            }
                        }
                    }
                    if let Some(proj_dirs) = dirs::data_local_dir() {
                        let data_json = proj_dirs.join("shiply").join("shiply_state.json");
                        if !legacy_candidates.contains(&data_json) {
                            legacy_candidates.push(data_json);
                        }
                    }
                }

                for candidate in legacy_candidates {
                    if candidate.exists() {
                        if let Ok(raw) = fs::read_to_string(&candidate) {
                            if let Ok(parsed) = serde_json::from_str::<LegacyJsonState>(&raw) {
                                if !parsed.projects.is_empty() || !parsed.deployments.is_empty() {
                                    db_projects = parsed.projects;
                                    db_deployments = parsed.deployments;
                                    needs_save = true;

                                    // Preserve backup file so user data is never lost
                                    let backup_path = candidate.with_extension("json.bak");
                                    let _ = fs::copy(&candidate, &backup_path);

                                    state.migration_notice = Some(format!(
                                        "Seamlessly migrated {} project(s) and {} deployment(s) from '{}' to SQLite database.",
                                        db_projects.len(),
                                        db_deployments.len(),
                                        candidate
                                            .file_name()
                                            .and_then(|s| s.to_str())
                                            .unwrap_or("shiply_state.json")
                                    ));
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            state.projects = db_projects;
            state.deployments = db_deployments;

            if needs_save || !db_path.exists() {
                let _ = save_all_to_db(&mut conn, &state.projects, &state.deployments);
            }
        }

        state.last_state_mtime = file_mtime(&db_path);
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
        let db_path = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")) {
            path.with_extension("db")
        } else {
            path.to_path_buf()
        };

        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create storage directory: {e}"))?;
            }
        }

        let mut conn = Connection::open(&db_path)
            .map_err(|e| format!("Failed to open SQLite database {}: {e}", db_path.display()))?;
        save_all_to_db(&mut conn, &self.projects, &self.deployments)
            .map_err(|e| format!("Failed to save state to SQLite database: {e}"))?;

        // If path was originally requested as JSON (e.g. drop-in export), also write JSON mirror
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")) {
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = fs::write(path, json);
            }
        }

        Ok(())
    }

    /// Synchronizes state with the app directory:
    /// 1. If `shiply_state.db` was updated externally, reloads projects & history.
    /// 2. If a `.json` file (containing `AppStorage`, `Project`, or `Vec<Project>`) or a `deploy*.ps1`
    ///    script was pasted into the app directory, merges it into `self` automatically and persists to SQLite.
    pub fn sync_from_app_directory(&mut self) -> Option<String> {
        let state_path = self.storage_path.clone()?;
        let mut notifications = Vec::new();

        if let Some(notice) = self.migration_notice.take() {
            notifications.push(notice);
        }

        // 1. Check if primary `shiply_state.db` was updated externally on disk
        if let Some(disk_mtime) = file_mtime(&state_path) {
            let is_changed = self
                .last_state_mtime
                .is_none_or(|prev| disk_mtime > prev);
            if is_changed {
                if let Ok(conn) = Connection::open(&state_path) {
                    if let (Ok(p), Ok(d)) = (
                        load_projects_from_db(&conn),
                        load_deployments_from_db(&conn),
                    ) {
                        self.projects = p;
                        self.deployments = d;
                        self.last_state_mtime = Some(disk_mtime);
                        notifications.push(format!(
                            "Loaded {} project(s) and {} deployment(s) from SQLite database ({})",
                            self.projects.len(),
                            self.deployments.len(),
                            state_path
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("shiply_state.db")
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
                // Ignore backup files
                if file_path
                    .to_str()
                    .is_some_and(|s| s.ends_with(".bak") || s.ends_with(".tmp"))
                {
                    continue;
                }
                if let Ok(raw) = fs::read_to_string(&file_path) {
                    if let Ok(single_proj) = serde_json::from_str::<Project>(&raw) {
                        if self.merge_projects(vec![single_proj]) > 0 {
                            merged_any = true;
                            notifications.push(format!(
                                "Auto-imported project from '{}'",
                                file_path
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("project.json")
                            ));
                        }
                    } else if let Ok(proj_list) = serde_json::from_str::<Vec<Project>>(&raw) {
                        let added_p = self.merge_projects(proj_list);
                        if added_p > 0 {
                            merged_any = true;
                            notifications.push(format!(
                                "Auto-imported {added_p} project(s) from '{}'",
                                file_path
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("projects.json")
                            ));
                        }
                    } else if let Ok(ext_storage) = serde_json::from_str::<LegacyJsonState>(&raw) {
                        if !ext_storage.projects.is_empty() || !ext_storage.deployments.is_empty() {
                            let added_p = self.merge_projects(ext_storage.projects);
                            let added_d = self.merge_deployments(ext_storage.deployments);
                            if added_p > 0 || added_d > 0 {
                                merged_any = true;
                                notifications.push(format!(
                                    "Auto-imported {added_p} project(s), {added_d} deployment(s) from '{}'",
                                    file_path
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("file.json")
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
                                file_path
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("deploy.ps1")
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

    /// Duplicates an existing project, including its environments and pipeline steps,
    /// giving the duplicate a unique display name and project ID, and immediately persists to SQLite.
    pub fn duplicate_project(&mut self, project_id: &str) -> Option<Project> {
        let orig = self.projects.iter().find(|p| p.id == project_id)?.clone();

        let base_name = &orig.name;
        let mut new_name = format!("{base_name} (Copy)");
        let mut counter = 2;
        while self.projects.iter().any(|p| p.name == new_name) {
            new_name = format!("{base_name} (Copy {counter})");
            counter += 1;
        }

        let mut new_id = format!("{}-copy", orig.id);
        let mut id_counter = 2;
        while self.projects.iter().any(|p| p.id == new_id) {
            new_id = format!("{}-copy-{}", orig.id, id_counter);
            id_counter += 1;
        }

        let new_project = orig.create_duplicate(new_id, new_name);
        self.projects.push(new_project.clone());
        let _ = self.save();
        Some(new_project)
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

    /// Inserts a new deployment record at the top (newest-first) or updates an existing record with the same ID,
    /// atomically persisting directly to SQLite.
    pub fn upsert_deployment(&mut self, record: DeploymentRecord) {
        if let Some(existing) = self.deployments.iter_mut().find(|d| d.id == record.id) {
            *existing = record.clone();
        } else {
            self.deployments.insert(0, record.clone());
        }

        if let Some(path) = &self.storage_path {
            if let Ok(conn) = Connection::open(path) {
                let _ = upsert_deployment_to_db(&conn, &record);
                self.last_state_mtime = file_mtime(path);
            }
        }
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

    /// Returns the environment filter tabs `(display_name, Some(env_key))` representing
    /// the existing environments for either a specific project or across all projects.
    pub fn existing_environment_tabs(
        &self,
        project_filter: Option<&str>,
    ) -> Vec<(String, Option<String>)> {
        let mut tabs = vec![("All Environments".to_string(), None)];
        if let Some(pid) = project_filter {
            if let Some(proj) = self.projects.iter().find(|p| p.id == pid) {
                for env in &proj.environments {
                    if !tabs
                        .iter()
                        .any(|(_, k)| k.as_deref() == Some(env.key.as_str()))
                    {
                        tabs.push((env.name.clone(), Some(env.key.clone())));
                    }
                }
            }
        } else {
            for proj in &self.projects {
                for env in &proj.environments {
                    if !tabs
                        .iter()
                        .any(|(_, k)| k.as_deref() == Some(env.key.as_str()))
                    {
                        tabs.push((env.name.clone(), Some(env.key.clone())));
                    }
                }
            }
        }
        tabs
    }

    /// Computes the next auto-bumped version for the Version input field based on the
    /// latest deployment history for `(project_id, env_key)` (or any environment in `project_id`),
    /// falling back to `package.json` or `"1.0.0"` if no deployment history exists yet.
    pub fn next_auto_bumped_version(
        &self,
        project_id: &str,
        env_key: &str,
        fallback_pkg_version: Option<&str>,
    ) -> String {
        if let Some(dep) = self
            .latest_deployment(project_id, env_key)
            .or_else(|| self.last_succeeded_deployment(project_id, env_key))
        {
            return bump_semver(&dep.version, VersionBump::Patch);
        }

        if let Some(dep) = self.deployments.iter().find(|d| d.project_id == project_id) {
            return bump_semver(&dep.version, VersionBump::Patch);
        }

        if let Some(pkg_ver) = fallback_pkg_version {
            let trimmed = pkg_ver.trim();
            if !trimmed.is_empty() && trimmed != "0.0.0" {
                return trimmed.to_string();
            }
        }

        "1.0.0".to_string()
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
        let file_path = tmp.path().join("shiply_state.db");

        let storage = AppStorage::load_or_default(&file_path);
        assert!(storage.projects.is_empty());
        assert!(storage.deployments.is_empty());
        assert!(file_path.exists());
    }

    #[test]
    fn test_seamless_migration_from_legacy_json_to_sqlite() {
        let tmp = TempDir::new().expect("tempdir");
        let json_path = tmp.path().join("shiply_state.json");
        let db_path = tmp.path().join("shiply_state.db");

        // 1. Create a legacy shiply_state.json with projects and deployments
        let sample_proj = Project {
            id: "legacy-proj".to_string(),
            name: "Legacy Project".to_string(),
            description: "From JSON".to_string(),
            local_path: r"C:\Legacy\Proj".to_string(),
            environments: Vec::new(),
        };
        let sample_dep = dummy_record(
            "dep-legacy-1",
            "legacy-proj",
            "prod",
            "1.2.3",
            DeploymentStatus::Succeeded,
        );

        let legacy_data = serde_json::json!({
            "projects": [sample_proj],
            "deployments": [sample_dep]
        });
        fs::write(&json_path, serde_json::to_string_pretty(&legacy_data).unwrap()).unwrap();

        // 2. Call load_or_default with the SQLite db path
        let storage = AppStorage::load_or_default(&db_path);

        // 3. Verify SQLite was created and populated seamlessly
        assert!(db_path.exists());
        assert_eq!(storage.projects.len(), 1);
        assert_eq!(storage.projects[0].id, "legacy-proj");
        assert_eq!(storage.projects[0].name, "Legacy Project");
        assert_eq!(storage.deployments.len(), 1);
        assert_eq!(storage.deployments[0].id, "dep-legacy-1");
        assert_eq!(storage.deployments[0].version, "1.2.3");

        // 4. Verify backup was preserved
        assert!(tmp.path().join("shiply_state.json.bak").exists());

        // 5. Subsequent load reads directly from SQLite
        let reloaded = AppStorage::load_or_default(&db_path);
        assert_eq!(reloaded.projects.len(), 1);
        assert_eq!(reloaded.deployments.len(), 1);
    }

    #[test]
    fn test_storage_persistence_and_auto_recognize_pasted_files() {
        let tmp = TempDir::new().expect("tempdir");
        let file_path = tmp.path().join("shiply_state.db");

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

    #[test]
    fn test_existing_environment_tabs_and_auto_bumped_version() {
        let mut storage = AppStorage::default();
        let mut proj = Project {
            id: "web-api".to_string(),
            name: "web-api".to_string(),
            description: "API Service".to_string(),
            local_path: r"C:\Projects\web-api".to_string(),
            environments: Vec::new(),
        };
        proj.add_environment("Development", None).unwrap();
        proj.add_environment("QA", None).unwrap();
        storage.projects.push(proj);

        // Tabs on All Deployments page (project_filter = None) reflect actual existing environments
        let all_tabs = storage.existing_environment_tabs(None);
        assert_eq!(
            all_tabs,
            vec![
                ("All Environments".to_string(), None),
                ("Development".to_string(), Some("development".to_string())),
                ("QA".to_string(), Some("qa".to_string())),
            ]
        );

        // Without history, falls back to package.json or 1.0.0
        assert_eq!(
            storage.next_auto_bumped_version("web-api", "development", Some("1.4.0")),
            "1.4.0"
        );
        assert_eq!(
            storage.next_auto_bumped_version("web-api", "development", None),
            "1.0.0"
        );

        // With deployment history, auto-bumps the latest deployment version
        storage.upsert_deployment(dummy_record(
            "d1",
            "web-api",
            "development",
            "1.4.2",
            DeploymentStatus::Succeeded,
        ));
        assert_eq!(
            storage.next_auto_bumped_version("web-api", "development", Some("1.4.2")),
            "1.4.3"
        );
    }

    #[test]
    fn test_storage_duplicate_project() {
        let tmp = TempDir::new().expect("tempdir");
        let db_path = tmp.path().join("shiply_state.db");
        let mut storage = AppStorage::load_or_default(&db_path);

        let proj = Project {
            id: "api-service".to_string(),
            name: "API Service".to_string(),
            description: "Main backend".to_string(),
            local_path: r"C:\apps\api".to_string(),
            environments: vec![],
        };
        storage.projects.push(proj);
        let _ = storage.save();

        // 1. First duplication
        let dup1 = storage.duplicate_project("api-service").expect("duplicate 1");
        assert_eq!(dup1.id, "api-service-copy");
        assert_eq!(dup1.name, "API Service (Copy)");
        assert_eq!(storage.projects.len(), 2);

        // 2. Second duplication of original
        let dup2 = storage.duplicate_project("api-service").expect("duplicate 2");
        assert_eq!(dup2.id, "api-service-copy-2");
        assert_eq!(dup2.name, "API Service (Copy 2)");
        assert_eq!(storage.projects.len(), 3);

        // 3. Reload from SQLite and verify persistence
        let reloaded = AppStorage::load_or_default(&db_path);
        assert_eq!(reloaded.projects.len(), 3);
        assert!(reloaded.projects.iter().any(|p| p.id == "api-service-copy"));
        assert!(reloaded.projects.iter().any(|p| p.id == "api-service-copy-2"));
    }
}
