use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder};

pub fn create_community_window(app: &AppHandle) -> Result<(), String> {
    let _window = WebviewWindowBuilder::new(
        app,
        "community",
        WebviewUrl::App("windows/community/index.html".into()),
    )
    .title("社区交流 - QuickClipboard")
    .inner_size(570.0, 350.0)
    .center()
    .resizable(true)
    .maximizable(false)
    .decorations(true)
    .transparent(true)
    .skip_taskbar(false)
    .visible(true)
    .focused(true);
    #[cfg(windows)]
    let _window = _window.drag_and_drop(false);
    let _window = _window
        .shadow(false)
        .build()
        .map_err(|e| format!("创建社区交流窗口失败: {}", e))?;

    Ok(())
}
