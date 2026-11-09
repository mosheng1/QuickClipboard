// 通用输入对话框插件

pub mod commands;
pub mod window;

use once_cell::sync::OnceCell;
use std::sync::Mutex;

// 存储输入结果
static INPUT_RESULT: OnceCell<Mutex<Option<String>>> = OnceCell::new();

// 存储对话框配置
static INPUT_OPTIONS: OnceCell<Mutex<Option<window::InputDialogOptions>>> = OnceCell::new();

// 初始化输入对话框插件
pub fn init() {
    INPUT_RESULT.get_or_init(|| Mutex::new(None));
    INPUT_OPTIONS.get_or_init(|| Mutex::new(None));
}

// 获取输入结果
pub(crate) fn get_result() -> Option<String> {
    INPUT_RESULT
        .get()
        .and_then(|r| r.lock().ok())
        .and_then(|r| r.clone())
}

// 设置输入结果
pub(crate) fn set_result(value: Option<String>) {
    if let Some(result_mutex) = INPUT_RESULT.get() {
        if let Ok(mut result) = result_mutex.lock() {
            *result = value;
        }
    }
}

// 清空输入结果
pub(crate) fn clear_result() {
    set_result(None);
}

// 设置对话框配置
pub(crate) fn set_options(options: window::InputDialogOptions) {
    if let Some(options_mutex) = INPUT_OPTIONS.get() {
        if let Ok(mut opts) = options_mutex.lock() {
            *opts = Some(options);
        }
    }
}

// 获取对话框配置
pub(crate) fn get_options() -> Option<window::InputDialogOptions> {
    INPUT_OPTIONS
        .get()
        .and_then(|o| o.lock().ok())
        .and_then(|o| o.clone())
}

// 清空对话框配置
pub(crate) fn clear_options() {
    if let Some(options_mutex) = INPUT_OPTIONS.get() {
        if let Ok(mut opts) = options_mutex.lock() {
            *opts = None;
        }
    }
}
