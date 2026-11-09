// 菜单事件处理

use super::builder::update_native_menu;
use tauri::{menu::MenuEvent, AppHandle};

pub fn handle_native_menu_event(app: &AppHandle, event: &MenuEvent) {
    let id = event.id().as_ref();

    match id {
        "toggle" => {
            if let Err(e) = crate::services::low_memory::toggle_panel() {
                eprintln!("切换低占用列表失败: {}", e);
            }
        }
        "exit-low-memory" => {
            if let Err(e) = crate::services::low_memory::exit_low_memory_mode(app) {
                eprintln!("退出低占用模式失败: {}", e);
                return;
            }
        }
        "toggle-hotkeys" => {
            toggle_hotkeys(app);
        }
        "toggle-clipboard-monitor" => {
            if let Err(e) = crate::commands::settings::toggle_clipboard_monitor(app) {
                eprintln!("切换剪贴板监听状态失败: {}", e);
            }
            let _ = update_native_menu(app);
        }
        "toggle-paste-format" => {
            if let Err(e) = crate::commands::settings::toggle_paste_with_format(app) {
                eprintln!("切换格式粘贴状态失败: {}", e);
            }
            let _ = update_native_menu(app);
        }
        "restart" => {
            crate::windows::tray::restart_app_gracefully(app);
        }
        "quit" => {
            crate::services::low_memory::set_user_requested_exit(true);
            app.exit(0);
        }
        _ => {}
    }
}

// 切换快捷键状态
fn toggle_hotkeys(app: &AppHandle) {
    let mut settings = crate::get_settings();
    settings.hotkeys_enabled = !settings.hotkeys_enabled;

    if let Err(e) = crate::update_settings(settings.clone()) {
        eprintln!("更新快捷键设置失败: {}", e);
        return;
    }

    if settings.hotkeys_enabled {
        if let Err(e) = crate::hotkey::reload_from_settings() {
            eprintln!("重新加载快捷键失败: {}", e);
        }
    } else {
        crate::hotkey::unregister_all();
    }

    let _ = update_native_menu(app);
}
