#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;
use std::fs;

mod commands;
pub mod maintenance;
mod security;
mod services;
mod startup_diagnostics;
mod utils;
mod windows;

pub use utils::{mouse, screen};
pub use services::{AppSettings, get_settings, update_settings, get_data_directory, hotkey, SoundPlayer, AppSounds};
pub use services::system::input_monitor;
pub use services::system::focus;
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

const STARTUP_UPDATE_CHECK_DELAY_MS: u64 = 800;

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
            if services::system::should_maintain_scheduled_task(&settings) {
                // 离电状态下若不允许离电自启动，跳过管理员提权，以普通用户身份运行
                if !settings.auto_start_on_battery && services::system::is_on_battery() {
                    startup_diagnostics::set_startup_stage("检查管理员启动：离电状态已跳过提权");
                } else {
                    startup_diagnostics::set_startup_stage("检查管理员启动：检测当前进程权限");
                    let is_admin = services::system::is_running_as_admin();

                    if is_admin {
                        startup_diagnostics::set_startup_stage("检查管理员启动：当前已是管理员，准备同步计划任务");
                        services::system::sync_scheduled_task(&settings);
                    } else {
                        startup_diagnostics::set_startup_stage("检查管理员启动：当前不是管理员，准备提权重启");
                        if services::system::elevate::try_elevate_and_restart() {
                            std::process::exit(0);
                        }
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
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if services::low_memory::is_low_memory_mode() {
                let _ = services::low_memory::toggle_panel();
                return;
            }
            if let Some(window) = app.get_webview_window("main") {
                show_main_window(&window);
            }
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
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
                commands::raise_main_window_topmost,
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
                commands::drop_proxy::drop_proxy_ensure,
                commands::drop_proxy::drop_proxy_show,
                commands::drop_proxy::drop_proxy_hide,
                commands::drop_proxy::drop_proxy_dispose,
                commands::drop_proxy::drop_proxy_route_paths_at_cursor,
                commands::drop_proxy::drop_proxy_save_resource,
                commands::drop_proxy::drop_proxy_save_url,
                commands::drop_proxy::drop_proxy_cleanup_orphan_resources,
                windows::preview_window::show_preview_window,
                windows::preview_window::close_preview_window,
                windows::preview_window::reveal_preview_window,
                windows::preview_window::finalize_hide_preview_window,
                windows::preview_window::get_preview_window_data,
                windows::transfer_shelf::commands::transfer_shelf_create,
                windows::transfer_shelf::commands::transfer_shelf_list,
                windows::transfer_shelf::commands::transfer_shelf_focus,
                windows::transfer_shelf::commands::transfer_shelf_rename,
                windows::transfer_shelf::commands::transfer_shelf_close,
                windows::transfer_shelf::commands::transfer_shelf_describe_paths,
                windows::transfer_shelf::commands::transfer_shelf_add_paths,
                windows::transfer_shelf::commands::transfer_shelf_send,
                windows::transfer_shelf::commands::transfer_shelf_upload_cloud,
                windows::transfer_shelf::commands::transfer_shelf_load_state,
                windows::transfer_shelf::commands::transfer_shelf_save_state,
                windows::transfer_shelf::commands::transfer_shelf_save_geometry,
                windows::transfer_shelf::commands::transfer_shelf_apply_geometry,
                windows::receive_box::commands::receive_box_open,
                windows::receive_box::commands::receive_box_focus,
                windows::receive_box::commands::receive_box_list_lan_files,
                windows::receive_box::commands::receive_box_list_cloud_files,
                windows::receive_box::commands::receive_box_download_cloud_file,
                windows::receive_box::commands::receive_box_open_local_file,
                windows::receive_box::commands::receive_box_reveal_local_file,
                windows::receive_box::commands::receive_box_delete_local_file,
                windows::receive_box::commands::receive_box_delete_cloud_file,
                windows::receive_box::commands::receive_box_add_to_transfer_shelf,
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
                commands::get_one_time_paste_enabled,
                commands::set_one_time_paste_enabled,
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
                commands::get_update_banner_state,
                commands::open_cached_update_window,
                commands::webdav_test_connection,
                commands::webdav_upload,
                commands::webdav_download,
                commands::webdav_download_all,
                commands::webdav_get_status,
                commands::webdav_get_last_report,
                commands::webdav_start_scheduler,
                commands::webdav_stop_scheduler,
                commands::webdav_has_saved_password,
                commands::webdav_set_password,
                commands::webdav_has_saved_encryption_password,
                commands::webdav_set_encryption_password,
                commands::sync_transfer_get_mode_infos,
                commands::sync_transfer_lan_get_status,
                commands::sync_transfer_lan_start_http_server,
                commands::sync_transfer_lan_stop_http_server,
                commands::sync_transfer_lan_refresh_pairing_code,
                commands::sync_transfer_lan_list_paired_peers,
                commands::sync_transfer_lan_remove_paired_peer,
                commands::sync_transfer_lan_pair_with_peer,
                commands::sync_transfer_lan_fetch_peer_snapshot,
                commands::sync_transfer_lan_get_local_snapshot,
                commands::sync_transfer_lan_discover_peers,
                commands::sync_transfer_lan_get_auto_sync_status,
                commands::sync_transfer_lan_update_auto_sync_settings,
                commands::sync_transfer_lan_pull_from_peer,
                commands::sync_transfer_lan_push_to_peer,
                commands::sync_transfer_lan_send_file_to_peer,
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
                commands::il_get_groups,
                commands::il_add_group,
                commands::il_update_group,
                commands::il_move_image_to_group,
                commands::il_delete_group,
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
                services::webdav_sync::sync_scheduler::set_app_handle(app.handle().clone());

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

                if settings.webdav_enabled {
                    services::webdav_sync::start_scheduler();
                }

                {
                    let app_handle = app.handle().clone();
                    tauri::async_runtime::spawn(async move {
                        services::sync_transfer::lan_start_configured_services(app_handle).await;
                    });
                }

                if settings.clipboard_monitor {
                    let _ = start_clipboard_monitor();
                }

                let _ = windows::main_window::restore_edge_snap_on_startup(&window);

                if settings.show_startup_notification {
                    let _ = services::show_startup_notification(app.handle());
                }

                startup_diagnostics::set_startup_stage("执行 setup：启动自动低占用检测");
                services::low_memory::init_auto_low_memory_manager(app.handle().clone());

                Ok(())
            })
            .build(tauri::generate_context!())
            .expect("运行 Tauri 应用失败");

    startup_diagnostics::set_startup_stage("运行应用事件循环");
    app.run(|app, event| {
            match event {
                tauri::RunEvent::Ready => {
                    startup_diagnostics::mark_ready();
                    windows::transfer_shelf::schedule_startup_restore_persisted_shelves(app.clone());
                }
                tauri::RunEvent::ExitRequested { api, .. } => {
                    if services::low_memory::is_low_memory_mode() 
                        && !services::low_memory::is_user_requested_exit() 
                    {
                        api.prevent_exit();
                    } else {
                        services::webdav_sync::crypto::clear_cached_keys();
                    }
                }
                tauri::RunEvent::WindowEvent { label, event: tauri::WindowEvent::Destroyed, .. } => {
                    if label == "main" && !services::low_memory::is_low_memory_mode() {
                        services::webdav_sync::crypto::clear_cached_keys();
                        app.exit(0);
                    }
                }
                _ => {}
            }
        });
}
