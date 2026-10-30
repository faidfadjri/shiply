use crate::domain::{normalize_env_key, EnvironmentConfig, PipelineStep, Project, StepKind};
use std::fs;
use std::path::{Path, PathBuf};

/// Discovers all `*.ps1` deployment scripts inside a directory (sorted alphabetically, with `deploy-dev.ps1` first).
pub fn discover_ps1_scripts_in_dir(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut scripts: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("ps1"))
        })
        .collect();

    scripts.sort_by(|a, b| {
        let a_name = a
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        let b_name = b
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        let rank = |name: &str| -> u8 {
            if name.contains("dev") {
                0
            } else if name.contains("stag") || name.contains("uat") {
                1
            } else if name.contains("prod") {
                2
            } else {
                3
            }
        };
        rank(&a_name).cmp(&rank(&b_name)).then(a_name.cmp(&b_name))
    });
    scripts
}

/// Imports a full `Project` from a `.ps1` file path (or project directory path).
/// Automatically fills the project name, local path, description, and environment(s).
pub fn import_project_from_path(
    input_path: &Path,
    include_sibling_scripts: bool,
) -> Result<Project, String> {
    let (project_dir, primary_script) = if input_path.is_file() {
        let parent = input_path
            .parent()
            .ok_or_else(|| "Could not determine parent directory of script.".to_string())?;
        (parent.to_path_buf(), Some(input_path.to_path_buf()))
    } else if input_path.is_dir() {
        let discovered = discover_ps1_scripts_in_dir(input_path);
        (input_path.to_path_buf(), discovered.first().cloned())
    } else {
        return Err(format!("Path '{}' does not exist.", input_path.display()));
    };

    let mut environments = Vec::new();
    if let Some(primary) = &primary_script {
        let env_cfg = parse_ps1_file(primary)?;
        environments.push(env_cfg);
    }

    if include_sibling_scripts {
        for script in discover_ps1_scripts_in_dir(&project_dir) {
            if Some(&script) == primary_script.as_ref() {
                continue;
            }
            if let Ok(sibling_env) = parse_ps1_file(&script) {
                if !environments.iter().any(|e| e.key == sibling_env.key) {
                    environments.push(sibling_env);
                }
            }
        }
    }

    let (pkg_name, pkg_desc) = read_package_json_name_and_desc(&project_dir);
    let (go_name, go_desc) = if pkg_name.is_none() {
        read_go_mod_name_and_desc(&project_dir)
    } else {
        (None, None)
    };

    let inferred_name = pkg_name
        .or(go_name)
        .or_else(|| {
            environments.first().map(|e| {
                strip_env_suffix(&e.app_name).to_string()
            })
        })
        .or_else(|| {
            project_dir
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "imported-project".to_string());

    let inferred_desc = pkg_desc
        .or(go_desc)
        .unwrap_or_else(|| {
            if environments.iter().any(|e| {
                e.steps.iter().any(|s| {
                    matches!(&s.kind, StepKind::RemoteCommand { command } if command.contains("pm2"))
                })
            }) {
                "Backend service (Node.js / PM2)".to_string()
            } else if environments.iter().any(|e| {
                e.steps.iter().any(|s| {
                    matches!(&s.kind, StepKind::LocalCommand { command, .. } if command.contains("go build") || command.contains("go test"))
                })
            }) {
                "Backend service (Go)".to_string()
            } else {
                "Imported from PowerShell deployment script (.ps1)".to_string()
            }
        });

    let id = normalize_env_key(&inferred_name);

    Ok(Project {
        id: if id.is_empty() {
            "project".to_string()
        } else {
            id
        },
        name: inferred_name,
        description: inferred_desc,
        local_path: project_dir.to_string_lossy().to_string(),
        environments,
        group: None,
        sort_order: None,
    })
}

/// Parses a `.ps1` deployment script file into an `EnvironmentConfig`.
pub fn parse_ps1_file(script_path: &Path) -> Result<EnvironmentConfig, String> {
    let raw = fs::read_to_string(script_path)
        .map_err(|e| format!("Failed to read '{}': {e}", script_path.display()))?;
    let filename = script_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("deploy-dev");
    parse_ps1_content(&raw, filename)
}

/// Parses the text content of a PowerShell deployment script (such as `deploy-dev.ps1` or `deploy-prod.ps1`)
/// into a structured `EnvironmentConfig` with auto-populated servers, paths, credentials, and pipeline steps.
pub fn parse_ps1_content(content: &str, filename_stem: &str) -> Result<EnvironmentConfig, String> {
    let app_name = extract_ps_string_var(content, "AppName")
        .unwrap_or_else(|| "app-service-dev".to_string());

    let vault_target = extract_ps_string_var(content, "CredentialTarget")
        .or_else(|| extract_ps_string_var(content, "VaultTarget"))
        .unwrap_or_else(|| "indyserver".to_string());

    let remote_user = extract_remote_user(content).unwrap_or_else(|| "appndq".to_string());

    let servers = extract_ps_array_var(content, "Servers");
    let servers = if servers.is_empty() {
        vec!["10.62.70.80".to_string()]
    } else {
        servers
    };

    let http_proxy = extract_ps_string_var(content, "HttpProxy");
    let env_source = extract_ps_string_var(content, "EnvSource");

    let (env_key, env_name) = infer_env_key_and_name(filename_stem, &app_name, env_source.as_deref());

    let raw_remote_dir = extract_non_empty_remote_dir(content)
        .unwrap_or_else(|| "/data/indy-service/apps/$AppName".to_string());

    let remote_dir = raw_remote_dir
        .replace("$AppName", &app_name)
        .replace("${AppName}", &app_name)
        .replace("$RemoteUser", &remote_user)
        .replace("${RemoteUser}", &remote_user);

    let mut steps = Vec::new();

    // 1. Check if script runs unit tests (`npm run test` or `go test`)
    if content.contains("npm run test") {
        steps.push(PipelineStep::new(
            "Run Unit Tests",
            StepKind::LocalCommand {
                command: "npm run test".to_string(),
                allow_failure: false,
                shell: None,
            },
        ));
    } else if let Some(go_test_cmd) = extract_go_test_command(content) {
        steps.push(PipelineStep::new(
            "Run Unit Tests (go test)",
            StepKind::LocalCommand {
                command: go_test_cmd,
                allow_failure: false,
                shell: None,
            },
        ));
    }

    // 2. Check if script runs build (`npm run build` or `go build`)
    let go_build_info = extract_go_build_info(content, &app_name);
    if content.contains("npm run build") {
        let non_fatal = content.contains("deploying the existing ./dist as-is")
            || content.contains("Build FAILED - deploying");
        steps.push(PipelineStep::new(
            "Build Application",
            StepKind::LocalCommand {
                command: "npm run build".to_string(),
                allow_failure: non_fatal,
                shell: None,
            },
        ));
    } else if let Some((go_build_cmd, _)) = &go_build_info {
        let non_fatal = content.contains("Build FAILED - deploying")
            || content.contains("deploying the existing");
        steps.push(PipelineStep::new(
            "Build Go Application",
            StepKind::LocalCommand {
                command: go_build_cmd.clone(),
                allow_failure: non_fatal,
                shell: None,
            },
        ));
    }

    // 3. Check for Test-Path verification of build output
    if let Some((_, Some(bin_path))) = &go_build_info {
        let test_path = extract_test_path_target(content).unwrap_or_else(|| bin_path.clone());
        steps.push(PipelineStep::new(
            format!("Verify {test_path}"),
            StepKind::VerifyLocalPath {
                path: test_path,
            },
        ));
    } else if let Some(explicit_path) = extract_test_path_target(content) {
        steps.push(PipelineStep::new(
            format!("Verify {explicit_path}"),
            StepKind::VerifyLocalPath {
                path: explicit_path,
            },
        ));
    } else if content.contains("Test-Path './dist/server.js'")
        || content.contains("Test-Path \"./dist/server.js\"")
    {
        steps.push(PipelineStep::new(
            "Verify ./dist/server.js",
            StepKind::VerifyLocalPath {
                path: "./dist/server.js".to_string(),
            },
        ));
    } else if content.contains("Test-Path -Path \"./dist\"")
        || content.contains("Test-Path './dist'")
    {
        steps.push(PipelineStep::new(
            "Verify ./dist Directory",
            StepKind::VerifyLocalPath {
                path: "./dist".to_string(),
            },
        ));
    }

    // 4. Check for remote directory preparation / cleanup before upload
    if content.contains("! -name node_modules -exec rm -rf") {
        steps.push(PipelineStep::new(
            "Clean Stale Remote Files (Preserve node_modules)",
            StepKind::RemoteCommand {
                command: "mkdir -p {remote_dir} && find {remote_dir} -mindepth 1 -maxdepth 1 ! -name node_modules -exec rm -rf {} +".to_string(),
            },
        ));
    } else if content.contains("find $RemoteDir") || content.contains("find $remote_dir") {
        steps.push(PipelineStep::new(
            "Clean Stale Remote Files",
            StepKind::RemoteCommand {
                command: "mkdir -p {remote_dir} && find {remote_dir} -mindepth 1 -maxdepth 1 -exec rm -rf {} +".to_string(),
            },
        ));
    } else if content.contains("mkdir -p $RemoteDir") {
        steps.push(PipelineStep::new(
            "Ensure Remote Directory Exists",
            StepKind::RemoteCommand {
                command: "mkdir -p {remote_dir}".to_string(),
            },
        ));
    }

    // 5. Check for SCP file upload
    if content.contains("scp ") {
        let copy_sources = extract_ps_array_var(content, "copySources");
        if !copy_sources.is_empty() {
            let filtered_sources: Vec<String> = copy_sources
                .into_iter()
                .filter(|s| !s.starts_with('$'))
                .collect();
            let stage_env = env_source.is_some() || content.contains("EnvStagingPath");
            steps.push(PipelineStep::new(
                "Upload Artifacts & Staged .env",
                StepKind::TransferFiles {
                    sources: if filtered_sources.is_empty() {
                        vec![
                            "./dist".to_string(),
                            "./package.json".to_string(),
                            "./package-lock.json".to_string(),
                        ]
                    } else {
                        filtered_sources
                    },
                    stage_env_file: stage_env,
                },
            ));
        } else if let Some((_, Some(bin_path))) = &go_build_info {
            let stage_env = env_source.is_some() || content.contains("EnvStagingPath");
            steps.push(PipelineStep::new(
                "Upload Go Binary & Staged .env",
                StepKind::TransferFiles {
                    sources: vec![bin_path.clone()],
                    stage_env_file: stage_env,
                },
            ));
        } else if content.contains("./dist/*") {
            steps.push(PipelineStep::new(
                "Upload ./dist/* via SCP",
                StepKind::TransferFiles {
                    sources: vec!["./dist/*".to_string()],
                    stage_env_file: env_source.is_some(),
                },
            ));
        } else {
            steps.push(PipelineStep::new(
                "Upload Build Artifacts",
                StepKind::TransferFiles {
                    sources: vec!["./dist".to_string()],
                    stage_env_file: env_source.is_some(),
                },
            ));
        }
    }

    // 6. Check for dependency installation over SSH (npm ci)
    if content.contains("$Npm ci") || content.contains("npm ci") {
        let cmd = if http_proxy.is_some() {
            "export http_proxy={http_proxy} && export https_proxy={http_proxy} && cd {remote_dir} && /data/{remote_user}/nodejs/bin/npm ci".to_string()
        } else {
            "cd {remote_dir} && /data/{remote_user}/nodejs/bin/npm ci".to_string()
        };
        steps.push(PipelineStep::new(
            "Install Dependencies (npm ci via Proxy)",
            StepKind::RemoteCommand { command: cmd },
        ));
    }

    // 7. Check for service management (systemctl restart, service restart, or PM2 restart)
    if content.contains("systemctl restart") {
        let cmd = if content.contains("sudo systemctl") {
            "sudo systemctl restart {app_name}".to_string()
        } else {
            "systemctl restart {app_name}".to_string()
        };
        steps.push(PipelineStep::new(
            "Restart Service (systemctl)",
            StepKind::RemoteCommand { command: cmd },
        ));
    } else if content.contains("service ") && content.contains("restart") {
        steps.push(PipelineStep::new(
            "Restart Service",
            StepKind::RemoteCommand {
                command: "sudo service {app_name} restart".to_string(),
            },
        ));
    } else if content.contains("restart $AppName") || content.contains("pm2 restart") {
        steps.push(PipelineStep::new(
            "Restart PM2 Process",
            StepKind::RemoteCommand {
                command: "/data/{remote_user}/nodejs/bin/pm2 restart {app_name}".to_string(),
            },
        ));
    }

    if content.contains("chmod -R a+rX") {
        steps.push(PipelineStep::new(
            "Set Read & Execute Permissions (chmod)",
            StepKind::RemoteCommand {
                command: "chmod -R a+rX {remote_dir}".to_string(),
            },
        ));
    }

    if steps.is_empty() {
        if go_build_info.is_some() {
            steps.push(PipelineStep::new(
                "Build Go Application",
                StepKind::LocalCommand {
                    command: "go build -o ./bin/app".to_string(),
                    allow_failure: false,
                    shell: None,
                },
            ));
        } else {
            steps.push(PipelineStep::new(
                "Build Project",
                StepKind::LocalCommand {
                    command: "npm run build".to_string(),
                    allow_failure: false,
                    shell: None,
                },
            ));
        }
    }

    Ok(EnvironmentConfig {
        key: env_key,
        name: env_name,
        app_name,
        servers,
        remote_user,
        vault_target,
        remote_dir,
        env_source,
        http_proxy,
        steps,
    })
}

fn strip_env_suffix(app_name: &str) -> &str {
    for suffix in ["-dev", "-development", "-prod", "-production", "-staging", "-stg", "-uat", "-qa"] {
        if let Some(stripped) = app_name.strip_suffix(suffix) {
            return stripped;
        }
    }
    app_name
}

fn infer_env_key_and_name(
    filename_stem: &str,
    app_name: &str,
    env_source: Option<&str>,
) -> (String, String) {
    let stem_lower = filename_stem.to_lowercase();
    let raw_key = if let Some(rest) = stem_lower.strip_prefix("deploy-") {
        rest.to_string()
    } else if let Some(rest) = stem_lower.strip_prefix("deploy_") {
        rest.to_string()
    } else if app_name.ends_with("-dev") {
        "dev".to_string()
    } else if app_name.ends_with("-staging") || app_name.ends_with("-stg") {
        "staging".to_string()
    } else if let Some(src) = env_source {
        if src.ends_with(".dev") {
            "dev".to_string()
        } else if src.ends_with(".prod") || src.ends_with(".production") {
            "production".to_string()
        } else if src.ends_with(".staging") {
            "staging".to_string()
        } else {
            "dev".to_string()
        }
    } else {
        "production".to_string()
    };

    let normalized = normalize_env_key(&raw_key);
    match normalized.as_str() {
        "dev" | "development" => ("dev".to_string(), "Development".to_string()),
        "prod" | "production" => ("production".to_string(), "Production".to_string()),
        "staging" | "stg" | "stage" => ("staging".to_string(), "Staging".to_string()),
        "uat" => ("uat".to_string(), "UAT".to_string()),
        "qa" => ("qa".to_string(), "QA".to_string()),
        other if !other.is_empty() => {
            let mut chars = other.chars();
            let title = match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => "Custom".to_string(),
            };
            (other.to_string(), title)
        }
        _ => ("dev".to_string(), "Development".to_string()),
    }
}

fn extract_ps_string_var(content: &str, var_name: &str) -> Option<String> {
    let target = format!("${var_name}");
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if let Some(pos) = trimmed.find(&target) {
            let after_var = &trimmed[pos + target.len()..];
            let after_var_trimmed = after_var.trim_start();
            if let Some(after_eq) = after_var_trimmed.strip_prefix('=') {
                let rhs = after_eq.trim().trim_end_matches(',');
                if let Some(val) = unquote_ps_literal(rhs) {
                    if !val.is_empty() {
                        return Some(val);
                    }
                }
            }
        }
    }
    None
}

fn extract_non_empty_remote_dir(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if let Some(pos) = trimmed.find("$RemoteDir") {
            let after_var = trimmed[pos + "$RemoteDir".len()..].trim_start();
            if let Some(after_eq) = after_var.strip_prefix('=') {
                let rhs = after_eq.trim().trim_end_matches(',');
                if let Some(val) = unquote_ps_literal(rhs) {
                    if !val.is_empty() {
                        return Some(val);
                    }
                }
            }
        }
    }
    None
}

fn extract_remote_user(content: &str) -> Option<String> {
    let mut found = None;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if let Some(pos) = trimmed.find("$RemoteUser") {
            let after_var = trimmed[pos + "$RemoteUser".len()..].trim_start();
            if let Some(after_eq) = after_var.strip_prefix('=') {
                let rhs = after_eq.trim().trim_end_matches(',');
                if let Some(val) = unquote_ps_literal(rhs) {
                    if !val.is_empty() && !val.starts_with('$') {
                        found = Some(val);
                    }
                }
            }
        }
    }
    found
}

fn extract_ps_array_var(content: &str, var_name: &str) -> Vec<String> {
    let target = format!("${var_name}");
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if let Some(pos) = trimmed.find(&target) {
            let after_var = trimmed[pos + target.len()..].trim_start();
            if let Some(after_eq) = after_var.strip_prefix('=') {
                let rhs = after_eq.trim();
                if let Some(start_paren) = rhs.find("@(") {
                    let inside = &rhs[start_paren + 2..];
                    if let Some(end_paren) = inside.find(')') {
                        let items_raw = &inside[..end_paren];
                        return items_raw
                            .split(',')
                            .map(|item| {
                                let t = item.trim();
                                unquote_ps_literal(t).unwrap_or_else(|| t.to_string())
                            })
                            .filter(|s| !s.is_empty())
                            .collect();
                    }
                }
            }
        }
    }
    Vec::new()
}

fn unquote_ps_literal(raw: &str) -> Option<String> {
    let t = raw.trim();
    if ((t.starts_with('\'') && t.ends_with('\'')) || (t.starts_with('"') && t.ends_with('"')))
        && t.len() >= 2
    {
        return Some(t[1..t.len() - 1].to_string());
    }
    None
}

fn read_package_json_name_and_desc(project_dir: &Path) -> (Option<String>, Option<String>) {
    let pkg_path = project_dir.join("package.json");
    let Ok(raw) = fs::read_to_string(pkg_path) else {
        return (None, None);
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return (None, None);
    };
    let name = json
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let desc = json
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    (name, desc)
}

fn read_go_mod_name_and_desc(project_dir: &Path) -> (Option<String>, Option<String>) {
    let mod_path = project_dir.join("go.mod");
    let Ok(raw) = fs::read_to_string(mod_path) else {
        return (None, None);
    };
    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("module ") {
            let mod_path = rest.trim().trim_matches(|c| c == '\'' || c == '"');
            let name = mod_path.rsplit('/').next().unwrap_or(mod_path).trim();
            if !name.is_empty() {
                return (
                    Some(name.to_string()),
                    Some("Backend service (Go)".to_string()),
                );
            }
        }
    }
    (None, None)
}

fn extract_go_test_command(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if let Some(pos) = trimmed.find("go test") {
            let cmd_part = &trimmed[pos..];
            let end_idx = cmd_part
                .find([';', '|', '&'])
                .unwrap_or(cmd_part.len());
            let final_cmd = cmd_part[..end_idx].trim();
            if !final_cmd.is_empty() {
                return Some(final_cmd.to_string());
            }
        }
    }
    None
}

fn extract_go_build_info(content: &str, app_name: &str) -> Option<(String, Option<String>)> {
    let mut build_line = None;
    let mut env_prefixes = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if (trimmed.starts_with("$env:GOOS")
            || trimmed.starts_with("$env:GOARCH")
            || trimmed.starts_with("GOOS=")
            || trimmed.starts_with("GOARCH="))
            && !trimmed.contains("go build")
        {
            env_prefixes.push(trimmed.trim_end_matches(';').to_string());
        }
        if trimmed.contains("go build") {
            build_line = Some(trimmed.to_string());
            break;
        }
    }

    let line = build_line?;
    let raw_cmd = if let Some(idx) = line.find("go build") {
        let mut part = &line[idx..];
        if let Some(end) = part.find(';') {
            part = &part[..end];
        }
        if let Some(end) = part.find("&&") {
            part = &part[..end];
        }
        if let Some(end) = part.find('|') {
            part = &part[..end];
        }
        part.trim().to_string()
    } else {
        line
    };

    let resolved_cmd = raw_cmd
        .replace("$AppName", app_name)
        .replace("${AppName}", app_name);

    let binary_path = if let Some(pos) = resolved_cmd.find("-o ") {
        let after_o = resolved_cmd[pos + 3..].trim_start();
        let token = after_o
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_matches(|c| c == '\'' || c == '"');
        if !token.is_empty() {
            Some(token.to_string())
        } else {
            None
        }
    } else {
        None
    };

    let full_command = if !env_prefixes.is_empty() {
        format!("{}; {}", env_prefixes.join("; "), resolved_cmd)
    } else {
        resolved_cmd
    };

    Some((full_command, binary_path))
}

fn extract_test_path_target(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if let Some(pos) = trimmed.find("Test-Path") {
            let after = trimmed[pos + "Test-Path".len()..].trim_start();
            let after = after.strip_prefix("-Path ").unwrap_or(after).trim_start();
            let target = if let Some(unquoted) =
                unquote_ps_literal(after.split_whitespace().next().unwrap_or(""))
            {
                unquoted
            } else if after.starts_with('\'') || after.starts_with('"') {
                let quote = after.chars().next().unwrap();
                let inner = after[1..].split(quote).next().unwrap_or("");
                inner.to_string()
            } else {
                after.split_whitespace().next().unwrap_or("").to_string()
            };
            let cleaned = target.trim_end_matches(')').trim();
            if !cleaned.is_empty() && !cleaned.starts_with('$') {
                return Some(cleaned.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const SAMPLE_NDM_DEV_PS1: &str = r#"
$AppName = 'ndm-dq-subscription-dev'
$CredentialTarget = 'indyserver'
$RemoteUser = $null
if (-not $RemoteUser) {
    $RemoteUser = 'appndq'
}
$Servers = @('10.62.70.80', '10.62.70.81')
$Pm2 = "/data/$RemoteUser/nodejs/bin/pm2"
$Npm = "/data/$RemoteUser/nodejs/bin/npm"
$HttpProxy = "http://10.54.18.42:8080"
$RemoteDir = "/data/indy-service/apps/$AppName"
$InstallCommand = "export http_proxy=$HttpProxy && export https_proxy=$HttpProxy && cd $RemoteDir && $Npm ci"
$EnvSource = './.env.dev'

npm run build
if ($LASTEXITCODE -ne 0) {
    Write-Host "  Build FAILED - deploying the existing ./dist as-is." -ForegroundColor Yellow
}
if (-not (Test-Path './dist/server.js')) {
    throw "./dist/server.js not found"
}
$CleanCommand = "mkdir -p $RemoteDir && find $RemoteDir -mindepth 1 -maxdepth 1 ! -name node_modules -exec rm -rf {} +"
$copySources = @('./dist', './package.json', './package-lock.json', $EnvStagingPath)
scp @SshOpts -r @copySources "${RemoteUser}@${server}:$RemoteDir"
Invoke-Remote -Server $server -Command $InstallCommand
Invoke-Remote -Server $server -Command "$Pm2 restart $AppName"
"#;

    #[test]
    fn test_parse_ndm_dq_subscription_deploy_dev_ps1() {
        let env = parse_ps1_content(SAMPLE_NDM_DEV_PS1, "deploy-dev").expect("parse deploy-dev.ps1");
        assert_eq!(env.key, "dev");
        assert_eq!(env.name, "Development");
        assert_eq!(env.app_name, "ndm-dq-subscription-dev");
        assert_eq!(env.vault_target, "indyserver");
        assert_eq!(env.remote_user, "appndq");
        assert_eq!(env.servers, vec!["10.62.70.80", "10.62.70.81"]);
        assert_eq!(
            env.remote_dir,
            "/data/indy-service/apps/ndm-dq-subscription-dev"
        );
        assert_eq!(env.env_source.as_deref(), Some("./.env.dev"));
        assert_eq!(env.http_proxy.as_deref(), Some("http://10.54.18.42:8080"));
        assert_eq!(env.steps.len(), 6);

        assert!(matches!(
            &env.steps[0].kind,
            StepKind::LocalCommand {
                command,
                allow_failure: true,
                ..
            } if command == "npm run build"
        ));
        assert!(matches!(
            &env.steps[1].kind,
            StepKind::VerifyLocalPath { path } if path == "./dist/server.js"
        ));
        assert!(matches!(
            &env.steps[3].kind,
            StepKind::TransferFiles { stage_env_file: true, .. }
        ));
    }

    #[test]
    fn test_import_project_from_ps1_file_auto_fills_project_and_environments() {
        let tmp = TempDir::new().expect("tempdir");
        let ps1_path = tmp.path().join("deploy-dev.ps1");
        fs::write(&ps1_path, SAMPLE_NDM_DEV_PS1).expect("write deploy-dev.ps1");
        fs::write(
            tmp.path().join("package.json"),
            r#"{"name": "ndm-dq-subscription", "version": "1.0.0"}"#,
        )
        .expect("write package.json");

        let proj = import_project_from_path(&ps1_path, true).expect("import project");
        assert_eq!(proj.id, "ndm-dq-subscription");
        assert_eq!(proj.name, "ndm-dq-subscription");
        assert_eq!(proj.environments.len(), 1);
        assert_eq!(proj.environments[0].key, "dev");
        assert_eq!(proj.environments[0].app_name, "ndm-dq-subscription-dev");
    }

    const SAMPLE_GO_DEPLOY_PS1: &str = r#"
$AppName = 'auth-service-dev'
$CredentialTarget = 'indyserver'
$RemoteUser = 'appndq'
$Servers = @('10.62.70.80')
$RemoteDir = "/data/indy-service/apps/$AppName"
$EnvSource = './.env.dev'

go test -v ./...
if ($LASTEXITCODE -ne 0) {
    throw "Tests failed"
}

$env:GOOS = "linux"
$env:GOARCH = "amd64"
go build -o ./bin/auth-service-dev ./cmd/server
if (-not (Test-Path './bin/auth-service-dev')) {
    throw "Binary missing"
}

scp @SshOpts ./bin/auth-service-dev "${RemoteUser}@${server}:$RemoteDir"
Invoke-Remote -Server $server -Command "systemctl restart auth-service"
"#;

    #[test]
    fn test_parse_go_project_ps1_content() {
        let env = parse_ps1_content(SAMPLE_GO_DEPLOY_PS1, "deploy-dev")
            .expect("parse Go deploy-dev.ps1");
        assert_eq!(env.key, "dev");
        assert_eq!(env.app_name, "auth-service-dev");
        assert_eq!(env.remote_user, "appndq");
        assert_eq!(env.servers, vec!["10.62.70.80"]);
        assert_eq!(env.remote_dir, "/data/indy-service/apps/auth-service-dev");
        assert_eq!(env.env_source.as_deref(), Some("./.env.dev"));
        assert_eq!(env.steps.len(), 5);

        // Step 0: go test
        assert!(matches!(
            &env.steps[0].kind,
            StepKind::LocalCommand {
                command,
                allow_failure: false,
                ..
            } if command == "go test -v ./..."
        ));

        // Step 1: go build with env vars
        assert!(matches!(
            &env.steps[1].kind,
            StepKind::LocalCommand {
                command,
                allow_failure: false,
                ..
            } if command == "$env:GOOS = \"linux\"; $env:GOARCH = \"amd64\"; go build -o ./bin/auth-service-dev ./cmd/server"
        ));

        // Step 2: verify binary path
        assert!(matches!(
            &env.steps[2].kind,
            StepKind::VerifyLocalPath { path } if path == "./bin/auth-service-dev"
        ));

        // Step 3: transfer files
        assert!(matches!(
            &env.steps[3].kind,
            StepKind::TransferFiles { .. }
        ));

        // Step 4: systemctl restart
        assert!(matches!(
            &env.steps[4].kind,
            StepKind::RemoteCommand { command } if command == "systemctl restart {app_name}"
        ));
    }

    #[test]
    fn test_import_go_project_from_path_with_go_mod() {
        let tmp = TempDir::new().expect("tempdir");
        let ps1_path = tmp.path().join("deploy.ps1");
        fs::write(&ps1_path, SAMPLE_GO_DEPLOY_PS1).expect("write deploy.ps1");
        fs::write(
            tmp.path().join("go.mod"),
            "module github.com/user/auth-service\n\ngo 1.22\n",
        )
        .expect("write go.mod");

        let proj = import_project_from_path(&ps1_path, true).expect("import Go project");
        assert_eq!(proj.id, "auth-service");
        assert_eq!(proj.name, "auth-service");
        assert_eq!(proj.description, "Backend service (Go)");
        assert_eq!(proj.environments.len(), 1);
        assert_eq!(proj.environments[0].key, "dev");
    }
}
