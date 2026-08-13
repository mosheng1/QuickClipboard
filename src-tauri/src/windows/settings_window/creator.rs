use tauri::AppHandle;

// 创建设置窗口
pub fn create_settings_window(app: &AppHandle) -> Result<(), String> {
    let settings_window = tauri::WebviewWindowBuilder::new(
        app,
        "settings",
        tauri::WebviewUrl::App("windows/settings/index.html".into()),
    )
    .title("设置 - 快速剪贴板")
    .inner_size(900.0, 630.0)
    .min_inner_size(800.0, 600.0)
    .center()
    .resizable(true)
    .maximizable(false)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .skip_taskbar(false)
    .visible(true)
    .focused(true)
    .drag_and_drop(false)
    .build()
    .map_err(|e| format!("创建设置窗口失败: {}", e))?;

    let settings_window_for_events = settings_window.clone();
    settings_window.on_window_event(move |event| match event {
        tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed => {
            crate::services::memory::schedule_cleanup_after_main_window_hide();
        }
        _ => {
            if settings_window_for_events.is_minimized().unwrap_or(false) {
                crate::services::memory::schedule_cleanup_after_main_window_hide();
            }
        }
    });

    Ok(())
}
