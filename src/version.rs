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
}
