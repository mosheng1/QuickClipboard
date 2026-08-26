use once_cell::sync::Lazy;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::panic;
use std::path::PathBuf;
use std::sync::Once;
use std::time::{SystemTime, UNIX_EPOCH};

static STARTUP_STAGE: Lazy<RwLock<String>> = Lazy::new(|| RwLock::new("准备启动应用".to_string()));
static STARTUP_STATE: Lazy<RwLock<String>> = Lazy::new(|| RwLock::new("starting".to_string()));
static PANIC_HOOK_ONCE: Once = Once::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StartupStatus {
    pid: u32,
    state: String,
    stage: String,
    updated_at_ms: u64,
}

pub fn set_startup_stage(stage: &str) {
    *STARTUP_STAGE.write() = stage.to_string();
    persist_status();
}

pub fn set_startup_stage_if_starting(stage: &str) {
    if STARTUP_STATE.read().as_str() == "starting" {
        set_startup_stage(stage);
    }
}

pub fn current_startup_stage() -> String {
    STARTUP_STAGE.read().clone()
}

pub fn mark_starting() {
    *STARTUP_STATE.write() = "starting".to_string();
    persist_status();
}

pub fn mark_ready() {
    *STARTUP_STATE.write() = "ready".to_string();
    *STARTUP_STAGE.write() = "应用已就绪".to_string();
    persist_status();
}

pub fn detect_blocking_previous_instance() -> Option<String> {
    let status = read_status()?;
    let current_pid = std::process::id();

    if status.pid == current_pid || status.state != "starting" {
        return None;
    }

    if !is_process_alive(status.pid) {
        return None;
    }

    Some(format!(
        "检测到一个可能异常卡住的旧进程，阻止了新实例正常启动。\n\n旧进程 PID：{}\n旧进程停留阶段：{}\n\n请先在任务管理器中结束该 QuickClipboard 进程，然后重新启动应用。\n\n如果问题仍然出现，请将此窗口截图反馈给开发者。",
        status.pid,
        status.stage
    ))
}

pub fn install_panic_hook() {
    PANIC_HOOK_ONCE.call_once(|| {
        let default_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            let startup_state = current_startup_state();
            let location = panic_info
                .location()
                .map(|loc| format!("{}:{}", loc.file(), loc.line()))
                .unwrap_or_else(|| "未知位置".to_string());

            let message = if let Some(msg) = panic_info.payload().downcast_ref::<&str>() {
                (*msg).to_string()
            } else if let Some(msg) = panic_info.payload().downcast_ref::<String>() {
                msg.clone()
            } else {
                "未知 panic".to_string()
            };

            let detail = format!(
                "启动阶段：{}\n\n异常位置：{}\n\npanic 信息：{}\n\n请将此窗口截图反馈给开发者。",
                current_startup_stage(),
                location,
                message
            );

            let suppress_dialog =
                should_suppress_startup_panic_dialog(startup_state.as_str(), &location, &message);
            append_panic_log(
                startup_state.as_str(),
                &current_startup_stage(),
                &location,
                &message,
                suppress_dialog,
            );

            if startup_state == "starting" && !suppress_dialog {
                *STARTUP_STATE.write() = "panic".to_string();
                persist_status();
                show_error_dialog("QuickClipboard 启动异常", &detail);
            }

            default_hook(panic_info);

            if startup_state == "starting" && !suppress_dialog {
                std::process::exit(1);
            }
        }));
    });
}

pub fn report_startup_error(summary: &str, error: impl std::fmt::Display) {
    *STARTUP_STATE.write() = "failed".to_string();
    persist_status();
    let detail = format!(
        "启动阶段：{}\n\n{}\n{}\n\n请将此窗口截图反馈给开发者。",
        current_startup_stage(),
        summary,
        error
    );
    show_error_dialog("QuickClipboard 启动失败", &detail);
}

#[cfg(windows)]
pub fn show_error_dialog(title: &str, message: &str) {
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MB_SYSTEMMODAL,
    };

    let title_wide: Vec<u16> = format!("{title}\0").encode_utf16().collect();
    let message_wide: Vec<u16> = format!("{message}\0").encode_utf16().collect();

    unsafe {
        MessageBoxW(
            None,
            PCWSTR(message_wide.as_ptr()),
            PCWSTR(title_wide.as_ptr()),
            MB_OK | MB_ICONERROR | MB_SETFOREGROUND | MB_SYSTEMMODAL,
        );
    }
}

#[cfg(not(windows))]
pub fn show_error_dialog(title: &str, message: &str) {
    eprintln!("{title}\n{message}");
}

fn persist_status() {
    let Some(path) = status_file_path() else {
        return;
    };

    let status = StartupStatus {
        pid: std::process::id(),
        state: STARTUP_STATE.read().clone(),
        stage: current_startup_stage(),
        updated_at_ms: current_time_ms(),
    };

    if let Ok(content) = serde_json::to_vec_pretty(&status) {
        let _ = fs::write(path, content);
    }
}

fn current_startup_state() -> String {
    STARTUP_STATE.read().clone()
}

fn should_suppress_startup_panic_dialog(
    startup_state: &str,
    location: &str,
    message: &str,
) -> bool {
    if startup_state != "starting" {
        return true;
    }

    is_known_tao_reentrant_panic(location, message)
}

fn is_known_tao_reentrant_panic(location: &str, message: &str) -> bool {
    location.contains("tao-")
        && location.contains("event_loop")
        && location.contains("runner.rs")
        && (message.contains("either event handler is re-entrant")
            || message.contains("RefCell already borrowed"))
}

fn append_panic_log(
    startup_state: &str,
    startup_stage: &str,
    location: &str,
    message: &str,
    suppress_dialog: bool,
) {
    let Some(path) = panic_log_file_path() else {
        return;
    };

    let now_ms = current_time_ms();
    let mode = if suppress_dialog {
        "仅记录"
    } else {
        "弹窗"
    };
    let entry = format!(
        "[{now_ms}] 状态: {startup_state}\n阶段: {startup_stage}\n位置: {location}\npanic: {message}\n处理: {mode}\n\n"
    );

    let mut options = fs::OpenOptions::new();
    options.create(true).append(true);

    if let Ok(mut file) = options.open(path) {
        let _ = file.write_all(entry.as_bytes());
    }
}

fn read_status() -> Option<StartupStatus> {
    let path = status_file_path()?;
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn status_file_path() -> Option<PathBuf> {
    let base_dir = dirs::data_local_dir()?.join("quickclipboard");
    fs::create_dir_all(&base_dir).ok()?;
    Some(base_dir.join("startup-status.json"))
}

fn panic_log_file_path() -> Option<PathBuf> {
    let base_dir = dirs::data_local_dir()?.join("quickclipboard");
    fs::create_dir_all(&base_dir).ok()?;
    Some(base_dir.join("startup-panic.log"))
}

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(windows)]
fn is_process_alive(pid: u32) -> bool {
    use windows::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    unsafe {
        let handle = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(handle) => handle,
            Err(_) => return false,
        };

        let mut exit_code = 0u32;
        let result = GetExitCodeProcess(handle, &mut exit_code).is_ok();
        let _ = CloseHandle(handle);

        result && exit_code == STILL_ACTIVE.0 as u32
    }
}

#[cfg(not(windows))]
fn is_process_alive(_pid: u32) -> bool {
    false
}
