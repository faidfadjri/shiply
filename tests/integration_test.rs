use shiply::credentials::{ENV_ASKPASS_FILE, ENV_ASKPASS_MODE};
use shiply::domain::{DeploymentStatus, EnvironmentConfig, PipelineStep, StepKind};
use shiply::executor::{run_deployment, DeploymentRequest};
use shiply::presets::default_projects;
use shiply::storage::AppStorage;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_presets_custom_environment_and_full_deployment_lifecycle() {
    let tmp = TempDir::new().expect("tempdir");
    let state_path = tmp.path().join("shiply_state.json");
    let mut storage = AppStorage::load_or_default(&state_path);

    let mut projects = default_projects();
    assert_eq!(projects.len(), 2);
    assert_eq!(projects[0].id, "ndm-dq-subscription");
    assert_eq!(projects[1].id, "indy-subscription");

    // Verify user can dynamically add a new custom environment (e.g. UAT) cloned from dev
    let uat_key = projects[0]
        .add_environment("UAT", Some("dev"))
        .expect("add custom UAT environment");
    assert_eq!(uat_key, "uat");
    let uat_env = projects[0].get_env("uat").expect("get uat env");
    assert_eq!(uat_env.app_name, "ndm-dq-subscription-uat");

    // Set up a mock project directory to test end-to-end deployment + version bump + redeploy
    let proj_dir = tmp.path().join("mock-service");
    fs::create_dir_all(proj_dir.join("dist")).expect("create dist");
    fs::write(proj_dir.join("dist/server.js"), "console.log('ready');").expect("write server.js");
    fs::write(
        proj_dir.join("package.json"),
        "{\n  \"name\": \"mock-service\",\n  \"version\": \"1.0.0\"\n}\n",
    )
    .expect("write package.json");

    let env_config = EnvironmentConfig {
        key: "uat".to_string(),
        name: "UAT".to_string(),
        app_name: "mock-service-uat".to_string(),
        servers: vec!["10.62.70.80".to_string()],
        remote_user: "appndq".to_string(),
        vault_target: "indyserver".to_string(),
        remote_dir: "/data/indy-service/apps/mock-service-uat".to_string(),
        env_source: None,
        http_proxy: None,
        steps: vec![
            PipelineStep::new(
                "Run Build",
                StepKind::LocalCommand {
                    command: "echo Building {app_name} v{version}".to_string(),
                    allow_failure: false,
                },
            ),
            PipelineStep::new(
                "Verify Artifact",
                StepKind::VerifyLocalPath {
                    path: "./dist/server.js".to_string(),
                },
            ),
        ],
    };

    let req = DeploymentRequest {
        deployment_id: "dep-uat-1".to_string(),
        project_id: "ndm-dq-subscription".to_string(),
        project_name: "ndm-dq-subscription".to_string(),
        local_path: proj_dir.clone(),
        env_config,
        version: "1.4.2".to_string(),
        bump_package_json: true,
        create_git_tag_on_success: false,
        manual_password: None,
        skip_ssh_preflight: true,
    };

    let record = run_deployment(req, None);
    assert_eq!(record.status, DeploymentStatus::Succeeded);
    storage.upsert_deployment(record);

    let last_ok = storage
        .last_succeeded_deployment("ndm-dq-subscription", "uat")
        .expect("should have succeeded uat deployment");
    assert_eq!(last_ok.version, "1.4.2");
    assert_eq!(last_ok.executed_steps.len(), 2);
}

#[test]
fn test_shiply_binary_askpass_mode() {
    let tmp = TempDir::new().expect("tempdir");
    let pw_file = tmp.path().join("secret_pw");
    fs::write(&pw_file, "IndyServerSecret!99").expect("write pw");

    let bin_path = env!("CARGO_BIN_EXE_shiply");
    let output = Command::new(bin_path)
        .env(ENV_ASKPASS_MODE, "1")
        .env(ENV_ASKPASS_FILE, &pw_file)
        .output()
        .expect("run shiply in askpass mode");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "IndyServerSecret!99"
    );
}
