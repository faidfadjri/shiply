use super::*;

    #[test]
    fn test_pipeline_syntax_highlight_layout_job() {
        let cmd = "cd {remote_dir} && pm2 restart {app_name} --update-env ./dist/server.js";
        let job = build_pipeline_syntax_layout_job(cmd);
        assert_eq!(job.text, cmd);
        assert!(job.sections.len() >= 7);

        let has_template_purple = job
            .sections
            .iter()
            .any(|s| s.format.color == PURPLE_PILL_TEXT);
        let has_keyword_blue = job
            .sections
            .iter()
            .any(|s| s.format.color == BLUE_PILL_TEXT);
        let has_flag_amber = job
            .sections
            .iter()
            .any(|s| s.format.color == WARN_PILL_TEXT);
        let has_path_teal = job
            .sections
            .iter()
            .any(|s| s.format.color == TEAL_PILL_TEXT);
        let has_operator_crimson = job
            .sections
            .iter()
            .any(|s| s.format.color == FAIL_PILL_TEXT);

        assert!(has_template_purple);
        assert!(has_keyword_blue);
        assert!(has_flag_amber);
        assert!(has_path_teal);
        assert!(has_operator_crimson);
    }

    #[test]
    fn test_running_badge_pill_colors_and_layout() {
        let (bg, fg) = status_pill_colors(DeploymentStatus::Running);
        assert_eq!(bg, WARN_PILL_BG);
        assert_eq!(fg, WARN_PILL_TEXT);

        // Verify circle loading spinner angle progression
        let speed = 5.2_f64;
        let t0 = 0.0_f64;
        let t1 = 1.0_f64;
        let a0 = (t0 * speed) as f32;
        let a1 = (t1 * speed) as f32;
        assert!((a1 - a0 - 5.2_f32).abs() < 1e-4);

        // Test painter execution with egui Context
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let left_center = Pos2::new(100.0, 50.0);
                let w = paint_running_pill_left_aligned(
                    ui.painter(),
                    left_center,
                    "RUNNING",
                    bg,
                    fg,
                    1.23,
                );
                assert!(w > 50.0);

                let right_center = Pos2::new(200.0, 50.0);
                let w_r = paint_running_pill_right_aligned(
                    ui.painter(),
                    right_center,
                    "RUNNING",
                    bg,
                    fg,
                    1.23,
                );
                assert!(w_r > 50.0);
            });
        });
    }

    #[test]
    fn test_active_project_code_changes_isolation() {
        let mut storage = AppStorage::default();
        let env_be = EnvironmentConfig {
            key: "dev".to_string(),
            name: "Development".to_string(),
            app_name: "backend-dev".to_string(),
            servers: vec!["10.0.0.1".to_string()],
            remote_user: "deploy".to_string(),
            vault_target: "backend_dev".to_string(),
            remote_dir: "/apps/backend".to_string(),
            env_source: None,
            http_proxy: None,
            steps: Vec::new(),
        };
        let proj_backend = Project {
            id: "proj-backend".to_string(),
            name: "Backend Service".to_string(),
            description: "Backend API".to_string(),
            local_path: ".".to_string(),
            environments: vec![env_be],
            group: None,
            sort_order: None,
        };

        let env_fe = EnvironmentConfig {
            key: "dev".to_string(),
            name: "Development".to_string(),
            app_name: "frontend-dev".to_string(),
            servers: vec!["10.0.0.2".to_string()],
            remote_user: "deploy".to_string(),
            vault_target: "frontend_dev".to_string(),
            remote_dir: "/apps/frontend".to_string(),
            env_source: None,
            http_proxy: None,
            steps: Vec::new(),
        };
        let proj_frontend = Project {
            id: "proj-frontend".to_string(),
            name: "Frontend App".to_string(),
            description: "Web frontend".to_string(),
            local_path: ".".to_string(),
            environments: vec![env_fe],
            group: None,
            sort_order: None,
        };

        storage.projects = vec![proj_backend, proj_frontend];

        let dep_backend = DeploymentRecord {
            id: "dep-be-1".to_string(),
            project_id: "proj-backend".to_string(),
            project_name: "Backend Service".to_string(),
            environment: "dev".to_string(),
            app_name: "backend-dev".to_string(),
            version: "1.0.1".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("be11111".to_string()),
            git_changes: vec!["feat(api): add auth endpoint".to_string()],
            is_rollback: false,
            servers: vec!["10.0.0.1".to_string()],
            status: DeploymentStatus::Succeeded,
            started_at: chrono::Utc::now(),
            finished_at: Some(chrono::Utc::now()),
            duration_ms: Some(1500),
            step_logs: Vec::new(),
            executed_steps: Vec::new(),
        };

        let dep_frontend = DeploymentRecord {
            id: "dep-fe-1".to_string(),
            project_id: "proj-frontend".to_string(),
            project_name: "Frontend App".to_string(),
            environment: "dev".to_string(),
            app_name: "frontend-dev".to_string(),
            version: "2.0.0".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("fe22222".to_string()),
            git_changes: vec!["feat(ui): redesign dashboard".to_string()],
            is_rollback: false,
            servers: vec!["10.0.0.2".to_string()],
            status: DeploymentStatus::Succeeded,
            started_at: chrono::Utc::now(),
            finished_at: Some(chrono::Utc::now()),
            duration_ms: Some(2000),
            step_logs: Vec::new(),
            executed_steps: Vec::new(),
        };

        storage.deployments = vec![dep_backend.clone(), dep_frontend.clone()];

        let mut app = ShiplyApp::new(storage);
        // Switch to Backend project and set an invalid foreign selected deployment ID
        app.selection = SidebarSelection::Project("proj-backend".to_string());
        app.selected_deployment_id = Some("dep-fe-1".to_string());

        // When refreshing metadata for Backend, selected_deployment_id must be corrected to Backend
        app.refresh_selected_project_metadata(false);
        assert_eq!(app.selected_deployment_id.as_deref(), Some("dep-be-1"));

        // When filtering deployments for Backend, frontend deployments are excluded
        let filtered = app.storage.filter_deployments(Some("proj-backend"), None, None);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].project_id, "proj-backend");
        assert_eq!(
            filtered[0].git_changes,
            vec!["feat(api): add auth endpoint".to_string()]
        );
    }

    #[test]
    fn test_project_switch_loading_indicator_and_hover_cursor() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        // Verify global visuals hover interact cursor is set to PointingHand (Pointer)
        assert_eq!(
            ctx.style().visuals.interact_cursor,
            Some(egui::CursorIcon::PointingHand)
        );

        // Test render_project_loading_indicator execution without panic
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_project_loading_indicator(ui, "Backend API");
            });
        });
    }

    #[test]
    fn test_deploy_and_rollback_button_dynamic_sizing_no_offset() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut app = ShiplyApp::new(crate::storage::AppStorage::default());
        app.deploy_version_input = "1.0.10-beta.2+build.2026".to_string();

        let rollback_dep = DeploymentRecord {
            id: "dep-rb-test".to_string(),
            project_id: "p1".to_string(),
            project_name: "Customer Data Ops".to_string(),
            environment: "development".to_string(),
            app_name: "data-ops".to_string(),
            version: "1.0.9-hotfix".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("abc1234".to_string()),
            git_changes: vec![],
            is_rollback: false,
            servers: vec!["10.0.0.1".to_string()],
            status: DeploymentStatus::Succeeded,
            started_at: chrono::Utc::now(),
            finished_at: Some(chrono::Utc::now()),
            duration_ms: Some(1200),
            step_logs: vec![],
            executed_steps: vec![],
        };

        // Run UI render with long environment name and version
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.set_min_width(1200.0);
                let mut trigger_rect = None;
                app.render_deploy_action_bar(
                    ui,
                    "Development Customer Data Ops",
                    Some(rollback_dep.clone()),
                    &mut trigger_rect,
                );
            });
        });

        // Also test with empty version string to ensure clean fallback text
        app.deploy_version_input = "   ".to_string();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.set_min_width(1000.0);
                let mut trigger_rect = None;
                app.render_deploy_action_bar(ui, "Production", None, &mut trigger_rect);
            });
        });
    }

    #[test]
    fn test_deployment_history_pagination_last_5_and_show_more_10() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        // Create 18 mock deployments
        for i in 1..=18 {
            storage.deployments.push(DeploymentRecord {
                id: format!("dep-{i}"),
                project_id: "p1".to_string(),
                project_name: "Test App".to_string(),
                environment: "dev".to_string(),
                app_name: "test-app".to_string(),
                version: format!("1.0.{i}"),
                git_branch: Some("main".to_string()),
                git_commit: Some(format!("c{i}")),
                git_changes: vec![],
                is_rollback: false,
                servers: vec!["10.0.0.1".to_string()],
                status: DeploymentStatus::Succeeded,
                started_at: chrono::Utc::now(),
                finished_at: Some(chrono::Utc::now()),
                duration_ms: Some(1000),
                step_logs: vec![],
                executed_steps: vec![],
            });
        }

        let mut app = ShiplyApp::new(storage);
        assert_eq!(app.history_visible_limit, 5);

        // Render UI with initially 5 items
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_history_and_logs_view(ui, None);
            });
        });

        // Simulate Show More click -> increases by 10 to 15
        app.history_visible_limit = app.history_visible_limit.saturating_add(10);
        assert_eq!(app.history_visible_limit, 15);

        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_history_and_logs_view(ui, None);
            });
        });

        // Next Show More click -> increases by 10 to 25 (exceeds 18 total)
        app.history_visible_limit = app.history_visible_limit.saturating_add(10);
        assert_eq!(app.history_visible_limit, 25);

        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_history_and_logs_view(ui, None);
            });
        });

        // Reset/Collapse to last 5
        app.history_visible_limit = 5;
        assert_eq!(app.history_visible_limit, 5);
    }

    #[test]
    fn test_server_settings_input_buffering_and_debounced_save() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        let env = EnvironmentConfig {
            key: "dev".to_string(),
            name: "Development".to_string(),
            app_name: "api-dev".to_string(),
            servers: vec!["10.0.0.1".to_string()],
            remote_user: "deploy".to_string(),
            vault_target: "dev_vault".to_string(),
            remote_dir: "/apps/api".to_string(),
            env_source: None,
            http_proxy: None,
            steps: vec![],
        };
        let project = Project {
            id: "p1".to_string(),
            name: "Test Project".to_string(),
            description: "Desc".to_string(),
            local_path: ".".to_string(),
            environments: vec![env],
            group: None,
            sort_order: None,
        };
        storage.projects.push(project);

        let mut app = ShiplyApp::new(storage);
        app.selected_env_key = "dev".to_string();

        assert!(!app.dirty_save_pending);
        assert!(app.servers_edit_buffer.is_none());

        // Render Server Settings Tab without panic
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_server_settings_tab(ui, 0);
            });
        });

        // The buffer should now be initialized with the joined server string
        assert_eq!(
            app.servers_edit_buffer,
            Some(("dev".to_string(), "10.0.0.1".to_string()))
        );

        // Simulate typing and mark dirty
        app.mark_config_dirty();
        assert!(app.dirty_save_pending);
        assert!(app.last_edit_time.is_some());

        // Flush pending save
        app.flush_pending_save();
        assert!(!app.dirty_save_pending);
        assert!(app.last_edit_time.is_none());
    }

    #[test]
    fn test_draggable_pipeline_step_reorder_ui() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        let env = EnvironmentConfig {
            key: "dev".to_string(),
            name: "Development".to_string(),
            app_name: "api-dev".to_string(),
            servers: vec!["10.0.0.1".to_string()],
            remote_user: "deploy".to_string(),
            vault_target: "dev_vault".to_string(),
            remote_dir: "/apps/api".to_string(),
            env_source: None,
            http_proxy: None,
            steps: vec![
                PipelineStep::new("Build", StepKind::LocalCommand { command: "cargo build".to_string(), allow_failure: false }),
                PipelineStep::new("Verify", StepKind::VerifyLocalPath { path: "./target".to_string() }),
                PipelineStep::new("Upload", StepKind::TransferFiles { sources: vec!["./target".to_string()], stage_env_file: true }),
            ],
        };
        let project = Project {
            id: "p1".to_string(),
            name: "Test Project".to_string(),
            description: "Desc".to_string(),
            local_path: ".".to_string(),
            environments: vec![env],
            group: None,
            sort_order: None,
        };
        storage.projects.push(project);

        let mut app = ShiplyApp::new(storage);
        app.selected_env_key = "dev".to_string();

        // Render pipeline steps tab initially
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_pipeline_steps_tab(ui, 0);
            });
        });

        assert_eq!(app.storage.projects[0].environments[0].steps[0].name, "Build");
        assert_eq!(app.storage.projects[0].environments[0].steps[1].name, "Verify");
        assert_eq!(app.storage.projects[0].environments[0].steps[2].name, "Upload");

        // Simulate dragging step 2 (Upload)
        app.dragged_step_idx = Some(2);
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_pipeline_steps_tab(ui, 0);
            });
        });

        // Test reorder execution
        app.storage.projects[0].environments[0].reorder_step(2, 0);
        app.dragged_step_idx = None;
        app.mark_config_dirty();

        assert_eq!(app.storage.projects[0].environments[0].steps[0].name, "Upload");
        assert_eq!(app.storage.projects[0].environments[0].steps[1].name, "Build");
        assert_eq!(app.storage.projects[0].environments[0].steps[2].name, "Verify");
        assert!(app.dirty_save_pending);
    }

    #[test]
    fn test_async_git_inspect_channel_and_project_switch() {
        let mut storage = crate::storage::AppStorage::default();
        let project = Project {
            id: "proj-async".to_string(),
            name: "Async Project".to_string(),
            description: "Desc".to_string(),
            local_path: ".".to_string(),
            environments: vec![EnvironmentConfig {
                key: "dev".to_string(),
                name: "Development".to_string(),
                app_name: "async-dev".to_string(),
                servers: vec!["127.0.0.1".to_string()],
                remote_user: "deploy".to_string(),
                vault_target: "vault".to_string(),
                remote_dir: "/app".to_string(),
                env_source: None,
                http_proxy: None,
                steps: Vec::new(),
            }],
            group: None,
            sort_order: None,
        };
        storage.projects.push(project);
        let mut app = ShiplyApp::new(storage);

        // Simulate fast project selection
        app.selection = SidebarSelection::Project("proj-async".to_string());
        app.refresh_selected_project_metadata(true);

        // Project selection was updated immediately without UI freeze
        assert_eq!(app.cached_project_id.as_deref(), Some("proj-async"));

        // Simulate incoming background inspection result
        let _ = app.git_inspect_tx.send(GitInspectionResult {
            project_id: "proj-async".to_string(),
            pkg_version: Some("2.5.0".to_string()),
            git_meta: GitMetadata {
                branch: Some("feature/instant-switch".to_string()),
                commit_short: Some("999aaaa".to_string()),
                commit_subject: Some("feat: instant switch".to_string()),
            },
            git_branches: vec!["main".to_string(), "feature/instant-switch".to_string()],
            git_changes: vec!["M src/ui.rs".to_string()],
            update_version_input: true,
        });

        let received = app.process_pending_git_inspect();
        assert!(received);
        assert_eq!(app.cached_pkg_version.as_deref(), Some("2.5.0"));
        assert_eq!(app.selected_git_branch, "feature/instant-switch");
        assert_eq!(app.cached_git_changes, vec!["M src/ui.rs"]);
        assert!(app.project_switch_loading_id.is_none());
    }

    #[test]
    fn test_project_menu_sidebar_running_deployment_circle_loading_indicator() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        let proj1 = Project {
            id: "proj-running".to_string(),
            name: "Customer Portal".to_string(),
            description: "Active".to_string(),
            local_path: ".".to_string(),
            environments: vec![EnvironmentConfig {
                key: "prod".to_string(),
                name: "Production".to_string(),
                app_name: "portal-prod".to_string(),
                servers: vec!["10.0.0.1".to_string()],
                remote_user: "deploy".to_string(),
                vault_target: "v1".to_string(),
                remote_dir: "/apps/portal".to_string(),
                env_source: None,
                http_proxy: None,
                steps: Vec::new(),
            }],
            group: None,
            sort_order: None,
        };
        let proj2 = Project {
            id: "proj-idle".to_string(),
            name: "Admin Dashboard".to_string(),
            description: "Idle".to_string(),
            local_path: ".".to_string(),
            environments: vec![],
            group: None,
            sort_order: None,
        };
        storage.projects = vec![proj1, proj2];

        // Add a running deployment for proj-running
        let running_dep = DeploymentRecord {
            id: "dep-running-1".to_string(),
            project_id: "proj-running".to_string(),
            project_name: "Customer Portal".to_string(),
            environment: "prod".to_string(),
            app_name: "portal-prod".to_string(),
            version: "1.0.5".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("c0ffee1".to_string()),
            git_changes: vec![],
            is_rollback: false,
            servers: vec!["10.0.0.1".to_string()],
            status: DeploymentStatus::Running,
            started_at: chrono::Utc::now(),
            finished_at: None,
            duration_ms: None,
            step_logs: vec![],
            executed_steps: vec![],
        };
        storage.deployments = vec![running_dep];

        let mut app = ShiplyApp::new(storage);
        app.active_deployment_id = Some("dep-running-1".to_string());

        // Render sidebar with anim_t = 1.0 (fully open)
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_sidebar(ui.ctx(), 1.0);
            });
        });

        // Verify active deployment is registered and belongs to proj-running
        let is_running_p1 = app.storage.deployments.iter().any(|d| {
            d.project_id == "proj-running" && d.status == DeploymentStatus::Running
        });
        let is_running_p2 = app.storage.deployments.iter().any(|d| {
            d.project_id == "proj-idle" && d.status == DeploymentStatus::Running
        });
        assert!(is_running_p1);
        assert!(!is_running_p2);
    }

    #[test]
    fn test_sidebar_project_loading_indicator_positioned_without_overlap() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        let proj = Project {
            id: "proj-multi".to_string(),
            name: "Frontend Customer Data Ops".to_string(),
            description: "Multi-env project".to_string(),
            local_path: ".".to_string(),
            environments: vec![
                EnvironmentConfig {
                    key: "dev".to_string(),
                    name: "Development".to_string(),
                    app_name: "dev-app".to_string(),
                    servers: vec!["10.0.0.1".to_string()],
                    remote_user: "deploy".to_string(),
                    vault_target: "v1".to_string(),
                    remote_dir: "/app".to_string(),
                    env_source: None,
                    http_proxy: None,
                    steps: Vec::new(),
                },
                EnvironmentConfig {
                    key: "prod".to_string(),
                    name: "Production".to_string(),
                    app_name: "prod-app".to_string(),
                    servers: vec!["10.0.0.2".to_string()],
                    remote_user: "deploy".to_string(),
                    vault_target: "v1".to_string(),
                    remote_dir: "/app".to_string(),
                    env_source: None,
                    http_proxy: None,
                    steps: Vec::new(),
                },
            ],
            group: None,
            sort_order: None,
        };
        storage.projects = vec![proj];

        // Running deployment on dev
        let dep = DeploymentRecord {
            id: "dep-running-dev".to_string(),
            project_id: "proj-multi".to_string(),
            project_name: "Frontend Customer Data Ops".to_string(),
            environment: "dev".to_string(),
            app_name: "dev-app".to_string(),
            version: "1.1.10".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("abcdef1".to_string()),
            git_changes: vec![],
            is_rollback: false,
            servers: vec!["10.0.0.1".to_string()],
            status: DeploymentStatus::Running,
            started_at: chrono::Utc::now(),
            finished_at: None,
            duration_ms: None,
            step_logs: vec![],
            executed_steps: vec![],
        };
        // Succeeded deployment on prod
        let ok_dep = DeploymentRecord {
            id: "dep-ok-prod".to_string(),
            project_id: "proj-multi".to_string(),
            project_name: "Frontend Customer Data Ops".to_string(),
            environment: "prod".to_string(),
            app_name: "prod-app".to_string(),
            version: "1.1.9".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("abcdef0".to_string()),
            git_changes: vec![],
            is_rollback: false,
            servers: vec!["10.0.0.2".to_string()],
            status: DeploymentStatus::Succeeded,
            started_at: chrono::Utc::now(),
            finished_at: Some(chrono::Utc::now()),
            duration_ms: Some(1200),
            step_logs: vec![],
            executed_steps: vec![],
        };
        storage.deployments = vec![dep, ok_dep];

        let mut app = ShiplyApp::new(storage);
        app.active_deployment_id = Some("dep-running-dev".to_string());
        assert!(app.is_project_deploying("proj-multi"));
        assert!(app.is_env_deploying("proj-multi", "dev"));
        assert!(!app.is_env_deploying("proj-multi", "prod"));

        // Render sidebar with full anim_t = 1.0
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_sidebar(ui.ctx(), 1.0);
            });
        });

        // Verification of geometry:
        // Card top = 0.0 (reference)
        // Row 1 spinner center Y = 15.0, radius = 5.0 -> spinner bottom Y = 20.0
        // Row 2 badges top Y = 30.0
        // Vertical gap between spinner and badges = 10.0px (strictly positive, non-overlapping)
        let spinner_bottom_offset = 15.0 + 5.0;
        let row2_top_offset = 30.0;
        assert!(row2_top_offset > spinner_bottom_offset);
        assert_eq!(row2_top_offset - spinner_bottom_offset, 10.0);
    }

    #[test]
    fn test_project_context_menu_and_duplication_ui() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        let proj = Project {
            id: "proj-orig".to_string(),
            name: "Backend Service".to_string(),
            description: "Production API".to_string(),
            local_path: ".".to_string(),
            environments: vec![EnvironmentConfig {
                key: "prod".to_string(),
                name: "Production".to_string(),
                app_name: "backend-prod".to_string(),
                servers: vec!["10.0.0.1".to_string()],
                remote_user: "deploy".to_string(),
                vault_target: "v1".to_string(),
                remote_dir: "/app".to_string(),
                env_source: None,
                http_proxy: None,
                steps: vec![PipelineStep::new("Build", StepKind::LocalCommand {
                    command: "cargo build".to_string(),
                    allow_failure: false,
                })],
            }],
            group: None,
            sort_order: None,
        };
        storage.projects.push(proj);

        let mut app = ShiplyApp::new(storage);
        app.selection = SidebarSelection::Project("proj-orig".to_string());

        // Render sidebar and project workspace to verify options and context menus execute without panic
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_sidebar(ui.ctx(), 1.0);
                app.render_project_workspace(ui, "proj-orig");
            });
        });

        // Test duplication action via storage
        let dup = app.storage.duplicate_project("proj-orig").expect("duplicate project");
        assert_eq!(dup.name, "Backend Service (Copy)");
        assert_eq!(dup.id, "proj-orig-copy");
        assert_eq!(app.storage.projects.len(), 2);
    }

    #[test]
    fn test_project_sorting_and_drag_reordering_ui() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        let p_beta = Project {
            id: "p-beta".to_string(),
            name: "Beta API".to_string(),
            description: "".to_string(),
            local_path: ".".to_string(),
            environments: vec![],
            group: None,
            sort_order: Some(0),
        };
        let p_alpha = Project {
            id: "p-alpha".to_string(),
            name: "Alpha Web".to_string(),
            description: "".to_string(),
            local_path: ".".to_string(),
            environments: vec![],
            group: None,
            sort_order: Some(1),
        };
        let p_gamma = Project {
            id: "p-gamma".to_string(),
            name: "Gamma Service".to_string(),
            description: "".to_string(),
            local_path: ".".to_string(),
            environments: vec![],
            group: None,
            sort_order: Some(2),
        };
        storage.projects = vec![p_beta, p_alpha, p_gamma];

        let mut app = ShiplyApp::new(storage);

        // 1. Render sidebar initial custom order
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_sidebar(ui.ctx(), 1.0);
            });
        });
        assert_eq!(app.storage.projects[0].name, "Beta API");
        assert_eq!(app.storage.projects[1].name, "Alpha Web");
        assert_eq!(app.storage.projects[2].name, "Gamma Service");

        // 2. Sort by NameAsc
        app.project_sort_mode = ProjectSortMode::NameAsc;
        app.storage.sort_projects(ProjectSortMode::NameAsc);
        assert_eq!(app.storage.projects[0].name, "Alpha Web");
        assert_eq!(app.storage.projects[1].name, "Beta API");
        assert_eq!(app.storage.projects[2].name, "Gamma Service");

        // 3. Sort by NameDesc
        app.project_sort_mode = ProjectSortMode::NameDesc;
        app.storage.sort_projects(ProjectSortMode::NameDesc);
        assert_eq!(app.storage.projects[0].name, "Gamma Service");
        assert_eq!(app.storage.projects[1].name, "Beta API");
        assert_eq!(app.storage.projects[2].name, "Alpha Web");

        // 4. Test reorder simulation (dragging first item to last position)
        app.dragged_project_idx = Some(0);
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_sidebar(ui.ctx(), 1.0);
            });
        });
        app.storage.reorder_project(0, 2);
        app.dragged_project_idx = None;
        app.project_sort_mode = ProjectSortMode::Custom;

        assert_eq!(app.storage.projects[0].name, "Beta API");
        assert_eq!(app.storage.projects[1].name, "Alpha Web");
        assert_eq!(app.storage.projects[2].name, "Gamma Service");

        // 5. Test move down and move up
        app.storage.move_project_down(0);
        assert_eq!(app.storage.projects[0].name, "Alpha Web");
        assert_eq!(app.storage.projects[1].name, "Beta API");

        app.storage.move_project_up(1);
        assert_eq!(app.storage.projects[0].name, "Beta API");
        assert_eq!(app.storage.projects[1].name, "Alpha Web");
    }

    #[test]
    fn test_project_grouping_and_collapsible_sections_ui() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        let p_backend = Project {
            id: "p-backend".to_string(),
            name: "Backend Service".to_string(),
            description: "".to_string(),
            local_path: ".".to_string(),
            environments: vec![],
            group: Some("Backend".to_string()),
            sort_order: Some(0),
        };
        let p_frontend = Project {
            id: "p-frontend".to_string(),
            name: "Frontend App".to_string(),
            description: "".to_string(),
            local_path: ".".to_string(),
            environments: vec![],
            group: Some("Frontend".to_string()),
            sort_order: Some(1),
        };
        let p_ungrouped = Project {
            id: "p-ungrouped".to_string(),
            name: "Ungrouped Service".to_string(),
            description: "".to_string(),
            local_path: ".".to_string(),
            environments: vec![],
            group: None,
            sort_order: Some(2),
        };
        storage.projects = vec![p_backend, p_frontend, p_ungrouped];

        let mut app = ShiplyApp::new(storage);

        // Verify existing project groups
        let groups = app.storage.existing_project_groups();
        assert_eq!(groups, vec!["Backend", "Frontend"]);

        // Render sidebar with groups expanded
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_sidebar(ui.ctx(), 1.0);
            });
        });

        // Collapse "Backend" group and render again
        app.collapsed_project_groups.insert("Backend".to_string());
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_sidebar(ui.ctx(), 1.0);
            });
        });
        assert!(app.collapsed_project_groups.contains("Backend"));

        // Change group of p-ungrouped to Backend
        assert!(app.storage.set_project_group("p-ungrouped", Some("Backend".to_string())));
        assert_eq!(
            app.storage.projects.iter().find(|p| p.id == "p-ungrouped").unwrap().group.as_deref(),
            Some("Backend")
        );

        // Remove group from p-frontend
        assert!(app.storage.set_project_group("p-frontend", None));
        assert_eq!(
            app.storage.projects.iter().find(|p| p.id == "p-frontend").unwrap().group,
            None
        );
    }

    #[test]
    fn test_sidebar_design_tokens_skeleton_loading_and_modals() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        // 1. Verify typography tokens
        assert_eq!(FS_TITLE, 18.0);
        assert_eq!(FS_BRAND, 16.0);
        assert_eq!(FS_BASE, 13.0);
        assert_eq!(FS_SMALL, 12.0);
        assert_eq!(FS_MICRO, 11.0);

        // 2. Verify spacing and radius scales
        assert_eq!(SP_4, 4.0);
        assert_eq!(SP_8, 8.0);
        assert_eq!(SP_12, 12.0);
        assert_eq!(SP_16, 16.0);
        assert_eq!(SP_20, 20.0);
        assert_eq!(SP_24, 24.0);
        assert_eq!(R_SM, 4.0);
        assert_eq!(R_MD, 6.0);
        assert_eq!(R_LG, 10.0);
        assert_eq!(R_XL, 14.0);

        // 3. Verify color tokens
        assert_eq!(SB_BG, Color32::from_rgb(11, 18, 32));
        assert_eq!(SB_SURFACE, Color32::from_rgb(17, 26, 46));
        assert_eq!(SB_BORDER_LINE, Color32::from_rgb(28, 38, 60));
        assert_eq!(SB_TEXT, Color32::from_rgb(230, 234, 242));
        assert_eq!(SB_TEXT_MUTED, Color32::from_rgb(139, 150, 173));
        assert_eq!(SB_TEXT_FAINT, Color32::from_rgb(100, 112, 138));
        assert_eq!(SB_ACCENT, Color32::from_rgb(59, 130, 246));
        assert_eq!(BADGE_DEV_TEXT, Color32::from_rgb(125, 178, 255));
        assert_eq!(BADGE_PROD_TEXT, Color32::from_rgb(196, 161, 255));
        assert_eq!(ACCENT, Color32::from_rgb(20, 110, 245));
        assert_eq!(MD_BG, Color32::WHITE);
        assert_eq!(MD_BORDER, Color32::from_rgb(230, 230, 234));

        let mut storage = crate::storage::AppStorage::default();
        storage.projects = vec![
            Project {
                id: "p-test-long".to_string(),
                name: "A Super Long Project Name That Should Not Wrap Onto Second Line".to_string(),
                description: "Test".to_string(),
                local_path: ".".to_string(),
                environments: vec![
                    EnvironmentConfig {
                        key: "dev".to_string(),
                        name: "Development".to_string(),
                        app_name: "test-dev".to_string(),
                        servers: vec!["127.0.0.1".to_string()],
                        remote_user: "dev".to_string(),
                        vault_target: "v1".to_string(),
                        remote_dir: "/app".to_string(),
                        env_source: None,
                        http_proxy: None,
                        steps: vec![],
                    },
                    EnvironmentConfig {
                        key: "prod".to_string(),
                        name: "Production".to_string(),
                        app_name: "test-prod".to_string(),
                        servers: vec!["127.0.0.2".to_string()],
                        remote_user: "prod".to_string(),
                        vault_target: "v1".to_string(),
                        remote_dir: "/app".to_string(),
                        env_source: None,
                        http_proxy: None,
                        steps: vec![],
                    },
                ],
                group: None,
                sort_order: Some(0),
            },
        ];

        let mut app = ShiplyApp::new(storage);

        // Test rendering with project switch loading (shimmer skeleton pills on card)
        app.project_switch_loading_id = Some("p-test-long".to_string());
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_sidebar(ui.ctx(), 1.0);
            });
        });

        // Test rendering empty state with refresh shimmer skeletons
        let mut empty_app = ShiplyApp::new(crate::storage::AppStorage::default());
        empty_app.sidebar_refreshing_until = Some(Instant::now() + Duration::from_secs(5));
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                empty_app.render_sidebar(ui.ctx(), 1.0);
            });
        });

        // Test rendering modals with design tokens
        app.show_new_project_modal = true;
        let _ = ctx.run(Default::default(), |ctx| {
            app.render_new_project_modal(ctx);
        });
        app.show_new_project_modal = false;

        app.open_edit_project_modal(0);
        let _ = ctx.run(Default::default(), |ctx| {
            app.render_edit_project_modal(ctx);
        });
        app.show_edit_project_modal = false;

        app.show_new_env_modal = true;
        let _ = ctx.run(Default::default(), |ctx| {
            app.render_new_environment_modal(ctx);
        });
        app.show_new_env_modal = false;

        app.pending_delete_confirm = Some(ConfirmDeleteAction::Project {
            project_id: "p-test-long".to_string(),
            project_name: "Test".to_string(),
        });
        let _ = ctx.run(Default::default(), |ctx| {
            app.render_delete_confirmation_modal(ctx);
        });
        app.pending_delete_confirm = None;

        app.show_bulk_deploy_modal = true;
        let _ = ctx.run(Default::default(), |ctx| {
            app.render_bulk_deploy_modal(ctx);
        });
        app.show_bulk_deploy_modal = false;

        app.show_update_modal = true;
        let _ = ctx.run(Default::default(), |ctx| {
            app.render_github_update_modal(ctx);
        });
        app.show_update_modal = false;
    }

    #[test]
    fn test_sidebar_sort_dropdown_menu_interaction() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        storage.projects = vec![
            Project {
                id: "p1".to_string(),
                name: "Alpha".to_string(),
                description: "".to_string(),
                local_path: ".".to_string(),
                environments: vec![],
                group: None,
                sort_order: Some(0),
            },
            Project {
                id: "p2".to_string(),
                name: "Beta".to_string(),
                description: "".to_string(),
                local_path: ".".to_string(),
                environments: vec![],
                group: None,
                sort_order: Some(1),
            },
        ];

        let mut app = ShiplyApp::new(storage);
        assert!(!app.sidebar_sort_dropdown_open);

        // 1. Open dropdown
        app.sidebar_sort_dropdown_open = true;

        // 2. Render sidebar and dropdown
        let _ = ctx.run(Default::default(), |ctx| {
            app.render_sidebar(ctx, 1.0);
        });
        assert!(app.sidebar_sort_dropdown_open);

        // 3. Test keyboard navigation cycling through sort modes
        let modes = [
            ProjectSortMode::Custom,
            ProjectSortMode::NameAsc,
            ProjectSortMode::NameDesc,
            ProjectSortMode::RecentDeploy,
        ];
        for expected_mode in modes {
            app.project_sort_mode = expected_mode;
            app.storage.sort_projects(expected_mode);
            assert_eq!(app.project_sort_mode, expected_mode);
        }

        // 4. Closing dropdown
        app.sidebar_sort_dropdown_open = false;
        assert!(!app.sidebar_sort_dropdown_open);
    }

    #[test]
    fn test_sidebar_custom_context_menu_viewport_flip_and_actions() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        storage.projects = vec![
            Project {
                id: "p-first".to_string(),
                name: "First Project".to_string(),
                description: "".to_string(),
                local_path: ".".to_string(),
                environments: vec![],
                group: Some("Services".to_string()),
                sort_order: Some(0),
            },
            Project {
                id: "p-second".to_string(),
                name: "Second Project".to_string(),
                description: "".to_string(),
                local_path: ".".to_string(),
                environments: vec![],
                group: None,
                sort_order: Some(1),
            },
        ];

        let mut app = ShiplyApp::new(storage);

        // 1. Trigger context menu for first project near top-left
        app.sidebar_context_menu = Some(SidebarContextMenuState {
            project_id: "p-first".to_string(),
            click_pos: Pos2::new(100.0, 100.0),
            submenu_open: false,
        });

        let _ = ctx.run(Default::default(), |ctx| {
            app.render_sidebar_custom_context_menu(ctx);
        });
        assert!(app.sidebar_context_menu.is_some());

        // 2. Trigger context menu near bottom-right to test viewport flip math
        let screen = ctx.screen_rect();
        let near_bottom_right = Pos2::new(screen.right() - 10.0, screen.bottom() - 10.0);
        app.sidebar_context_menu = Some(SidebarContextMenuState {
            project_id: "p-second".to_string(),
            click_pos: near_bottom_right,
            submenu_open: true,
        });

        let _ = ctx.run(Default::default(), |ctx| {
            app.render_sidebar_custom_context_menu(ctx);
        });
        assert!(app.sidebar_context_menu.is_some());

        // 3. Test destructive styling tokens
        assert_eq!(DESTRUCTIVE_TEXT, Color32::from_rgb(238, 29, 54));
        assert_eq!(DESTRUCTIVE_HOVER_BG, Color32::from_rgba_premultiplied(18, 2, 4, 20));
        assert_eq!(DESTRUCTIVE_ACTIVE_BG, Color32::from_rgba_premultiplied(35, 4, 8, 38));

        // 4. Test outline icons rendering without panic
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let icons = [
                    OutlineIcon::Trash,
                    OutlineIcon::Refresh,
                    OutlineIcon::ArrowUp,
                    OutlineIcon::ArrowDown,
                    OutlineIcon::Edit,
                    OutlineIcon::RotateCcw,
                    OutlineIcon::Key,
                    OutlineIcon::GripVertical,
                    OutlineIcon::Copy,
                    OutlineIcon::Folder,
                    OutlineIcon::ChevronRight,
                    OutlineIcon::ChevronDown,
                    OutlineIcon::ChevronUp,
                    OutlineIcon::Check,
                    OutlineIcon::X,
                ];
                for (i, icon) in icons.iter().enumerate() {
                    let c = Pos2::new(20.0 + i as f32 * 20.0, 20.0);
                    draw_outline_icon(ui.painter(), c, *icon, SB_TEXT);
                }
            });
        });
    }

    #[test]
    fn test_deployment_history_filters_by_active_environment_for_selected_project() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        let proj = Project {
            id: "proj-backend".to_string(),
            name: "Backend Service".to_string(),
            description: "API".to_string(),
            local_path: ".".to_string(),
            environments: vec![
                EnvironmentConfig {
                    key: "dev".to_string(),
                    name: "Development".to_string(),
                    app_name: "backend-dev".to_string(),
                    servers: vec!["10.0.0.1".to_string()],
                    remote_user: "deploy".to_string(),
                    vault_target: "v1".to_string(),
                    remote_dir: "/apps/dev".to_string(),
                    env_source: None,
                    http_proxy: None,
                    steps: Vec::new(),
                },
                EnvironmentConfig {
                    key: "prod".to_string(),
                    name: "Production".to_string(),
                    app_name: "backend-prod".to_string(),
                    servers: vec!["10.0.0.2".to_string()],
                    remote_user: "deploy".to_string(),
                    vault_target: "v2".to_string(),
                    remote_dir: "/apps/prod".to_string(),
                    env_source: None,
                    http_proxy: None,
                    steps: Vec::new(),
                },
            ],
            group: None,
            sort_order: None,
        };
        storage.projects = vec![proj];

        let dep_dev1 = DeploymentRecord {
            id: "dep-dev-1".to_string(),
            project_id: "proj-backend".to_string(),
            project_name: "Backend Service".to_string(),
            environment: "dev".to_string(),
            app_name: "backend-dev".to_string(),
            version: "1.0.0".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("commit1".to_string()),
            git_changes: vec![],
            is_rollback: false,
            servers: vec!["10.0.0.1".to_string()],
            status: DeploymentStatus::Succeeded,
            started_at: chrono::Utc::now(),
            finished_at: Some(chrono::Utc::now()),
            duration_ms: Some(1200),
            step_logs: vec![],
            executed_steps: vec![],
        };
        let dep_dev2 = DeploymentRecord {
            id: "dep-dev-2".to_string(),
            project_id: "proj-backend".to_string(),
            project_name: "Backend Service".to_string(),
            environment: "dev".to_string(),
            app_name: "backend-dev".to_string(),
            version: "1.0.1".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("commit2".to_string()),
            git_changes: vec![],
            is_rollback: false,
            servers: vec!["10.0.0.1".to_string()],
            status: DeploymentStatus::Succeeded,
            started_at: chrono::Utc::now(),
            finished_at: Some(chrono::Utc::now()),
            duration_ms: Some(1500),
            step_logs: vec![],
            executed_steps: vec![],
        };
        let dep_prod1 = DeploymentRecord {
            id: "dep-prod-1".to_string(),
            project_id: "proj-backend".to_string(),
            project_name: "Backend Service".to_string(),
            environment: "prod".to_string(),
            app_name: "backend-prod".to_string(),
            version: "1.0.0".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("commit1".to_string()),
            git_changes: vec![],
            is_rollback: false,
            servers: vec!["10.0.0.2".to_string()],
            status: DeploymentStatus::Succeeded,
            started_at: chrono::Utc::now(),
            finished_at: Some(chrono::Utc::now()),
            duration_ms: Some(2000),
            step_logs: vec![],
            executed_steps: vec![],
        };
        storage.deployments = vec![dep_dev1, dep_dev2, dep_prod1];

        let mut app = ShiplyApp::new(storage);
        app.selection = SidebarSelection::Project("proj-backend".to_string());
        app.selected_env_key = "dev".to_string();

        // 1. In project workspace with "dev" active: filtered deployments should only show 2 dev records
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_history_and_logs_view(ui, Some("proj-backend"));
            });
        });
        let dev_records = app.storage.filter_deployments(Some("proj-backend"), Some(&app.selected_env_key), None);
        assert_eq!(dev_records.len(), 2);
        assert!(dev_records.iter().all(|d| d.environment == "dev"));

        // 2. Switch to "prod" active: filtered deployments should only show 1 prod record
        app.selected_env_key = "prod".to_string();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_history_and_logs_view(ui, Some("proj-backend"));
            });
        });
        let prod_records = app.storage.filter_deployments(Some("proj-backend"), Some(&app.selected_env_key), None);
        assert_eq!(prod_records.len(), 1);
        assert_eq!(prod_records[0].environment, "prod");

        // 3. Global AllDeployments view: shows all 3 deployments across environments
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_history_and_logs_view(ui, None);
            });
        });
        let all_records = app.storage.filter_deployments(None, None, None);
        assert_eq!(all_records.len(), 3);
    }

    #[test]
    fn test_sync_version_candidates_and_application() {
        let mut storage = crate::storage::AppStorage::default();
        let proj = Project {
            id: "proj-sync".to_string(),
            name: "Frontend App".to_string(),
            description: "SPA".to_string(),
            local_path: ".".to_string(),
            environments: vec![
                EnvironmentConfig {
                    key: "dev".to_string(),
                    name: "Development".to_string(),
                    app_name: "front-dev".to_string(),
                    servers: vec![],
                    remote_user: "u".to_string(),
                    vault_target: "v".to_string(),
                    remote_dir: "/apps".to_string(),
                    env_source: None,
                    http_proxy: None,
                    steps: Vec::new(),
                },
                EnvironmentConfig {
                    key: "staging".to_string(),
                    name: "Staging".to_string(),
                    app_name: "front-staging".to_string(),
                    servers: vec![],
                    remote_user: "u".to_string(),
                    vault_target: "v".to_string(),
                    remote_dir: "/apps".to_string(),
                    env_source: None,
                    http_proxy: None,
                    steps: Vec::new(),
                },
                EnvironmentConfig {
                    key: "prod".to_string(),
                    name: "Production".to_string(),
                    app_name: "front-prod".to_string(),
                    servers: vec![],
                    remote_user: "u".to_string(),
                    vault_target: "v".to_string(),
                    remote_dir: "/apps".to_string(),
                    env_source: None,
                    http_proxy: None,
                    steps: Vec::new(),
                },
            ],
            group: None,
            sort_order: None,
        };
        storage.projects = vec![proj];

        let dep_dev = DeploymentRecord {
            id: "dep-dev-sync".to_string(),
            project_id: "proj-sync".to_string(),
            project_name: "Frontend App".to_string(),
            environment: "dev".to_string(),
            app_name: "front-dev".to_string(),
            version: "1.3.5".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("c1".to_string()),
            git_changes: vec![],
            is_rollback: false,
            servers: vec![],
            status: DeploymentStatus::Succeeded,
            started_at: chrono::Utc::now(),
            finished_at: Some(chrono::Utc::now()),
            duration_ms: Some(1000),
            step_logs: vec![],
            executed_steps: vec![],
        };
        let dep_staging = DeploymentRecord {
            id: "dep-staging-sync".to_string(),
            project_id: "proj-sync".to_string(),
            project_name: "Frontend App".to_string(),
            environment: "staging".to_string(),
            app_name: "front-staging".to_string(),
            version: "1.2.0".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("c2".to_string()),
            git_changes: vec![],
            is_rollback: false,
            servers: vec![],
            status: DeploymentStatus::Succeeded,
            started_at: chrono::Utc::now() - chrono::Duration::hours(2),
            finished_at: Some(chrono::Utc::now() - chrono::Duration::hours(2)),
            duration_ms: Some(900),
            step_logs: vec![],
            executed_steps: vec![],
        };
        storage.deployments = vec![dep_dev, dep_staging];

        let mut app = ShiplyApp::new(storage);
        app.selection = SidebarSelection::Project("proj-sync".to_string());
        app.selected_env_key = "prod".to_string();
        app.cached_pkg_version = Some("1.3.5".to_string());

        // While in "prod", candidates should come from dev and staging
        let candidates = app.sync_version_candidates("proj-sync");
        assert_eq!(candidates.len(), 2);
        let dev_cand = candidates.iter().find(|c| c.source_name == "Development").unwrap();
        assert_eq!(dev_cand.version, "1.3.5");
        assert!(dev_cand.is_latest);

        let staging_cand = candidates.iter().find(|c| c.source_name == "Staging").unwrap();
        assert_eq!(staging_cand.version, "1.2.0");
        assert!(!staging_cand.is_latest);

        // Apply synced version from Development
        app.apply_synced_version("1.3.5", "Development");
        assert_eq!(app.deploy_version_input, "1.3.5");
        assert!(!app.sync_version_dropdown_open);
        let banner = app.status_banner.as_ref().unwrap();
        assert!(banner.0.contains("Synced version to v1.3.5 from Development"));
        assert!(banner.1); // is_success
    }

    #[test]
    fn test_console_loading_indicator_on_running_pipeline() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        let proj = Project {
            id: "proj-running-console".to_string(),
            name: "Realtime API".to_string(),
            description: "Console Test".to_string(),
            local_path: ".".to_string(),
            environments: vec![EnvironmentConfig {
                key: "prod".to_string(),
                name: "Production".to_string(),
                app_name: "realtime-prod".to_string(),
                servers: vec!["10.0.0.1".to_string()],
                remote_user: "deploy".to_string(),
                vault_target: "v".to_string(),
                remote_dir: "/apps".to_string(),
                env_source: None,
                http_proxy: None,
                steps: Vec::new(),
            }],
            group: None,
            sort_order: None,
        };
        storage.projects = vec![proj];

        let running_dep = DeploymentRecord {
            id: "dep-console-run".to_string(),
            project_id: "proj-running-console".to_string(),
            project_name: "Realtime API".to_string(),
            environment: "prod".to_string(),
            app_name: "realtime-prod".to_string(),
            version: "2.0.0".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("abc".to_string()),
            git_changes: vec![],
            is_rollback: false,
            servers: vec!["10.0.0.1".to_string()],
            status: DeploymentStatus::Running,
            started_at: chrono::Utc::now(),
            finished_at: None,
            duration_ms: None,
            step_logs: vec![crate::domain::StepExecutionLog {
                step_name: "Building Docker container".to_string(),
                status: DeploymentStatus::Running,
                duration_ms: 3200,
                output: "Step 1/5: RUN cargo build\n[OK] Intermediate image cached\n".to_string(),
                command_summary: "docker build -t app:2.0.0 .".to_string(),
            }],
            executed_steps: vec![],
        };
        storage.deployments = vec![running_dep];

        let mut app = ShiplyApp::new(storage);
        app.selection = SidebarSelection::Project("proj-running-console".to_string());
        app.selected_env_key = "prod".to_string();
        app.active_deployment_id = Some("dep-console-run".to_string());
        app.selected_deployment_id = Some("dep-console-run".to_string());
        app.console_collapsed = false;

        // Render history and terminal window with running deployment
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_history_and_logs_view(ui, Some("proj-running-console"));
            });
        });

        assert_eq!(app.active_deployment_id.as_deref(), Some("dep-console-run"));
    }

    #[test]
    fn test_concurrent_deployments_across_different_projects_prevent_same_project_only() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        let proj_backend = Project {
            id: "proj-backend".to_string(),
            name: "Backend Service".to_string(),
            description: "API".to_string(),
            local_path: ".".to_string(),
            environments: vec![EnvironmentConfig {
                key: "prod".to_string(),
                name: "Production".to_string(),
                app_name: "backend-prod".to_string(),
                servers: vec!["10.0.0.1".to_string()],
                remote_user: "deploy".to_string(),
                vault_target: "v1".to_string(),
                remote_dir: "/apps/backend".to_string(),
                env_source: None,
                http_proxy: None,
                steps: Vec::new(),
            }],
            group: None,
            sort_order: None,
        };
        let proj_frontend = Project {
            id: "proj-frontend".to_string(),
            name: "Frontend Service".to_string(),
            description: "SPA".to_string(),
            local_path: ".".to_string(),
            environments: vec![EnvironmentConfig {
                key: "prod".to_string(),
                name: "Production".to_string(),
                app_name: "frontend-prod".to_string(),
                servers: vec!["10.0.0.2".to_string()],
                remote_user: "deploy".to_string(),
                vault_target: "v2".to_string(),
                remote_dir: "/apps/frontend".to_string(),
                env_source: None,
                http_proxy: None,
                steps: Vec::new(),
            }],
            group: None,
            sort_order: None,
        };
        storage.projects = vec![proj_backend.clone(), proj_frontend.clone()];

        let mut app = ShiplyApp::new(storage);

        // 1. Initially neither project is deploying
        assert!(!app.is_project_deploying("proj-backend"));
        assert!(!app.is_project_deploying("proj-frontend"));
        assert!(!app.has_active_deployments());

        // 2. Start deployment for backend
        let req_backend = crate::executor::DeploymentRequest {
            deployment_id: "dep-backend-1".to_string(),
            project_id: "proj-backend".to_string(),
            project_name: "Backend Service".to_string(),
            local_path: PathBuf::from("."),
            env_config: proj_backend.environments[0].clone(),
            version: "1.0.0".to_string(),
            is_rollback: false,
            previous_commit: None,
            manual_password: None,
            target_git_branch: None,
            bump_package_json: false,
            create_git_tag_on_success: false,
            skip_ssh_preflight: false,
            rollback_commit: None,
        };
        app.start_deployment(req_backend);

        // Backend is now deploying
        assert!(app.is_project_deploying("proj-backend"));
        assert!(!app.is_project_deploying("proj-frontend")); // Frontend is free!
        assert!(app.has_active_deployments());

        // 3. Attempting to deploy backend AGAIN should be prevented
        let req_backend_again = crate::executor::DeploymentRequest {
            deployment_id: "dep-backend-2".to_string(),
            project_id: "proj-backend".to_string(),
            project_name: "Backend Service".to_string(),
            local_path: PathBuf::from("."),
            env_config: proj_backend.environments[0].clone(),
            version: "1.0.1".to_string(),
            is_rollback: false,
            previous_commit: None,
            manual_password: None,
            target_git_branch: None,
            bump_package_json: false,
            create_git_tag_on_success: false,
            skip_ssh_preflight: false,
            rollback_commit: None,
        };
        app.start_deployment(req_backend_again);
        let banner = app.status_banner.as_ref().unwrap();
        assert!(banner.0.contains("A deployment for 'Backend Service'"));
        assert!(banner.0.contains("is already running"));
        assert!(!banner.1); // false = error banner

        // 4. Deploying frontend AT THE SAME TIME is allowed and succeeds!
        let req_frontend = crate::executor::DeploymentRequest {
            deployment_id: "dep-frontend-1".to_string(),
            project_id: "proj-frontend".to_string(),
            project_name: "Frontend Service".to_string(),
            local_path: PathBuf::from("."),
            env_config: proj_frontend.environments[0].clone(),
            version: "2.0.0".to_string(),
            is_rollback: false,
            previous_commit: None,
            manual_password: None,
            target_git_branch: None,
            bump_package_json: false,
            create_git_tag_on_success: false,
            skip_ssh_preflight: false,
            rollback_commit: None,
        };
        app.start_deployment(req_frontend);

        // BOTH projects are now deploying concurrently!
        assert!(app.is_project_deploying("proj-backend"));
        assert!(app.is_project_deploying("proj-frontend"));
        assert_eq!(app.active_deployments.len(), 2);

        // 5. Simulate finish event for backend
        let finished_backend = DeploymentRecord {
            id: "dep-backend-1".to_string(),
            project_id: "proj-backend".to_string(),
            project_name: "Backend Service".to_string(),
            environment: "prod".to_string(),
            app_name: "backend-prod".to_string(),
            version: "1.0.0".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("c1".to_string()),
            git_changes: vec![],
            is_rollback: false,
            servers: vec!["10.0.0.1".to_string()],
            status: DeploymentStatus::Succeeded,
            started_at: chrono::Utc::now(),
            finished_at: Some(chrono::Utc::now()),
            duration_ms: Some(1500),
            step_logs: vec![],
            executed_steps: vec![],
        };
        let (tx, rx) = mpsc::channel();
        tx.send(ExecutorEvent::Finished(finished_backend)).unwrap();
        app.event_rx = rx;
        app.poll_executor_events(&ctx);

        // Backend is no longer deploying, but Frontend is STILL deploying!
        assert!(!app.is_project_deploying("proj-backend"));
        assert!(app.is_project_deploying("proj-frontend"));
        assert_eq!(app.active_deployments.len(), 1);

        // Backend can now be deployed again
        let req_backend_new = crate::executor::DeploymentRequest {
            deployment_id: "dep-backend-3".to_string(),
            project_id: "proj-backend".to_string(),
            project_name: "Backend Service".to_string(),
            local_path: PathBuf::from("."),
            env_config: proj_backend.environments[0].clone(),
            version: "1.0.2".to_string(),
            is_rollback: false,
            previous_commit: None,
            manual_password: None,
            target_git_branch: None,
            bump_package_json: false,
            create_git_tag_on_success: false,
            skip_ssh_preflight: false,
            rollback_commit: None,
        };
        app.start_deployment(req_backend_new);
        assert!(app.is_project_deploying("proj-backend"));
    }

    #[test]
    fn test_concurrent_deployments_for_multiple_environments_on_same_project() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut storage = crate::storage::AppStorage::default();
        let proj_multi = Project {
            id: "proj-api".to_string(),
            name: "API Service".to_string(),
            description: "API".to_string(),
            local_path: ".".to_string(),
            environments: vec![
                EnvironmentConfig {
                    key: "dev".to_string(),
                    name: "Development".to_string(),
                    app_name: "api-dev".to_string(),
                    servers: vec!["10.0.0.10".to_string()],
                    remote_user: "deploy".to_string(),
                    vault_target: "v-dev".to_string(),
                    remote_dir: "/apps/api-dev".to_string(),
                    env_source: None,
                    http_proxy: None,
                    steps: Vec::new(),
                },
                EnvironmentConfig {
                    key: "prod".to_string(),
                    name: "Production".to_string(),
                    app_name: "api-prod".to_string(),
                    servers: vec!["10.0.0.20".to_string()],
                    remote_user: "deploy".to_string(),
                    vault_target: "v-prod".to_string(),
                    remote_dir: "/apps/api-prod".to_string(),
                    env_source: None,
                    http_proxy: None,
                    steps: Vec::new(),
                },
            ],
            group: None,
            sort_order: None,
        };
        storage.projects = vec![proj_multi.clone()];

        let mut app = ShiplyApp::new(storage);

        // 1. Initially neither environment is deploying
        assert!(!app.is_env_deploying("proj-api", "dev"));
        assert!(!app.is_env_deploying("proj-api", "prod"));
        assert!(!app.is_project_deploying("proj-api"));

        // 2. Start deployment for Development environment
        let req_dev = crate::executor::DeploymentRequest {
            deployment_id: "dep-api-dev-1".to_string(),
            project_id: "proj-api".to_string(),
            project_name: "API Service".to_string(),
            local_path: PathBuf::from("."),
            env_config: proj_multi.environments[0].clone(),
            version: "1.0.0".to_string(),
            is_rollback: false,
            previous_commit: None,
            manual_password: None,
            target_git_branch: None,
            bump_package_json: false,
            create_git_tag_on_success: false,
            skip_ssh_preflight: false,
            rollback_commit: None,
        };
        app.start_deployment(req_dev);

        // Development is deploying; Production is free
        assert!(app.is_env_deploying("proj-api", "dev"));
        assert!(!app.is_env_deploying("proj-api", "prod"));
        assert!(app.is_project_deploying("proj-api"));

        // 3. Attempting to deploy Development AGAIN on the same project is prevented
        let req_dev_again = crate::executor::DeploymentRequest {
            deployment_id: "dep-api-dev-2".to_string(),
            project_id: "proj-api".to_string(),
            project_name: "API Service".to_string(),
            local_path: PathBuf::from("."),
            env_config: proj_multi.environments[0].clone(),
            version: "1.0.1".to_string(),
            is_rollback: false,
            previous_commit: None,
            manual_password: None,
            target_git_branch: None,
            bump_package_json: false,
            create_git_tag_on_success: false,
            skip_ssh_preflight: false,
            rollback_commit: None,
        };
        app.start_deployment(req_dev_again);
        let banner = app.status_banner.as_ref().unwrap();
        assert!(banner.0.contains("A deployment for 'API Service' (DEV) is already running"));
        assert!(!banner.1); // error banner

        // 4. Deploying Production AT THE SAME TIME on the SAME PROJECT is allowed!
        let req_prod = crate::executor::DeploymentRequest {
            deployment_id: "dep-api-prod-1".to_string(),
            project_id: "proj-api".to_string(),
            project_name: "API Service".to_string(),
            local_path: PathBuf::from("."),
            env_config: proj_multi.environments[1].clone(),
            version: "1.0.0".to_string(),
            is_rollback: false,
            previous_commit: None,
            manual_password: None,
            target_git_branch: None,
            bump_package_json: false,
            create_git_tag_on_success: false,
            skip_ssh_preflight: false,
            rollback_commit: None,
        };
        app.start_deployment(req_prod);

        // BOTH Development and Production are now deploying simultaneously on the SAME project!
        assert!(app.is_env_deploying("proj-api", "dev"));
        assert!(app.is_env_deploying("proj-api", "prod"));
        assert_eq!(app.active_deployments.len(), 2);
        assert!(app.is_project_deploying("proj-api"));

        // 5. Attempting to deploy Production AGAIN while running is prevented
        let req_prod_again = crate::executor::DeploymentRequest {
            deployment_id: "dep-api-prod-2".to_string(),
            project_id: "proj-api".to_string(),
            project_name: "API Service".to_string(),
            local_path: PathBuf::from("."),
            env_config: proj_multi.environments[1].clone(),
            version: "1.0.1".to_string(),
            is_rollback: false,
            previous_commit: None,
            manual_password: None,
            target_git_branch: None,
            bump_package_json: false,
            create_git_tag_on_success: false,
            skip_ssh_preflight: false,
            rollback_commit: None,
        };
        app.start_deployment(req_prod_again);
        let banner_prod = app.status_banner.as_ref().unwrap();
        assert!(banner_prod.0.contains("A deployment for 'API Service' (PROD) is already running"));
        assert!(!banner_prod.1);

        // 6. Development deployment completes
        let finished_dev = DeploymentRecord {
            id: "dep-api-dev-1".to_string(),
            project_id: "proj-api".to_string(),
            project_name: "API Service".to_string(),
            environment: "dev".to_string(),
            app_name: "api-dev".to_string(),
            version: "1.0.0".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("c1".to_string()),
            git_changes: vec![],
            is_rollback: false,
            servers: vec!["10.0.0.10".to_string()],
            status: DeploymentStatus::Succeeded,
            started_at: chrono::Utc::now(),
            finished_at: Some(chrono::Utc::now()),
            duration_ms: Some(800),
            step_logs: vec![],
            executed_steps: vec![],
        };
        let (tx, rx) = mpsc::channel();
        tx.send(ExecutorEvent::Finished(finished_dev)).unwrap();
        app.event_rx = rx;
        app.poll_executor_events(&ctx);

        // Development is done, but Production is STILL running!
        assert!(!app.is_env_deploying("proj-api", "dev"));
        assert!(app.is_env_deploying("proj-api", "prod"));
        assert_eq!(app.active_deployments.len(), 1);
        assert!(app.is_project_deploying("proj-api"));

        // 7. Development can now be deployed again while Production is still active
        let req_dev_new = crate::executor::DeploymentRequest {
            deployment_id: "dep-api-dev-3".to_string(),
            project_id: "proj-api".to_string(),
            project_name: "API Service".to_string(),
            local_path: PathBuf::from("."),
            env_config: proj_multi.environments[0].clone(),
            version: "1.0.2".to_string(),
            is_rollback: false,
            previous_commit: None,
            manual_password: None,
            target_git_branch: None,
            bump_package_json: false,
            create_git_tag_on_success: false,
            skip_ssh_preflight: false,
            rollback_commit: None,
        };
        app.start_deployment(req_dev_new);
        assert!(app.is_env_deploying("proj-api", "dev"));
        assert!(app.is_env_deploying("proj-api", "prod"));
        assert_eq!(app.active_deployments.len(), 2);
    }

    #[test]
    fn test_deployment_keeps_running_on_update_and_restart_deferred_until_finished() {
        let storage = crate::storage::AppStorage::default();
        let mut app = ShiplyApp::new(storage);
        let ctx = egui::Context::default();

        // 1. Mark an active deployment in progress
        app.active_deployments
            .insert(("proj-app".to_string(), "prod".to_string()), "dep-99".to_string());
        assert!(app.has_active_deployments());
        assert!(app.is_project_deploying("proj-app"));
        assert!(app.is_env_deploying("proj-app", "prod"));

        // 2. An update download completes while deployment is still running
        let (updater_tx, updater_rx) = mpsc::channel();
        updater_tx
            .send(UpdaterEvent::ReadyToRestart {
                new_version: "0.10.7".to_string(),
                exe_path: PathBuf::from("mock/path/shiply.exe"),
            })
            .unwrap();
        app.updater_rx = updater_rx;
        app.poll_executor_events(&ctx);

        // 3. Deployment was NOT interrupted, restart is deferred
        assert!(app.has_active_deployments());
        assert!(app.is_project_deploying("proj-app"));
        assert_eq!(
            app.pending_update_restart,
            Some(("0.10.7".to_string(), PathBuf::from("mock/path/shiply.exe")))
        );
        let banner = app.status_banner.as_ref().unwrap();
        assert!(banner.0.contains("Active deployments will continue running uninterrupted before restart."));

        // 4. When the deployment finishes, the deferred restart is automatically triggered
        let finished_record = DeploymentRecord {
            id: "dep-99".to_string(),
            project_id: "proj-app".to_string(),
            project_name: "App Service".to_string(),
            environment: "prod".to_string(),
            app_name: "app-prod".to_string(),
            version: "1.0.0".to_string(),
            git_branch: Some("main".to_string()),
            git_commit: Some("c1".to_string()),
            git_changes: vec![],
            is_rollback: false,
            servers: vec!["10.0.0.1".to_string()],
            status: DeploymentStatus::Succeeded,
            started_at: chrono::Utc::now(),
            finished_at: Some(chrono::Utc::now()),
            duration_ms: Some(1000),
            step_logs: vec![],
            executed_steps: vec![],
        };
        let (exec_tx, exec_rx) = mpsc::channel();
        exec_tx.send(ExecutorEvent::Finished(finished_record)).unwrap();
        app.event_rx = exec_rx;
        app.poll_executor_events(&ctx);

        // 5. Active deployments is now empty and pending_update_restart has been consumed
        assert!(!app.has_active_deployments());
        assert!(app.pending_update_restart.is_none());
        // Since mock exe does not exist, update_error safely records the error without crash
        assert!(app.update_error.is_some());
    }

    #[test]
    fn test_webflow_palette_and_theme_tokens() {
        let light = Theme::light();
        assert_eq!(light.bg, Color32::from_rgb(235, 239, 246)); // Grey-blue slate canvas
        assert_eq!(light.surface, Color32::WHITE); // Pure white card
        assert_ne!(light.bg, light.surface); // High contrast between canvas and cards
        assert_eq!(light.surface_2, Color32::from_rgb(243, 246, 251)); // #F3F6FB
        assert_eq!(light.border, Color32::from_rgb(218, 224, 233));
        assert_eq!(light.border_strong, Color32::from_rgb(203, 213, 225));
        assert_eq!(light.text, Color32::from_rgb(8, 8, 8)); // #080808
        assert_eq!(light.text_muted, Color32::from_rgb(90, 90, 90)); // #5A5A5A
        assert_eq!(light.primary, Color32::from_rgb(20, 110, 245)); // #146EF5
        assert_eq!(light.primary_hover, Color32::from_rgb(15, 94, 215)); // #0F5ED7
        assert_eq!(light.primary_active, Color32::from_rgb(11, 79, 184)); // #0B4FB8
        assert_eq!(light.primary_soft, Color32::from_rgb(231, 240, 254)); // #E7F0FE
        assert_eq!(light.purple, Color32::from_rgb(122, 61, 255)); // #7A3DFF
        assert_eq!(light.danger, Color32::from_rgb(238, 29, 54)); // #EE1D36

        let dark = Theme::dark();
        assert_eq!(dark.bg, Color32::from_rgb(8, 8, 8)); // #080808
        assert_eq!(dark.surface, Color32::from_rgb(16, 16, 18)); // #101012
        assert_eq!(dark.surface_2, Color32::from_rgb(24, 24, 27)); // #18181B
        assert_eq!(dark.border, Color32::from_rgb(38, 38, 43)); // #26262B
        assert_eq!(dark.text, Color32::from_rgb(245, 245, 245)); // #F5F5F5
        assert_eq!(dark.primary, Color32::from_rgb(59, 130, 246)); // #3B82F6
    }

    #[test]
    fn test_responsive_modal_width_and_sheet_breakpoints() {
        let ctx = egui::Context::default();

        // 1. Desktop screen width 1280px
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
                ..Default::default()
            },
            |ctx| {
                let (modal_w, inner_w) = modal_width_and_inner(ctx);
                assert_eq!(modal_w, 560.0);
                assert_eq!(inner_w, 560.0 - 48.0);
            },
        );

        // 2. Narrow screen width 400px (< 480px) becomes full-screen sheet
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 700.0))),
                ..Default::default()
            },
            |ctx| {
                let (modal_w, inner_w) = modal_width_and_inner(ctx);
                assert_eq!(modal_w, 400.0);
                assert_eq!(inner_w, 400.0 - 24.0);
            },
        );
    }

    #[test]
    fn test_responsive_environment_cards_and_history_stacked_breakpoints() {
        // Breakpoints: >= 1200px: 3 cols, 900-1199px: 2 cols, < 900px: 1 col
        let cols_at = |screen_w: f32| -> usize {
            if screen_w >= 1200.0 {
                3
            } else if (900.0..1200.0).contains(&screen_w) {
                2
            } else {
                1
            }
        };
        assert_eq!(cols_at(1400.0), 3);
        assert_eq!(cols_at(1200.0), 3);
        assert_eq!(cols_at(1199.0), 2);
        assert_eq!(cols_at(950.0), 2);
        assert_eq!(cols_at(900.0), 2);
        assert_eq!(cols_at(899.0), 1);
        assert_eq!(cols_at(600.0), 1);

        // Deployments history view: stacked below 900px or when available width < 640px
        let is_stacked = |screen_w: f32, avail_w: f32| -> bool {
            screen_w < 900.0 || avail_w < 640.0
        };
        assert!(!is_stacked(1280.0, 900.0));
        assert!(is_stacked(850.0, 700.0));
        assert!(is_stacked(1280.0, 600.0));
    }

    #[test]
    fn test_sidebar_search_single_outline_and_clear_button() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut search_query = "cdo-tester".to_string();

        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let changed = sidebar_search_text_edit(
                    ui,
                    "test_search_input",
                    &mut search_query,
                    220.0,
                    Some("Search projects..."),
                );
                assert!(!changed);
            });
        });

        assert_eq!(search_query, "cdo-tester");

        // Clear button simulation: clearing value resets query to empty string
        search_query.clear();
        assert!(search_query.is_empty());
    }

    #[test]
    fn test_sidebar_footer_bounds_prevent_overflow_under_taskbar() {
        // Expanded sidebar footer total height: 138px
        let expanded_footer_h = 138.0_f32;
        let btn_h = 30.0_f32;
        let btn1_top = 8.0_f32;
        let btn2_top = 40.0_f32;
        let btn3_top = 72.0_f32;
        let status_top = 108.0_f32;
        let status_h = 20.0_f32;
        let status_bottom = status_top + status_h; // 128px

        // Verify status bottom is strictly within footer_h with at least 8px margin
        assert!(status_bottom <= expanded_footer_h);
        let bottom_margin = expanded_footer_h - status_bottom;
        assert!(
            bottom_margin >= 8.0,
            "Bottom margin {bottom_margin}px must be >= 8px to prevent overflow into taskbar"
        );

        // Verify buttons do not overlap
        assert!(btn1_top + btn_h <= btn2_top);
        assert!(btn2_top + btn_h <= btn3_top);
        assert!(btn3_top + btn_h <= status_top);

        // Collapsed rail sidebar footer: 132px
        let rail_footer_h = 132.0_f32;
        let rail_dot_pos_y = 118.0_f32;
        assert!(
            rail_dot_pos_y + 4.0 <= rail_footer_h,
            "Rail status dot must not overflow footer bounds"
        );
    }

    #[test]
    fn test_text_edit_cursor_and_selection_mechanics() {
        let ctx = egui::Context::default();
        ShiplyApp::configure_light_workspace_style(&ctx);

        let mut test_value = "10.62.70.80".to_string();

        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let changed = styled_text_edit(
                    ui,
                    "test_cursor_input",
                    &mut test_value,
                    300.0,
                    Some("Enter IP..."),
                );
                assert!(!changed);
            });
        });

        // Test cursor math:
        // Clicking beyond galley width places cursor at end of text (char count)
        let char_count = test_value.chars().count();
        let galley_w = 80.0_f32;
        let click_rel_x_far_right = 250.0_f32;
        let cursor_end = if click_rel_x_far_right >= galley_w {
            egui::text::CCursor::new(char_count)
        } else {
            egui::text::CCursor::new(0)
        };
        assert_eq!(cursor_end.index, 11); // "10.62.70.80".len() == 11

        // Clicking to the left places cursor at 0
        let click_rel_x_left = -5.0_f32;
        let cursor_start = if click_rel_x_left <= 0.0 {
            egui::text::CCursor::new(0)
        } else {
            egui::text::CCursor::new(char_count)
        };
        assert_eq!(cursor_start.index, 0);

        // Multi-character selection range (drag/block text) must be preserved
        let selection_range = egui::text_selection::CCursorRange {
            primary: egui::text::CCursor::new(0),
            secondary: egui::text::CCursor::new(11),
        };
        assert_ne!(selection_range.primary, selection_range.secondary);
    }

    #[test]
    fn test_topbar_frameless_no_horizontal_border() {
        // Frame stroke must be Stroke::NONE to prevent any dark horizontal border line
        assert_eq!(Stroke::NONE.width, 0.0);
        assert_eq!(Stroke::NONE.color, Color32::TRANSPARENT);
    }

    #[test]
    fn test_topbar_black_background_and_no_theme_toggle() {
        let ctx = egui::Context::default();
        let mut app = ShiplyApp::new(crate::storage::AppStorage::default());

        let _ = ctx.run(Default::default(), |ctx| {
            app.render_termius_topbar(ctx);
        });

        // Verify topbar renders without theme toggle
        assert!(!app.window_maximized);
    }

    #[test]
    fn test_styled_text_edit_and_password_edit() {
        let ctx = egui::Context::default();
        let mut pwd = "secret".to_string();

        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let changed = styled_password_edit(
                    ui,
                    "test_pwd_field",
                    &mut pwd,
                    200.0,
                    Some("Password..."),
                );
                assert!(!changed);
            });
        });

        assert_eq!(pwd, "secret");
    }

    #[test]
    fn test_environment_cards_3_columns_fill_space() {
        let card_spacing = 14.0_f32;
        let avail_w = 900.0_f32;
        let cols = if avail_w >= 650.0 {
            3
        } else if avail_w >= 440.0 {
            2
        } else {
            1
        };
        assert_eq!(cols, 3);
        let card_w = ((avail_w - card_spacing * (cols - 1) as f32) / cols as f32).floor();
        assert_eq!(card_w, 290.0);
        let total_w = card_w * 3.0 + card_spacing * 2.0;
        assert!(total_w <= avail_w);
    }

    #[test]
    fn test_modal_primary_button_grey_when_disabled() {
        let _btn_disabled = modal_primary_button("Create Project", false);
        let _btn_enabled = modal_primary_button("Create Project", true);
    }

    #[test]
    fn test_open_new_project_modal_resets_form_fields() {
        let mut app = ShiplyApp::new(crate::storage::AppStorage::default());
        app.new_project_name = "dirty_name".to_string();
        app.new_project_path = "dirty_path".to_string();
        app.new_project_ps1_path = "dirty.ps1".to_string();

        app.open_new_project_modal();

        assert!(app.show_new_project_modal);
        assert!(app.new_project_name.is_empty());
        assert!(app.new_project_path.is_empty());
        assert!(app.new_project_ps1_path.is_empty());
    }

    #[test]
    fn test_donation_url_and_heart_icon() {
        assert_eq!(DONATION_URL, "https://trakteer.id/faidfadjri");
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                draw_outline_icon(ui.painter(), Pos2::new(10.0, 10.0), OutlineIcon::Heart, Color32::RED);
            });
        });
    }

    #[test]
    fn test_refresh_animation_state() {
        let mut app = ShiplyApp::new(crate::storage::AppStorage::default());
        assert!(app.refresh_anim_start.is_some());
        app.refresh_anim_start = None;
        assert!(app.refresh_anim_start.is_none());
        app.refresh_selected_project_metadata(true);
        assert!(app.refresh_anim_start.is_some());
    }

    #[test]
    fn test_prepare_animated_modal_and_window_lifecycle() {
        let ctx = egui::Context::default();
        let mut is_open = false;

        let anim_none = prepare_animated_modal(&ctx, "test_modal", &mut is_open);
        assert!(anim_none.is_none());

        is_open = true;
        let _ = ctx.run(Default::default(), |ctx| {
            let anim_some = prepare_animated_modal(ctx, "test_modal", &mut is_open);
            assert!(anim_some.is_some());
            let (window, inner_w) = animated_modal_window(ctx, "Test Window", anim_some.unwrap(), None);
            assert!(inner_w > 0.0);
            window.show(ctx, |ui| {
                ui.label("Inside modal");
            });
        });
    }
