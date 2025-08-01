// 窗口服务模块
//
// 整合窗口管理相关功能

use tauri::{Manager, WebviewWindow};

/// 打开设置窗口
pub async fn open_settings_window(app: tauri::AppHandle) -> Result<(), String> {
    // 检查设置窗口是否已经存在
    if let Some(settings_window) = app.get_webview_window("settings") {
        // 如果窗口已存在，检查是否最小化并恢复
        let is_minimized = settings_window.is_minimized().unwrap_or(false);

        if is_minimized {
            // 如果窗口被最小化，先取消最小化
            settings_window
                .unminimize()
                .map_err(|e| format!("取消最小化设置窗口失败: {}", e))?;
        }

        // 显示并聚焦窗口
        settings_window
            .show()
            .map_err(|e| format!("显示设置窗口失败: {}", e))?;
        settings_window
            .set_focus()
            .map_err(|e| format!("聚焦设置窗口失败: {}", e))?;
    } else {
        // 如果窗口不存在，创建新窗口
        let settings_window = tauri::WebviewWindowBuilder::new(
            &app,
            "settings",
            tauri::WebviewUrl::App("settings.html".into()),
        )
        .title("设置 - 快速剪贴板")
        .inner_size(900.0, 650.0)
        .min_inner_size(800.0, 600.0)
        .center()
        .resizable(true)
        .maximizable(true)
        .decorations(false) // 去除标题栏
        .build()
        .map_err(|e| format!("创建设置窗口失败: {}", e))?;

        // 设置窗口圆角（Windows 11）
        #[cfg(windows)]
        {
            if let Err(e) = crate::window_effects::set_window_rounded(&settings_window) {
                println!("设置设置窗口圆角失败: {}", e);
            }
        }

        settings_window
            .show()
            .map_err(|e| format!("显示设置窗口失败: {}", e))?;

        // 设置窗口关闭事件处理
        let app_handle = app.clone();
        settings_window.on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                // 当设置窗口关闭时，隐藏主窗口（如果它是自动显示的）
                if let Some(main_window) = app_handle.get_webview_window("main") {
                    let _ = crate::window_management::hide_main_window_if_auto_shown(&main_window);
                }
            }
        });
    }

    Ok(())
}

/// 打开文本编辑窗口
pub async fn open_text_editor_window(app: tauri::AppHandle) -> Result<(), String> {
    // 检查文本编辑窗口是否已经存在
    if let Some(editor_window) = app.get_webview_window("text-editor") {
        // 如果窗口已存在，显示并聚焦
        editor_window
            .show()
            .map_err(|e| format!("显示文本编辑窗口失败: {}", e))?;
        editor_window
            .set_focus()
            .map_err(|e| format!("聚焦文本编辑窗口失败: {}", e))?;
    } else {
        // 如果窗口不存在，创建新窗口
        let editor_window = tauri::WebviewWindowBuilder::new(
            &app,
            "text-editor",
            tauri::WebviewUrl::App("textEditor.html".into()),
        )
        .title("文本编辑器 - 快速剪贴板")
        .inner_size(800.0, 600.0)
        .min_inner_size(400.0, 300.0)
        .center()
        .resizable(true)
        .maximizable(true)
        .decorations(false) // 去除标题栏
        .build()
        .map_err(|e| format!("创建文本编辑窗口失败: {}", e))?;

        // 设置窗口圆角（Windows 11）
        #[cfg(windows)]
        {
            if let Err(e) = crate::window_effects::set_window_rounded(&editor_window) {
                println!("设置文本编辑窗口圆角失败: {}", e);
            }
        }

        editor_window
            .show()
            .map_err(|e| format!("显示文本编辑窗口失败: {}", e))?;

        // 设置窗口关闭事件处理
        let app_handle = app.clone();
        editor_window.on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                // 当编辑窗口关闭时，可以执行一些清理操作
                println!("文本编辑窗口已关闭");
            }
        });

        // 在开发模式下打开开发者工具
        #[cfg(debug_assertions)]
        {
            editor_window.open_devtools();
        }
    }

    Ok(())
}
