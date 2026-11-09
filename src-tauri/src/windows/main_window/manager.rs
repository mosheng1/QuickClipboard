use tauri::{AppHandle, Manager, WebviewWindow};

pub fn get_main_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window("main")
}

pub fn is_main_window_visible(app: &AppHandle) -> bool {
    if let Some(window) = get_main_window(app) {
        window.is_visible().unwrap_or(false)
    } else {
        false
    }
}
