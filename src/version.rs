use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionBump {
    Patch,
    Minor,
    Major,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitMetadata {
    pub branch: Option<String>,
    pub commit_short: Option<String>,
    pub commit_subject: Option<String>,
}

/// Bumps a semantic version string (`1.2.3` or `v1.2.3`).
/// Falls back gracefully if the input is not strict `major.minor.patch`.
pub fn bump_semver(current: &str, bump: VersionBump) -> String {
    let trimmed = current.trim();
    let has_v = trimmed.starts_with('v') || trimmed.starts_with('V');
    let raw = trimmed.trim_start_matches(['v', 'V']);
    let core = raw.split('-').next().unwrap_or(raw);

    let mut parts = core.split('.').map(|s| s.parse::<u64>().unwrap_or(0));
    let major = parts.next().unwrap_or(0);
    let minor = parts.next().unwrap_or(0);
    let patch = parts.next().unwrap_or(0);

    let (next_major, next_minor, next_patch) = match bump {
        VersionBump::Patch => (major, minor, patch + 1),
        VersionBump::Minor => (major, minor + 1, 0),
        VersionBump::Major => (major + 1, 0, 0),
    };

    if has_v {
        format!("v{next_major}.{next_minor}.{next_patch}")
    } else {
        format!("{next_major}.{next_minor}.{next_patch}")
    }
}

/// Reads the `"version"` field from `<project_dir>/package.json` if present.
pub fn read_package_json_version(project_dir: &Path) -> Option<String> {
    let pkg_path = project_dir.join("package.json");
    let content = fs::read_to_string(pkg_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json.get("version")?
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Updates only the top-level `"version"` value in `<project_dir>/package.json`,
/// preserving the original indentation and key order.
pub fn update_package_json_version(project_dir: &Path, new_version: &str) -> Result<(), String> {
    let pkg_path = project_dir.join("package.json");
    let content = fs::read_to_string(&pkg_path)
        .map_err(|e| format!("Failed to read {}: {e}", pkg_path.display()))?;

    let clean_version = new_version.trim().trim_start_matches(['v', 'V']);
    if clean_version.is_empty() {
        return Err("Version string cannot be empty".to_string());
    }

    // Verify it is valid JSON first
    let _: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Invalid JSON in {}: {e}", pkg_path.display()))?;

    let mut replaced = false;
    let line_ending = if content.contains("\r\n") { "\r\n" } else { "\n" };
    let mut updated_lines = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim_start();
        if !replaced && trimmed.starts_with("\"version\"") {
            if let Some(colon_idx) = line.find(':') {
                let prefix = &line[..=colon_idx];
                let after_colon = line[colon_idx + 1..].trim();
                let trailing_comma = if after_colon.ends_with(',') { "," } else { "" };
                updated_lines.push(format!("{prefix} \"{clean_version}\"{trailing_comma}"));
                replaced = true;
                continue;
            }
        }
        updated_lines.push(line.to_string());
    }

    if !replaced {
        return Err(format!(
            "Could not find top-level \"version\" field in {}",
            pkg_path.display()
        ));
    }

    let mut final_output = updated_lines.join(line_ending);
    if content.ends_with('\n') {
        final_output.push_str(line_ending);
    }

    fs::write(&pkg_path, final_output)
        .map_err(|e| format!("Failed to write {}: {e}", pkg_path.display()))
}

/// Configures a `Command` to not pop up a console window on Windows.
pub fn configure_background_command(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
}

fn run_git_query(project_dir: &Path, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new("git");
    cmd.current_dir(project_dir).args(args);
    configure_background_command(&mut cmd);
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

pub fn read_git_metadata(project_dir: &Path) -> GitMetadata {
    if !project_dir.exists() {
        return GitMetadata::default();
    }
    GitMetadata {
        branch: run_git_query(project_dir, &["rev-parse", "--abbrev-ref", "HEAD"]),
        commit_short: run_git_query(project_dir, &["rev-parse", "--short", "HEAD"]),
        commit_subject: run_git_query(project_dir, &["log", "-1", "--pretty=%s"]),
    }
}

/// Lists all available Git branches (local and remote-tracking) in `project_dir`.
pub fn list_git_branches(project_dir: &Path) -> Vec<String> {
    if !project_dir.exists() {
        return Vec::new();
    }
    let mut branches = Vec::new();

    if let Some(local_raw) = run_git_query(project_dir, &["branch", "--format=%(refname:short)"]) {
        for line in local_raw.lines() {
            let b = line.trim();
            if !b.is_empty() && !b.starts_with('(') && !branches.iter().any(|x| x == b) {
                branches.push(b.to_string());
            }
        }
    }

    if let Some(remote_raw) =
        run_git_query(project_dir, &["branch", "-r", "--format=%(refname:short)"])
    {
        for line in remote_raw.lines() {
            let trimmed = line.trim();
            if trimmed.ends_with("/HEAD") || trimmed == "origin" {
                continue;
            }
            let short = trimmed.strip_prefix("origin/").unwrap_or(trimmed);
            if !short.is_empty() && !branches.iter().any(|x| x == short) {
                branches.push(short.to_string());
            }
        }
    }

    branches
}

/// Checks out a specific Git branch, tag, or commit hash in `project_dir`.
pub fn checkout_git_branch(project_dir: &Path, branch_or_ref: &str) -> Result<String, String> {
    let target = branch_or_ref.trim();
    if target.is_empty() {
        return Err("Branch or commit reference cannot be empty.".to_string());
    }
    let mut cmd = Command::new("git");
    cmd.current_dir(project_dir).args(["checkout", target]);
    configure_background_command(&mut cmd);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute git checkout: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!("git checkout '{target}' failed: {stderr}"));
    }

    Ok(format!("Checked out '{target}'"))
}

/// Reads code changes (commit messages and changed file stats) from Git for a deployment record.
/// If `previous_commit` is provided and differs from `HEAD`, returns commits in `previous_commit..HEAD`.
/// Otherwise, returns the most recent commits and diff summary on `HEAD`.
pub fn read_git_code_changes(project_dir: &Path, previous_commit: Option<&str>) -> Vec<String> {
    if !project_dir.exists() {
        return Vec::new();
    }

    let current_head = run_git_query(project_dir, &["rev-parse", "--short", "HEAD"]);
    let mut changes = Vec::new();

    if let (Some(prev), Some(head)) = (previous_commit, current_head.as_deref()) {
        let prev_clean = prev.trim();
        if !prev_clean.is_empty() && prev_clean != head {
            let range = format!("{prev_clean}..HEAD");
            if let Some(log_out) = run_git_query(
                project_dir,
                &["log", &range, "--pretty=format:%h %s (%an)", "-n", "12"],
            ) {
                for line in log_out.lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        changes.push(trimmed.to_string());
                    }
                }
            }
            if let Some(stat_out) = run_git_query(project_dir, &["diff", "--shortstat", &range]) {
                let stat = stat_out.trim();
                if !stat.is_empty() {
                    changes.push(format!("Diff summary: {stat}"));
                }
            }
            if !changes.is_empty() {
                return changes;
            }
        }
    }

    // Fallback: latest commits on current branch + latest commit diff stat
    if let Some(log_out) = run_git_query(
        project_dir,
        &["log", "-n", "3", "--pretty=format:%h %s (%an)"],
    ) {
        for line in log_out.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                changes.push(trimmed.to_string());
            }
        }
    }
    if let Some(stat_out) =
        run_git_query(project_dir, &["show", "--shortstat", "--format=", "HEAD"])
    {
        let stat = stat_out.trim();
        if !stat.is_empty() {
            changes.push(format!("Latest commit changes: {stat}"));
        }
    }

    changes
}

/// Creates or updates an annotated git tag for the deployment in `project_dir`.
pub fn create_git_tag(
    project_dir: &Path,
    version: &str,
    env_name: &str,
) -> Result<String, String> {
    let trimmed = version.trim();
    if trimmed.is_empty() {
        return Err("Version cannot be empty when creating a Git tag".to_string());
    }
    let tag_name = if trimmed.starts_with('v') || trimmed.starts_with('V') {
        trimmed.to_string()
    } else {
        format!("v{trimmed}")
    };

    let msg = format!("Shiply deployment {tag_name} ({env_name})");
    let mut cmd = Command::new("git");
    cmd.current_dir(project_dir)
        .args(["tag", "-f", "-a", &tag_name, "-m", &msg]);
    configure_background_command(&mut cmd);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute git tag: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!("git tag failed: {stderr}"));
    }

    Ok(tag_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_bump_semver() {
        assert_eq!(bump_semver("1.0.0", VersionBump::Patch), "1.0.1");
        assert_eq!(bump_semver("1.0.0", VersionBump::Minor), "1.1.0");
        assert_eq!(bump_semver("1.0.0", VersionBump::Major), "2.0.0");
        assert_eq!(bump_semver("v0.2.9", VersionBump::Patch), "v0.2.10");
        assert_eq!(bump_semver("0.0.0", VersionBump::Patch), "0.0.1");
    }

    #[test]
    fn test_read_and_update_package_json_preserves_formatting() {
        let tmp = TempDir::new().expect("tempdir");
        let pkg_path = tmp.path().join("package.json");
        let sample = "{\n    \"name\": \"ndm-dq-subscription\",\n    \"version\": \"1.0.0\",\n    \"private\": true\n}\n";
        fs::write(&pkg_path, sample).expect("write package.json");

        assert_eq!(
            read_package_json_version(tmp.path()).as_deref(),
            Some("1.0.0")
        );

        update_package_json_version(tmp.path(), "v1.2.3").expect("update version");
        assert_eq!(
            read_package_json_version(tmp.path()).as_deref(),
            Some("1.2.3")
        );

        let updated_raw = fs::read_to_string(&pkg_path).expect("read updated");
        assert!(updated_raw.contains("    \"version\": \"1.2.3\","));
        assert!(updated_raw.contains("    \"name\": \"ndm-dq-subscription\","));
    }

    #[test]
    fn test_git_branches_checkout_and_code_changes() {
        let tmp = TempDir::new().expect("tempdir");
        let dir = tmp.path();

        let run_git = |args: &[&str]| {
            let mut c = Command::new("git");
            c.current_dir(dir).args(args);
            configure_background_command(&mut c);
            let out = c.output().expect("git command");
            assert!(out.status.success(), "git {:?} failed", args);
        };

        run_git(&["init", "-b", "main"]);
        run_git(&["config", "user.email", "test@shiply.dev"]);
        run_git(&["config", "user.name", "Shiply Tester"]);

        fs::write(dir.join("app.js"), "console.log('v1');\n").expect("write v1");
        run_git(&["add", "."]);
        run_git(&["commit", "-m", "feat: initial app commit"]);

        let first_meta = read_git_metadata(dir);
        let first_commit = first_meta.commit_short.expect("first commit hash");

        run_git(&["checkout", "-b", "staging"]);
        fs::write(dir.join("app.js"), "console.log('v2');\n").expect("write v2");
        run_git(&["add", "."]);
        run_git(&["commit", "-m", "feat: update app to v2"]);

        let branches = list_git_branches(dir);
        assert!(branches.contains(&"main".to_string()));
        assert!(branches.contains(&"staging".to_string()));

        let changes = read_git_code_changes(dir, Some(&first_commit));
        assert!(!changes.is_empty());
        assert!(changes.iter().any(|c| c.contains("feat: update app to v2")));

        // Checkout back to main
        checkout_git_branch(dir, "main").expect("checkout main");
        assert_eq!(read_git_metadata(dir).branch.as_deref(), Some("main"));
    }
}
