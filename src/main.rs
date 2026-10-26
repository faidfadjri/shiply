#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use shiply::credentials::try_handle_askpass_invocation;
use shiply::installer::{
    check_install_state, default_install_dir, install_or_update_self, is_setup_mode, InstallState,
    SetupWizardApp, APP_VERSION,
};
use shiply::storage::AppStorage;
use shiply::ui::ShiplyApp;
use std::env;

fn main() -> eframe::Result<()> {
    // 1. If invoked by OpenSSH (ssh/scp) as SSH_ASKPASS helper, output the secret and exit immediately.
    if try_handle_askpass_invocation() {
        return Ok(());
    }

    // 2. If invoked with `--install` / `--silent` / `install`, install/update binary + shortcuts headlessly and exit.
    let args: Vec<String> = env::args().skip(1).collect();
    if args
        .iter()
        .any(|a| a == "--install" || a == "install" || a == "--silent")
    {
        match install_or_update_self() {
            Ok(msg) => {
                println!("{msg}");
                return Ok(());
            }
            Err(err) => {
                eprintln!("Installation error: {err}");
                std::process::exit(1);
            }
        }
    }

    // 3. If launched as `setup.exe` (or `--setup`), open the graphical Desktop Setup Wizard!
    if is_setup_mode(&args) {
        let setup_options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title(format!("Shiply v{APP_VERSION} Setup"))
                .with_inner_size([520.0, 390.0])
                .with_min_inner_size([500.0, 370.0])
                .with_resizable(false),
            ..Default::default()
        };
        return eframe::run_native(
            "Shiply Setup",
            setup_options,
            Box::new(|_cc| Ok(Box::new(SetupWizardApp::default()))),
        );
    }

    // 4. Auto-update installed copy when launching a newer .exe version directly
    let mut startup_notice = None;
    if let InstallState::UpdateReady {
        installed_version,
        running_version,
        ..
    } = check_install_state(&default_install_dir(), APP_VERSION)
    {
        if let Ok(msg) = install_or_update_self() {
            startup_notice = Some(format!(
                "Auto-updated installed app from v{installed_version} to v{running_version} ({msg})"
            ));
        }
    }

    let storage_path = AppStorage::default_storage_path();
    let storage = AppStorage::load_or_default(storage_path);

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(format!("Shiply v{APP_VERSION} — Mini Deployment Platform"))
            .with_inner_size([1340.0, 840.0])
            .with_min_inner_size([980.0, 640.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Shiply",
        native_options,
        Box::new(move |_cc| {
            let mut app = ShiplyApp::new(storage);
            if let Some(notice) = startup_notice {
                app.set_status_banner(notice, true);
            }
            Ok(Box::new(app))
        }),
    )
}
