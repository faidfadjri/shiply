use crate::domain::{normalize_env_key, EnvironmentConfig, PipelineStep, Project, StepKind};

pub fn default_projects() -> Vec<Project> {
    vec![
        ndm_dq_subscription_project(),
        indy_subscription_project(),
    ]
}

pub fn ndm_dq_subscription_project() -> Project {
    let environments = vec![
        ndm_backend_env(
            "dev",
            "Development",
            "ndm-dq-subscription-dev",
            "./.env.dev",
            true,
        ),
        ndm_backend_env(
            "staging",
            "Staging",
            "ndm-dq-subscription-staging",
            "./.env.staging",
            false,
        ),
        ndm_backend_env(
            "production",
            "Production",
            "ndm-dq-subscription",
            "./.env.prod",
            false,
        ),
    ];

    Project {
        id: "ndm-dq-subscription".to_string(),
        name: "ndm-dq-subscription".to_string(),
        description: "Backend REST API service for NDM DQ (Node.js / TypeScript / PM2)".to_string(),
        local_path: r"D:\NTI\Projects\backend\ndm-dq-subscription".to_string(),
        environments,
        group: Some("Backend".to_string()),
        sort_order: Some(0),
    }
}

fn ndm_backend_env(
    key: &str,
    name: &str,
    app_name: &str,
    env_source: &str,
    allow_build_failure: bool,
) -> EnvironmentConfig {
    let remote_dir = format!("/data/indy-service/apps/{app_name}");
    let steps = vec![
        PipelineStep::new(
            "Verify Environment File",
            StepKind::VerifyLocalPath {
                path: "{env_source}".to_string(),
            },
        ),
        PipelineStep::new(
            "Verify package.json",
            StepKind::VerifyLocalPath {
                path: "./package.json".to_string(),
            },
        ),
        PipelineStep::new(
            "Build Application",
            StepKind::LocalCommand {
                command: "npm run build".to_string(),
                allow_failure: allow_build_failure,
            },
        ),
        PipelineStep::new(
            "Verify Build Artifact (dist/server.js)",
            StepKind::VerifyLocalPath {
                path: "./dist/server.js".to_string(),
            },
        ),
        PipelineStep::new(
            "Clean Remote Stale Files (Preserve node_modules)",
            StepKind::RemoteCommand {
                command: "mkdir -p {remote_dir} && find {remote_dir} -mindepth 1 -maxdepth 1 ! -name node_modules -exec rm -rf {} +".to_string(),
            },
        ),
        PipelineStep::new(
            "Upload Artifacts & Staged .env",
            StepKind::TransferFiles {
                sources: vec![
                    "./dist".to_string(),
                    "./package.json".to_string(),
                    "./package-lock.json".to_string(),
                ],
                stage_env_file: true,
            },
        ),
        PipelineStep::new(
            "Install Dependencies (npm ci via Proxy)",
            StepKind::RemoteCommand {
                command: "export http_proxy={http_proxy} && export https_proxy={http_proxy} && cd {remote_dir} && /data/{remote_user}/nodejs/bin/npm ci".to_string(),
            },
        ),
        PipelineStep::new(
            "Restart PM2 Process",
            StepKind::RemoteCommand {
                command: "/data/{remote_user}/nodejs/bin/pm2 restart {app_name}".to_string(),
            },
        ),
    ];

    EnvironmentConfig {
        key: key.to_string(),
        name: name.to_string(),
        app_name: app_name.to_string(),
        servers: vec!["10.62.70.80".to_string(), "10.62.70.81".to_string()],
        remote_user: "appndq".to_string(),
        vault_target: "indyserver".to_string(),
        remote_dir,
        env_source: Some(env_source.to_string()),
        http_proxy: Some("http://10.54.18.42:8080".to_string()),
        steps,
    }
}

pub fn indy_subscription_project() -> Project {
    let environments = vec![
        indy_frontend_env("dev", "Development", "indy-subscription-dev"),
        indy_frontend_env("staging", "Staging", "indy-subscription-staging"),
        indy_frontend_env("production", "Production", "indy-subscription"),
    ];

    Project {
        id: "indy-subscription".to_string(),
        name: "indy-subscription".to_string(),
        description: "Frontend web application for Indy Subscription (React / Vite)".to_string(),
        local_path: r"D:\NTI\Projects\frontend\indy-subscription".to_string(),
        environments,
        group: Some("Frontend".to_string()),
        sort_order: Some(1),
    }
}

fn indy_frontend_env(key: &str, name: &str, app_name: &str) -> EnvironmentConfig {
    let remote_dir = format!("/home/appndq/apps/{app_name}");
    let steps = vec![
        PipelineStep::new(
            "Run All Tests",
            StepKind::LocalCommand {
                command: "npm run test".to_string(),
                allow_failure: false,
            },
        ),
        PipelineStep::new(
            "Build Project",
            StepKind::LocalCommand {
                command: "npm run build".to_string(),
                allow_failure: false,
            },
        ),
        PipelineStep::new(
            "Verify Build Directory (./dist)",
            StepKind::VerifyLocalPath {
                path: "./dist".to_string(),
            },
        ),
        PipelineStep::new(
            "Ensure Remote Directory Exists",
            StepKind::RemoteCommand {
                command: "mkdir -p {remote_dir}".to_string(),
            },
        ),
        PipelineStep::new(
            "Upload Build Artifacts (./dist/*)",
            StepKind::TransferFiles {
                sources: vec!["./dist/*".to_string()],
                stage_env_file: false,
            },
        ),
        PipelineStep::new(
            "Ensure Read & Execute Permissions",
            StepKind::RemoteCommand {
                command: "chmod -R a+rX {remote_dir}".to_string(),
            },
        ),
    ];

    EnvironmentConfig {
        key: key.to_string(),
        name: name.to_string(),
        app_name: app_name.to_string(),
        servers: vec!["10.62.70.79".to_string()],
        remote_user: "appndq".to_string(),
        vault_target: "indyserver".to_string(),
        remote_dir,
        env_source: None,
        http_proxy: None,
        steps,
    }
}

pub fn create_custom_project(name: &str, local_path: &str, description: &str) -> Project {
    let slug = normalize_env_key(name);
    let mut environments = Vec::new();

    for (key, display_name, suffix) in [
        ("dev", "Development", "-dev"),
        ("staging", "Staging", "-staging"),
        ("production", "Production", ""),
    ] {
        let app_name = format!("{slug}{suffix}");
        let remote_dir = format!("/home/appndq/apps/{app_name}");
        environments.push(EnvironmentConfig {
            key: key.to_string(),
            name: display_name.to_string(),
            app_name,
            servers: vec!["10.62.70.79".to_string()],
            remote_user: "appndq".to_string(),
            vault_target: "indyserver".to_string(),
            remote_dir,
            env_source: None,
            http_proxy: None,
            steps: vec![
                PipelineStep::new(
                    "Build Project",
                    StepKind::LocalCommand {
                        command: "npm run build".to_string(),
                        allow_failure: false,
                    },
                ),
                PipelineStep::new(
                    "Verify Build Directory",
                    StepKind::VerifyLocalPath {
                        path: "./dist".to_string(),
                    },
                ),
                PipelineStep::new(
                    "Ensure Remote Directory",
                    StepKind::RemoteCommand {
                        command: "mkdir -p {remote_dir}".to_string(),
                    },
                ),
                PipelineStep::new(
                    "Upload Artifacts",
                    StepKind::TransferFiles {
                        sources: vec!["./dist/*".to_string()],
                        stage_env_file: false,
                    },
                ),
            ],
        });
    }

    Project {
        id: format!("{slug}-{}", uuid::Uuid::new_v4().simple()),
        name: name.trim().to_string(),
        description: description.trim().to_string(),
        local_path: local_path.trim().to_string(),
        environments,
        group: None,
        sort_order: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_projects_match_learned_scripts() {
        let projects = default_projects();
        assert_eq!(projects.len(), 2);

        let backend = &projects[0];
        assert_eq!(backend.name, "ndm-dq-subscription");
        let backend_dev = backend.get_env("dev").expect("dev env");
        assert_eq!(backend_dev.app_name, "ndm-dq-subscription-dev");
        assert_eq!(backend_dev.servers, vec!["10.62.70.80", "10.62.70.81"]);
        assert_eq!(backend_dev.env_source.as_deref(), Some("./.env.dev"));
        assert_eq!(
            backend_dev.http_proxy.as_deref(),
            Some("http://10.54.18.42:8080")
        );

        let backend_prod = backend.get_env("production").expect("prod env");
        assert_eq!(backend_prod.app_name, "ndm-dq-subscription");
        assert_eq!(backend_prod.env_source.as_deref(), Some("./.env.prod"));

        let frontend = &projects[1];
        assert_eq!(frontend.name, "indy-subscription");
        let frontend_dev = frontend.get_env("dev").expect("dev env");
        assert_eq!(frontend_dev.app_name, "indy-subscription-dev");
        assert_eq!(frontend_dev.servers, vec!["10.62.70.79"]);
        assert_eq!(
            frontend_dev.remote_dir,
            "/home/appndq/apps/indy-subscription-dev"
        );
    }
}
