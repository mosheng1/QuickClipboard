// 菜单构建

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconId,
    AppHandle,
};

fn parse_accelerator(shortcut: &str) -> Option<String> {
    if shortcut.is_empty() {
        return None;
    }
    let accelerator = shortcut
        .replace("Ctrl+", "CmdOrCtrl+")
        .replace("Win+", "Super+");
    Some(accelerator)
}

// 创建完整托盘菜单
fn create_menu(app: &AppHandle) -> Result<Menu<tauri::Wry>, String> {
    let menu = Menu::new(app).map_err(|e| e.to_string())?;

    let settings = crate::get_settings();
    let is_force_update = crate::windows::updater_window::is_force_update_mode();

    let toggle = MenuItem::with_id(
        app,
        "toggle",
        "切换低占用列表",
        !is_force_update,
        parse_accelerator(&settings.toggle_shortcut).as_deref(),
    )
    .map_err(|e| e.to_string())?;
    menu.append(&toggle).map_err(|e| e.to_string())?;

    let sep2 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    menu.append(&sep2).map_err(|e| e.to_string())?;

    let hotkeys_label = if settings.hotkeys_enabled {
        "禁用快捷键"
    } else {
        "启用快捷键"
    };
    let toggle_hotkeys =
        MenuItem::with_id(app, "toggle-hotkeys", hotkeys_label, true, None::<&str>)
            .map_err(|e| e.to_string())?;
    menu.append(&toggle_hotkeys).map_err(|e| e.to_string())?;

    let monitor_label = if settings.clipboard_monitor {
        "禁用剪贴板监听"
    } else {
        "启用剪贴板监听"
    };
    let toggle_monitor = MenuItem::with_id(
        app,
        "toggle-clipboard-monitor",
        monitor_label,
        true,
        parse_accelerator(&settings.toggle_clipboard_monitor_shortcut).as_deref(),
    )
    .map_err(|e| e.to_string())?;
    menu.append(&toggle_monitor).map_err(|e| e.to_string())?;

    let format_label = if settings.paste_with_format {
        "禁用格式粘贴"
    } else {
        "启用格式粘贴"
    };
    let toggle_format = MenuItem::with_id(
        app,
        "toggle-paste-format",
        format_label,
        true,
        parse_accelerator(&settings.toggle_paste_with_format_shortcut).as_deref(),
    )
    .map_err(|e| e.to_string())?;
    menu.append(&toggle_format).map_err(|e| e.to_string())?;

    let sep3 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    menu.append(&sep3).map_err(|e| e.to_string())?;

    let exit_low_memory = MenuItem::with_id(
        app,
        "exit-low-memory",
        "退出低占用模式",
        !is_force_update,
        None::<&str>,
    )
    .map_err(|e| e.to_string())?;
    menu.append(&exit_low_memory).map_err(|e| e.to_string())?;

    let sep4 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    menu.append(&sep4).map_err(|e| e.to_string())?;

    let restart = MenuItem::with_id(app, "restart", "重启程序", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    menu.append(&restart).map_err(|e| e.to_string())?;

    let quit =
        MenuItem::with_id(app, "quit", "退出", true, None::<&str>).map_err(|e| e.to_string())?;
    menu.append(&quit).map_err(|e| e.to_string())?;

    Ok(menu)
}

pub fn create_native_menu(app: &AppHandle) -> Result<Menu<tauri::Wry>, String> {
    create_menu(app)
}

// 更新托盘菜单
pub fn update_native_menu(app: &AppHandle) -> Result<(), String> {
    let tray_id = TrayIconId::new("main-tray");
    if let Some(tray) = app.tray_by_id(&tray_id) {
        let menu = create_native_menu(app)?;
        tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
    }
    Ok(())
}
