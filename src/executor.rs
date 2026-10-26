use crate::credentials::{
    resolve_credentials, ENV_ASKPASS_FILE, ENV_ASKPASS_MODE,
};
use crate::domain::{
    DeploymentRecord, DeploymentStatus, EnvironmentConfig, PipelineStep, StepExecutionLog,
    StepKind,
};
use crate::version::{
    checkout_git_branch, configure_background_command, create_git_tag, read_git_code_changes,
    read_git_metadata, update_package_json_version,
};
use chrono::Utc;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::Sender;
use std::time::Instant;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DeploymentRequest {
    pub deployment_id: String,
    pub project_id: String,
    pub project_name: String,
    pub local_path: PathBuf,
    pub env_config: EnvironmentConfig,
    pub version: String,
    pub bump_package_json: bool,
    pub create_git_tag_on_success: bool,
    pub manual_password: Option<String>,
    pub skip_ssh_preflight: bool,
    pub target_git_branch: Option<String>,
    pub rollback_commit: Option<String>,
    pub previous_commit: Option<String>,
    pub is_rollback: bool,
}

#[derive(Debug, Clone)]
pub enum ExecutorEvent {
    Updated(DeploymentRecord),
    Finished(DeploymentRecord),
}

struct AskpassGuard {
    dir: PathBuf,
    password_file: PathBuf,
}

impl AskpassGuard {
    fn new(password: &str) -> Result<Self, String> {
        let dir = env::temp_dir().join(format!("shiply-askpass-{}", Uuid::new_v4().simple()));
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create temp askpass directory: {e}"))?;
        let password_file = dir.join("p");
        fs::write(&password_file, password.as_bytes())
            .map_err(|e| format!("Failed to write temporary askpass secret: {e}"))?;
        Ok(Self { dir, password_file })
    }

    fn apply_to_command(&self, cmd: &mut Command) {
        if let Ok(exe) = env::current_exe() {
            cmd.env("SSH_ASKPASS", exe);
            cmd.env("SSH_ASKPASS_REQUIRE", "force");
            cmd.env("DISPLAY", "shiply:0");
            cmd.env(ENV_ASKPASS_MODE, "1");
            cmd.env(ENV_ASKPASS_FILE, &self.password_file);
        }
    }
}

impl Drop for AskpassGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

struct StagingGuard {
    dir: PathBuf,
}

impl StagingGuard {
    fn stage_env_file(source_file: &Path) -> Result<(Self, PathBuf), String> {
        let dir = env::temp_dir().join(format!("shiply-stage-{}", Uuid::new_v4().simple()));
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create temporary staging directory: {e}"))?;
        let dest = dir.join(".env");
        fs::copy(source_file, &dest).map_err(|e| {
            format!(
                "Failed to stage '{}' as '{}': {e}",
                source_file.display(),
                dest.display()
            )
        })?;
        Ok((Self { dir }, dest))
    }
}

impl Drop for StagingGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Resolves the path to an OpenSSH binary (`ssh` or `scp`) across Windows, macOS, and Linux.
pub fn resolve_openssh_binary(tool: &str) -> PathBuf {
    #[cfg(windows)]
    let exe_name = format!("{tool}.exe");
    #[cfg(not(windows))]
    let exe_name = tool.to_string();

    if let Some(paths) = env::var_os("PATH") {
        for dir in env::split_paths(&paths) {
            let candidate = dir.join(&exe_name);
            if candidate.is_file() {
                return candidate;
            }
        }
    }

    #[cfg(windows)]
    {
        let mut fallback_dirs = Vec::new();
        if let Ok(sys_root) = env::var("SystemRoot") {
            fallback_dirs.push(PathBuf::from(sys_root).join("System32").join("OpenSSH"));
        }
        if let Ok(prog_files) = env::var("ProgramFiles") {
            fallback_dirs.push(PathBuf::from(&prog_files).join("Git").join("usr").join("bin"));
            fallback_dirs.push(PathBuf::from(prog_files).join("OpenSSH-Win64"));
        }
        for dir in fallback_dirs {
            let candidate = dir.join(&exe_name);
            if candidate.is_file() {
                return candidate;
            }
        }
    }

    PathBuf::from(tool)
}

/// Expands source paths relative to `project_dir`, including trailing `/*` wildcards
/// (e.g. `./dist/*` in `indy-subscription`).
pub fn expand_transfer_sources(
    project_dir: &Path,
    sources: &[String],
) -> Result<Vec<PathBuf>, String> {
    let mut expanded = Vec::new();
    for raw in sources {
        let trimmed = raw.trim();
        if let Some(dir_prefix) = trimmed
            .strip_suffix("/*")
            .or_else(|| trimmed.strip_suffix(r"\*"))
        {
            let base_dir = project_dir.join(dir_prefix);
            if !base_dir.is_dir() {
                return Err(format!(
                    "Source directory '{}' does not exist",
                    base_dir.display()
                ));
            }
            let mut entries: Vec<PathBuf> = fs::read_dir(&base_dir)
                .map_err(|e| format!("Failed to read '{}': {e}", base_dir.display()))?
                .filter_map(|entry| entry.ok().map(|e| e.path()))
                .collect();
            entries.sort();
            if entries.is_empty() {
                return Err(format!(
                    "Source directory '{}' is empty (nothing matched '{trimmed}')",
                    base_dir.display()
                ));
            }
            expanded.extend(entries);
        } else {
            let path = project_dir.join(trimmed);
            if !path.exists() {
                return Err(format!("Source path '{}' does not exist", path.display()));
            }
            expanded.push(path);
        }
    }
    Ok(expanded)
}

fn run_command_capture(cmd: &mut Command) -> Result<(bool, String), String> {
    configure_background_command(cmd);
    let output = cmd
        .output()
        .map_err(|e| format!("Failed to spawn process: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut combined = String::new();
    if !stdout.trim().is_empty() {
        combined.push_str(stdout.trim_end());
    }
    if !stderr.trim().is_empty() {
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str(stderr.trim_end());
    }
    Ok((output.status.success(), combined))
}

fn execute_local_shell_command(
    project_dir: &Path,
    command_str: &str,
) -> Result<(bool, String), String> {
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/C", command_str]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-c", command_str]);
        c
    };
    cmd.current_dir(project_dir);
    run_command_capture(&mut cmd)
}

fn build_ssh_command(
    remote_user: &str,
    server: &str,
    remote_cmd: &str,
    askpass: Option<&AskpassGuard>,
) -> Command {
    let ssh_bin = resolve_openssh_binary("ssh");
    let mut cmd = Command::new(ssh_bin);
    cmd.args([
        "-o",
        "StrictHostKeyChecking=no",
        "-o",
        "ConnectTimeout=10",
        "-o",
        "NumberOfPasswordPrompts=1",
    ]);
    if let Some(guard) = askpass {
        guard.apply_to_command(&mut cmd);
    } else {
        cmd.args(["-o", "BatchMode=yes"]);
    }
    cmd.arg(format!("{remote_user}@{server}"));
    cmd.arg(remote_cmd);
    cmd
}

fn build_scp_command(
    remote_user: &str,
    server: &str,
    remote_dir: &str,
    sources: &[PathBuf],
    askpass: Option<&AskpassGuard>,
) -> Command {
    let scp_bin = resolve_openssh_binary("scp");
    let mut cmd = Command::new(scp_bin);
    cmd.args([
        "-o",
        "StrictHostKeyChecking=no",
        "-o",
        "ConnectTimeout=10",
        "-o",
        "NumberOfPasswordPrompts=1",
        "-r",
    ]);
    if let Some(guard) = askpass {
        guard.apply_to_command(&mut cmd);
    } else {
        cmd.args(["-o", "BatchMode=yes"]);
    }
    for src in sources {
        cmd.arg(src);
    }
    cmd.arg(format!("{remote_user}@{server}:{remote_dir}"));
    cmd
}

/// Runs the deployment pipeline synchronously (intended to be called on a background thread)
/// and sends live updates through `tx` if provided.
pub fn run_deployment(
    req: DeploymentRequest,
    tx: Option<&Sender<ExecutorEvent>>,
) -> DeploymentRecord {
    let started_instant = Instant::now();
    let started_at = Utc::now();

    let enabled_steps: Vec<PipelineStep> = req
        .env_config
        .steps
        .iter()
        .filter(|s| s.enabled)
        .cloned()
        .collect();

    let initial_meta = read_git_metadata(&req.local_path);
    let initial_changes =
        read_git_code_changes(&req.local_path, req.previous_commit.as_deref());

    let mut record = DeploymentRecord {
        id: req.deployment_id.clone(),
        project_id: req.project_id.clone(),
        project_name: req.project_name.clone(),
        environment: req.env_config.key.clone(),
        app_name: req.env_config.app_name.clone(),
        version: req.version.clone(),
        git_branch: initial_meta.branch.clone(),
        git_commit: initial_meta.commit_short.clone(),
        git_changes: initial_changes,
        is_rollback: req.is_rollback,
        servers: req.env_config.servers.clone(),
        status: DeploymentStatus::Running,
        started_at,
        finished_at: None,
        duration_ms: None,
        step_logs: Vec::new(),
        executed_steps: enabled_steps.clone(),
    };

    let notify = |rec: &DeploymentRecord| {
        if let Some(sender) = tx {
            let _ = sender.send(ExecutorEvent::Updated(rec.clone()));
        }
    };

    notify(&record);

    // Optional Rollback or Git Branch Checkout pre-step
    if req.is_rollback {
        let step_start = Instant::now();
        let target_ref = req
            .rollback_commit
            .as_deref()
            .or(req.target_git_branch.as_deref());
        let mut rollback_msg = format!(
            "Rolling back '{}' ({}) to version v{}",
            req.project_name, req.env_config.name, req.version
        );
        if let Some(git_ref) = target_ref {
            if !git_ref.trim().is_empty() {
                match checkout_git_branch(&req.local_path, git_ref) {
                    Ok(out) => {
                        rollback_msg.push_str(&format!("\n{out}"));
                    }
                    Err(err) => {
                        rollback_msg.push_str(&format!(
                            "\n[Note] Could not switch git ref '{git_ref}' ({err}); proceeding with rollback pipeline."
                        ));
                    }
                }
            }
        }
        let updated_meta = read_git_metadata(&req.local_path);
        if updated_meta.commit_short.is_some() {
            record.git_commit = updated_meta.commit_short;
            record.git_branch = updated_meta.branch.or(req.target_git_branch.clone());
            record.git_changes =
                read_git_code_changes(&req.local_path, req.previous_commit.as_deref());
        }
        record.step_logs.push(StepExecutionLog {
            step_name: format!("Rollback to v{}", req.version),
            command_summary: format!("rollback -> v{}", req.version),
            status: DeploymentStatus::Succeeded,
            output: rollback_msg,
            duration_ms: step_start.elapsed().as_millis() as u64,
        });
        notify(&record);
    } else if let Some(target_branch) = &req.target_git_branch {
        let trimmed_branch = target_branch.trim();
        if !trimmed_branch.is_empty()
            && initial_meta.branch.as_deref() != Some(trimmed_branch)
        {
            let step_start = Instant::now();
            match checkout_git_branch(&req.local_path, trimmed_branch) {
                Ok(out) => {
                    let updated_meta = read_git_metadata(&req.local_path);
                    record.git_branch = updated_meta.branch;
                    record.git_commit = updated_meta.commit_short;
                    record.git_changes =
                        read_git_code_changes(&req.local_path, req.previous_commit.as_deref());
                    record.step_logs.push(StepExecutionLog {
                        step_name: "Checkout Git Branch".to_string(),
                        command_summary: format!("git checkout {trimmed_branch}"),
                        status: DeploymentStatus::Succeeded,
                        output: out,
                        duration_ms: step_start.elapsed().as_millis() as u64,
                    });
                    notify(&record);
                }
                Err(err) => {
                    record.step_logs.push(StepExecutionLog {
                        step_name: "Checkout Git Branch".to_string(),
                        command_summary: format!("git checkout {trimmed_branch}"),
                        status: DeploymentStatus::Failed,
                        output: err,
                        duration_ms: step_start.elapsed().as_millis() as u64,
                    });
                    return finalize_record(record, DeploymentStatus::Failed, started_instant, tx);
                }
            }
        }
    }

    // Optional pre-step: bump/restore version in package.json
    if req.bump_package_json && req.local_path.join("package.json").exists() {
        let step_start = Instant::now();
        match update_package_json_version(&req.local_path, &req.version) {
            Ok(()) => {
                record.step_logs.push(StepExecutionLog {
                    step_name: "Update package.json Version".to_string(),
                    command_summary: format!("package.json -> version {}", req.version),
                    status: DeploymentStatus::Succeeded,
                    output: format!(
                        "Updated package.json version to '{}' in {}",
                        req.version,
                        req.local_path.display()
                    ),
                    duration_ms: step_start.elapsed().as_millis() as u64,
                });
                notify(&record);
            }
            Err(err) => {
                record.step_logs.push(StepExecutionLog {
                    step_name: "Update package.json Version".to_string(),
                    command_summary: format!("package.json -> version {}", req.version),
                    status: DeploymentStatus::Failed,
                    output: err,
                    duration_ms: step_start.elapsed().as_millis() as u64,
                });
                return finalize_record(record, DeploymentStatus::Failed, started_instant, tx);
            }
        }
    }

    // Determine if any enabled step requires SSH / SCP
    let needs_ssh = enabled_steps.iter().any(|s| {
        matches!(
            s.kind,
            StepKind::RemoteCommand { .. } | StepKind::TransferFiles { .. }
        )
    });

    let resolved_creds = resolve_credentials(
        &req.env_config.remote_user,
        &req.env_config.vault_target,
        req.manual_password.as_deref(),
    );

    let mut env_cfg = req.env_config.clone();
    env_cfg.remote_user = resolved_creds.username.clone();

    let askpass_guard = if let Some(pw) = &resolved_creds.password {
        match AskpassGuard::new(pw) {
            Ok(guard) => Some(guard),
            Err(err) => {
                record.step_logs.push(StepExecutionLog {
                    step_name: "Initialize SSH AskPass".to_string(),
                    command_summary: "Setup temporary SSH_ASKPASS helper".to_string(),
                    status: DeploymentStatus::Failed,
                    output: err,
                    duration_ms: 0,
                });
                return finalize_record(record, DeploymentStatus::Failed, started_instant, tx);
            }
        }
    } else {
        None
    };

    // Pre-flight SSH verification against all servers before building or wiping remote directories
    if needs_ssh && !req.skip_ssh_preflight {
        let step_start = Instant::now();
        if env_cfg.servers.is_empty() {
            record.step_logs.push(StepExecutionLog {
                step_name: "Pre-flight SSH Verification".to_string(),
                command_summary: "Verify target servers configured".to_string(),
                status: DeploymentStatus::Failed,
                output: "No target servers configured for this environment.".to_string(),
                duration_ms: 0,
            });
            return finalize_record(record, DeploymentStatus::Failed, started_instant, tx);
        }

        let mut preflight_log = format!(
            "Credential source: {}\nVerifying SSH access to {}...",
            resolved_creds.source_description,
            env_cfg.servers.join(", ")
        );

        for server in &env_cfg.servers {
            let mut ssh_cmd =
                build_ssh_command(&env_cfg.remote_user, server, "true", askpass_guard.as_ref());
            match run_command_capture(&mut ssh_cmd) {
                Ok((true, _)) => {
                    preflight_log
                        .push_str(&format!("\n  [OK] {}@{}", env_cfg.remote_user, server));
                }
                Ok((false, out)) => {
                    preflight_log.push_str(&format!(
                        "\n  [FAILED] {}@{}: {}",
                        env_cfg.remote_user,
                        server,
                        if out.is_empty() {
                            "Authentication or connection failed. Check OS Credential Manager ('indyserver') or enter SSH password in Shiply."
                        } else {
                            &out
                        }
                    ));
                    record.step_logs.push(StepExecutionLog {
                        step_name: "Pre-flight SSH Verification".to_string(),
                        command_summary: format!(
                            "ssh {}@({}) true",
                            env_cfg.remote_user,
                            env_cfg.servers.join(", ")
                        ),
                        status: DeploymentStatus::Failed,
                        output: preflight_log,
                        duration_ms: step_start.elapsed().as_millis() as u64,
                    });
                    return finalize_record(record, DeploymentStatus::Failed, started_instant, tx);
                }
                Err(err) => {
                    preflight_log.push_str(&format!("\n  [ERROR] {server}: {err}"));
                    record.step_logs.push(StepExecutionLog {
                        step_name: "Pre-flight SSH Verification".to_string(),
                        command_summary: "ssh pre-flight check".to_string(),
                        status: DeploymentStatus::Failed,
                        output: preflight_log,
                        duration_ms: step_start.elapsed().as_millis() as u64,
                    });
                    return finalize_record(record, DeploymentStatus::Failed, started_instant, tx);
                }
            }
        }

        record.step_logs.push(StepExecutionLog {
            step_name: "Pre-flight SSH Verification".to_string(),
            command_summary: format!(
                "ssh {}@({}) true",
                env_cfg.remote_user,
                env_cfg.servers.join(", ")
            ),
            status: DeploymentStatus::Succeeded,
            output: preflight_log,
            duration_ms: step_start.elapsed().as_millis() as u64,
        });
        notify(&record);
    }

    // Execute each enabled pipeline step in order
    for step in &enabled_steps {
        let step_start = Instant::now();
        let summary = env_cfg.interpolate(&step.summary(), &req.version);

        match &step.kind {
            StepKind::VerifyLocalPath { path } => {
                let interpolated = env_cfg.interpolate(path, &req.version);
                let full_path = req.local_path.join(&interpolated);
                if full_path.exists() {
                    record.step_logs.push(StepExecutionLog {
                        step_name: step.name.clone(),
                        command_summary: summary,
                        status: DeploymentStatus::Succeeded,
                        output: format!("Verified path exists: {}", full_path.display()),
                        duration_ms: step_start.elapsed().as_millis() as u64,
                    });
                    notify(&record);
                } else {
                    record.step_logs.push(StepExecutionLog {
                        step_name: step.name.clone(),
                        command_summary: summary,
                        status: DeploymentStatus::Failed,
                        output: format!(
                            "Required path '{}' was not found in '{}'. Deployment aborted.",
                            interpolated,
                            req.local_path.display()
                        ),
                        duration_ms: step_start.elapsed().as_millis() as u64,
                    });
                    return finalize_record(record, DeploymentStatus::Failed, started_instant, tx);
                }
            }

            StepKind::LocalCommand {
                command,
                allow_failure,
            } => {
                let interpolated = env_cfg.interpolate(command, &req.version);
                match execute_local_shell_command(&req.local_path, &interpolated) {
                    Ok((true, out)) => {
                        record.step_logs.push(StepExecutionLog {
                            step_name: step.name.clone(),
                            command_summary: summary,
                            status: DeploymentStatus::Succeeded,
                            output: if out.is_empty() {
                                format!("Command '{}' completed (exit 0).", interpolated)
                            } else {
                                out
                            },
                            duration_ms: step_start.elapsed().as_millis() as u64,
                        });
                        notify(&record);
                    }
                    Ok((false, out)) if *allow_failure => {
                        record.step_logs.push(StepExecutionLog {
                            step_name: step.name.clone(),
                            command_summary: summary,
                            status: DeploymentStatus::Succeeded,
                            output: format!(
                                "{out}\n[Warning] Command exited with non-zero status, but allow_failure is enabled. Proceeding with existing artifacts."
                            ),
                            duration_ms: step_start.elapsed().as_millis() as u64,
                        });
                        notify(&record);
                    }
                    Ok((false, out)) => {
                        record.step_logs.push(StepExecutionLog {
                            step_name: step.name.clone(),
                            command_summary: summary,
                            status: DeploymentStatus::Failed,
                            output: if out.is_empty() {
                                format!(
                                    "Command '{}' failed with non-zero exit code.",
                                    interpolated
                                )
                            } else {
                                out
                            },
                            duration_ms: step_start.elapsed().as_millis() as u64,
                        });
                        return finalize_record(
                            record,
                            DeploymentStatus::Failed,
                            started_instant,
                            tx,
                        );
                    }
                    Err(err) => {
                        record.step_logs.push(StepExecutionLog {
                            step_name: step.name.clone(),
                            command_summary: summary,
                            status: DeploymentStatus::Failed,
                            output: err,
                            duration_ms: step_start.elapsed().as_millis() as u64,
                        });
                        return finalize_record(
                            record,
                            DeploymentStatus::Failed,
                            started_instant,
                            tx,
                        );
                    }
                }
            }

            StepKind::TransferFiles {
                sources,
                stage_env_file,
            } => {
                let interpolated_sources: Vec<String> = sources
                    .iter()
                    .map(|s| env_cfg.interpolate(s, &req.version))
                    .collect();

                let mut resolved_files =
                    match expand_transfer_sources(&req.local_path, &interpolated_sources) {
                        Ok(files) => files,
                        Err(err) => {
                            record.step_logs.push(StepExecutionLog {
                                step_name: step.name.clone(),
                                command_summary: summary,
                                status: DeploymentStatus::Failed,
                                output: err,
                                duration_ms: step_start.elapsed().as_millis() as u64,
                            });
                            return finalize_record(
                                record,
                                DeploymentStatus::Failed,
                                started_instant,
                                tx,
                            );
                        }
                    };

                let mut _staging_guard = None;
                if *stage_env_file {
                    let Some(env_src_rel) = &env_cfg.env_source else {
                        record.step_logs.push(StepExecutionLog {
                            step_name: step.name.clone(),
                            command_summary: summary,
                            status: DeploymentStatus::Failed,
                            output:
                                "Step requires staging an env file, but no env_source is configured."
                                    .to_string(),
                            duration_ms: step_start.elapsed().as_millis() as u64,
                        });
                        return finalize_record(
                            record,
                            DeploymentStatus::Failed,
                            started_instant,
                            tx,
                        );
                    };

                    let env_src_path = req.local_path.join(env_src_rel);
                    match StagingGuard::stage_env_file(&env_src_path) {
                        Ok((guard, staged_path)) => {
                            resolved_files.push(staged_path);
                            _staging_guard = Some(guard);
                        }
                        Err(err) => {
                            record.step_logs.push(StepExecutionLog {
                                step_name: step.name.clone(),
                                command_summary: summary,
                                status: DeploymentStatus::Failed,
                                output: err,
                                duration_ms: step_start.elapsed().as_millis() as u64,
                            });
                            return finalize_record(
                                record,
                                DeploymentStatus::Failed,
                                started_instant,
                                tx,
                            );
                        }
                    }
                }

                let mut transfer_output = String::new();
                for server in &env_cfg.servers {
                    let mut scp_cmd = build_scp_command(
                        &env_cfg.remote_user,
                        server,
                        &env_cfg.remote_dir,
                        &resolved_files,
                        askpass_guard.as_ref(),
                    );
                    match run_command_capture(&mut scp_cmd) {
                        Ok((true, out)) => {
                            transfer_output.push_str(&format!(
                                "[{server}] Uploaded {} item(s) to {}@{}:{}\n",
                                resolved_files.len(),
                                env_cfg.remote_user,
                                server,
                                env_cfg.remote_dir
                            ));
                            if !out.is_empty() {
                                transfer_output.push_str(&out);
                                transfer_output.push('\n');
                            }
                        }
                        Ok((false, out)) => {
                            transfer_output
                                .push_str(&format!("[{server}] SCP upload failed: {out}"));
                            record.step_logs.push(StepExecutionLog {
                                step_name: step.name.clone(),
                                command_summary: summary,
                                status: DeploymentStatus::Failed,
                                output: transfer_output.trim().to_string(),
                                duration_ms: step_start.elapsed().as_millis() as u64,
                            });
                            return finalize_record(
                                record,
                                DeploymentStatus::Failed,
                                started_instant,
                                tx,
                            );
                        }
                        Err(err) => {
                            transfer_output.push_str(&format!("[{server}] Error: {err}"));
                            record.step_logs.push(StepExecutionLog {
                                step_name: step.name.clone(),
                                command_summary: summary,
                                status: DeploymentStatus::Failed,
                                output: transfer_output.trim().to_string(),
                                duration_ms: step_start.elapsed().as_millis() as u64,
                            });
                            return finalize_record(
                                record,
                                DeploymentStatus::Failed,
                                started_instant,
                                tx,
                            );
                        }
                    }
                }

                record.step_logs.push(StepExecutionLog {
                    step_name: step.name.clone(),
                    command_summary: summary,
                    status: DeploymentStatus::Succeeded,
                    output: transfer_output.trim().to_string(),
                    duration_ms: step_start.elapsed().as_millis() as u64,
                });
                notify(&record);
            }

            StepKind::RemoteCommand { command } => {
                let interpolated = env_cfg.interpolate(command, &req.version);
                let mut remote_output = String::new();

                for server in &env_cfg.servers {
                    let mut ssh_cmd = build_ssh_command(
                        &env_cfg.remote_user,
                        server,
                        &interpolated,
                        askpass_guard.as_ref(),
                    );
                    match run_command_capture(&mut ssh_cmd) {
                        Ok((true, out)) => {
                            remote_output.push_str(&format!("[{server}] OK\n"));
                            if !out.is_empty() {
                                remote_output.push_str(&out);
                                remote_output.push('\n');
                            }
                        }
                        Ok((false, out)) => {
                            remote_output.push_str(&format!("[{server}] Command failed:\n{out}"));
                            record.step_logs.push(StepExecutionLog {
                                step_name: step.name.clone(),
                                command_summary: summary,
                                status: DeploymentStatus::Failed,
                                output: remote_output.trim().to_string(),
                                duration_ms: step_start.elapsed().as_millis() as u64,
                            });
                            return finalize_record(
                                record,
                                DeploymentStatus::Failed,
                                started_instant,
                                tx,
                            );
                        }
                        Err(err) => {
                            remote_output.push_str(&format!("[{server}] Error: {err}"));
                            record.step_logs.push(StepExecutionLog {
                                step_name: step.name.clone(),
                                command_summary: summary,
                                status: DeploymentStatus::Failed,
                                output: remote_output.trim().to_string(),
                                duration_ms: step_start.elapsed().as_millis() as u64,
                            });
                            return finalize_record(
                                record,
                                DeploymentStatus::Failed,
                                started_instant,
                                tx,
                            );
                        }
                    }
                }

                record.step_logs.push(StepExecutionLog {
                    step_name: step.name.clone(),
                    command_summary: summary,
                    status: DeploymentStatus::Succeeded,
                    output: remote_output.trim().to_string(),
                    duration_ms: step_start.elapsed().as_millis() as u64,
                });
                notify(&record);
            }
        }
    }

    // Optional post-step: create Git tag on success
    if req.create_git_tag_on_success {
        let step_start = Instant::now();
        match create_git_tag(&req.local_path, &req.version, &req.env_config.key) {
            Ok(tag_name) => {
                record.step_logs.push(StepExecutionLog {
                    step_name: "Create Git Tag".to_string(),
                    command_summary: format!("git tag -f -a {tag_name}"),
                    status: DeploymentStatus::Succeeded,
                    output: format!("Created Git tag '{tag_name}'"),
                    duration_ms: step_start.elapsed().as_millis() as u64,
                });
                notify(&record);
            }
            Err(err) => {
                record.step_logs.push(StepExecutionLog {
                    step_name: "Create Git Tag".to_string(),
                    command_summary: format!("git tag {}", req.version),
                    status: DeploymentStatus::Failed,
                    output: err,
                    duration_ms: step_start.elapsed().as_millis() as u64,
                });
                return finalize_record(record, DeploymentStatus::Failed, started_instant, tx);
            }
        }
    }

    finalize_record(record, DeploymentStatus::Succeeded, started_instant, tx)
}

fn finalize_record(
    mut record: DeploymentRecord,
    status: DeploymentStatus,
    started_instant: Instant,
    tx: Option<&Sender<ExecutorEvent>>,
) -> DeploymentRecord {
    record.status = status;
    record.finished_at = Some(Utc::now());
    record.duration_ms = Some(started_instant.elapsed().as_millis() as u64);
    if let Some(sender) = tx {
        let _ = sender.send(ExecutorEvent::Finished(record.clone()));
    }
    record
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_expand_transfer_sources_with_wildcard() {
        let tmp = TempDir::new().expect("tempdir");
        let dist_dir = tmp.path().join("dist");
        fs::create_dir_all(&dist_dir).expect("create dist");
        fs::write(dist_dir.join("index.html"), "<html></html>").expect("write index");
        fs::write(dist_dir.join("app.js"), "console.log(1)").expect("write js");

        let expanded =
            expand_transfer_sources(tmp.path(), &["./dist/*".to_string()]).expect("expand");
        assert_eq!(expanded.len(), 2);
    }

    #[test]
    fn test_run_local_deployment_pipeline_and_package_bump() {
        let tmp = TempDir::new().expect("tempdir");
        fs::write(
            tmp.path().join("package.json"),
            "{\n  \"name\": \"demo\",\n  \"version\": \"1.0.0\"\n}\n",
        )
        .expect("write pkg");
        let dist_dir = tmp.path().join("dist");
        fs::create_dir_all(&dist_dir).expect("mkdir dist");
        fs::write(dist_dir.join("server.js"), "// built").expect("write server.js");

        let env_config = EnvironmentConfig {
            key: "dev".to_string(),
            name: "Development".to_string(),
            app_name: "demo-dev".to_string(),
            servers: vec![],
            remote_user: "appndq".to_string(),
            vault_target: "indyserver".to_string(),
            remote_dir: "/data/demo".to_string(),
            env_source: None,
            http_proxy: None,
            steps: vec![
                PipelineStep::new(
                    "Echo Version",
                    StepKind::LocalCommand {
                        command: "echo Deploying {version}".to_string(),
                        allow_failure: false,
                    },
                ),
                PipelineStep::new(
                    "Check Artifact",
                    StepKind::VerifyLocalPath {
                        path: "./dist/server.js".to_string(),
                    },
                ),
            ],
        };

        let req = DeploymentRequest {
            deployment_id: "test-dep-1".to_string(),
            project_id: "demo".to_string(),
            project_name: "Demo".to_string(),
            local_path: tmp.path().to_path_buf(),
            env_config: env_config.clone(),
            version: "1.1.0".to_string(),
            bump_package_json: true,
            create_git_tag_on_success: false,
            manual_password: None,
            skip_ssh_preflight: true,
            target_git_branch: None,
            rollback_commit: None,
            previous_commit: None,
            is_rollback: false,
        };

        let record = run_deployment(req, None);
        assert_eq!(record.status, DeploymentStatus::Succeeded);
        assert_eq!(record.step_logs.len(), 3); // Bump + Echo + Check Artifact
        assert_eq!(
            crate::version::read_package_json_version(tmp.path()).as_deref(),
            Some("1.1.0")
        );

        // Test Rollback back to v1.0.0
        let rollback_req = DeploymentRequest {
            deployment_id: "test-dep-rollback".to_string(),
            project_id: "demo".to_string(),
            project_name: "Demo".to_string(),
            local_path: tmp.path().to_path_buf(),
            env_config,
            version: "1.0.0".to_string(),
            bump_package_json: true,
            create_git_tag_on_success: false,
            manual_password: None,
            skip_ssh_preflight: true,
            target_git_branch: None,
            rollback_commit: None,
            previous_commit: None,
            is_rollback: true,
        };
        let rollback_record = run_deployment(rollback_req, None);
        assert_eq!(rollback_record.status, DeploymentStatus::Succeeded);
        assert!(rollback_record.is_rollback);
        assert_eq!(
            crate::version::read_package_json_version(tmp.path()).as_deref(),
            Some("1.0.0")
        );
    }
}
