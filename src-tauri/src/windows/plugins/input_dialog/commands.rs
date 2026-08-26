// 输入对话框命令
use super::window::{show_dialog, InputDialogOptions, InputType};
use tauri::AppHandle;

// 前端获取对话框配置
#[tauri::command]
pub fn get_input_dialog_options() -> Result<InputDialogOptions, String> {
    super::get_options().ok_or_else(|| "配置未初始化".to_string())
}

// 前端提交输入结果
#[tauri::command]
pub fn submit_input_dialog(value: Option<String>) -> Result<(), String> {
    super::set_result(value);
    Ok(())
}

// 前端调用显示输入对话框的命令
#[tauri::command]
pub async fn show_input(
    app: AppHandle,
    title: String,
    message: String,
    placeholder: Option<String>,
    default_value: Option<String>,
    input_type: Option<String>,
    min_value: Option<i32>,
    max_value: Option<i32>,
) -> Result<Option<String>, String> {
    // 解析输入框类型
    let input_type_enum = match input_type.as_deref() {
        Some("number") => InputType::Number,
        _ => InputType::Text,
    };

    let options = InputDialogOptions {
        title,
        message,
        placeholder,
        default_value,
        input_type: input_type_enum,
        min_value,
        max_value,
    };

    show_dialog(app, options).await
}
