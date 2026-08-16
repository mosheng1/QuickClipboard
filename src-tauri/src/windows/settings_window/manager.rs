use super::creator::create_settings_window;
use tauri::{AppHandle, Manager};

pub fn open_settings_window(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        if window.is_minimized().unwrap_or(false) {
            window
                .unminimize()
                .map_err(|e| format!("取消最小化设置窗口失败: {}", e))?;
        }
        window
            .show()
            .map_err(|e| format!("显示设置窗口失败: {}", e))?;
        window
            .set_focus()
            .map_err(|e| format!("聚焦设置窗口失败: {}", e))?;
    } else {
        create_settings_window(app)?;
        if let Some(window) = app.get_webview_window("settings") {
            window
                .set_focus()
                .map_err(|e| format!("聚焦设置窗口失败: {}", e))?;
        }
    }

    Ok(())
}
