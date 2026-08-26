use serde_json::Value;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

#[tauri::command]
pub fn set_mouse_position(x: i32, y: i32) -> Result<(), String> {
    crate::utils::mouse::set_cursor_position(x, y)
}

#[tauri::command]
pub fn get_mouse_position() -> (i32, i32) {
    crate::utils::mouse::get_cursor_position()
}

// 检查 AI 翻译配置
#[tauri::command]
pub fn check_ai_translation_config() -> Result<Value, String> {
    use crate::services::get_settings;

    let settings = get_settings();
    let is_configured = !settings.ai_api_key.is_empty() && settings.ai_translation_enabled;

    Ok(serde_json::json!({
        "is_configured": is_configured,
        "enabled": settings.ai_translation_enabled,
        "api_key_set": !settings.ai_api_key.is_empty(),
    }))
}

// 启用 AI 翻译取消快捷键
#[tauri::command]
pub fn enable_ai_translation_cancel_shortcut() -> Result<(), String> {
    Ok(())
}

// 禁用 AI 翻译取消快捷键
#[tauri::command]
pub fn disable_ai_translation_cancel_shortcut() -> Result<(), String> {
    Ok(())
}

// 复制纯文本到剪贴板
#[tauri::command]
pub fn copy_text_to_clipboard(text: String) -> Result<(), String> {
    use clipboard_rs::{Clipboard, ClipboardContext};

    let ctx = ClipboardContext::new().map_err(|e| format!("创建剪贴板上下文失败: {}", e))?;
    ctx.set_text(text)
        .map_err(|e| format!("设置文本到剪贴板失败: {}", e))
}

// 检查 Win+V 是否已在系统中被禁用
#[tauri::command]
pub fn check_win_v_hotkey_disabled() -> Result<bool, String> {
    Ok(crate::services::system::win_v_hotkey::is_win_v_hotkey_disabled())
}

// 禁用系统 Win+V 快捷键并重启资源管理器
#[tauri::command]
pub fn disable_win_v_hotkey_and_restart() -> Result<(), String> {
    crate::services::system::win_v_hotkey::disable_win_v_hotkey()
}

// 启用系统 Win+V 快捷键并重启资源管理器
#[tauri::command]
pub fn enable_win_v_hotkey_and_restart() -> Result<(), String> {
    crate::services::system::win_v_hotkey::enable_win_v_hotkey()
}

#[tauri::command]
pub fn prompt_disable_win_v_hotkey_if_needed(app: tauri::AppHandle) -> Result<bool, String> {
    if crate::services::system::win_v_hotkey::is_win_v_hotkey_disabled() {
        return Ok(true);
    }

    let settings = crate::services::get_settings();
    let is_zh = settings.language.starts_with("zh");

    let (message, error_prefix) = if is_zh {
        (
            "当前全局快捷键使用 Win+V，为避免与系统自带的 Win+V 剪贴板快捷键冲突，需要在系统中禁用 Win+V 并重启资源管理器。\n\n是否现在修改注册表并重启资源管理器？",
            "禁用系统 Win+V 快捷键失败：",
        )
    } else {
        (
            "Your global shortcut is set to Win+V. To avoid conflicts with the Windows built-in Win+V clipboard history, the system Win+V shortcut must be disabled and Explorer must be restarted.\n\nDisable the system Win+V shortcut and restart Explorer now?",
            "Failed to disable system Win+V shortcut: ",
        )
    };

    let should_disable = app
        .dialog()
        .message(message)
        .buttons(MessageDialogButtons::OkCancel)
        .blocking_show();

    if !should_disable {
        return Ok(false);
    }

    if let Err(e) = crate::services::system::win_v_hotkey::disable_win_v_hotkey() {
        let _ = app
            .dialog()
            .message(format!("{}{}", error_prefix, e))
            .buttons(MessageDialogButtons::Ok)
            .blocking_show();
        return Ok(false);
    }

    Ok(true)
}

#[tauri::command]
pub fn prompt_enable_win_v_hotkey(app: tauri::AppHandle) -> Result<bool, String> {
    let settings = crate::services::get_settings();
    let is_zh = settings.language.starts_with("zh");

    let (message, error_prefix) = if is_zh {
        (
            "恢复系统 Win+V 快捷键会还原 Windows 自带的剪贴板历史快捷键（Win+V），并重启资源管理器。\n\n是否现在恢复？",
            "恢复系统 Win+V 快捷键失败：",
        )
    } else {
        (
            "Restoring the system Win+V shortcut will bring back the Windows built-in clipboard history (Win+V) and restart Explorer.\n\nRestore now?",
            "Failed to restore system Win+V shortcut: ",
        )
    };

    let should_enable = app
        .dialog()
        .message(message)
        .buttons(MessageDialogButtons::OkCancel)
        .blocking_show();

    if !should_enable {
        return Ok(false);
    }

    if let Err(e) = crate::services::system::win_v_hotkey::enable_win_v_hotkey() {
        let _ = app
            .dialog()
            .message(format!("{}{}", error_prefix, e))
            .buttons(MessageDialogButtons::Ok)
            .blocking_show();
        return Ok(false);
    }

    Ok(true)
}

// 进入低占用模式
#[tauri::command]
pub fn enter_low_memory_mode(app: tauri::AppHandle) -> Result<(), String> {
    crate::services::low_memory::enter_low_memory_mode(&app)
}

// 退出低占用模式
#[tauri::command]
pub fn exit_low_memory_mode(app: tauri::AppHandle) -> Result<(), String> {
    crate::services::low_memory::exit_low_memory_mode(&app)
}

// 检查是否处于低占用模式
#[tauri::command]
pub fn is_low_memory_mode() -> bool {
    crate::services::low_memory::is_low_memory_mode()
}
