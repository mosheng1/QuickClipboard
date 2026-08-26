use crate::utils::app_links;
use crate::windows::plugins::context_menu::window::{
    show_menu, ContextMenuRequest, MenuAppearance, MenuBehavior, MenuButton as CtxMenuButton,
    MenuItem as CtxMenuItem, MenuPlacement,
};
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;

fn get_pin_images_dir() -> Result<PathBuf, String> {
    let data_dir = crate::services::get_data_directory()?;
    Ok(data_dir.join("pin_images"))
}

pub fn get_pin_images_list() -> Vec<(String, String)> {
    let pin_dir = match get_pin_images_dir() {
        Ok(dir) => dir,
        Err(_) => return vec![],
    };

    if !pin_dir.exists() {
        return vec![];
    }

    let mut images: Vec<(String, String, std::time::SystemTime)> = Vec::new();
    let image_extensions = ["png", "jpg", "jpeg", "gif", "bmp", "webp", "ico"];

    if let Ok(entries) = fs::read_dir(&pin_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if image_extensions.contains(&ext.to_lowercase().as_str()) {
                        let file_name = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("未知")
                            .to_string();
                        let file_path = path.to_string_lossy().to_string();
                        let modified = entry
                            .metadata()
                            .and_then(|m| m.modified())
                            .unwrap_or(std::time::UNIX_EPOCH);
                        images.push((file_name, file_path, modified));
                    }
                }
            }
        }
    }

    images.sort_by(|a, b| b.2.cmp(&a.2));
    images
        .into_iter()
        .map(|(name, path, _)| (name, path))
        .collect()
}

const MAX_PIN_IMAGES_DISPLAY: usize = 20;

// 创建分隔线菜单项
fn separator_item() -> CtxMenuItem {
    CtxMenuItem::separator()
}

// 创建普通菜单项
fn menu_item(id: &str, label: &str, icon: Option<&str>) -> CtxMenuItem {
    menu_item_with_state(id, label, icon, false)
}

fn menu_item_with_state(id: &str, label: &str, icon: Option<&str>, disabled: bool) -> CtxMenuItem {
    CtxMenuItem::item(id, label, icon).with_disabled(disabled)
}

// 构建贴图子菜单项列表
fn build_pin_images_children() -> Vec<CtxMenuItem> {
    let images = get_pin_images_list();
    let total_count = images.len();
    let mut children = Vec::new();

    if images.is_empty() {
        children.push(menu_item_with_state("empty", "(暂无贴图)", None, true));
    } else {
        for (idx, (name, path)) in images.iter().take(MAX_PIN_IMAGES_DISPLAY).enumerate() {
            let display_name = if name.len() > 30 {
                format!("{}...", &name[..27])
            } else {
                name.clone()
            };
            children.push(
                CtxMenuItem::item(
                    format!("pin-image-{}", idx),
                    display_name,
                    Some("ti ti-photo"),
                )
                .with_preview_image(path.clone()),
            );
        }

        if total_count > MAX_PIN_IMAGES_DISPLAY {
            children.push(separator_item());
            children.push(menu_item(
                "pin-open-folder",
                &format!("更多... (共{}张)", total_count),
                Some("ti ti-dots"),
            ));
        }
    }

    children.push(separator_item());
    children.push(menu_item(
        "pin-open-folder",
        "打开贴图目录",
        Some("ti ti-folder"),
    ));

    children
}

// 托盘菜单
pub async fn show_tray_menu(app: AppHandle) -> Result<(), String> {
    let settings = crate::get_settings();
    let is_force_update = crate::windows::updater_window::is_force_update_mode();

    let hotkeys_label = if settings.hotkeys_enabled {
        "禁用快捷键"
    } else {
        "启用快捷键"
    };
    let monitor_label = if settings.clipboard_monitor {
        "禁用剪贴板监听"
    } else {
        "启用剪贴板监听"
    };

    let items = vec![
        menu_item_with_state(
            "toggle",
            "显示/隐藏",
            Some("ti ti-app-window"),
            is_force_update,
        ),
        separator_item(),
        menu_item_with_state("settings", "设置", Some("ti ti-settings"), is_force_update),
        CtxMenuItem::submenu(
            "pin-images",
            "贴图",
            Some("ti ti-pinned"),
            build_pin_images_children(),
        )
        .with_disabled(is_force_update),
        separator_item(),
        CtxMenuItem::submenu(
            "file-hub",
            "文件中转",
            Some("ti ti-transfer"),
            vec![
                menu_item("transfer-shelf", "新建文件盒", Some("ti ti-package")),
                menu_item("receive-box", "打开收件盒", Some("ti ti-inbox")),
            ],
        )
        .with_disabled(is_force_update),
        separator_item(),
        menu_item_with_state(
            "toggle-hotkeys",
            hotkeys_label,
            Some("ti ti-keyboard"),
            is_force_update,
        ),
        menu_item_with_state(
            "toggle-clipboard-monitor",
            monitor_label,
            Some("ti ti-clipboard"),
            is_force_update,
        ),
        separator_item(),
        menu_item_with_state(
            "low-memory-mode",
            "进入低占用模式",
            Some("ti ti-leaf"),
            is_force_update,
        ),
        separator_item(),
        CtxMenuItem::button_row(
            "tray-links",
            "",
            vec![
                CtxMenuButton::new("open-website", "官网").with_icon("ti ti-world"),
                CtxMenuButton::new("open-github", "GitHub").with_icon("ti ti-brand-github"),
                CtxMenuButton::new("open-qq-group", "社区交流").with_icon("ti ti-users"),
            ],
        ),
        separator_item(),
        menu_item("restart", "重启程序", Some("ti ti-refresh")),
        menu_item("quit", "退出", Some("ti ti-power")),
    ];

    let (cursor_x, cursor_y) = crate::mouse::get_cursor_position();
    let theme = if settings.theme.is_empty() {
        "auto".to_string()
    } else {
        settings.theme
    };

    let appearance = MenuAppearance {
        theme: Some(theme),
        light_theme_style: Some(settings.light_theme_style),
        dark_theme_style: Some(settings.dark_theme_style),
        ui_animation_enabled: Some(settings.ui_animation_enabled),
        custom_font_enabled: Some(settings.custom_font_enabled),
        custom_font_type: Some(settings.custom_font_type),
        custom_font_path: Some(settings.custom_font_path),
        custom_font_url: Some(settings.custom_font_url),
        custom_font_family: Some(settings.custom_font_family),
    };

    let options = ContextMenuRequest::new(items)
        .with_placement(MenuPlacement::physical_point(cursor_x, cursor_y))
        .with_appearance(appearance)
        .with_behavior(MenuBehavior::tray());

    if let Ok(Some(selected_id)) = show_menu(app.clone(), options).await {
        handle_tray_menu_selection(&app, &selected_id);
    }

    Ok(())
}

// 处理托盘菜单选择
fn handle_tray_menu_selection(app: &AppHandle, selected_id: &str) {
    match selected_id {
        "open-website" => {
            if let Ok(links) = app_links::app_links() {
                let _ = tauri_plugin_opener::open_url(&links.website, None::<&str>);
            }
        }
        "open-github" => {
            if let Ok(links) = app_links::app_links() {
                let _ = tauri_plugin_opener::open_url(&links.github, None::<&str>);
            }
        }
        "open-qq-group" => {
            let _ = crate::windows::community_window::open_community_window(app);
        }
        "transfer-shelf" => {
            if let Err(e) = crate::windows::transfer_shelf::open_or_create_shelf(app) {
                eprintln!("创建文件盒失败: {}", e);
            }
        }
        "receive-box" => {
            if let Err(e) = crate::windows::receive_box::open_receive_box(app) {
                eprintln!("打开收件盒失败: {}", e);
            }
        }
        "toggle" => {
            crate::toggle_main_window_visibility(app);
        }
        "settings" => {
            let _ = crate::windows::settings_window::open_settings_window(app);
        }
        "toggle-hotkeys" => {
            toggle_hotkeys(app);
        }
        "toggle-clipboard-monitor" => {
            if let Err(e) = crate::commands::settings::toggle_clipboard_monitor(app) {
                eprintln!("切换剪贴板监听状态失败: {}", e);
            }
        }
        "low-memory-mode" => {
            if let Err(e) = crate::services::low_memory::enter_low_memory_mode(app) {
                eprintln!("进入低占用模式失败: {}", e);
            }
        }
        "restart" => {
            super::restart_app_gracefully(app);
        }
        "quit" => {
            app.exit(0);
        }
        "pin-open-folder" => {
            open_pin_images_folder();
        }
        id if id.starts_with("pin-image-") => {
            if let Some(idx_str) = id.strip_prefix("pin-image-") {
                if let Ok(idx) = idx_str.parse::<usize>() {
                    let images = get_pin_images_list();
                    if let Some((_, file_path)) = images.get(idx) {
                        let app = app.clone();
                        let file_path = file_path.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(e) = crate::windows::pin_image_window::pin_image_from_file(
                                app, file_path, None, None, None, None, None, None, None, None,
                                None, None, None,
                            )
                            .await
                            {
                                eprintln!("创建贴图窗口失败: {}", e);
                            }
                        });
                    }
                }
            }
        }
        _ => {}
    }
}

// 切换快捷键状态
fn toggle_hotkeys(app: &AppHandle) {
    let mut settings = crate::get_settings();
    settings.hotkeys_enabled = !settings.hotkeys_enabled;
    let enabled = settings.hotkeys_enabled;

    if let Err(e) = crate::update_settings(settings.clone()) {
        eprintln!("更新快捷键设置失败: {}", e);
        return;
    }

    if enabled {
        if let Err(e) = crate::hotkey::reload_from_settings() {
            eprintln!("重新加载快捷键失败: {}", e);
        }
    } else {
        crate::hotkey::unregister_all();
    }

    let message = if enabled {
        "快捷键已启用"
    } else {
        "快捷键已禁用"
    };
    let _ = crate::services::notification::show_notification(app, "QuickClipboard", message);
}

// 打开贴图目录
fn open_pin_images_folder() {
    if let Ok(data_dir) = crate::services::get_data_directory() {
        let pin_images_dir = data_dir.join("pin_images");
        if !pin_images_dir.exists() {
            let _ = std::fs::create_dir_all(&pin_images_dir);
        }
        let _ = tauri_plugin_opener::open_path(&pin_images_dir, None::<&str>);
    }
}
