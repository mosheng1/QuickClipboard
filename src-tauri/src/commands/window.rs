use crate::services::database::{ClipboardItem, FavoriteItem};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, WebviewWindow};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ClipboardUpdatedEventPayload {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<ClipboardItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insert_index: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FavoriteUpdatedEventPayload {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<FavoriteItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insert_index: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MainWindowRefreshNeededPayload {
    pub clipboard: bool,
    pub favorites: bool,
    pub groups: bool,
}

pub fn emit_clipboard_updated_event(
    app: &AppHandle,
    payload: Option<ClipboardUpdatedEventPayload>,
) -> Result<(), String> {
    use tauri::Emitter;
    if !crate::windows::main_window::is_main_window_visible_for_updates() {
        crate::windows::main_window::mark_clipboard_refresh_pending();
        return Ok(());
    }

    app.emit(
        "clipboard-updated",
        payload.unwrap_or(ClipboardUpdatedEventPayload {
            kind: "unknown".to_string(),
            item: None,
            insert_index: None,
            total_count: None,
        }),
    )
    .map_err(|e| format!("发射剪贴板更新事件失败: {}", e))
}

pub fn emit_quick_texts_updated_event(
    app: &AppHandle,
    payload: Option<FavoriteUpdatedEventPayload>,
) -> Result<(), String> {
    use tauri::Emitter;
    if !crate::windows::main_window::is_main_window_visible_for_updates() {
        crate::windows::main_window::mark_favorites_refresh_pending();
        return Ok(());
    }

    app.emit(
        "quick-texts-updated",
        payload.unwrap_or(FavoriteUpdatedEventPayload {
            kind: "unknown".to_string(),
            item: None,
            insert_index: None,
        }),
    )
    .map_err(|e| format!("发射收藏列表更新事件失败: {}", e))
}

pub fn emit_main_window_refresh_needed_event(app: &AppHandle) -> Result<(), String> {
    use tauri::Emitter;

    let (clipboard, favorites, groups) = crate::windows::main_window::take_pending_refresh_flags();
    if !clipboard && !favorites && !groups {
        return Ok(());
    }

    app.emit(
        "main-window-refresh-needed",
        MainWindowRefreshNeededPayload {
            clipboard,
            favorites,
            groups,
        },
    )
    .map_err(|e| format!("发射主窗口刷新请求事件失败: {}", e))
}

#[tauri::command]
pub fn start_custom_drag(
    window: WebviewWindow,
    mouse_screen_x: i32,
    mouse_screen_y: i32,
) -> Result<(), String> {
    crate::start_drag(&window, mouse_screen_x, mouse_screen_y)
}

#[tauri::command]
pub fn stop_custom_drag(window: WebviewWindow) -> Result<(), String> {
    crate::stop_drag(&window)
}

#[tauri::command]
pub fn toggle_main_window(app: AppHandle) -> Result<(), String> {
    crate::toggle_main_window_visibility(&app);
    Ok(())
}

#[tauri::command]
pub fn hide_main_window(window: WebviewWindow) -> Result<(), String> {
    crate::hide_main_window(&window);
    Ok(())
}

#[tauri::command]
pub fn show_main_window(window: WebviewWindow) -> Result<(), String> {
    crate::show_main_window(&window);
    Ok(())
}

#[tauri::command]
pub fn raise_main_window_topmost(window: WebviewWindow) -> Result<(), String> {
    crate::windows::main_window::refresh_always_on_top(&window)
}

#[tauri::command]
pub fn check_window_snap(window: WebviewWindow) -> Result<(), String> {
    crate::check_snap(&window)
}

#[tauri::command]
pub fn position_window_at_cursor(window: WebviewWindow) -> Result<(), String> {
    crate::position_at_cursor(&window)
}

#[tauri::command]
pub fn center_main_window(window: WebviewWindow) -> Result<(), String> {
    crate::center_window(&window)
}

#[tauri::command]
pub fn get_data_directory() -> Result<String, String> {
    let data_dir = crate::services::get_data_directory()?;
    data_dir
        .to_str()
        .ok_or("数据目录路径转换失败".to_string())
        .map(|s| s.to_string())
}

#[tauri::command]
pub fn focus_clipboard_window(window: WebviewWindow) -> Result<(), String> {
    crate::services::system::focus_clipboard_window(window)
}

#[tauri::command]
pub fn save_current_focus(app: AppHandle) -> Result<(), String> {
    crate::services::system::save_current_focus(app)
}

#[tauri::command]
pub fn restore_last_focus() -> Result<(), String> {
    crate::services::system::restore_last_focus()
}

#[tauri::command]
pub fn hide_main_window_if_auto_shown(window: WebviewWindow) -> Result<(), String> {
    crate::hide_main_window(&window);
    Ok(())
}

#[tauri::command]
pub fn set_window_pinned(window: WebviewWindow, pinned: bool) -> Result<(), String> {
    crate::windows::main_window::set_pinned(pinned);

    window
        .set_always_on_top(pinned)
        .map_err(|e| format!("设置窗口置顶失败: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn toggle_window_visibility(app: AppHandle) -> Result<(), String> {
    crate::toggle_main_window_visibility(&app);
    Ok(())
}

#[tauri::command]
pub async fn open_settings_window(app: AppHandle) -> Result<(), String> {
    crate::windows::settings_window::open_settings_window(&app)
}

#[tauri::command]
pub async fn open_community_window(app: AppHandle) -> Result<(), String> {
    crate::windows::community_window::open_community_window(&app)
}

#[tauri::command]
pub async fn open_text_editor_window(
    app: AppHandle,
    item_id: String,
    item_type: String,
    item_index: Option<i32>,
    group_name: Option<String>,
) -> Result<(), String> {
    crate::windows::text_editor_window::open_text_editor_window(
        &app, &item_id, &item_type, item_index, group_name,
    )
}

// 剪贴板更新事件
#[tauri::command]
pub fn emit_clipboard_updated(
    app: AppHandle,
    payload: Option<ClipboardUpdatedEventPayload>,
) -> Result<(), String> {
    emit_clipboard_updated_event(&app, payload)
}

// 收藏列表更新事件
#[tauri::command]
pub fn emit_quick_texts_updated(
    app: AppHandle,
    payload: Option<FavoriteUpdatedEventPayload>,
) -> Result<(), String> {
    emit_quick_texts_updated_event(&app, payload)
}

// 刷新所有窗口
#[tauri::command]
pub fn reload_all_windows(app: AppHandle) -> Result<(), String> {
    let windows = app.webview_windows();
    for (_label, window) in windows {
        let _ = window.eval("location.reload()");
    }
    Ok(())
}

#[tauri::command]
pub async fn check_updates_and_open_window(app: AppHandle) -> Result<bool, String> {
    crate::windows::updater_window::check_updates_and_open_window(&app).await
}

#[tauri::command]
pub fn get_update_banner_state() -> Option<crate::windows::updater_window::UpdateBannerState> {
    crate::windows::updater_window::get_update_banner_state()
}

#[tauri::command]
pub async fn open_cached_update_window(app: AppHandle) -> Result<bool, String> {
    crate::windows::updater_window::open_cached_update_window(&app).await
}
