use tauri::{AppHandle, Manager};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use super::state::{
    is_low_memory_mode,
    last_window_activity_at_ms,
    mark_window_activity,
    set_low_memory_mode,
    set_user_requested_exit,
    should_enter_low_memory,
    try_mark_auto_manager_started,
    try_start_exit_low_memory,
    finish_exit_low_memory,
};

// 需要销毁的 WebView 窗口列表
const WEBVIEW_LABELS: &[&str] = &[
    "main",
    "quickpaste", 
    "context-menu",
    "settings",
    "text-editor",
    "screenshot",
    "updater",
];

const AUTO_LOW_MEMORY_WINDOW_LABELS: &[&str] = &[
    "settings",
    "text-editor",
    "updater",
    "preview-window",
    "context-menu",
    "screenshot",
];

#[derive(Debug, Clone, PartialEq, Eq)]
enum AutoLowMemoryLogState {
    Disabled,
    Blocked(Vec<String>),
    Counting {
        idle_minutes: u64,
        started_at_ms: u64,
    },
}

static AUTO_LOW_MEMORY_LOG_STATE: Lazy<Mutex<Option<AutoLowMemoryLogState>>> =
    Lazy::new(|| Mutex::new(None));

// 进入低占用模式
pub fn enter_low_memory_mode(app: &AppHandle) -> Result<(), String> {
    if is_low_memory_mode() {
        return Ok(());
    }

    let _ = super::hide_panel();
    set_user_requested_exit(false);
    set_low_memory_mode(true);

    if let Err(e) = crate::windows::tray::switch_to_native_menu(app) {
        set_low_memory_mode(false);
        return Err(e);
    }

    // 停止边缘监控
    crate::windows::main_window::stop_edge_monitoring();

    // 禁用鼠标监控
    crate::input_monitor::disable_mouse_monitoring();

    // 禁用导航键监听
    crate::input_monitor::disable_navigation_keys();

    destroy_all_webviews(app);

    // 清理内存
    crate::services::memory::cleanup_memory();
    
    let _ = crate::services::notification::show_notification(
        app,
        "低占用模式",
        "已进入低占用模式，所有窗口已关闭。\n使用托盘菜单或使用快捷键可恢复。",
    );
    
    println!("[低占用模式] 已进入");
    Ok(())
}

// 退出低占用模式
pub fn exit_low_memory_mode(app: &AppHandle) -> Result<(), String> {
    if !is_low_memory_mode() {
        return Ok(());
    }

    if !try_start_exit_low_memory() {
        return Err("正在退出低占用模式，请稍后再试".to_string());
    }

    let _ = super::hide_panel();
    set_user_requested_exit(false);
    mark_window_activity();

    let _ = crate::services::notification::show_notification(
        app,
        "低占用模式",
        "已退出低占用模式，主窗口已恢复。",
    );

    let result = crate::windows::tray::switch_to_webview_menu(app)
        .and_then(|_| recreate_main_window(app))
        .and_then(|_| {
            let _ = crate::quickpaste::init_quickpaste_window(app);
            Ok(())
        });

    if result.is_ok() {
        set_low_memory_mode(false);
    }

    finish_exit_low_memory();

    if let Err(e) = result {
        return Err(e);
    }

    println!("[低占用模式] 已退出");
    Ok(())
}

pub fn init_auto_low_memory_manager(app: AppHandle) {
    if !try_mark_auto_manager_started() {
        return;
    }

    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));

        loop {
            interval.tick().await;

            if is_low_memory_mode() {
                continue;
            }

            let settings = crate::get_settings();
            if !settings.auto_low_memory_enabled {
                log_auto_low_memory_state(AutoLowMemoryLogState::Disabled);
                continue;
            }

            let visible_windows = collect_visible_windows_for_auto_low_memory(&app);
            if !visible_windows.is_empty() {
                log_auto_low_memory_state(AutoLowMemoryLogState::Blocked(visible_windows));
                mark_window_activity();
                continue;
            }

            let idle_minutes = settings.auto_low_memory_idle_minutes.max(1) as u64;
            let idle_threshold_ms = idle_minutes * 60 * 1000;
            let last_activity_at_ms = last_window_activity_at_ms();
            if last_activity_at_ms == 0 {
                continue;
            }

            log_auto_low_memory_state(AutoLowMemoryLogState::Counting {
                idle_minutes,
                started_at_ms: last_activity_at_ms,
            });

            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_millis() as u64)
                .unwrap_or(last_activity_at_ms);

            if !should_enter_low_memory(
                settings.auto_low_memory_enabled,
                false,
                last_activity_at_ms,
                now_ms,
                idle_threshold_ms,
            ) {
                continue;
            }

            println!(
                "[低占用模式][自动检测] 所有相关窗口已隐藏，并已空闲 {} 分钟，准备自动进入低占用模式",
                idle_minutes
            );

            if let Err(error) = enter_low_memory_mode(&app) {
                eprintln!("自动进入低占用模式失败: {}", error);
                mark_window_activity();
                continue;
            }

            println!("[低占用模式] 因窗口空闲自动进入");
        }
    });
}

fn collect_visible_windows_for_auto_low_memory(app: &AppHandle) -> Vec<String> {
    let mut visible_windows = Vec::new();

    if is_main_window_visible_for_auto_low_memory() {
        visible_windows.push("main".to_string());
    }

    if crate::windows::quickpaste::is_visible() {
        visible_windows.push("quickpaste".to_string());
    }

    for label in AUTO_LOW_MEMORY_WINDOW_LABELS {
        let visible = app
            .get_webview_window(label)
            .and_then(|window| window.is_visible().ok())
            .unwrap_or(false);

        if visible {
            visible_windows.push((*label).to_string());
        }
    }

    visible_windows
}

fn is_main_window_visible_for_auto_low_memory() -> bool {
    let state = crate::windows::main_window::get_window_state();

    if state.is_hidden {
        return false;
    }

    state.state == crate::windows::main_window::WindowState::Visible
}

fn log_auto_low_memory_state(next_state: AutoLowMemoryLogState) {
    let mut last_state = AUTO_LOW_MEMORY_LOG_STATE.lock();
    if last_state.as_ref() == Some(&next_state) {
        return;
    }

    match &next_state {
        AutoLowMemoryLogState::Disabled => {
            println!("[低占用模式][自动检测] 自动进入低占用模式未启用");
        }
        AutoLowMemoryLogState::Blocked(windows) => {
            println!(
                "[低占用模式][自动检测] 检测到窗口仍在显示，暂停空闲计时: {}",
                windows.join(", ")
            );
        }
        AutoLowMemoryLogState::Counting {
            idle_minutes,
            started_at_ms: _,
        } => {
            println!(
                "[低占用模式][自动检测] 所有相关窗口均已隐藏，开始空闲计时，{} 分钟后自动进入低占用模式",
                idle_minutes
            );
        }
    }

    *last_state = Some(next_state);
}

// 销毁所有 WebView 窗口
fn destroy_all_webviews(app: &AppHandle) {
    for (label, window) in app.webview_windows() {
        if label.starts_with("pin-image-") {
            let _ = window.destroy();
        }
    }

    for label in WEBVIEW_LABELS {
        if let Some(window) = app.get_webview_window(label) {
            let _ = window.destroy();
        }
    }
}

// 重建主窗口
fn recreate_main_window(app: &AppHandle) -> Result<(), String> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};

    if app.get_webview_window("main").is_some() {
        return Ok(());
    }

    let settings = crate::get_settings();

    let (width, height) = if settings.remember_window_size {
        settings.saved_window_size.unwrap_or((360, 520))
    } else {
        (360, 520)
    };
    let window = WebviewWindowBuilder::new(
        app,
        "main",
        WebviewUrl::App("windows/main/index.html".into()),
    )
    .title("快速剪贴板")
    .inner_size(360.0, 520.0)
    .min_inner_size(350.0, 500.0)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false) 
    .resizable(true)
    .maximizable(false)
    .minimizable(false)
    .center()
    .focused(false)
    .visible_on_all_workspaces(true)
    .disable_drag_drop_handler() 
    .build()
    .map_err(|e| format!("重建主窗口失败: {}", e))?;

    if settings.remember_window_size {
        crate::windows::main_window::apply_saved_window_size(&window, width, height);
    }
    
    let _ = window.set_focusable(false);
    
    #[cfg(debug_assertions)]
    let _ = window.open_devtools();

    crate::input_monitor::update_main_window(window.clone());

    crate::init_edge_monitor(window.clone());

    let _ = crate::windows::main_window::restore_edge_snap_on_startup(&window);

    crate::input_monitor::enable_mouse_monitoring();

    Ok(())
}
