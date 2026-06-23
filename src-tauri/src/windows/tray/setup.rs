use image::GenericImageView;
use tauri::{
    tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState},
    AppHandle, Manager,
};

use super::create_click_handler;

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let icon = {
        let icon_data = include_bytes!("../../../icons/icon64.png");
        let img = image::load_from_memory(icon_data)?;
        let rgba = img.to_rgba8();
        let (width, height) = img.dimensions();
        tauri::image::Image::new_owned(rgba.into_raw(), width, height)
    };

    let app_handle = app.clone();
    let app_handle_for_enter = app.clone();
    let app_handle_for_menu = app.clone();
    let click_handler = create_click_handler(app_handle.clone());
    
    let _tray = TrayIconBuilder::with_id("main-tray")
        .tooltip("快速剪贴板")
        .icon(icon)
        .show_menu_on_left_click(false)
        .on_menu_event(move |_app, event| {
            super::native_menu::set_menu_visible(false);
            super::handle_native_menu_event(&app_handle_for_menu, &event);
        })
        .on_tray_icon_event(move |_tray, event| {
            match event {
                TrayIconEvent::Click { button, button_state, .. } => {
                    match button {
                        MouseButton::Left if button_state == MouseButtonState::Up => {
                            if crate::services::low_memory::is_low_memory_mode() {
                                if let Err(e) = crate::services::low_memory::show_panel() {
                                    eprintln!("低占用模式显示列表窗口失败: {}", e);
                                }
                            } else {
                                click_handler();
                            }
                        }
                        MouseButton::Right if button_state == MouseButtonState::Up => {
                            if crate::services::low_memory::is_low_memory_mode() {
                                super::native_menu::set_menu_visible(true);
                            } else {
                                let app = app_handle.clone();
                                tauri::async_runtime::spawn(async move {
                                    if let Err(e) = super::menu::show_tray_menu(app).await {
                                        eprintln!("显示托盘菜单失败: {}", e);
                                    }
                                });
                            }
                        }
                        _ => {}
                    }
                }
                TrayIconEvent::Enter { .. } => {
                    let _ = crate::services::system::save_current_focus(app_handle_for_enter.clone());
                }
                _ => {}
            }
        })
        .build(app)?;

    // 启动时按设置隐藏托盘图标
    if !crate::services::get_settings().show_tray_icon {
        if let Some(tray) = app.tray_by_id("main-tray") {
            if let Err(e) = tray.set_visible(false) {
                eprintln!("隐藏托盘图标失败: {}", e);
            }
        }
    }

    Ok(())
}

