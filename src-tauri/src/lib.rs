#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;
use std::fs;

mod commands;
mod security;
mod services;
mod startup_diagnostics;
mod utils;
mod windows;

pub use utils::{mouse, screen};
pub use services::{AppSettings, get_settings, update_settings, get_data_directory, hotkey, SoundPlayer, AppSounds};
pub use services::system::input_monitor;
pub use services::system::focus;
#[cfg(target_os = "linux")]
pub use services::system::ipc_socket::{start_ipc_server, send_command, is_server_running};
pub use services::clipboard::{
    start_clipboard_monitor, stop_clipboard_monitor,
    is_monitor_running as is_clipboard_monitor_running,
    set_app_handle as set_clipboard_app_handle,
};
pub use windows::main_window::{
    get_main_window, is_main_window_visible, show_main_window, hide_main_window,
    toggle_main_window_visibility, start_drag, stop_drag, is_dragging, check_snap, 
    snap_to_edge, restore_from_snap, is_window_snapped, hide_snapped_window, 
    show_snapped_window, init_edge_monitor, WindowState, SnapEdge, get_window_state, 
    set_window_state,
};
pub use utils::positioning::{position_at_cursor, center_window, get_window_bounds};
pub use windows::tray::setup_tray;
pub use windows::settings_window::open_settings_window;
pub use windows::quickpaste;
pub use windows::plugins::context_menu::is_context_menu_visible;
pub use services::low_memory::{is_low_memory_mode, enter_low_memory_mode, exit_low_memory_mode};
pub use startup_diagnostics::install_panic_hook as install_startup_panic_hook;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    startup_diagnostics::set_startup_stage("执行启动安全检查");
    startup_diagnostics::mark_starting();
    security::check_webview_security();
    if let Some(message) = startup_diagnostics::detect_blocking_previous_instance() {
        startup_diagnostics::show_error_dialog("QuickClipboard 检测到异常旧进程", &message);
        return;
    }
    #[cfg(windows)]
    {
        use std::process::Command;
        startup_diagnostics::set_startup_stage("处理安装器启动参数");
        let args: Vec<String> = std::env::args().collect();
        let is_installer_launch = args.iter().any(|a| a == "--installer-launch");
        let already_restarted = args.iter().any(|a| a == "--qc-restarted");
        if is_installer_launch && !already_restarted {
            if let Ok(exe) = std::env::current_exe() {
                let mut cmd = Command::new(exe);
                for a in std::env::args().skip(1) {
                    if a == "--installer-launch" {
                        continue;
                    }
                    cmd.arg(a);
                }
                cmd.arg("--qc-restarted");
                let _ = cmd.spawn();
                std::process::exit(0);
            }
        }

        startup_diagnostics::set_startup_stage("检查管理员启动配置");
        #[cfg(not(debug_assertions))]
        if let Ok(settings) = services::settings::load_settings_from_file() {
            if settings.run_as_admin {
                startup_diagnostics::set_startup_stage("检查管理员启动：检测当前进程权限");
                let is_admin = services::system::is_running_as_admin();
                
                if is_admin {
                    startup_diagnostics::set_startup_stage("检查管理员启动：当前已是管理员，准备同步计划任务");
                    let _ = services::system::create_scheduled_task();
                } else {
                    startup_diagnostics::set_startup_stage("检查管理员启动：当前不是管理员，准备提权重启");
                    if services::system::elevate::try_elevate_and_restart() {
                        std::process::exit(0);
                    }
                }
            }
        }
        #[cfg(debug_assertions)]
        {
            eprintln!("[dev] 开发模式：跳过管理员提权检查（run_as_admin 设置在 release 模式下生效）");
        }
    }
    
    startup_diagnostics::set_startup_stage("构建 Tauri 应用");
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if services::low_memory::is_low_memory_mode() {
                let _ = services::low_memory::toggle_panel();
                return;
            }
            let action = argv.iter().find_map(|a| {
                if a == "--toggle" { Some("toggle") }
                else if a == "--quickpaste" { Some("quickpaste") }
                else if a == "--settings" { Some("settings") }
                else { None }
            }).unwrap_or("toggle");
            match action {
                "quickpaste" => {
                    if let Err(e) = windows::quickpaste::show_quickpaste_window(app) {
                        eprintln!("显示便捷粘贴窗口失败: {}", e);
                    }
                }
                "settings" => {
                    let app_clone = app.clone();
                    std::thread::spawn(move || {
                        if let Err(e) = windows::settings_window::open_settings_window(&app_clone) {
                            eprintln!("打开设置窗口失败: {}", e);
                        }
                    });
                }
                _ => {
                    toggle_main_window_visibility(app);
                }
            }
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_drag::init())
        .plugin(tauri_plugin_store::Builder::new().build());
    
    #[cfg(feature = "gpu-image-viewer")]
    let builder = builder.plugin(gpu_image_viewer::init());

    #[cfg(feature = "screenshot-suite")]
    let builder = builder.plugin(screenshot_suite::init());
        
    let app = builder.invoke_handler(tauri::generate_handler![
                commands::start_custom_drag,
                commands::stop_custom_drag,
                commands::toggle_main_window,
                commands::hide_main_window,
                commands::show_main_window,
                commands::check_window_snap,
                commands::position_window_at_cursor,
                commands::center_main_window,
                commands::get_data_directory,
                commands::focus_clipboard_window,
                commands::save_current_focus,
                commands::restore_last_focus,
                commands::hide_main_window_if_auto_shown,
                commands::set_window_pinned,
                commands::toggle_window_visibility,
                commands::open_settings_window,
                commands::open_community_window,
                commands::open_text_editor_window,
                windows::preview_window::show_preview_window,
                windows::preview_window::close_preview_window,
                windows::preview_window::reveal_preview_window,
                windows::preview_window::finalize_hide_preview_window,
                windows::preview_window::get_preview_window_data,
                commands::emit_clipboard_updated,
                commands::emit_quick_texts_updated,
                commands::get_clipboard_history,
                commands::get_clipboard_total_count,
                commands::get_clipboard_item_by_id_cmd,
                commands::get_clipboard_item_paste_options_cmd,
                commands::update_clipboard_item_cmd,
                commands::toggle_pin_clipboard_item,
                commands::paste_text_direct,
                commands::paste_image_file,
                commands::move_clipboard_item,
                commands::move_clipboard_item_by_id,
                commands::apply_history_limit,
                commands::paste_content,
                commands::delete_clipboard_item,
                commands::delete_clipboard_items,
                commands::clear_clipboard_history,
                commands::save_image_from_path,
                commands::copy_image_to_clipboard,
                commands::copy_files_to_clipboard,
                commands::copy_clipboard_item,
                commands::merge_copy_clipboard_items,
                commands::merge_paste_clipboard_items,
                commands::resolve_image_path,
                commands::get_favorites_history,
                commands::get_favorites_total_count,
                commands::get_favorite_item_by_id_cmd,
                commands::get_favorite_item_paste_options_cmd,
                commands::add_quick_text,
                commands::update_quick_text,
                commands::copy_favorite_item,
                commands::merge_copy_favorite_items,
                commands::merge_paste_favorite_items,
                commands::move_favorite_item_cmd,
                commands::add_clipboard_to_favorites,
                commands::move_quick_text_to_group,
                commands::delete_quick_text,
                commands::delete_favorite_items,
                commands::get_groups,
                commands::add_group,
                commands::update_group,
                commands::delete_group,
                commands::reorder_groups,
                commands::reload_settings,
                commands::save_settings,
                commands::reset_settings_to_default,
                commands::get_settings_cmd,
                commands::set_edge_hide_enabled,
                commands::get_all_windows_info_cmd,
                commands::is_portable_mode,
                commands::get_app_version,
                commands::get_data_directory_cmd,
                commands::set_auto_start,
                commands::get_auto_start_status,
                commands::set_run_as_admin,
                commands::get_run_as_admin_status,
                commands::is_running_as_admin,
                commands::is_admin_task_ready,
                commands::restart_as_admin,
                commands::reload_hotkeys,
                commands::enable_hotkeys,
                commands::disable_hotkeys,
                commands::is_hotkeys_enabled,
                commands::get_shortcut_statuses,
                commands::get_shortcut_status,
                commands::save_window_position,
                commands::save_window_size,
                commands::save_quickpaste_window_size,
                commands::dm_get_current_storage_path,
                commands::dm_get_default_storage_path,
                commands::dm_check_target_has_data,
                commands::dm_change_storage_path,
                commands::dm_reset_storage_path_to_default,
                commands::dm_export_data_zip,
                commands::dm_import_data_zip,
                commands::dm_reset_all_data,
                commands::dm_list_backups,
                commands::set_mouse_position,
                commands::get_mouse_position,
                commands::start_screenshot,
                commands::start_screenshot_quick_save,
                commands::start_screenshot_quick_pin,
                commands::start_screenshot_quick_ocr,
                commands::copy_text_to_clipboard,
                commands::check_ai_translation_config,
                commands::enable_ai_translation_cancel_shortcut,
                commands::disable_ai_translation_cancel_shortcut,
                commands::check_win_v_hotkey_disabled,
                commands::disable_win_v_hotkey_and_restart,
                commands::enable_win_v_hotkey_and_restart,
                commands::prompt_disable_win_v_hotkey_if_needed,
                commands::prompt_enable_win_v_hotkey,
                commands::enter_low_memory_mode,
                commands::exit_low_memory_mode,
                commands::is_low_memory_mode,
                commands::play_sound,
                commands::play_beep,
                commands::play_copy_sound,
                commands::play_paste_sound,
                commands::play_scroll_sound,
                commands::get_app_links_cmd,
                commands::reload_all_windows,
                commands::check_updates_and_open_window,
                commands::lan_sync_get_snapshot,
                commands::lan_sync_get_info,
                commands::lan_sync_set_enabled,
                commands::lan_sync_start_server,
                commands::lan_sync_connect_peer,
                commands::lan_sync_get_server_pair_code,
                commands::lan_sync_refresh_server_pair_code,
                commands::lan_sync_disconnect_peer,
                commands::lan_sync_list_trusted_devices,
                commands::lan_sync_disconnect_device,
                commands::lan_sync_remove_trusted_device,
                commands::lan_sync_sync_clipboard_item,
                commands::lan_sync_sync_favorite_item,
                commands::lan_chat_list_connected_devices,
                commands::lan_chat_send_text,
                commands::lan_chat_send_file_offer,
                commands::lan_chat_accept_file_offer,
                commands::lan_chat_reject_file_offer,
                commands::lan_chat_cancel_transfer,
                commands::lan_chat_prepare_files,
                commands::lan_chat_reveal_file,
                commands::lan_chat_drop_proxy_ensure,
                commands::lan_chat_drop_proxy_show,
                commands::lan_chat_drop_proxy_hide,
                commands::lan_chat_drop_proxy_dispose,
                windows::plugins::context_menu::commands::show_context_menu,
                windows::plugins::context_menu::commands::get_context_menu_options,
                windows::plugins::context_menu::commands::submit_context_menu,
                windows::plugins::context_menu::commands::close_all_context_menus,
                windows::plugins::context_menu::commands::update_context_menu_regions,
                windows::plugins::context_menu::commands::resize_context_menu,
                windows::plugins::input_dialog::commands::show_input,
                windows::plugins::input_dialog::commands::get_input_dialog_options,
                windows::plugins::input_dialog::commands::submit_input_dialog,
                windows::pin_image_window::pin_image_from_file,
                windows::pin_image_window::get_pin_image_data,
                windows::pin_image_window::animate_window_resize,
                windows::pin_image_window::close_pin_image_window_by_self,
                windows::pin_image_window::close_image_preview,
                windows::pin_image_window::save_pin_image_as,
                windows::pin_image_window::start_pin_edit_mode,
                utils::screen::get_all_screens,
                utils::system::get_system_text_scale,
                commands::il_init,
                commands::il_save_image,
                commands::il_get_image_list,
                commands::il_get_image_count,
                commands::il_delete_image,
                commands::il_rename_image,
                commands::il_get_images_dir,
                commands::il_get_gifs_dir,
                commands::recognize_image_ocr,
                commands::recognize_file_ocr,
                #[cfg(feature = "gpu-image-viewer")]
                windows::native_pin_window::create_native_pin_window,
                #[cfg(feature = "gpu-image-viewer")]
                windows::native_pin_window::confirm_native_pin_edit,
                #[cfg(feature = "gpu-image-viewer")]
                windows::native_pin_window::cancel_native_pin_edit,
                #[cfg(feature = "gpu-image-viewer")]
                windows::native_pin_window::show_native_image_preview,
                #[cfg(feature = "gpu-image-viewer")]
                windows::native_pin_window::close_native_image_preview,
                #[cfg(feature = "gpu-image-viewer")]
                windows::native_pin_window::create_native_pin_from_file,
            ])
        
    .setup(|app| {
                startup_diagnostics::set_startup_stage("执行 setup：初始化 store");
                services::store::init(app.handle());
                startup_diagnostics::set_startup_stage("执行 setup：初始化低占用状态");
                services::low_memory::init_window_activity_timestamp();
                startup_diagnostics::set_startup_stage("执行 setup：初始化低占用面板");
                services::low_memory::init_panel(app.handle().clone())?;
                #[cfg(desktop)]
                {
                    use tauri_plugin_autostart::MacosLauncher;
                    startup_diagnostics::set_startup_stage("执行 setup：初始化开机自启插件");
                    app.handle().plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![])))?;
                }
                
                startup_diagnostics::set_startup_stage("执行 setup：获取主窗口");
                let window = app.get_webview_window("main").ok_or("无法获取主窗口")?;
                let _ = window.set_focusable(false);
                #[cfg(debug_assertions)]
                let _ = window.open_devtools();
                
                if services::is_portable_build() {
                    if let Ok(exe) = std::env::current_exe() {
                        if let Some(dir) = exe.parent() {
                            let marker = dir.join("portable.flag");
                            if !marker.exists() {
                                let _ = std::fs::write(&marker, b"portable\n");
                            }
                        }
                    }
                }

                startup_diagnostics::set_startup_stage("执行 setup：初始化数据库");
                let db_path_buf = get_data_directory()?.join("quickclipboard.db");
                let db_path_str = db_path_buf.to_str().ok_or("数据库路径无效")?;
                if let Err(e1) = services::database::init_database(db_path_str) {
                    if let Some(dir) = db_path_buf.parent() {
                        for name in ["quickclipboard.db-wal", "quickclipboard.db-shm"] {
                            let p = dir.join(name);
                            if p.exists() { let _ = fs::remove_file(&p); }
                        }
                    }
                    services::database::init_database(db_path_str)
                        .map_err(|e2| format!("数据库初始化失败(已尝试清理 wal/shm): {} -> {}", e1, e2))?;
                }
                let _ = services::database::connection::with_connection(|conn| {
                    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
                });
                
                startup_diagnostics::set_startup_stage("执行 setup：加载设置");
                let mut settings = get_settings();
                
                if let Some((w, h)) = settings.saved_window_size.filter(|_| settings.remember_window_size) {
                    windows::main_window::apply_saved_window_size(&window, w, h);
                }
                let settings_exists = services::settings::storage::SettingsStorage::exists().unwrap_or(true);
                if !settings_exists {
                    if let Ok(count) = services::database::get_clipboard_count() {
                        if count as u64 > settings.history_limit {
                            settings.history_limit = count as u64;
                            let _ = update_settings(settings.clone());
                        }
                    }
                }
                let _ = services::database::limit_clipboard_history(settings.history_limit);
                
                startup_diagnostics::set_startup_stage("执行 setup：初始化屏幕与输入监听");
                utils::init_screen_utils(app.handle().clone());
                hotkey::init_hotkey_manager(app.handle().clone(), window.clone());
                input_monitor::init_input_monitor(window.clone());
                #[cfg(target_os = "windows")]
                services::system::raw_input::start_raw_input_if_needed();
                #[cfg(target_os = "windows")]
                services::system::display_change_monitor::start_display_change_monitor_if_needed();
                init_edge_monitor(window.clone());
                startup_diagnostics::set_startup_stage("执行 setup：创建托盘图标");
                setup_tray(app.handle())?;
                startup_diagnostics::set_startup_stage("执行 setup：加载全局快捷键");
                hotkey::reload_from_settings()?;
                startup_diagnostics::set_startup_stage("执行 setup：初始化扩展窗口状态");
                windows::plugins::context_menu::init();
                windows::plugins::input_dialog::init();
                quickpaste::init_quickpaste_state();
                let _ = quickpaste::init_quickpaste_window(&app.handle());
                set_clipboard_app_handle(app.handle().clone());

                windows::pin_image_window::init_pin_image_window();
                #[cfg(feature = "gpu-image-viewer")]
                windows::native_pin_window::setup_event_listener(app.handle());
                focus::start_focus_listener(app.handle().clone());

                #[cfg(feature = "screenshot-suite")]
                {
                    if let Ok(json) = serde_json::to_value(&settings) {
                        screenshot_suite::config::update_config(json);
                    }
                }

                if settings.clipboard_monitor {
                    let _ = start_clipboard_monitor();
                }

                if !settings.lan_sync_auto_start && settings.lan_sync_enabled {
                    settings.lan_sync_enabled = false;
                    let _ = update_settings(settings.clone());
                }

                {
                    let cfg = settings.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = crate::services::lan_sync::set_enabled(false).await;
                        let _ = crate::services::lan_sync::disconnect_peer().await;

                        if !cfg.lan_sync_enabled || cfg.lan_sync_mode == "off" {
                            return;
                        }

                        let _ = crate::services::lan_sync::set_enabled(true).await;

                        match cfg.lan_sync_mode.as_str() {
                            "server" => {
                                let _ = crate::services::lan_sync::start_server(cfg.lan_sync_server_port).await;
                            }
                            "client" => {
                                let _ = crate::services::lan_sync::connect_peer(
                                    &cfg.lan_sync_peer_url,
                                    cfg.lan_sync_auto_reconnect,
                                    None,
                                )
                                .await;
                            }
                            _ => {}
                        }
                    });
                }

                {
                    let app_handle = app.handle().clone();
                    tauri::async_runtime::spawn(async move {
                        use std::collections::{HashMap, HashSet};
                        use std::time::{Duration, Instant};

                        struct AttachmentProgress {
                            total_len: u64,
                            ranges: Vec<(u64, u64)>,
                            covered_len: u64,
                            last_seen: Instant,
                        }

                        fn add_range(progress: &mut AttachmentProgress, start: u64, end: u64) {
                            if end <= start {
                                return;
                            }

                            let mut s = start;
                            let mut e = end;
                            let mut out: Vec<(u64, u64)> = Vec::with_capacity(progress.ranges.len() + 1);
                            let mut inserted = false;

                            for (rs, re) in progress.ranges.iter().copied() {
                                if re < s {
                                    out.push((rs, re));
                                    continue;
                                }
                                if e < rs {
                                    if !inserted {
                                        out.push((s, e));
                                        inserted = true;
                                    }
                                    out.push((rs, re));
                                    continue;
                                }

                                s = s.min(rs);
                                e = e.max(re);
                            }

                            if !inserted {
                                out.push((s, e));
                            }

                            out.sort_by_key(|(a, _)| *a);
                            let mut merged: Vec<(u64, u64)> = Vec::with_capacity(out.len());
                            for (rs, re) in out {
                                if let Some((ls, le)) = merged.last_mut() {
                                    if rs <= *le {
                                        *le = (*le).max(re);
                                        *ls = (*ls).min(rs);
                                        continue;
                                    }
                                }
                                merged.push((rs, re));
                            }
                            progress.ranges = merged;
                            progress.covered_len = progress
                                .ranges
                                .iter()
                                .map(|(rs, re)| re.saturating_sub(*rs))
                                .sum();
                        }

                        let mut attachment_progress: HashMap<String, AttachmentProgress> = HashMap::new();
                        let mut last_connected_chat_devices: HashSet<String> = HashSet::new();
                        let mut last_purge = Instant::now();

                        let mut rx = crate::services::lan_sync::subscribe().await;
                        loop {
                            let ev = rx.recv().await;
                            let Ok(ev) = ev else { continue; };

                            if last_purge.elapsed() > Duration::from_secs(60) {
                                let now = Instant::now();
                                attachment_progress.retain(|_, p| now.duration_since(p.last_seen) < Duration::from_secs(300));
                                last_purge = now;
                            }

                            match ev {
                                lan_sync_core::CoreEvent::RemoteClipboardRecord { record } => {
                                    let settings = services::get_settings();
                                    if !settings.lan_sync_receive_enabled {
                                        continue;
                                    }

                                    if record.source_device_id == crate::services::lan_sync::device_id() {
                                        continue;
                                    }

                                    if record.content_type == "file" {
                                        continue;
                                    }

                                    let record2 = record.clone();
                                    let inserted = tokio::task::spawn_blocking(move || {
                                        crate::services::database::insert_remote_clipboard_record(&record2)
                                    })
                                    .await
                                    .ok()
                                    .and_then(|r| r.ok());

                                    if inserted.is_some() {
                                        if record.content_type == "image" {
                                            if let Some(image_ids) = record.image_id.as_ref().filter(|s| !s.trim().is_empty()) {
                                                for iid in image_ids.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                                                    let data_dir = crate::services::get_data_directory();
                                                    if let Ok(data_dir) = data_dir {
                                                        let p = data_dir
                                                            .join("clipboard_images")
                                                            .join(format!("{}.png", iid));
                                                        if !p.exists() {
                                                            let _ = crate::services::lan_sync::request_attachment(
                                                                Some(record.source_device_id.as_str()),
                                                                iid,
                                                            )
                                                            .await;
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        if settings.lan_sync_receive_write_clipboard {
                                            let should_write = if record.content_type == "image" {
                                                record.image_id.as_ref().is_some_and(|image_ids| {
                                                    if image_ids.trim().is_empty() {
                                                        return false;
                                                    }
                                                    if let Ok(data_dir) = crate::services::get_data_directory() {
                                                        for iid in image_ids.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                                                            let p = data_dir
                                                                .join("clipboard_images")
                                                                .join(format!("{}.png", iid));
                                                            if !p.exists() {
                                                                return false;
                                                            }
                                                        }
                                                        return true;
                                                    }
                                                    false
                                                })
                                            } else {
                                                true
                                            };

                                            if should_write {
                                                let _ = crate::services::paste::set_clipboard_from_item(
                                                    &record.content_type,
                                                    &record.content,
                                                    &record.html_content,
                                                    &record.raw_formats,
                                                    true,
                                                );
                                            }
                                        }

                                        use tauri::Emitter;
                                        let _ = app_handle.emit("clipboard-updated", ());
                                    }
                                }
                                lan_sync_core::CoreEvent::AttachmentRequest { requester_device_id, preferred_provider_device_id: _, image_id } => {
                                    let settings = services::get_settings();
                                    if !settings.lan_sync_receive_enabled {
                                        continue;
                                    }
                                    let _ = crate::services::lan_sync::handle_attachment_request(&requester_device_id, &image_id).await;
                                }
                                lan_sync_core::CoreEvent::RemoteAttachmentChunk { image_id, total_len, offset, data } => {
                                    let settings = services::get_settings();
                                    if !settings.lan_sync_receive_enabled {
                                        continue;
                                    }
                                    let end = offset.saturating_add(data.len() as u64);
                                    let now = Instant::now();
                                    let progress = attachment_progress.entry(image_id.clone()).or_insert_with(|| AttachmentProgress {
                                        total_len,
                                        ranges: Vec::new(),
                                        covered_len: 0,
                                        last_seen: now,
                                    });
                                    progress.total_len = total_len;
                                    progress.last_seen = now;
                                    add_range(progress, offset, end);
                                    let is_complete = progress.covered_len >= total_len;

                                    let app_handle2 = app_handle.clone();
                                    let image_id2 = image_id.clone();
                                    let _ = tokio::task::spawn_blocking(move || {
                                        use std::io::{Seek, SeekFrom, Write};

                                        let data_dir = crate::services::get_data_directory()?;
                                        let images_dir = data_dir.join("clipboard_images");
                                        std::fs::create_dir_all(&images_dir).map_err(|e| e.to_string())?;

                                        let path = images_dir.join(format!("{}.png", image_id2));
                                        let mut f = std::fs::OpenOptions::new()
                                            .create(true)
                                            .write(true)
                                            .read(true)
                                            .open(&path)
                                            .map_err(|e| e.to_string())?;

                                        f.seek(SeekFrom::Start(offset)).map_err(|e| e.to_string())?;
                                        f.write_all(&data).map_err(|e| e.to_string())?;

                                        if is_complete {
                                            f.set_len(total_len).map_err(|e| e.to_string())?;
                                            use tauri::Emitter;
                                            let _ = app_handle2.emit("clipboard-updated", ());
                                        }

                                        Ok::<(), String>(())
                                    }).await;

                                    if is_complete {
                                        attachment_progress.remove(&image_id);
                                    }
                                }
                                lan_sync_core::CoreEvent::PeerDiscovered { device_id, device_name } => {
                                    crate::services::lan_sync::remember_peer_device(device_id, device_name);
                                }
                                lan_sync_core::CoreEvent::Paired { device_id, device_name, pair_secret } => {
                                    crate::services::lan_sync::on_paired(device_id, pair_secret, device_name);
                                }
                                lan_sync_core::CoreEvent::StatusChanged { snapshot } => {
                                    let mut current_devices: HashSet<String> = snapshot
                                        .server_connected_device_ids
                                        .iter()
                                        .filter(|device_id| !device_id.trim().is_empty())
                                        .cloned()
                                        .collect();
                                    if let Some(device_id) = snapshot
                                        .connected_peer_device_id
                                        .filter(|device_id| !device_id.trim().is_empty())
                                    {
                                        current_devices.insert(device_id);
                                    }

                                    let removed_devices = last_connected_chat_devices
                                        .difference(&current_devices)
                                        .cloned()
                                        .collect::<Vec<_>>();
                                    last_connected_chat_devices = current_devices;

                                    if !removed_devices.is_empty() {
                                        crate::services::lan_sync::cleanup_chat_transfers_for_devices(
                                            removed_devices,
                                            "连接已断开，传输已中断",
                                        )
                                        .await;
                                    }
                                }
                                lan_sync_core::CoreEvent::ChatText { message } => {
                                    use tauri::Emitter;
                                    let _ = app_handle.emit(
                                        "lan-chat-event",
                                        serde_json::json!({
                                            "type": "chat_text",
                                            "message": message
                                        }),
                                    );
                                }
                                lan_sync_core::CoreEvent::ChatFileOffer { offer } => {
                                    crate::services::lan_sync::handle_incoming_file_offer_message(offer).await;
                                }
                                lan_sync_core::CoreEvent::ChatFileAccept { decision } => {
                                    crate::services::lan_sync::handle_incoming_file_accept(decision).await;
                                }
                                lan_sync_core::CoreEvent::ChatFileReject { decision } => {
                                    crate::services::lan_sync::handle_incoming_file_reject(decision).await;
                                }
                                lan_sync_core::CoreEvent::ChatFileCancel { cancel } => {
                                    crate::services::lan_sync::handle_incoming_file_cancel_message(cancel).await;
                                }
                                _ => {}
                            }
                        }
                    });
                }
                
                let _ = windows::main_window::restore_edge_snap_on_startup(&window);

                if settings.show_startup_notification {
                    let _ = services::show_startup_notification(app.handle());
                }

                windows::updater_window::start_update_checker(app.handle().clone());

                startup_diagnostics::set_startup_stage("执行 setup：完成启动收尾");
                services::memory::init();
                services::low_memory::init_auto_low_memory_manager(app.handle().clone());
                startup_diagnostics::mark_ready();

                #[cfg(target_os = "linux")]
                services::system::ipc_socket::start_ipc_server(app.handle().clone());

            Ok(())
        })
        .build(tauri::generate_context!());

    let app = match app {
        Ok(app) => app,
        Err(error) => {
            startup_diagnostics::report_startup_error("构建应用失败：", error);
            return;
        }
    };

    startup_diagnostics::set_startup_stage("运行应用事件循环");

    // 检测首次启动时的 CLI 参数
    #[cfg(target_os = "linux")]
    let linux_start_arg: Option<String> = std::env::args().find(|a| {
        a == "--toggle" || a == "--quickpaste" || a == "--settings"
    });
    #[cfg(not(target_os = "linux"))]
    let linux_start_arg: Option<String> = None;

    app.run(move |app, event| {
            // Wayland: 首次启动带参数时，在事件循环就绪后显示窗口
            #[cfg(target_os = "linux")]
            if let Some(ref arg) = linux_start_arg {
                if matches!(event, tauri::RunEvent::Ready) {
                    let arg = arg.clone();
                    let handle = app.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        match arg.as_str() {
                            "--quickpaste" => { let _ = windows::quickpaste::show_quickpaste_window(&handle); }
                            "--settings" => { let _ = windows::settings_window::open_settings_window(&handle); }
                            _ => { toggle_main_window_visibility(&handle); }
                        }
                    });
                }
            }

            match event {
                tauri::RunEvent::ExitRequested { api, .. } => {
                    if services::low_memory::is_low_memory_mode() 
                        && !services::low_memory::is_user_requested_exit() 
                    {
                        api.prevent_exit();
                    }
                }
                tauri::RunEvent::WindowEvent { label, event: tauri::WindowEvent::Destroyed, .. } => {
                    if label == "main" && !services::low_memory::is_low_memory_mode() {
                        let _ = crate::windows::chat_drop_proxy::dispose_chat_drop_proxy(app);
                        app.exit(0);
                    }
                }
                _ => {}
            }
        });
}
