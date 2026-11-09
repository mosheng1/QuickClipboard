use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

pub fn show_startup_notification(app: &AppHandle) -> Result<(), String> {
    let settings = crate::services::get_settings();

    if !settings.show_startup_notification {
        return Ok(());
    }

    let shortcut = if !settings.toggle_shortcut.is_empty() {
        settings.toggle_shortcut.clone()
    } else {
        "Shift+Space".to_string()
    };

    let notification_body = format!("QuickClipboard已启动\n按 {} 打开剪贴板窗口", shortcut);

    app.notification()
        .builder()
        .title("QuickClipboard")
        .body(&notification_body)
        .show()
        .map_err(|e| format!("显示通知失败: {}", e))?;

    Ok(())
}

// 显示通用消息通知
#[allow(dead_code)]
pub fn show_notification(app: &AppHandle, title: &str, body: &str) -> Result<(), String> {
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|e| format!("显示通知失败: {}", e))?;

    Ok(())
}
