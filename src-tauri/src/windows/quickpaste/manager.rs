use super::state::set_visible;
use crate::services::system::raw_input::{
    disable_quickpaste_keyboard_mode, enable_quickpaste_keyboard_mode,
};
use crate::utils::positioning::center_at_cursor;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

fn create_window(app: &AppHandle) -> Result<tauri::WebviewWindow, String> {
    let settings = crate::get_settings();
    let window = WebviewWindowBuilder::new(
        app,
        "quickpaste",
        WebviewUrl::App("windows/quickpaste/index.html".into()),
    )
    .title("便捷粘贴")
    .inner_size(
        settings.quickpaste_window_width as f64,
        settings.quickpaste_window_height as f64,
    )
    .min_inner_size(200.0, 300.0)
    .max_inner_size(800.0, 1000.0)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .center()
    .visible(false)
    .resizable(true)
    .focused(false)
    .focusable(false)
    .maximizable(false)
    .minimizable(false);
    #[cfg(windows)]
    let window = window.drag_and_drop(false);
    let window = window.build().map_err(|e| e.to_string())?;

    #[cfg(debug_assertions)]
    window.open_devtools();

    Ok(window)
}

fn get_or_create_window(app: &AppHandle) -> Result<tauri::WebviewWindow, String> {
    app.get_webview_window("quickpaste")
        .map(Ok)
        .unwrap_or_else(|| create_window(app))
}

pub fn init_quickpaste_window(app: &AppHandle) -> Result<(), String> {
    if !crate::get_settings().quickpaste_enabled || app.get_webview_window("quickpaste").is_some() {
        return Ok(());
    }
    create_window(app).map(|_| ())
}

pub fn show_quickpaste_window(app: &AppHandle) -> Result<(), String> {
    let settings = crate::get_settings();
    if !settings.quickpaste_enabled {
        return Ok(());
    }
    let _ = crate::services::system::save_current_focus(app.clone());

    let window = get_or_create_window(app)?;
    center_at_cursor(&window)?;
    let _ = window.show();
    let _ = window.set_always_on_top(false);
    let _ = window.set_always_on_top(true);
    set_visible(true);

    if settings.quickpaste_paste_on_modifier_release {
        enable_quickpaste_keyboard_mode();
    }

    let _ = window.emit("quickpaste-show", ());
    Ok(())
}

pub fn hide_quickpaste_window(app: &AppHandle) -> Result<(), String> {
    disable_quickpaste_keyboard_mode();
    if let Some(window) = app.get_webview_window("quickpaste") {
        let _ = window.hide();
    }
    set_visible(false);
    crate::services::memory::schedule_cleanup_after_main_window_hide();
    Ok(())
}
