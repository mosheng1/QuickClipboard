// 输入对话框窗口管理
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, WebviewWindowBuilder};

// 输入框类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputType {
    Text,
    Number,
}

impl Default for InputType {
    fn default() -> Self {
        InputType::Text
    }
}

// 输入对话框的配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputDialogOptions {
    // 对话框标题
    pub title: String,
    // 提示文本
    pub message: String,
    // 输入框占位符
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    // 默认值
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
    // 输入框类型
    #[serde(default)]
    pub input_type: InputType,
    // 最小值（仅用于 number 类型）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_value: Option<i32>,
    // 最大值（仅用于 number 类型）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_value: Option<i32>,
}

// 创建并显示输入对话框窗口
pub async fn show_dialog(
    app: AppHandle,
    options: InputDialogOptions,
) -> Result<Option<String>, String> {
    // 清空之前的结果和配置
    super::clear_result();
    super::clear_options();

    // 保存配置供前端读取
    super::set_options(options.clone());

    // 创建对话框窗口
    let window = WebviewWindowBuilder::new(
        &app,
        "input-dialog",
        tauri::WebviewUrl::App("plugins/input_dialog/inputDialog.html".into()),
    )
    .title(&options.title)
    .inner_size(400.0, 200.0)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .decorations(true)
    .center()
    .always_on_top(true)
    .focused(true)
    .visible(false);
    #[cfg(windows)]
    let window = window.drag_and_drop(false);
    let window = window
        .build()
        .map_err(|e| format!("创建输入对话框失败: {}", e))?;

    // 显示窗口
    window
        .show()
        .map_err(|e| format!("显示对话框失败: {}", e))?;

    // 等待用户输入完成
    let app_clone = app.clone();
    let (tx, rx) = tokio::sync::oneshot::channel();

    // 监听窗口关闭事件
    let window_label = window.label().to_string();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if app_clone.get_webview_window(&window_label).is_none() {
                // 窗口已关闭，返回结果
                let _ = tx.send(());
                break;
            }
        }
    });

    // 等待窗口关闭
    let _ = rx.await;

    // 获取结果
    Ok(super::get_result())
}
