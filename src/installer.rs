use crate::version::configure_background_command;
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Layout, Margin, Pos2, Rect, RichText,
    Sense, Stroke, Vec2,
};
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

/// Returns `true` if the current binary is named `setup.exe` (or `shiply-setup.exe`) or `--setup` was passed.
pub fn is_setup_mode(args: &[String]) -> bool {
    if args.iter().any(|a| a == "--setup" || a == "setup") {
        return true;
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(stem) = exe.file_stem().and_then(|s| s.to_str()) {
            let lower = stem.to_lowercase();
            if lower == "setup" || lower.starts_with("shiply-setup") || lower.starts_with("shiply_setup") {
                return true;
            }
        }
    }
    false
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
    fs::create_dir_all(install_dir).map_err(|e| {
        format!(
            "Failed to create install directory '{}': {e}",
            install_dir.display()
        )
    })?;

    let target_exe = install_dir.join(installed_exe_name());
    let backup_exe = install_dir.join(format!("{}.old", installed_exe_name()));

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
            let _ = fs::rename(&target_exe, &backup_exe);
        }

        fs::copy(source_exe, &target_exe).map_err(|e| {
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
    install_with_options(&default_install_dir(), true, true)
}

pub fn install_with_options(
    install_dir: &Path,
    desktop_shortcut: bool,
    start_menu_shortcut: bool,
) -> Result<String, String> {
    let current_exe = env::current_exe()
        .map_err(|e| format!("Could not determine current executable path: {e}"))?;
    let target_exe = install_binary_to_dir(&current_exe, install_dir, APP_VERSION)?;

    #[cfg(windows)]
    {
        if desktop_shortcut || start_menu_shortcut {
            let _ = create_windows_shortcuts(
                &target_exe,
                install_dir,
                desktop_shortcut,
                start_menu_shortcut,
            );
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (desktop_shortcut, start_menu_shortcut);
    }

    Ok(format!(
        "Shiply v{APP_VERSION} installed at {}",
        target_exe.display()
    ))
}

#[cfg(windows)]
fn create_windows_shortcuts(
    target_exe: &Path,
    working_dir: &Path,
    desktop: bool,
    start_menu: bool,
) -> Result<(), String> {
    let target_str = target_exe.to_string_lossy().replace('\'', "''");
    let work_str = working_dir.to_string_lossy().replace('\'', "''");

    let mut path_exprs = Vec::new();
    if start_menu {
        path_exprs.push("[IO.Path]::Combine($env:APPDATA, 'Microsoft\\Windows\\Start Menu\\Programs\\Shiply.lnk')");
    }
    if desktop {
        path_exprs.push("[IO.Path]::Combine([Environment]::GetFolderPath('Desktop'), 'Shiply.lnk')");
    }
    if path_exprs.is_empty() {
        return Ok(());
    }

    let script = format!(
        "$ws = New-Object -ComObject WScript.Shell; \
         $paths = @({}); \
         foreach ($p in $paths) {{ \
             $s = $ws.CreateShortcut($p); \
             $s.TargetPath = '{target_str}'; \
             $s.WorkingDirectory = '{work_str}'; \
             $s.Description = 'Shiply - Mini Deployment Platform'; \
             $s.Save(); \
         }}",
        path_exprs.join(", ")
    );

    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
    configure_background_command(&mut cmd);
    let _ = cmd.output();
    Ok(())
}

/// Native Graphical Desktop Setup Wizard (`setup.exe`)
pub struct SetupWizardApp {
    install_dir_input: String,
    create_desktop_shortcut: bool,
    create_start_menu_shortcut: bool,
    launch_after_install: bool,
    status_message: Option<(String, bool)>,
    installed_successfully: bool,
}

impl Default for SetupWizardApp {
    fn default() -> Self {
        Self {
            install_dir_input: default_install_dir().to_string_lossy().to_string(),
            create_desktop_shortcut: true,
            create_start_menu_shortcut: true,
            launch_after_install: true,
            status_message: None,
            installed_successfully: false,
        }
    }
}

impl eframe::App for SetupWizardApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut visuals = egui::Visuals::light();
        visuals.panel_fill = Color32::from_rgb(241, 245, 249);
        visuals.extreme_bg_color = Color32::from_rgb(241, 245, 249);
        visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(37, 99, 235, 45);
        visuals.selection.stroke = Stroke::new(1.5_f32, Color32::from_rgb(37, 99, 235));
        ctx.set_visuals(visuals);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(241, 245, 249))
                    .inner_margin(Margin::symmetric(24, 22)),
            )
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(Color32::WHITE)
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(203, 213, 225)))
                    .corner_radius(12.0)
                    .inner_margin(Margin::symmetric(24, 22))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());

                        // Header with Logo
                        let avail_w = ui.available_width();
                        let (hdr_rect, _) =
                            ui.allocate_exact_size(Vec2::new(avail_w, 38.0), Sense::hover());
                        let logo_rect = Rect::from_min_size(
                            Pos2::new(hdr_rect.left(), hdr_rect.center().y - 16.0),
                            Vec2::new(32.0, 32.0),
                        );
                        ui.painter().rect_filled(
                            logo_rect,
                            CornerRadius::same(7),
                            Color32::from_rgb(37, 99, 235),
                        );
                        ui.painter().text(
                            logo_rect.center(),
                            Align2::CENTER_CENTER,
                            "S",
                            FontId::proportional(17.0),
                            Color32::WHITE,
                        );
                        ui.painter().text(
                            Pos2::new(logo_rect.right() + 12.0, hdr_rect.center().y - 7.0),
                            Align2::LEFT_CENTER,
                            format!("Shiply v{APP_VERSION} Setup"),
                            FontId::proportional(18.0),
                            Color32::from_rgb(15, 23, 42),
                        );
                        ui.painter().text(
                            Pos2::new(logo_rect.right() + 12.0, hdr_rect.center().y + 11.0),
                            Align2::LEFT_CENTER,
                            "Universal Desktop Mini Deployment Platform",
                            FontId::proportional(12.0),
                            Color32::from_rgb(100, 116, 139),
                        );

                        ui.add_space(16.0);

                        let dir_path = PathBuf::from(self.install_dir_input.trim());
                        let state = check_install_state(&dir_path, APP_VERSION);
                        let mode_text = match state {
                            InstallState::NotInstalled => {
                                "Fresh installation — starts with a clean workspace."
                            }
                            InstallState::InstalledCurrent { .. } => {
                                "Reinstall / Repair current version (preserves your projects & history)."
                            }
                            InstallState::UpdateReady { .. } => {
                                "Upgrade existing installation to the latest version (preserves your projects & history)."
                            }
                        };
                        ui.label(
                            RichText::new(mode_text)
                                .size(12.5)
                                .strong()
                                .color(Color32::from_rgb(37, 99, 235)),
                        );

                        ui.add_space(12.0);
                        ui.label(
                            RichText::new("Installation Folder:")
                                .size(12.5)
                                .strong()
                                .color(Color32::from_rgb(51, 65, 85)),
                        );
                        ui.add_space(4.0);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.install_dir_input)
                                .font(FontId::proportional(13.0))
                                .background_color(Color32::from_rgb(241, 245, 249))
                                .desired_width(avail_w)
                                .margin(Margin::symmetric(10, 7)),
                        );

                        ui.add_space(14.0);
                        ui.checkbox(
                            &mut self.create_desktop_shortcut,
                            RichText::new("Create Desktop shortcut (Shiply.lnk)")
                                .size(13.0)
                                .color(Color32::from_rgb(15, 23, 42)),
                        );
                        ui.checkbox(
                            &mut self.create_start_menu_shortcut,
                            RichText::new("Create Start Menu shortcut")
                                .size(13.0)
                                .color(Color32::from_rgb(15, 23, 42)),
                        );
                        ui.checkbox(
                            &mut self.launch_after_install,
                            RichText::new("Launch Shiply immediately after installation")
                                .size(13.0)
                                .color(Color32::from_rgb(15, 23, 42)),
                        );

                        if let Some((msg, ok)) = &self.status_message {
                            ui.add_space(10.0);
                            let (bg, fg) = if *ok {
                                (
                                    Color32::from_rgb(220, 252, 231),
                                    Color32::from_rgb(21, 128, 61),
                                )
                            } else {
                                (
                                    Color32::from_rgb(254, 226, 226),
                                    Color32::from_rgb(185, 28, 28),
                                )
                            };
                            let (r, _) =
                                ui.allocate_exact_size(Vec2::new(avail_w, 32.0), Sense::hover());
                            ui.painter().rect_filled(r, CornerRadius::same(6), bg);
                            ui.painter().text(
                                Pos2::new(r.left() + 10.0, r.center().y),
                                Align2::LEFT_CENTER,
                                msg,
                                FontId::proportional(12.0),
                                fg,
                            );
                        }

                        ui.add_space(18.0);
                        ui.allocate_ui_with_layout(
                            Vec2::new(avail_w, 38.0),
                            Layout::right_to_left(Align::Center),
                            |ui| {
                                ui.set_max_height(38.0);
                                if self.installed_successfully {
                                    let finish_btn = egui::Button::new(
                                        RichText::new("Finish & Close")
                                            .size(13.5)
                                            .strong()
                                            .color(Color32::WHITE),
                                    )
                                    .fill(Color32::from_rgb(21, 128, 61))
                                    .corner_radius(7.0)
                                    .min_size(Vec2::new(140.0, 36.0));
                                    if ui.add(finish_btn).clicked() {
                                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                    }
                                } else {
                                    let install_btn = egui::Button::new(
                                        RichText::new(format!("Install Shiply v{APP_VERSION}"))
                                            .size(13.5)
                                            .strong()
                                            .color(Color32::WHITE),
                                    )
                                    .fill(Color32::from_rgb(37, 99, 235))
                                    .corner_radius(7.0)
                                    .min_size(Vec2::new(175.0, 36.0));

                                    if ui.add(install_btn).clicked() {
                                        let target_dir =
                                            PathBuf::from(self.install_dir_input.trim());
                                        match install_with_options(
                                            &target_dir,
                                            self.create_desktop_shortcut,
                                            self.create_start_menu_shortcut,
                                        ) {
                                            Ok(msg) => {
                                                self.status_message = Some((msg, true));
                                                self.installed_successfully = true;
                                                if self.launch_after_install {
                                                    let installed_exe =
                                                        target_dir.join(installed_exe_name());
                                                    let _ = Command::new(&installed_exe)
                                                        .current_dir(&target_dir)
                                                        .spawn();
                                                    ctx.send_viewport_cmd(
                                                        egui::ViewportCommand::Close,
                                                    );
                                                }
                                            }
                                            Err(err) => {
                                                self.status_message = Some((err, false));
                                            }
                                        }
                                    }

                                    let cancel_btn = egui::Button::new(
                                        RichText::new("Cancel")
                                            .size(13.0)
                                            .strong()
                                            .color(Color32::from_rgb(51, 65, 85)),
                                    )
                                    .fill(Color32::from_rgb(248, 250, 252))
                                    .stroke(Stroke::new(
                                        1.0_f32,
                                        Color32::from_rgb(203, 213, 225),
                                    ))
                                    .corner_radius(7.0)
                                    .min_size(Vec2::new(90.0, 36.0));
                                    if ui.add(cancel_btn).clicked() {
                                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                    }
                                }
                            },
                        );
                    });
            });
    }
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
