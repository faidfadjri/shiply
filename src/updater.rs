use crate::installer::{default_install_dir, install_binary_to_dir, installed_exe_name, APP_VERSION};
use crate::version::configure_background_command;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

/// GitHub `<OWNER>/<REPO>` extracted from `.git/config` (`https://github.com/faidfadjri/shiply.git`).
pub const GITHUB_REPO: &str = "faidfadjri/shiply";

/// GitHub Releases API endpoint for fetching the latest published release.
pub const GITHUB_RELEASES_LATEST_URL: &str =
    "https://api.github.com/repos/faidfadjri/shiply/releases/latest";

/// Public GitHub Releases page URL for browser fallback.
pub const GITHUB_RELEASES_PAGE_URL: &str = "https://github.com/faidfadjri/shiply/releases";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub version: String,
    pub name: String,
    pub body: String,
    pub published_at: String,
    pub html_url: String,
    pub download_url: String,
    pub asset_name: String,
    pub asset_size: u64,
}

#[derive(Debug, Clone)]
pub enum UpdaterEvent {
    Checking,
    UpdateAvailable {
        release: ReleaseInfo,
        manual: bool,
    },
    UpToDate {
        current_version: String,
        manual: bool,
    },
    CheckFailed {
        error: String,
        manual: bool,
    },
    DownloadProgress {
        downloaded_bytes: u64,
        total_bytes: u64,
        fraction: f32,
        status_text: String,
    },
    ReadyToRestart {
        new_version: String,
        exe_path: PathBuf,
    },
    UpdateFailed {
        error: String,
    },
}

#[derive(Debug, Deserialize)]
struct GitHubReleasePayload {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<GitHubAssetPayload>,
}

#[derive(Debug, Clone, Deserialize)]
struct GitHubAssetPayload {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

/// Normalizes a version or tag string by stripping a leading `v`/`V` and whitespace.
pub fn normalize_version_tag(raw: &str) -> String {
    raw.trim()
        .trim_start_matches(['v', 'V'])
        .trim()
        .to_string()
}

/// Parses a semantic version string (`major.minor.patch[-prerelease]`) into comparable components.
fn parse_semver_tuple(raw: &str) -> Option<(u64, u64, u64, Option<String>)> {
    let cleaned = normalize_version_tag(raw);
    if cleaned.is_empty() {
        return None;
    }

    // Strip build metadata (+...)
    let without_build = cleaned.split('+').next().unwrap_or(&cleaned);
    let (core, prerelease) = match without_build.split_once('-') {
        Some((c, pre)) => (c, Some(pre.to_string())),
        None => (without_build, None),
    };

    let mut parts = core.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next().unwrap_or("0").parse::<u64>().ok()?;
    let patch = parts.next().unwrap_or("0").parse::<u64>().ok()?;

    Some((major, minor, patch, prerelease))
}

/// Returns `true` if `candidate` is strictly newer than `current` according to SemVer rules.
pub fn is_newer_semver(current: &str, candidate: &str) -> bool {
    let Some((cur_maj, cur_min, cur_pat, cur_pre)) = parse_semver_tuple(current) else {
        return false;
    };
    let Some((can_maj, can_min, can_pat, can_pre)) = parse_semver_tuple(candidate) else {
        return false;
    };

    match (can_maj, can_min, can_pat).cmp(&(cur_maj, cur_min, cur_pat)) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Equal => {
            // Stable release (None) is newer than a pre-release (Some) of the same core version
            match (&cur_pre, &can_pre) {
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (Some(a), Some(b)) => b > a,
                (None, None) => false,
            }
        }
    }
}

/// Selects the best matching release asset for the target OS (`windows`, `macos`, `linux`).
fn select_platform_asset<'a>(
    assets: &'a [GitHubAssetPayload],
    target_os: &str,
) -> Option<&'a GitHubAssetPayload> {
    if assets.is_empty() {
        return None;
    }

    let is_metadata = |name_lower: &str| {
        name_lower.ends_with(".json")
            || name_lower.ends_with(".sha256")
            || name_lower.ends_with(".sig")
            || name_lower.ends_with(".txt")
            || name_lower.ends_with(".ico")
    };

    match target_os {
        "windows" => {
            // 1. Prefer direct shiply.exe or shiply-windows*.exe binary for seamless in-place update
            if let Some(a) = assets.iter().find(|a| {
                let n = a.name.to_lowercase();
                (n == "shiply.exe"
                    || (n.contains("shiply") && n.contains("windows") && n.ends_with(".exe")))
                    && !n.contains("setup")
            }) {
                return Some(a);
            }
            // 2. Any .exe asset (including setup.exe since setup.exe and shiply.exe share the same binary!)
            if let Some(a) = assets
                .iter()
                .find(|a| a.name.to_lowercase().ends_with(".exe"))
            {
                return Some(a);
            }
        }
        "macos" => {
            if let Some(a) = assets.iter().find(|a| {
                let n = a.name.to_lowercase();
                !is_metadata(&n) && (n.contains("macos") || n.contains("darwin") || n.contains("mac"))
            }) {
                return Some(a);
            }
        }
        "linux" => {
            if let Some(a) = assets.iter().find(|a| {
                let n = a.name.to_lowercase();
                !is_metadata(&n) && (n.contains("linux") || n == "shiply")
            }) {
                return Some(a);
            }
        }
        _ => {}
    }

    assets
        .iter()
        .find(|a| !is_metadata(&a.name.to_lowercase()))
}

/// Parses a GitHub `/releases/latest` JSON payload and compares it against `current_version`.
/// Returns `Ok(Some(ReleaseInfo))` when a newer version is available, or `Ok(None)` when up to date.
pub fn parse_github_release_json(
    raw_json: &str,
    current_version: &str,
) -> Result<Option<ReleaseInfo>, String> {
    parse_github_release_json_for_os(raw_json, current_version, env::consts::OS)
}

pub fn parse_github_release_json_for_os(
    raw_json: &str,
    current_version: &str,
    target_os: &str,
) -> Result<Option<ReleaseInfo>, String> {
    let trimmed = raw_json.trim();
    if trimmed.is_empty() {
        return Err("Empty response from GitHub Releases API.".to_string());
    }

    let payload: GitHubReleasePayload = serde_json::from_str(trimmed)
        .map_err(|e| format!("Failed to parse GitHub Release JSON: {e}"))?;

    if payload.draft {
        return Ok(None);
    }

    let candidate_version = normalize_version_tag(&payload.tag_name);
    if !is_newer_semver(current_version, &candidate_version) {
        return Ok(None);
    }

    let html_url = payload
        .html_url
        .unwrap_or_else(|| GITHUB_RELEASES_PAGE_URL.to_string());

    let (download_url, asset_name, asset_size) =
        if let Some(asset) = select_platform_asset(&payload.assets, target_os) {
            (
                asset.browser_download_url.clone(),
                asset.name.clone(),
                asset.size,
            )
        } else {
            (String::new(), String::new(), 0)
        };

    let name = payload
        .name
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| format!("Shiply v{candidate_version}"));

    let body = payload
        .body
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            format!("Release v{candidate_version} is available on GitHub Releases.")
        });

    let published_at = payload
        .published_at
        .unwrap_or_else(|| "Recently published".to_string());

    Ok(Some(ReleaseInfo {
        tag_name: payload.tag_name,
        version: candidate_version,
        name,
        body,
        published_at,
        html_url,
        download_url,
        asset_name,
        asset_size,
    }))
}

/// Fetches the latest GitHub Release JSON using `curl` (with PowerShell fallback on Windows).
fn fetch_latest_release_json() -> Result<String, String> {
    let mut cmd = Command::new("curl");
    cmd.args([
        "-sSL",
        "--max-time",
        "12",
        "-H",
        "Accept: application/vnd.github+json",
        "-H",
        "User-Agent: Shiply-Desktop-Updater",
        GITHUB_RELEASES_LATEST_URL,
    ]);
    configure_background_command(&mut cmd);

    if let Ok(output) = cmd.output() {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout).to_string();
            if text.contains("\"tag_name\"") {
                return Ok(text);
            }
        }
    }

    #[cfg(windows)]
    {
        let ps_script = format!(
            "$ProgressPreference = 'SilentlyContinue'; \
             [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; \
             $resp = Invoke-WebRequest -Uri '{}' -Headers @{{'User-Agent'='Shiply-Desktop-Updater'; 'Accept'='application/vnd.github+json'}} -UseBasicParsing -TimeoutSec 12; \
             $resp.Content",
            GITHUB_RELEASES_LATEST_URL
        );
        let mut ps = Command::new("powershell");
        ps.args(["-NoProfile", "-NonInteractive", "-Command", &ps_script]);
        configure_background_command(&mut ps);
        if let Ok(output) = ps.output() {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout).to_string();
                if text.contains("\"tag_name\"") {
                    return Ok(text);
                }
            }
        }
    }

    Err(format!(
        "Could not reach GitHub Releases for {GITHUB_REPO} (check network or verify a release is published)."
    ))
}

/// Spawns a non-blocking background thread to check GitHub Releases against `CARGO_PKG_VERSION`.
pub fn spawn_update_check(tx: Sender<UpdaterEvent>, manual: bool) {
    let _ = tx.send(UpdaterEvent::Checking);
    thread::spawn(move || {
        match fetch_latest_release_json() {
            Ok(json) => match parse_github_release_json(&json, APP_VERSION) {
                Ok(Some(release)) => {
                    let _ = tx.send(UpdaterEvent::UpdateAvailable { release, manual });
                }
                Ok(None) => {
                    let _ = tx.send(UpdaterEvent::UpToDate {
                        current_version: APP_VERSION.to_string(),
                        manual,
                    });
                }
                Err(err) => {
                    let _ = tx.send(UpdaterEvent::CheckFailed {
                        error: err,
                        manual,
                    });
                }
            },
            Err(err) => {
                let _ = tx.send(UpdaterEvent::CheckFailed {
                    error: err,
                    manual,
                });
            }
        }
    });
}

/// Replaces `target_exe` with `new_binary_path` safely on all platforms (including Windows where
/// a running `.exe` cannot be overwritten directly, but CAN be renamed to `.old` while running).
pub fn replace_executable_atomic(
    new_binary_path: &Path,
    target_exe: &Path,
) -> Result<(), String> {
    if !new_binary_path.is_file() {
        return Err(format!(
            "Downloaded update binary '{}' does not exist.",
            new_binary_path.display()
        ));
    }

    if let Some(parent) = target_exe.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let backup_exe = target_exe.with_extension("exe.old");
    if backup_exe.exists() {
        let _ = fs::remove_file(&backup_exe);
    }

    if target_exe.exists() {
        fs::rename(target_exe, &backup_exe).map_err(|e| {
            format!(
                "Failed to move running binary '{}' to '{}': {e}",
                target_exe.display(),
                backup_exe.display()
            )
        })?;
    }

    if let Err(err) = fs::copy(new_binary_path, target_exe) {
        // Restore backup if copy failed
        if backup_exe.exists() && !target_exe.exists() {
            let _ = fs::rename(&backup_exe, target_exe);
        }
        return Err(format!(
            "Failed to write updated binary to '{}': {err}",
            target_exe.display()
        ));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(target_exe) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(target_exe, perms);
        }
    }

    // Attempt to remove backup (on Windows this may succeed on next launch if still mapped)
    let _ = fs::remove_file(&backup_exe);
    Ok(())
}

/// Downloads the release binary in a background thread, streams progress updates (`0.0..=1.0`),
/// replaces the current executable and installed copy, and notifies the UI when ready to restart.
pub fn spawn_download_and_apply_update(release: ReleaseInfo, tx: Sender<UpdaterEvent>) {
    thread::spawn(move || {
        if release.download_url.trim().is_empty() {
            let _ = tx.send(UpdaterEvent::UpdateFailed {
                error: format!(
                    "Release {} does not contain a binary asset for {}. Opening release page instead.",
                    release.tag_name,
                    env::consts::OS
                ),
            });
            return;
        }

        let staging_dir = env::temp_dir().join("shiply-updater");
        if let Err(e) = fs::create_dir_all(&staging_dir) {
            let _ = tx.send(UpdaterEvent::UpdateFailed {
                error: format!("Could not create temp update folder: {e}"),
            });
            return;
        }

        let temp_file = staging_dir.join(format!(
            "shiply-update-{}-{}",
            release.version,
            installed_exe_name()
        ));
        let _ = fs::remove_file(&temp_file);

        let expected_total = release.asset_size;
        let _ = tx.send(UpdaterEvent::DownloadProgress {
            downloaded_bytes: 0,
            total_bytes: expected_total,
            fraction: 0.04,
            status_text: format!("Connecting to GitHub Releases ({})...", release.tag_name),
        });

        let mut cmd = Command::new("curl");
        cmd.args([
            "-sSL",
            "--fail",
            "-H",
            "User-Agent: Shiply-Desktop-Updater",
            "-o",
        ]);
        cmd.arg(&temp_file);
        cmd.arg(&release.download_url);
        configure_background_command(&mut cmd);

        match cmd.spawn() {
            Ok(mut child) => {
                let mut tick: u32 = 0;
                loop {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            if !status.success() {
                                let _ = tx.send(UpdaterEvent::UpdateFailed {
                                    error: format!(
                                        "Download failed with status {status} from {}",
                                        release.download_url
                                    ),
                                });
                                return;
                            }
                            break;
                        }
                        Ok(None) => {
                            let current_bytes =
                                fs::metadata(&temp_file).map(|m| m.len()).unwrap_or(0);
                            let fraction = if expected_total > 0 {
                                ((current_bytes as f32) / (expected_total as f32)).clamp(0.05, 0.92)
                            } else {
                                (0.10 + (tick as f32 * 0.03)).min(0.88)
                            };
                            let mb_done = current_bytes as f64 / (1024.0 * 1024.0);
                            let status_text = if expected_total > 0 {
                                let mb_total = expected_total as f64 / (1024.0 * 1024.0);
                                format!(
                                    "Downloading {} ({mb_done:.2} MB / {mb_total:.2} MB)...",
                                    release.asset_name
                                )
                            } else {
                                format!("Downloading {} ({mb_done:.2} MB)...", release.asset_name)
                            };
                            let _ = tx.send(UpdaterEvent::DownloadProgress {
                                downloaded_bytes: current_bytes,
                                total_bytes: expected_total,
                                fraction,
                                status_text,
                            });
                            tick = tick.saturating_add(1);
                            thread::sleep(Duration::from_millis(80));
                        }
                        Err(e) => {
                            let _ = tx.send(UpdaterEvent::UpdateFailed {
                                error: format!("Error waiting for download process: {e}"),
                            });
                            return;
                        }
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(UpdaterEvent::UpdateFailed {
                    error: format!("Failed to start downloader: {e}"),
                });
                return;
            }
        }

        let final_size = fs::metadata(&temp_file).map(|m| m.len()).unwrap_or(0);
        if final_size < 1024 {
            let _ = tx.send(UpdaterEvent::UpdateFailed {
                error: "Downloaded update binary is empty or corrupted.".to_string(),
            });
            return;
        }

        let _ = tx.send(UpdaterEvent::DownloadProgress {
            downloaded_bytes: final_size,
            total_bytes: expected_total.max(final_size),
            fraction: 0.95,
            status_text: format!("Installing Shiply v{}...", release.version),
        });

        let current_exe = match env::current_exe() {
            Ok(p) => p,
            Err(e) => {
                let _ = tx.send(UpdaterEvent::UpdateFailed {
                    error: format!("Could not resolve current executable path: {e}"),
                });
                return;
            }
        };

        // 1. Replace current running executable atomically
        if let Err(err) = replace_executable_atomic(&temp_file, &current_exe) {
            let _ = tx.send(UpdaterEvent::UpdateFailed { error: err });
            return;
        }

        // 2. Also update the user's standard installation directory if different
        let install_dir = default_install_dir();
        let _ = install_binary_to_dir(&temp_file, &install_dir, &release.version);
        let _ = fs::remove_file(&temp_file);

        let _ = tx.send(UpdaterEvent::DownloadProgress {
            downloaded_bytes: final_size,
            total_bytes: expected_total.max(final_size),
            fraction: 1.0,
            status_text: format!("Shiply v{} installed! Restarting...", release.version),
        });

        let _ = tx.send(UpdaterEvent::ReadyToRestart {
            new_version: release.version,
            exe_path: current_exe,
        });
    });
}

/// Spawns a fresh instance of the updated executable and requests the current process to exit.
pub fn restart_application(exe_path: &Path) -> Result<(), String> {
    let mut cmd = Command::new(exe_path);
    if let Some(parent) = exe_path.parent() {
        cmd.current_dir(parent);
    }
    cmd.spawn()
        .map_err(|e| format!("Failed to relaunch '{}': {e}", exe_path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_is_newer_semver_comparison() {
        assert!(is_newer_semver("0.3.0", "0.3.1"));
        assert!(is_newer_semver("v0.3.0", "v0.4.0"));
        assert!(is_newer_semver("0.3.0", "1.0.0"));
        assert!(is_newer_semver("0.3.0-beta.1", "0.3.0"));
        assert!(!is_newer_semver("0.3.0", "0.3.0"));
        assert!(!is_newer_semver("0.3.0", "0.2.9"));
        assert!(!is_newer_semver("0.3.0", "0.3.0-alpha.1"));
    }

    #[test]
    fn test_parse_github_release_json_detects_newer_version_and_platform_asset() {
        let sample_json = r####"{
            "tag_name": "v0.4.0",
            "name": "Shiply v0.4.0 — Auto-Update Release",
            "body": "### What's New\n- Added GitHub Releases in-app auto-updater\n- Centered Setup dialog",
            "published_at": "2026-10-26T08:00:00Z",
            "html_url": "https://github.com/faidfadjri/shiply/releases/tag/v0.4.0",
            "draft": false,
            "prerelease": false,
            "assets": [
                {
                    "name": "latest.json",
                    "browser_download_url": "https://github.com/faidfadjri/shiply/releases/download/v0.4.0/latest.json",
                    "size": 512
                },
                {
                    "name": "shiply-linux-x86_64",
                    "browser_download_url": "https://github.com/faidfadjri/shiply/releases/download/v0.4.0/shiply-linux-x86_64",
                    "size": 8400000
                },
                {
                    "name": "shiply.exe",
                    "browser_download_url": "https://github.com/faidfadjri/shiply/releases/download/v0.4.0/shiply.exe",
                    "size": 9200000
                }
            ]
        }"####;

        let win_rel = parse_github_release_json_for_os(sample_json, "0.3.0", "windows")
            .expect("valid json")
            .expect("should detect update");
        assert_eq!(win_rel.tag_name, "v0.4.0");
        assert_eq!(win_rel.version, "0.4.0");
        assert_eq!(win_rel.asset_name, "shiply.exe");
        assert_eq!(win_rel.asset_size, 9_200_000);
        assert!(win_rel.body.contains("Added GitHub Releases"));

        let linux_rel = parse_github_release_json_for_os(sample_json, "0.3.0", "linux")
            .expect("valid json")
            .expect("should detect update");
        assert_eq!(linux_rel.asset_name, "shiply-linux-x86_64");

        // Same or newer current version returns None
        let up_to_date =
            parse_github_release_json_for_os(sample_json, "0.4.0", "windows").expect("valid json");
        assert!(up_to_date.is_none());
    }

    #[test]
    fn test_replace_executable_atomic() {
        let tmp = TempDir::new().expect("tempdir");
        let running_exe = tmp.path().join("shiply.exe");
        let downloaded_exe = tmp.path().join("shiply-update-0.4.0.exe");

        fs::write(&running_exe, b"old-binary-v0.3.0").expect("write old");
        fs::write(&downloaded_exe, b"new-binary-v0.4.0").expect("write new");

        replace_executable_atomic(&downloaded_exe, &running_exe).expect("atomic replace");
        assert_eq!(
            fs::read(&running_exe).expect("read updated"),
            b"new-binary-v0.4.0"
        );
    }
}
