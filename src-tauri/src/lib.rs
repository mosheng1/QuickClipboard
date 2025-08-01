// =================== 模块引入 ===================
mod admin_privileges;
mod ai_config;
mod ai_translator;
mod clipboard_content;
mod clipboard_history;
mod clipboard_monitor;
mod commands;
mod database;
mod file_handler;
mod global_state;
mod groups;
mod image_manager;
mod key_state_monitor;
mod mouse_hook;
mod paste_utils;
mod preview_window;
mod quick_texts;
mod screenshot;
mod services;
mod settings;
mod shortcut_interceptor;
mod sound_manager;
mod text_input_simulator;
mod tray;
mod utils;
mod window_effects;
mod window_management;

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Emitter;

pub use commands::*;
pub use window_effects::*;

use tauri::Manager;

// 全局初始化状态
static BACKEND_INITIALIZED: AtomicBool = AtomicBool::new(false);

// =================== 启动横幅 ===================
fn print_startup_banner() {
    println!();
    println!("███╗   ███╗ ██████╗ ███████╗██╗  ██╗███████╗███╗   ██╗ ██████╗ ");
    println!("████╗ ████║██╔═══██╗██╔════╝██║  ██║██╔════╝████╗  ██║██╔════╝ ");
    println!("██╔████╔██║██║   ██║███████╗███████║█████╗  ██╔██╗ ██║██║  ███╗");
    println!("██║╚██╔╝██║██║   ██║╚════██║██╔══██║██╔══╝  ██║╚██╗██║██║   ██║");
    println!("██║ ╚═╝ ██║╚██████╔╝███████║██║  ██║███████╗██║ ╚████║╚██████╔╝");
    println!("╚═╝     ╚═╝ ╚═════╝ ╚══════╝╚═╝  ╚═╝╚══════╝╚═╝  ╚═══╝ ╚═════╝ ");
    println!();
    println!("QuickClipboard v1.0.0 - 快速剪贴板管理工具");
    println!("Author: MoSheng | Built with Tauri + Rust");
    println!("Starting application...");
    println!();
}

// =================== 内部函数 ===================

// 发送启动通知的内部函数
fn send_startup_notification_internal(app_handle: &tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_notification::NotificationExt;

    // 检查设置是否启用了启动通知
    let app_settings = settings::get_global_settings();
    if !app_settings.show_startup_notification {
        println!("启动通知已禁用，跳过发送");
        return Ok(());
    }

    let admin_status = admin_privileges::get_admin_status();
    let status_text = if admin_status.is_admin {
        "（管理员模式）"
    } else {
        ""
    };

    // 获取当前设置的快捷键
    let app_settings = settings::get_global_settings();
    let shortcut_key = if app_settings.toggle_shortcut.is_empty() {
        "Win+V".to_string()
    } else {
        app_settings.toggle_shortcut.clone()
    };

    let notification_body = format!(
        "QuickClipboard 已启动{}\n按 {} 打开剪贴板",
        status_text, shortcut_key
    );

    match app_handle
        .notification()
        .builder()
        .title("QuickClipboard")
        .body(&notification_body)
        .show()
    {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("发送通知失败: {}", e)),
    }
}

// =================== Tauri 应用入口 ===================
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 输出启动横幅
    print_startup_banner();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_sql::Builder::new().build())
        .on_menu_event(|app, event| match event.id().as_ref() {
            "toggle" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = commands::toggle_window_visibility(window);
                }
            }
            "settings" => {
                if let Some(window) = app.get_webview_window("main") {
                    // 检查主窗口是否已经可见
                    let was_visible = window.is_visible().unwrap_or(false);

                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = window.emit("open-settings", ());

                    // 如果主窗口之前不可见，标记为自动显示并启用鼠标监听
                    if !was_visible {
                        window_management::set_main_window_auto_shown(true);

                        // 确保窗口设置为工具窗口并启用鼠标监听
                        #[cfg(windows)]
                        {
                            let _ = window_management::set_tool_window(&window);
                            mouse_hook::enable_mouse_monitoring();
                        }

                        println!("主窗口因设置菜单自动显示，已启用鼠标监听");
                    }
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .setup(|app| {
            // 初始化数据库
            if let Err(e) = database::initialize_database() {
                println!("数据库初始化失败: {}", e);
            } else {
                // 执行数据迁移
                if let Err(e) = database::migrate_from_json() {
                    println!("数据迁移失败: {}", e);
                }
            }

            // 首先尝试加载历史记录
            clipboard_history::load_history();
            // 加载常用文本
            quick_texts::load_quick_texts();
            // 初始化分组系统
            if let Err(e) = groups::init_groups() {
                println!("分组系统初始化失败: {}", e);
            }

            // 获取主窗口
            let main_window = app.get_webview_window("main").unwrap();
            #[cfg(windows)]
            {
                mouse_hook::MAIN_WINDOW_HANDLE.set(main_window.clone()).ok();

                // 初始化快捷键拦截器
                shortcut_interceptor::initialize_shortcut_interceptor(main_window.clone());
            }

            // 开发模式下自动打开开发者工具
            #[cfg(debug_assertions)]
            {
                main_window.open_devtools();
            }

            // 设置窗口效果
            // window_effects::set_window_blur(&main_window);

            // 设置窗口圆角（Windows 11）
            #[cfg(windows)]
            {
                if let Err(e) = window_effects::set_window_rounded(&main_window) {
                    println!("设置窗口圆角失败: {}", e);
                }
            }

            // 设置为工具窗口，避免抢占焦点，并确保始终置顶
            #[cfg(windows)]
            {
                if let Err(e) = window_management::set_tool_window(&main_window) {
                    println!("设置工具窗口失败: {}", e);
                }
                // 确保窗口始终置顶
                if let Err(e) = main_window.set_always_on_top(true) {
                    println!("设置窗口置顶失败: {}", e);
                }
            }

            // 初始化时获取剪贴板内容并初始化监听器状态
            clipboard_monitor::initialize_clipboard_state();

            // 初始化分组系统
            match groups::init_groups() {
                Ok(_) => {}
                Err(_e) => {
                    // 不要因为分组系统失败而阻止应用启动
                }
            }

            // 初始化音效管理器
            if let Err(_e) = sound_manager::initialize_sound_manager() {
                // 静默处理错误
            }

            // 初始化预览窗口
            preview_window::init_preview_window();

            // 加载并应用设置
            // println!("正在加载应用设置...");
            let app_settings = settings::get_global_settings();

            // 检查管理员运行设置
            if app_settings.run_as_admin && !admin_privileges::is_running_as_admin() {
                println!("设置要求以管理员权限运行，但当前不是管理员权限，正在重启...");
                if let Err(e) = admin_privileges::restart_as_admin() {
                    println!("以管理员权限重启失败: {}", e);
                    // 继续运行，但可能某些功能受限
                } else {
                    // 重启成功，当前进程会退出
                    return Ok(());
                }
            }

            // 应用历史记录数量限制
            clipboard_history::set_history_limit(app_settings.history_limit as usize);

            // 应用剪贴板监听设置
            clipboard_history::set_monitoring_enabled(app_settings.clipboard_monitor);

            // 应用忽略重复内容设置
            clipboard_history::set_ignore_duplicates(app_settings.ignore_duplicates);

            // 应用保存图片设置
            clipboard_history::set_save_images(app_settings.save_images);

            // 应用数字快捷键设置
            #[cfg(windows)]
            global_state::set_number_shortcuts_enabled(app_settings.number_shortcuts);

            // 应用预览窗口快捷键设置
            #[cfg(windows)]
            global_state::update_preview_shortcut_config(&app_settings.preview_shortcut);

            // 配置快捷键拦截器并启用
            #[cfg(windows)]
            {
                let toggle_shortcut = if app_settings.toggle_shortcut.is_empty() {
                    "Win+V".to_string()
                } else {
                    app_settings.toggle_shortcut.clone()
                };
                shortcut_interceptor::update_shortcut_to_intercept(&toggle_shortcut);

                // 配置预览快捷键拦截器
                let preview_shortcut = if app_settings.preview_shortcut.is_empty() {
                    "Ctrl+`".to_string()
                } else {
                    app_settings.preview_shortcut.clone()
                };
                shortcut_interceptor::update_preview_shortcut_to_intercept(&preview_shortcut);

                shortcut_interceptor::enable_shortcut_interception();
            }

            // 应用音效设置
            let sound_settings = sound_manager::SoundSettings {
                enabled: app_settings.sound_enabled,
                volume: (app_settings.sound_volume / 100.0) as f32,
                copy_sound_path: app_settings.copy_sound_path,
                paste_sound_path: app_settings.paste_sound_path,
                preset: app_settings.sound_preset,
            };
            sound_manager::update_sound_settings(sound_settings);

            // 启动剪贴板监听器
            clipboard_monitor::start_clipboard_monitor(app.handle().clone());

            // 初始化托盘图标
            match tray::setup_tray(&app.handle()) {
                Ok(_) => {}
                Err(_e) => {
                    // 静默处理错误
                }
            }

            // 设置窗口关闭事件处理 - 隐藏到托盘而不是退出
            let main_window_clone = main_window.clone();
            main_window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    // 阻止默认的关闭行为
                    api.prevent_close();
                    // 隐藏窗口到托盘
                    let _ = main_window_clone.hide();
                }
            });

            // 注册全局快捷键
            #[cfg(desktop)]
            {
                // 启动按键监控系统（仅 Windows）
                #[cfg(windows)]
                {
                    // 启动新的按键状态监控系统
                    key_state_monitor::start_keyboard_polling_system();
                    // 安装鼠标钩子但不启用监听
                    mouse_hook::enable_mouse_monitoring();
                    mouse_hook::disable_mouse_monitoring();

                    // 安装快捷键拦截钩子
                    shortcut_interceptor::install_shortcut_hook();
                }
            }

            // 发送启动通知
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                // 等待一小段时间确保应用完全启动
                std::thread::sleep(std::time::Duration::from_millis(1000));

                // 发送启动通知
                let _ = send_startup_notification_internal(&app_handle);
            });

            // 标记后端初始化完成
            BACKEND_INITIALIZED.store(true, Ordering::Relaxed);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            get_clipboard_text,
            set_clipboard_text,
            get_clipboard_history,
            refresh_clipboard,
            set_window_pinned,
            get_window_pinned,
            toggle_window_visibility,
            set_clipboard_image,
            focus_clipboard_window,
            restore_last_focus,
            get_quick_texts,
            add_quick_text,
            update_quick_text,
            delete_quick_text,
            add_clipboard_to_favorites,
            enable_mouse_monitoring_command,
            disable_mouse_monitoring_command,
            set_startup_launch,
            set_history_limit,
            reorder_clipboard_history,
            reorder_quick_texts,
            get_groups,
            add_group,
            update_group,
            hide_main_window_if_auto_shown,
            delete_group,
            get_quick_texts_by_group,
            move_quick_text_to_group,
            add_clipboard_to_group,
            open_settings_window,
            get_settings,
            reload_settings,
            save_settings,
            browse_sound_file,
            test_sound,
            play_paste_sound,
            play_scroll_sound,
            set_super_topmost,
            get_sound_status,
            clear_sound_cache,
            get_active_sound_count,
            log_debug,
            get_image_data_url,
            get_image_thumbnail_url,
            set_preview_index,
            cancel_preview,
            delete_clipboard_item,
            update_clipboard_item,
            emit_clipboard_updated,
            emit_quick_texts_updated,
            clear_clipboard_history,
            open_text_editor_window,
            notify_preview_tab_change,
            get_main_window_state,
            update_theme_setting,
            get_app_version,
            get_admin_status,
            restart_as_admin,
            is_backend_initialized,
            send_system_notification,
            send_startup_notification,
            screenshot::open_screenshot_window,
            screenshot::close_screenshot_window,
            screenshot::take_screenshot,
            screenshot::take_fullscreen_screenshot,
            commands::test_ai_translation,
            commands::translate_and_input_text,
            commands::translate_and_paste_text,
            commands::translate_and_input_on_copy,
            commands::translate_text_smart,
            commands::is_currently_pasting,
            commands::check_ai_translation_config,
            commands::get_available_ai_models,
            commands::test_ai_config,
            commands::cancel_translation,
            commands::enable_ai_translation_cancel_shortcut,
            commands::disable_ai_translation_cancel_shortcut,
            commands::copy_files_to_directory,
            commands::get_file_info,
            commands::get_clipboard_files,
            commands::set_clipboard_files,
            commands::move_clipboard_item_to_front,
            commands::paste_content,
            commands::open_file_location,
            commands::read_image_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
