use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Normalizes an environment name or key into a lowercase slug (e.g. `"Pre-Prod"` -> `"pre-prod"`).
pub fn normalize_env_key(input: &str) -> String {
    let raw = input
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>();
    raw.split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Formats a short uppercase badge label for any environment key or name.
pub fn format_env_badge(env_key: &str) -> String {
    match env_key.trim().to_lowercase().as_str() {
        "dev" | "development" => "DEV".to_string(),
        "staging" | "stage" | "stg" => "STAGING".to_string(),
        "prod" | "production" => "PROD".to_string(),
        other => other.to_uppercase(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StepKind {
    /// Execute a command locally inside the project directory (e.g., `npm run test`, `npm run build`).
    LocalCommand {
        command: String,
        allow_failure: bool,
    },
    /// Verify that a local file or directory exists before continuing (e.g., `./dist/server.js`).
    VerifyLocalPath {
        path: String,
    },
    /// Transfer local paths (and optional staged `.env` file) via SCP to `{remote_dir}` on each target server.
    TransferFiles {
        sources: Vec<String>,
        stage_env_file: bool,
    },
    /// Execute a shell command over SSH on each target server.
    /// Supports placeholders: `{remote_dir}`, `{remote_user}`, `{app_name}`, `{http_proxy}`, `{version}`.
    RemoteCommand {
        command: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineStep {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub kind: StepKind,
}

impl PipelineStep {
    pub fn new(name: impl Into<String>, kind: StepKind) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            enabled: true,
            kind,
        }
    }

    pub fn summary(&self) -> String {
        match &self.kind {
            StepKind::LocalCommand {
                command,
                allow_failure,
            } => {
                if *allow_failure {
                    format!("[Local - non-fatal] {command}")
                } else {
                    format!("[Local] {command}")
                }
            }
            StepKind::VerifyLocalPath { path } => format!("[Verify Path] {path}"),
            StepKind::TransferFiles {
                sources,
                stage_env_file,
            } => {
                let mut list = sources.join(", ");
                if *stage_env_file {
                    if !list.is_empty() {
                        list.push_str(", ");
                    }
                    list.push_str("{env_source} -> .env");
                }
                format!("[SCP Upload] {list} -> {{remote_dir}}")
            }
            StepKind::RemoteCommand { command } => format!("[Remote SSH] {command}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentConfig {
    /// Unique key within a project (e.g. `"dev"`, `"staging"`, `"production"`, `"uat"`).
    pub key: String,
    /// Human-readable display name (e.g. `"Development"`, `"Staging"`, `"Production"`, `"UAT"`).
    pub name: String,
    pub app_name: String,
    pub servers: Vec<String>,
    pub remote_user: String,
    pub vault_target: String,
    pub remote_dir: String,
    pub env_source: Option<String>,
    pub http_proxy: Option<String>,
    pub steps: Vec<PipelineStep>,
}

impl EnvironmentConfig {
    pub fn badge_label(&self) -> String {
        format_env_badge(&self.key)
    }

    /// Interpolates placeholders `{remote_dir}`, `{remote_user}`, `{app_name}`, `{http_proxy}`, `{env_source}`, `{version}`
    /// in any command or path string.
    pub fn interpolate(&self, template: &str, version: &str) -> String {
        let proxy = self.http_proxy.as_deref().unwrap_or("");
        let env_src = self.env_source.as_deref().unwrap_or("");
        template
            .replace("{remote_dir}", &self.remote_dir)
            .replace("{remote_user}", &self.remote_user)
            .replace("{app_name}", &self.app_name)
            .replace("{http_proxy}", proxy)
            .replace("{env_source}", env_src)
            .replace("{version}", version)
    }

    pub fn move_step_up(&mut self, index: usize) {
        if index > 0 && index < self.steps.len() {
            self.steps.swap(index - 1, index);
        }
    }

    pub fn move_step_down(&mut self, index: usize) {
        if index + 1 < self.steps.len() {
            self.steps.swap(index, index + 1);
        }
    }

    /// Moves a step from one index to another (drag and drop reordering).
    pub fn reorder_step(&mut self, from: usize, to: usize) {
        if from != to && from < self.steps.len() && to < self.steps.len() {
            let step = self.steps.remove(from);
            self.steps.insert(to, step);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: String,
    pub local_path: String,
    pub environments: Vec<EnvironmentConfig>,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub sort_order: Option<i32>,
}

impl Project {
    pub fn get_env(&self, env_key: &str) -> Option<&EnvironmentConfig> {
        self.environments.iter().find(|e| e.key == env_key)
    }

    pub fn get_env_mut(&mut self, env_key: &str) -> Option<&mut EnvironmentConfig> {
        self.environments.iter_mut().find(|e| e.key == env_key)
    }

    /// Adds a new custom environment (e.g. `"UAT"`, `"QA"`, `"Canary"`), optionally cloning
    /// pipeline steps and server settings from an existing environment key.
    pub fn add_environment(
        &mut self,
        display_name: &str,
        clone_from_key: Option<&str>,
    ) -> Result<String, String> {
        let trimmed_name = display_name.trim();
        let key = normalize_env_key(trimmed_name);
        if key.is_empty() {
            return Err("Environment name cannot be empty.".to_string());
        }
        if self.get_env(&key).is_some() {
            return Err(format!("Environment '{key}' already exists in this project."));
        }

        let base_slug = normalize_env_key(&self.name);
        let new_app_name = if key == "production" || key == "prod" {
            base_slug.clone()
        } else {
            format!("{base_slug}-{key}")
        };

        let template = clone_from_key
            .filter(|k| !k.is_empty())
            .and_then(|k| self.get_env(k))
            .cloned();

        let new_env = if let Some(tpl) = template {
            let remote_dir = if !tpl.remote_dir.is_empty() && tpl.remote_dir.ends_with(&tpl.app_name) {
                let prefix = &tpl.remote_dir[..tpl.remote_dir.len() - tpl.app_name.len()];
                format!("{prefix}{new_app_name}")
            } else if !tpl.remote_user.is_empty() {
                format!("/home/{}/apps/{new_app_name}", tpl.remote_user)
            } else {
                String::new()
            };

            let env_source = tpl.env_source.as_ref().map(|_| format!("./.env.{key}"));
            let steps = tpl
                .steps
                .into_iter()
                .map(|s| PipelineStep {
                    id: Uuid::new_v4().to_string(),
                    ..s
                })
                .collect();

            EnvironmentConfig {
                key: key.clone(),
                name: trimmed_name.to_string(),
                app_name: new_app_name,
                servers: tpl.servers,
                remote_user: tpl.remote_user,
                vault_target: tpl.vault_target,
                remote_dir,
                env_source,
                http_proxy: tpl.http_proxy,
                steps,
            }
        } else {
            EnvironmentConfig {
                key: key.clone(),
                name: trimmed_name.to_string(),
                app_name: new_app_name,
                servers: Vec::new(),
                remote_user: String::new(),
                vault_target: String::new(),
                remote_dir: String::new(),
                env_source: None,
                http_proxy: None,
                steps: vec![PipelineStep::new(
                    "Build Project",
                    StepKind::LocalCommand {
                        command: "npm run build".to_string(),
                        allow_failure: false,
                    },
                )],
            }
        };

        self.environments.push(new_env);
        Ok(key)
    }

    /// Removes an environment by key. Allows deleting any environment, including the last one.
    pub fn remove_environment(&mut self, env_key: &str) -> Result<(), String> {
        let Some(pos) = self.environments.iter().position(|e| e.key == env_key) else {
            return Err(format!("Environment '{env_key}' not found."));
        };
        self.environments.remove(pos);
        Ok(())
    }

    /// Creates a complete duplicate of this project with a new ID and display name,
    /// giving every pipeline step a fresh unique UUID.
    pub fn create_duplicate(&self, new_id: String, new_name: String) -> Self {
        let mut new_environments = self.environments.clone();
        for env in &mut new_environments {
            for step in &mut env.steps {
                step.id = Uuid::new_v4().to_string();
            }
        }
        Project {
            id: new_id,
            name: new_name,
            description: self.description.clone(),
            local_path: self.local_path.clone(),
            environments: new_environments,
            group: self.group.clone(),
            sort_order: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeploymentStatus {
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl DeploymentStatus {
    pub fn label(&self) -> &'static str {
        match self {
            DeploymentStatus::Running => "Running",
            DeploymentStatus::Succeeded => "Succeeded",
            DeploymentStatus::Failed => "Failed",
            DeploymentStatus::Cancelled => "Cancelled",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepExecutionLog {
    pub step_name: String,
    pub command_summary: String,
    pub status: DeploymentStatus,
    pub output: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentRecord {
    pub id: String,
    pub project_id: String,
    pub project_name: String,
    /// Key of the environment deployed (e.g. `"dev"`, `"staging"`, `"production"`, `"uat"`).
    pub environment: String,
    pub app_name: String,
    pub version: String,
    pub git_branch: Option<String>,
    pub git_commit: Option<String>,
    /// Summary of Git commits / code changes included in this deployment.
    #[serde(default)]
    pub git_changes: Vec<String>,
    /// Indicates whether this deployment was triggered as a Rollback.
    #[serde(default)]
    pub is_rollback: bool,
    pub servers: Vec<String>,
    pub status: DeploymentStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<u64>,
    pub step_logs: Vec<StepExecutionLog>,
    /// Snapshot of the steps executed so a deployment can be accurately re-run or rolled back.
    pub executed_steps: Vec<PipelineStep>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_env_config() -> EnvironmentConfig {
        EnvironmentConfig {
            key: "dev".to_string(),
            name: "Development".to_string(),
            app_name: "ndm-dq-subscription-dev".to_string(),
            servers: vec!["10.62.70.80".to_string(), "10.62.70.81".to_string()],
            remote_user: "appndq".to_string(),
            vault_target: "indyserver".to_string(),
            remote_dir: "/data/indy-service/apps/ndm-dq-subscription-dev".to_string(),
            env_source: Some("./.env.dev".to_string()),
            http_proxy: Some("http://10.54.18.42:8080".to_string()),
            steps: vec![
                PipelineStep::new(
                    "Build",
                    StepKind::LocalCommand {
                        command: "npm run build".to_string(),
                        allow_failure: false,
                    },
                ),
                PipelineStep::new(
                    "Restart PM2",
                    StepKind::RemoteCommand {
                        command: "/data/{remote_user}/nodejs/bin/pm2 restart {app_name}".to_string(),
                    },
                ),
            ],
        }
    }

    #[test]
    fn test_environment_interpolation() {
        let cfg = sample_env_config();
        let cmd = "export http_proxy={http_proxy} && cd {remote_dir} && /data/{remote_user}/nodejs/bin/pm2 restart {app_name} --update-env # {version}";
        let rendered = cfg.interpolate(cmd, "1.2.3");
        assert_eq!(
            rendered,
            "export http_proxy=http://10.54.18.42:8080 && cd /data/indy-service/apps/ndm-dq-subscription-dev && /data/appndq/nodejs/bin/pm2 restart ndm-dq-subscription-dev --update-env # 1.2.3"
        );
    }

    #[test]
    fn test_step_reordering() {
        let mut cfg = sample_env_config();
        assert_eq!(cfg.steps[0].name, "Build");
        assert_eq!(cfg.steps[1].name, "Restart PM2");

        cfg.move_step_down(0);
        assert_eq!(cfg.steps[0].name, "Restart PM2");
        assert_eq!(cfg.steps[1].name, "Build");

        cfg.move_step_up(1);
        assert_eq!(cfg.steps[0].name, "Build");
        assert_eq!(cfg.steps[1].name, "Restart PM2");

        // Test drag-and-drop reorder
        cfg.reorder_step(0, 1);
        assert_eq!(cfg.steps[0].name, "Restart PM2");
        assert_eq!(cfg.steps[1].name, "Build");

        cfg.reorder_step(1, 0);
        assert_eq!(cfg.steps[0].name, "Build");
        assert_eq!(cfg.steps[1].name, "Restart PM2");
    }

    #[test]
    fn test_add_and_remove_custom_environment() {
        let mut proj = Project {
            id: "ndm-dq-subscription".to_string(),
            name: "ndm-dq-subscription".to_string(),
            description: "Backend".to_string(),
            local_path: r"D:\NTI\Projects\backend\ndm-dq-subscription".to_string(),
            environments: vec![sample_env_config()],
            group: None,
            sort_order: None,
        };

        let uat_key = proj
            .add_environment("UAT", Some("dev"))
            .expect("add UAT env");
        assert_eq!(uat_key, "uat");

        let uat = proj.get_env("uat").expect("find uat");
        assert_eq!(uat.name, "UAT");
        assert_eq!(uat.badge_label(), "UAT");
        assert_eq!(uat.app_name, "ndm-dq-subscription-uat");
        assert_eq!(
            uat.remote_dir,
            "/data/indy-service/apps/ndm-dq-subscription-uat"
        );
        assert_eq!(uat.env_source.as_deref(), Some("./.env.uat"));
        assert_eq!(uat.steps.len(), 2);

        // Duplicate should fail
        assert!(proj.add_environment("uat", None).is_err());

        // Remove UAT and remove dev (even the last environment can be deleted)
        proj.remove_environment("uat").expect("remove uat");
        proj.remove_environment("dev").expect("remove last env");
        assert!(proj.environments.is_empty());
    }

    #[test]
    fn test_project_create_duplicate() {
        let orig = Project {
            id: "proj-1".to_string(),
            name: "Original Service".to_string(),
            description: "Production API".to_string(),
            local_path: r"C:\apps\api".to_string(),
            environments: vec![sample_env_config()],
            group: Some("Backend".to_string()),
            sort_order: Some(1),
        };

        let dup = orig.create_duplicate("proj-1-copy".to_string(), "Original Service (Copy)".to_string());
        assert_eq!(dup.id, "proj-1-copy");
        assert_eq!(dup.name, "Original Service (Copy)");
        assert_eq!(dup.description, "Production API");
        assert_eq!(dup.local_path, r"C:\apps\api");
        assert_eq!(dup.environments.len(), 1);
        assert_eq!(dup.environments[0].steps.len(), 2);
        assert_eq!(dup.group.as_deref(), Some("Backend"));
        assert_eq!(dup.sort_order, None);
        // Verify pipeline step IDs are refreshed with new distinct UUIDs
        assert_ne!(dup.environments[0].steps[0].id, orig.environments[0].steps[0].id);
        assert_ne!(dup.environments[0].steps[1].id, orig.environments[0].steps[1].id);
    }
}
