use super::creator::create_text_editor_window;
use tauri::{AppHandle, Manager};

pub fn open_text_editor_window(
    app: &AppHandle,
    item_id: &str,
    item_type: &str,
    item_index: Option<i32>,
    group_name: Option<String>,
) -> Result<(), String> {
    let window_label = create_text_editor_window(app, item_id, item_type, item_index, group_name)?;
    if let Some(window) = app.get_webview_window(&window_label) {
        window
            .set_focus()
            .map_err(|e| format!("聚焦文本编辑器窗口失败: {}", e))?;
    }
    Ok(())
}
