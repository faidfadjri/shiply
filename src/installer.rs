use crate::version::configure_background_command;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallState {
    NotInstalled,
    InstalledCurrent {
        exe_path: PathBuf,
        version: String,
    },
    UpdateReady {
        exe_path: PathBuf,
        installed_version: String,
        running_version: String,
    },
}

pub fn default_install_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(local_app_data) = dirs::data_local_dir() {
            return local_app_data.join("Programs").join("Shiply");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs::home_dir() {
            return home.join("Applications").join("Shiply");
        }
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        if let Some(home) = dirs::home_dir() {
            return home.join(".local").join("bin");
        }
    }
    PathBuf::from("shiply-install")
}

pub fn installed_exe_name() -> &'static str {
    #[cfg(windows)]
    {
        "shiply.exe"
    }
    #[cfg(not(windows))]
    {
        "shiply"
    }
}

pub fn check_install_state(install_dir: &Path, running_version: &str) -> InstallState {
    let exe_path = install_dir.join(installed_exe_name());
    if !exe_path.is_file() {
        return InstallState::NotInstalled;
    }

    let version_file = install_dir.join("version.txt");
    let installed_version = fs::read_to_string(&version_file)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "0.0.0".to_string());

    if installed_version == running_version {
        InstallState::InstalledCurrent {
            exe_path,
            version: installed_version,
        }
    } else {
        InstallState::UpdateReady {
            exe_path,
            installed_version,
            running_version: running_version.to_string(),
        }
    }
}

/// Copies `source_exe` into `install_dir/shiply(.exe)`, safely replacing any existing version
/// (even if locked on Windows by renaming the old binary to `.old` first) and records `version.txt`.
pub fn install_binary_to_dir(
    source_exe: &Path,
    install_dir: &Path,
    version: &str,
) -> Result<PathBuf, String> {
    fs::create_dir_all(install_dir)
        .map_err(|e| format!("Failed to create install directory '{}': {e}", install_dir.display()))?;

    let target_exe = install_dir.join(installed_exe_name());
    let backup_exe = install_dir.join(format!("{}.old", installed_exe_name()));

    // Clean up any leftover .old binary from a previous update
    if backup_exe.exists() {
        let _ = fs::remove_file(&backup_exe);
    }

    let same_file = source_exe
        .canonicalize()
        .ok()
        .zip(target_exe.canonicalize().ok())
        .is_some_and(|(a, b)| a == b);

    if !same_file {
        if target_exe.exists() {
            // Move existing executable to .old so Windows allows overwriting even if an instance was running
            let _ = fs::rename(&target_exe, &backup_exe);
        }

        fs::copy(source_exe, &target_exe).map_err(|e| {
            // Restore backup if copy failed
            if backup_exe.exists() && !target_exe.exists() {
                let _ = fs::rename(&backup_exe, &target_exe);
            }
            format!(
                "Failed to copy '{}' to '{}': {e}",
                source_exe.display(),
                target_exe.display()
            )
        })?;

        let _ = fs::remove_file(&backup_exe);
    }

    let version_file = install_dir.join("version.txt");
    fs::write(&version_file, version.trim())
        .map_err(|e| format!("Failed to write version metadata: {e}"))?;

    Ok(target_exe)
}

/// Installs or updates the current running `shiply.exe` into the user's system install directory
/// and creates Start Menu & Desktop shortcuts on Windows.
pub fn install_or_update_self() -> Result<String, String> {
    let current_exe = env::current_exe()
        .map_err(|e| format!("Could not determine current executable path: {e}"))?;
    let install_dir = default_install_dir();
    let target_exe = install_binary_to_dir(&current_exe, &install_dir, APP_VERSION)?;

    #[cfg(windows)]
    {
        let _ = create_windows_shortcuts(&target_exe, &install_dir);
    }

    Ok(format!(
        "Shiply v{APP_VERSION} installed/updated at {}",
        target_exe.display()
    ))
}

#[cfg(windows)]
fn create_windows_shortcuts(target_exe: &Path, working_dir: &Path) -> Result<(), String> {
    let target_str = target_exe.to_string_lossy().replace('\'', "''");
    let work_str = working_dir.to_string_lossy().replace('\'', "''");

    let script = format!(
        "$ws = New-Object -ComObject WScript.Shell; \
         $paths = @(\
             [IO.Path]::Combine($env:APPDATA, 'Microsoft\\Windows\\Start Menu\\Programs\\Shiply.lnk'), \
             [IO.Path]::Combine([Environment]::GetFolderPath('Desktop'), 'Shiply.lnk')\
         ); \
         foreach ($p in $paths) {{ \
             $s = $ws.CreateShortcut($p); \
             $s.TargetPath = '{target_str}'; \
             $s.WorkingDirectory = '{work_str}'; \
             $s.Description = 'Shiply - Mini Deployment Platform'; \
             $s.Save(); \
         }}"
    );

    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
    configure_background_command(&mut cmd);
    let _ = cmd.output();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_install_and_auto_update_over_existing_version() {
        let tmp = TempDir::new().expect("tempdir");
        let src_dir = tmp.path().join("src_build");
        let install_dir = tmp.path().join("installed_app");
        fs::create_dir_all(&src_dir).expect("create src_dir");

        let src_exe = src_dir.join(installed_exe_name());
        fs::write(&src_exe, b"binary-v0.1.0").expect("write v0.1.0");

        // 1. Before install: NotInstalled
        assert_eq!(
            check_install_state(&install_dir, "0.1.0"),
            InstallState::NotInstalled
        );

        // 2. First install (v0.1.0)
        let installed_path =
            install_binary_to_dir(&src_exe, &install_dir, "0.1.0").expect("first install");
        assert!(installed_path.exists());
        assert_eq!(
            check_install_state(&install_dir, "0.1.0"),
            InstallState::InstalledCurrent {
                exe_path: installed_path.clone(),
                version: "0.1.0".to_string(),
            }
        );

        // 3. When running a newer version (v0.2.0), state reports UpdateReady
        assert_eq!(
            check_install_state(&install_dir, "0.2.0"),
            InstallState::UpdateReady {
                exe_path: installed_path.clone(),
                installed_version: "0.1.0".to_string(),
                running_version: "0.2.0".to_string(),
            }
        );

        // 4. Reinstall/upgrade with v0.2.0 overwrites binary and updates version.txt
        fs::write(&src_exe, b"binary-v0.2.0-updated").expect("write v0.2.0");
        let updated_path =
            install_binary_to_dir(&src_exe, &install_dir, "0.2.0").expect("upgrade install");
        assert_eq!(
            fs::read(&updated_path).expect("read updated binary"),
            b"binary-v0.2.0-updated"
        );
        assert_eq!(
            check_install_state(&install_dir, "0.2.0"),
            InstallState::InstalledCurrent {
                exe_path: updated_path,
                version: "0.2.0".to_string(),
            }
        );
    }
}
