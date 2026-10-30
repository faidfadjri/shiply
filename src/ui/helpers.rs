use super::*;

// =============================================================================
// Native OS File / Folder Dialogs & Helpers
// =============================================================================

pub(crate) fn pick_ps1_file_dialog() -> Option<String> {
    #[cfg(windows)]
    {
        let script = r#"
Add-Type -AssemblyName System.Windows.Forms
$dlg = New-Object System.Windows.Forms.OpenFileDialog
$dlg.Filter = "PowerShell Scripts (*.ps1)|*.ps1|All Files (*.*)|*.*"
$dlg.Title = "Select PowerShell Deployment Script (.ps1)"
if ($dlg.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
    Write-Output $dlg.FileName
}
"#;
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-Sta", "-Command", script]);
        configure_background_command(&mut cmd);
        let out = cmd.output().ok()?;
        let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if path.is_empty() {
            None
        } else {
            Some(path)
        }
    }
    #[cfg(not(windows))]
    {
        None
    }
}

pub(crate) fn pick_folder_dialog() -> Option<String> {
    #[cfg(windows)]
    {
        let script = r#"
Add-Type -AssemblyName System.Windows.Forms
$dlg = New-Object System.Windows.Forms.FolderBrowserDialog
$dlg.Description = "Select Local Project Directory"
if ($dlg.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
    Write-Output $dlg.SelectedPath
}
"#;
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-Sta", "-Command", script]);
        configure_background_command(&mut cmd);
        let out = cmd.output().ok()?;
        let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if path.is_empty() {
            None
        } else {
            Some(path)
        }
    }
    #[cfg(not(windows))]
    {
        None
    }
}

pub(crate) fn open_directory_in_os(dir: &Path) {
    #[cfg(windows)]
    {
        let _ = Command::new("explorer").arg(dir).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(dir).spawn();
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let _ = Command::new("xdg-open").arg(dir).spawn();
    }
}

pub(crate) fn open_url_in_browser(url: &str) {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", "start", "", url]);
        configure_background_command(&mut cmd);
        let _ = cmd.spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(url).spawn();
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let _ = Command::new("xdg-open").arg(url).spawn();
    }
}

#[allow(dead_code)]
pub(crate) fn open_windows_credential_manager() {
    let _ = Command::new("control.exe")
        .args(["/name", "Microsoft.CredentialManager"])
        .spawn();
}

pub(crate) fn draw_elevated_card(painter: &egui::Painter, rect: Rect, fill: Color32, stroke: Stroke) {
    let shadow_rect = rect.translate(Vec2::new(0.0, 2.0));
    painter.rect_filled(
        shadow_rect,
        CornerRadius::same(10),
        Color32::from_black_alpha(8),
    );
    painter.rect_filled(rect, CornerRadius::same(10), fill);
    painter.rect_stroke(rect, CornerRadius::same(10), stroke, StrokeKind::Inside);
}

pub(crate) fn draw_divider(ui: &mut egui::Ui, color: Color32) {
    let avail_w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(avail_w, 1.0), Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(1.0_f32, color),
    );
}

pub(crate) fn render_pill(ui: &mut egui::Ui, text: &str, bg: Color32, fg: Color32) {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), FontId::proportional(FONT_XS), fg);
    let pill_w = galley.size().x + 16.0;
    let pill_h = 22.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(pill_w, pill_h), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(5), bg);
    ui.painter().galley(
        Pos2::new(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        fg,
    );
}

pub(crate) fn paint_pill_left_aligned(
    painter: &egui::Painter,
    left_center: Pos2,
    text: &str,
    bg: Color32,
    fg: Color32,
) -> f32 {
    let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(FONT_XS), fg);
    let w = galley.size().x + 16.0;
    let h = 22.0;
    let rect = Rect::from_min_size(
        Pos2::new(left_center.x, left_center.y - h / 2.0),
        Vec2::new(w, h),
    );
    painter.rect_filled(rect, CornerRadius::same(5), bg);
    painter.galley(
        Pos2::new(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        fg,
    );
    w
}

pub(crate) fn paint_pill_right_aligned(
    painter: &egui::Painter,
    right_center: Pos2,
    text: &str,
    bg: Color32,
    fg: Color32,
) -> f32 {
    let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(FONT_XS), fg);
    let w = galley.size().x + 16.0;
    let h = 22.0;
    let rect = Rect::from_min_size(
        Pos2::new(right_center.x - w, right_center.y - h / 2.0),
        Vec2::new(w, h),
    );
    painter.rect_filled(rect, CornerRadius::same(5), bg);
    painter.galley(
        Pos2::new(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        fg,
    );
    w
}

#[allow(dead_code)]
pub(crate) fn paint_small_sidebar_pill(
    painter: &egui::Painter,
    left_center: Pos2,
    text: &str,
    bg: Color32,
    fg: Color32,
) -> f32 {
    let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(FONT_XS), fg);
    let w = galley.size().x + 12.0;
    let h = 19.0;
    let rect = Rect::from_min_size(
        Pos2::new(left_center.x, left_center.y - h / 2.0),
        Vec2::new(w, h),
    );
    painter.rect_filled(rect, CornerRadius::same(4), bg);
    painter.galley(
        Pos2::new(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        fg,
    );
    w
}

pub(crate) fn status_pill_colors(status: DeploymentStatus) -> (Color32, Color32) {
    match status {
        DeploymentStatus::Succeeded => (SUCCESS_PILL_BG, SUCCESS_PILL_TEXT),
        DeploymentStatus::Failed => (FAIL_PILL_BG, FAIL_PILL_TEXT),
        DeploymentStatus::Running => (WARN_PILL_BG, WARN_PILL_TEXT),
        DeploymentStatus::Cancelled => (PILL_CONTAINER_BG, TEXT_SECONDARY),
    }
}

/// Paints a shimmering skeleton placeholder with a smooth 1.4s infinite wave.
pub(crate) fn paint_shimmer_rect(painter: &egui::Painter, rect: Rect, radius: f32, time: f64) {
    let phase = ((time % 1.4) / 1.4) as f32;
    let alpha_f = 0.06 + 0.06 * (0.5 + 0.5 * (phase * std::f32::consts::TAU).sin());
    let a = (alpha_f * 255.0).clamp(15.0, 31.0) as u8;
    let fill = Color32::from_rgba_premultiplied(a, a, a, a);
    painter.rect_filled(rect, CornerRadius::same(radius as u8), fill);
}

/// Paints a 20px environment badge adhering to sidebar tokens (--fs-micro, --r-sm, padding 0 8px).
pub(crate) fn paint_sidebar_env_badge(
    painter: &egui::Painter,
    pos: Pos2,
    text: &str,
    bg: Color32,
    fg: Color32,
    max_w: Option<f32>,
) -> f32 {
    let font_id = FontId::monospace(FS_MICRO);
    let pad_x = 8.0_f32;
    let h = 20.0_f32;

    let galley = if let Some(mw) = max_w {
        let max_text_w = (mw - pad_x * 2.0).max(8.0);
        let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), font_id, fg);
        job.wrap.max_width = max_text_w;
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = false;
        painter.layout_job(job)
    } else {
        painter.layout_no_wrap(text.to_string(), font_id, fg)
    };

    let w = galley.size().x + pad_x * 2.0;
    let rect = Rect::from_min_size(pos, Vec2::new(w, h));
    painter.rect_filled(rect, CornerRadius::same(R_SM as u8), bg);
    painter.galley(
        Pos2::new(rect.left() + pad_x, rect.center().y - galley.size().y / 2.0),
        galley,
        fg,
    );
    w
}

/// Draws an animated circular loading spinner with a faint 360-degree background ring
/// and an energetic rotating foreground arc.
pub(crate) fn draw_circle_loading_spinner(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    color: Color32,
    time: f64,
) {
    let stroke_w = (radius * 0.32).clamp(1.2, 2.0);
    // 1. Faint circular track underneath
    let track_color = color.gamma_multiply(0.25);
    painter.circle_stroke(center, radius, Stroke::new(stroke_w, track_color));

    // 2. Energetic spinning foreground arc (~270 degrees)
    let speed = 5.2; // ~0.83 turns per second
    let start_angle = (time * speed) as f32;
    let sweep_angle = std::f32::consts::PI * 1.5;
    let segments = 24;
    let step = sweep_angle / segments as f32;

    for i in 0..segments {
        let a1 = start_angle + i as f32 * step;
        let a2 = start_angle + (i + 1) as f32 * step;
        let p1 = Pos2::new(center.x + radius * a1.cos(), center.y + radius * a1.sin());
        let p2 = Pos2::new(center.x + radius * a2.cos(), center.y + radius * a2.sin());
        painter.line_segment([p1, p2], Stroke::new(stroke_w, color));
    }
}

/// Paints a status badge pill left-aligned with an animated circular loading spinner
/// on the left side of the label text.
pub(crate) fn paint_running_pill_left_aligned(
    painter: &egui::Painter,
    left_center: Pos2,
    text: &str,
    bg: Color32,
    fg: Color32,
    time: f64,
) -> f32 {
    let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(FONT_XS), fg);
    let spinner_r = 4.2;
    let spinner_w = spinner_r * 2.0;
    let gap = 6.0;
    let pad_x = 8.0;
    let w = pad_x + spinner_w + gap + galley.size().x + pad_x;
    let h = 22.0;
    let rect = Rect::from_min_size(
        Pos2::new(left_center.x, left_center.y - h / 2.0),
        Vec2::new(w, h),
    );

    painter.rect_filled(rect, CornerRadius::same(5), bg);
    painter.rect_stroke(
        rect,
        CornerRadius::same(5),
        Stroke::new(1.0_f32, fg.gamma_multiply(0.28)),
        StrokeKind::Inside,
    );

    let spinner_c = Pos2::new(rect.left() + pad_x + spinner_r, rect.center().y);
    draw_circle_loading_spinner(painter, spinner_c, spinner_r, fg, time);

    let text_pos = Pos2::new(
        spinner_c.x + spinner_r + gap,
        rect.center().y - galley.size().y / 2.0,
    );
    painter.galley(text_pos, galley, fg);

    w
}

/// Paints a status badge pill right-aligned with an animated circular loading spinner
/// on the left side of the label text.
pub(crate) fn paint_running_pill_right_aligned(
    painter: &egui::Painter,
    right_center: Pos2,
    text: &str,
    bg: Color32,
    fg: Color32,
    time: f64,
) -> f32 {
    let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(FONT_XS), fg);
    let spinner_r = 4.2;
    let spinner_w = spinner_r * 2.0;
    let gap = 6.0;
    let pad_x = 8.0;
    let w = pad_x + spinner_w + gap + galley.size().x + pad_x;
    let h = 22.0;
    let rect = Rect::from_min_size(
        Pos2::new(right_center.x - w, right_center.y - h / 2.0),
        Vec2::new(w, h),
    );

    painter.rect_filled(rect, CornerRadius::same(5), bg);
    painter.rect_stroke(
        rect,
        CornerRadius::same(5),
        Stroke::new(1.0_f32, fg.gamma_multiply(0.28)),
        StrokeKind::Inside,
    );

    let spinner_c = Pos2::new(rect.left() + pad_x + spinner_r, rect.center().y);
    draw_circle_loading_spinner(painter, spinner_c, spinner_r, fg, time);

    let text_pos = Pos2::new(
        spinner_c.x + spinner_r + gap,
        rect.center().y - galley.size().y / 2.0,
    );
    painter.galley(text_pos, galley, fg);

    w
}

/// Renders a modern, realistic pulsating loading skeleton while switching active project.
pub(crate) fn render_project_workspace_skeleton(ui: &mut egui::Ui, proj_name: &str) {
    ui.ctx().request_repaint();
    let time = ui.input(|i| i.time);
    let full_w = ui.available_width();

    // Subtle pulsating shimmer between light slate tones
    let pulse = ((time * 4.0).sin() * 0.5 + 0.5) as f32;
    let r = (226.0 + (242.0 - 226.0) * pulse) as u8;
    let g = (232.0 + (246.0 - 232.0) * pulse) as u8;
    let b = (240.0 + (250.0 - 240.0) * pulse) as u8;
    let skel_fill = Color32::from_rgb(r, g, b);
    let skel_border = Color32::from_rgb(
        (215.0 + 12.0 * pulse) as u8,
        (223.0 + 10.0 * pulse) as u8,
        (234.0 + 8.0 * pulse) as u8,
    );

    // 1. Top Project Header Bar Skeleton
    horizontal_row(ui, 36.0, |ui| {
        let (title_rect, _) = ui.allocate_exact_size(Vec2::new(190.0, 28.0), Sense::hover());
        ui.painter().rect_filled(title_rect, CornerRadius::same(6), skel_fill);
        ui.painter().text(
            Pos2::new(title_rect.left() + 10.0, title_rect.center().y),
            Align2::LEFT_CENTER,
            proj_name,
            FontId::proportional(FONT_MD),
            Color32::from_rgb(148, 163, 184),
        );

        ui.add_space(8.0);
        let (badge_rect, _) = ui.allocate_exact_size(Vec2::new(56.0, 20.0), Sense::hover());
        ui.painter().rect_filled(badge_rect, CornerRadius::same(4), skel_fill);

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // Action button skeletons
            for _ in 0..3 {
                let (btn_rect, _) = ui.allocate_exact_size(Vec2::new(32.0, 32.0), Sense::hover());
                ui.painter().rect_filled(btn_rect, CornerRadius::same(6), skel_fill);
                ui.add_space(4.0);
            }
            // Branch selector skeleton
            let (branch_rect, _) = ui.allocate_exact_size(Vec2::new(160.0, 32.0), Sense::hover());
            ui.painter().rect_filled(branch_rect, CornerRadius::same(6), skel_fill);
        });
    });

    ui.add_space(4.0);

    // Subtitle / directory path skeleton line
    let (sub_rect, _) = ui.allocate_exact_size(Vec2::new(320.0, 14.0), Sense::hover());
    ui.painter().rect_filled(sub_rect, CornerRadius::same(4), skel_fill);

    ui.add_space(16.0);

    // 2. Environment Cards Row Skeleton
    let card_w = ((full_w - 24.0) / 3.0).clamp(160.0, 240.0);
    horizontal_row(ui, ENV_CARD_HEIGHT, |ui| {
        for i in 0..3 {
            let (card_rect, _) = ui.allocate_exact_size(Vec2::new(card_w, ENV_CARD_HEIGHT), Sense::hover());
            ui.painter().rect_filled(card_rect, CornerRadius::same(8), CARD_BG);
            ui.painter().rect_stroke(
                card_rect,
                CornerRadius::same(8),
                Stroke::new(1.0_f32, CARD_BORDER),
                StrokeKind::Inside,
            );

            // Card title skeleton badge
            let badge_w = if i == 0 { 80.0 } else if i == 1 { 64.0 } else { 92.0 };
            let env_badge_rect = Rect::from_min_size(
                Pos2::new(card_rect.left() + 12.0, card_rect.top() + 12.0),
                Vec2::new(badge_w, 20.0),
            );
            ui.painter().rect_filled(env_badge_rect, CornerRadius::same(4), skel_fill);

            // Version line skeleton
            let ver_rect = Rect::from_min_size(
                Pos2::new(card_rect.left() + 12.0, card_rect.top() + 42.0),
                Vec2::new(card_w * 0.65, 16.0),
            );
            ui.painter().rect_filled(ver_rect, CornerRadius::same(4), skel_fill);

            // Host / server line skeleton
            let host_rect = Rect::from_min_size(
                Pos2::new(card_rect.left() + 12.0, card_rect.top() + 68.0),
                Vec2::new(card_w * 0.45, 12.0),
            );
            ui.painter().rect_filled(host_rect, CornerRadius::same(3), skel_fill);

            ui.add_space(10.0);
        }
    });

    ui.add_space(16.0);

    // 3. Navigation Tabs Bar Skeleton
    horizontal_row(ui, 34.0, |ui| {
        for w in [130.0, 140.0, 120.0] {
            let (tab_rect, _) = ui.allocate_exact_size(Vec2::new(w, 32.0), Sense::hover());
            ui.painter().rect_filled(tab_rect, CornerRadius::same(6), skel_fill);
            ui.add_space(8.0);
        }
    });

    ui.add_space(12.0);

    // 4. Large Main Workspace Content Skeleton Card (Pipeline steps preview)
    egui::Frame::new()
        .fill(CARD_BG)
        .stroke(Stroke::new(1.0_f32, CARD_BORDER))
        .corner_radius(10.0)
        .inner_margin(Margin::same(18))
        .show(ui, |ui| {
            // Header inside card
            horizontal_row(ui, 28.0, |ui| {
                let (ch_rect, _) = ui.allocate_exact_size(Vec2::new(200.0, 20.0), Sense::hover());
                ui.painter().rect_filled(ch_rect, CornerRadius::same(4), skel_fill);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let (act_rect, _) = ui.allocate_exact_size(Vec2::new(110.0, 28.0), Sense::hover());
                    ui.painter().rect_filled(act_rect, CornerRadius::same(6), skel_fill);
                });
            });

            ui.add_space(12.0);

            // 3 Pipeline Step Skeletons
            for (step_title_w, step_cmd_w) in [(180.0, 0.75), (240.0, 0.9), (140.0, 0.6)] {
                let step_card_h = 76.0;
                let (step_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), step_card_h), Sense::hover());
                ui.painter().rect_filled(step_rect, CornerRadius::same(8), SUBTLE_BG);
                ui.painter().rect_stroke(
                    step_rect,
                    CornerRadius::same(8),
                    Stroke::new(1.0_f32, skel_border),
                    StrokeKind::Inside,
                );

                // Step number circle skeleton
                let num_pos = Pos2::new(step_rect.left() + 22.0, step_rect.top() + 22.0);
                ui.painter().circle_filled(num_pos, 10.0, skel_fill);

                // Step title skeleton
                let st_rect = Rect::from_min_size(
                    Pos2::new(step_rect.left() + 42.0, step_rect.top() + 14.0),
                    Vec2::new(step_title_w, 16.0),
                );
                ui.painter().rect_filled(st_rect, CornerRadius::same(4), skel_fill);

                // Step command skeleton bar
                let cmd_w = (step_rect.width() - 56.0) * step_cmd_w;
                let sc_rect = Rect::from_min_size(
                    Pos2::new(step_rect.left() + 42.0, step_rect.top() + 42.0),
                    Vec2::new(cmd_w, 20.0),
                );
                ui.painter().rect_filled(sc_rect, CornerRadius::same(4), skel_fill);

                ui.add_space(10.0);
            }
        });
}

/// Backward-compatible wrapper for render_project_loading_indicator
#[allow(dead_code)]
pub(crate) fn render_project_loading_indicator(ui: &mut egui::Ui, proj_name: &str) {
    render_project_workspace_skeleton(ui, proj_name);
}

pub(crate) fn render_light_status_badge(ui: &mut egui::Ui, status: DeploymentStatus) {
    let (bg, fg) = status_pill_colors(status);
    let text = status.label().to_uppercase();
    if status == DeploymentStatus::Running {
        ui.ctx().request_repaint();
        let time = ui.input(|i| i.time);
        let spinner_r = 4.2;
        let spinner_w = spinner_r * 2.0;
        let gap = 6.0;
        let pad_x = 8.0;
        let galley = ui
            .painter()
            .layout_no_wrap(text, FontId::proportional(FONT_XS), fg);
        let pill_w = pad_x + spinner_w + gap + galley.size().x + pad_x;
        let pill_h = 22.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(pill_w, pill_h), Sense::hover());
        ui.painter().rect_filled(rect, CornerRadius::same(5), bg);
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(5),
            Stroke::new(1.0_f32, fg.gamma_multiply(0.28)),
            StrokeKind::Inside,
        );
        let spinner_c = Pos2::new(rect.left() + pad_x + spinner_r, rect.center().y);
        draw_circle_loading_spinner(ui.painter(), spinner_c, spinner_r, fg, time);
        ui.painter().galley(
            Pos2::new(
                spinner_c.x + spinner_r + gap,
                rect.center().y - galley.size().y / 2.0,
            ),
            galley,
            fg,
        );
    } else {
        render_pill(ui, &text, bg, fg);
    }
}

pub(crate) fn env_pastel_pill_colors(env_key: &str) -> (Color32, Color32) {
    match env_key.to_lowercase().as_str() {
        "dev" | "development" => (BLUE_PILL_BG, BLUE_PILL_TEXT),
        "staging" | "stage" | "stg" => (WARN_PILL_BG, WARN_PILL_TEXT),
        "prod" | "production" => (PURPLE_PILL_BG, PURPLE_PILL_TEXT),
        _ => (TEAL_PILL_BG, TEAL_PILL_TEXT),
    }
}

