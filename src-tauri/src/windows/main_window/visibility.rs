use super::state::{set_window_state, WindowState};
use tauri::{AppHandle, LogicalSize, Manager, WebviewWindow};

#[cfg(not(target_os = "windows"))]
const ALWAYS_ON_TOP_REFRESH_DELAY_MS: u64 = 10;

pub(crate) fn normalize_saved_window_size_for_restore(
    _window: &WebviewWindow,
    width: u32,
    height: u32,
) -> (f64, f64) {
    (width.max(150) as f64, height.max(150) as f64)
}

pub(crate) fn apply_saved_window_size(window: &WebviewWindow, width: u32, height: u32) {
    let (logical_width, logical_height) =
        normalize_saved_window_size_for_restore(window, width, height);
    let _ = window.set_size(LogicalSize::new(logical_width, logical_height));
}

fn capture_window_logical_size(window: &WebviewWindow) -> Result<(u32, u32), String> {
    let size = window.inner_size().map_err(|e| e.to_string())?;
    let scale_factor = window.scale_factor().map_err(|e| e.to_string())?.max(1.0);

    Ok((
        ((size.width as f64) / scale_factor).round().max(1.0) as u32,
        ((size.height as f64) / scale_factor).round().max(1.0) as u32,
    ))
}

// 显示主窗口
pub fn show_main_window(window: &WebviewWindow) {
    if crate::services::system::is_front_app_globally_disabled_from_settings() {
        return;
    }

    let state = super::state::get_window_state();

    if state.is_snapped && state.is_hidden {
        let _ = super::show_snapped_window(window);
        return;
    }

    if state.is_snapped && !state.is_hidden {
        let _ = super::restore_from_snap(window);
    }

    show_normal_window(window);
    let _ = refresh_always_on_top(window);
}

// 隐藏主窗口
pub fn hide_main_window(window: &WebviewWindow) {
    if crate::is_context_menu_visible() {
        return;
    }

    let state = super::state::get_window_state();

    if state.is_snapped {
        if !state.is_hidden {
            let _ = super::hide_snapped_window(window);
        }
        return;
    }

    hide_normal_window(window);
}

pub fn toggle_main_window_visibility(app: &AppHandle) {
    if crate::services::low_memory::is_low_memory_mode() {
        if crate::get_settings().auto_exit_low_memory_mode {
            if let Err(e) = crate::services::low_memory::exit_low_memory_mode(app) {
                eprintln!("自动退出低占用模式失败: {}", e);
                return;
            }

            if let Some(window) = super::get_main_window(app) {
                show_main_window(&window);
            }
            return;
        }

        if let Err(e) = crate::services::low_memory::toggle_panel() {
            eprintln!("切换低占用列表失败: {}", e);
        }
        return;
    }

    if let Some(window) = super::get_main_window(app) {
        if crate::services::system::is_front_app_globally_disabled_from_settings() {
            return;
        }

        let state = super::state::get_window_state();

        let should_show =
            state.is_snapped && state.is_hidden || state.state != WindowState::Visible;

        if should_show {
            show_main_window(&window);
        } else {
            hide_main_window(&window);
        }
    }
}

fn show_normal_window(window: &WebviewWindow) {
    crate::windows::preview_window::resume_preview_after_main_window_show();

    let state = super::state::get_window_state();
    let was_visible = state.state == WindowState::Visible;

    // 根据配置定位窗口
    let settings = crate::get_settings();
    match settings.window_position_mode.as_str() {
        "remember" => {
            if let Some((x, y)) = settings
                .saved_window_position
                .or(settings.edge_snap_position)
            {
                let _ = crate::utils::positioning::position_at_saved_or_cursor(window, x, y);
            } else {
                let _ = crate::utils::positioning::position_at_cursor(window);
            }
        }
        "center" => {
            let _ = crate::utils::positioning::center_window(window);
        }
        _ => {
            let _ = crate::utils::positioning::position_at_cursor(window);
        }
    }

    let _ = window.show();

    if !was_visible {
        use tauri::Emitter;
        let _ = window.emit("window-show-animation", ());
        let _ =
            crate::commands::window::emit_main_window_refresh_needed_event(&window.app_handle());
    }

    set_window_state(WindowState::Visible);

    if !was_visible {
        crate::services::webdav_sync::notify_main_window_shown(window.app_handle().clone());
    }

    crate::input_monitor::enable_mouse_monitoring();
    crate::input_monitor::enable_navigation_keys();
}

fn should_skip_always_on_top_refresh() -> bool {
    crate::is_context_menu_visible()
}

#[cfg(target_os = "windows")]
pub fn refresh_always_on_top(window: &WebviewWindow) -> Result<(), String> {
    if should_skip_always_on_top_refresh() {
        return Ok(());
    }

    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
    };

    let hwnd = window
        .hwnd()
        .map_err(|e| format!("获取主窗口句柄失败: {}", e))?;

    unsafe {
        SetWindowPos(
            HWND(hwnd.0 as *mut _),
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )
        .map_err(|e| format!("提升主窗口置顶顺序失败: {}", e))?;
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn refresh_always_on_top(window: &WebviewWindow) -> Result<(), String> {
    if should_skip_always_on_top_refresh() {
        return Ok(());
    }

    window
        .set_always_on_top(false)
        .map_err(|e| format!("取消窗口置顶失败: {}", e))?;
    std::thread::sleep(std::time::Duration::from_millis(
        ALWAYS_ON_TOP_REFRESH_DELAY_MS,
    ));
    window
        .set_always_on_top(true)
        .map_err(|e| format!("恢复窗口置顶失败: {}", e))?;
    Ok(())
}

fn hide_normal_window(window: &WebviewWindow) {
    use tauri::Emitter;
    use tauri::Manager;

    let window_state = super::state::get_window_state();
    crate::windows::preview_window::suppress_preview_for_main_window_hide(&window.app_handle());
    let _ = crate::windows::pin_image_window::close_image_preview(window.app_handle().clone());
    #[cfg(feature = "gpu-image-viewer")]
    let _ = crate::windows::native_pin_window::close_native_image_preview();
    let _ = crate::windows::preview_window::close_preview_window(window.app_handle().clone());

    let _ = window.emit("window-hide-animation", ());

    let settings = crate::get_settings();
    if settings.clipboard_animation_enabled {
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    let mut settings_to_save = None;

    if settings.window_position_mode == "remember"
        && !(window_state.is_snapped && window_state.is_hidden)
    {
        if let Ok(position) = window.outer_position() {
            let should_save_position = window
                .outer_size()
                .ok()
                .and_then(|size| {
                    let width = size.width.min(i32::MAX as u32) as i32;
                    let height = size.height.min(i32::MAX as u32) as i32;
                    crate::screen::ScreenUtils::is_window_rect_visible_for_restore(
                        window.app_handle(),
                        position.x,
                        position.y,
                        width,
                        height,
                    )
                    .ok()
                })
                .unwrap_or(true);

            if should_save_position {
                let mut updated = crate::get_settings();
                updated.saved_window_position = Some((position.x, position.y));
                settings_to_save = Some(updated);
            }
        }
    }

    if settings.remember_window_size {
        if let Ok(size) = capture_window_logical_size(window) {
            let mut updated = settings_to_save.unwrap_or_else(crate::get_settings);
            updated.saved_window_size = Some(size);
            settings_to_save = Some(updated);
        }
    }

    if let Some(updated) = settings_to_save {
        let _ = crate::services::update_settings(updated);
    }

    if !super::state::is_pinned() {
        let _ = window.set_always_on_top(false);
    }

    let _ = window.hide();
    set_window_state(WindowState::Hidden);
    crate::services::memory::schedule_cleanup_after_main_window_hide();

    crate::input_monitor::disable_mouse_monitoring();
    crate::input_monitor::disable_navigation_keys();
}
