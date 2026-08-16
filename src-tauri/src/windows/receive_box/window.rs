use tauri::{
    AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};

pub const RECEIVE_BOX_LABEL: &str = "receive-box";
const DEFAULT_WIDTH: f64 = 240.0;
const DEFAULT_HEIGHT: f64 = 280.0;
const MIN_WIDTH: f64 = 180.0;
const MIN_HEIGHT: f64 = 230.0;
const CURSOR_MARGIN: i32 = 16;

pub fn create_receive_box_window(app: &AppHandle, focus: bool) -> Result<WebviewWindow, String> {
    if let Some(existing) = app.get_webview_window(RECEIVE_BOX_LABEL) {
        let _ = existing.unminimize();
        let _ = existing.show();
        if focus {
            let _ = existing.set_focus();
        }
        return Ok(existing);
    }

    let window = WebviewWindowBuilder::new(
        app,
        RECEIVE_BOX_LABEL,
        WebviewUrl::App("windows/receiveBox/index.html".into()),
    )
    .title("收件盒")
    .inner_size(DEFAULT_WIDTH, DEFAULT_HEIGHT)
    .min_inner_size(MIN_WIDTH, MIN_HEIGHT)
    .resizable(true)
    .maximizable(false)
    .minimizable(true)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(false)
    .focused(focus)
    .visible(false)
    .disable_drag_drop_handler()
    .build()
    .map_err(|e| format!("创建收件盒窗口失败: {}", e))?;

    place_initial_position(app, &window);
    bind_window_events(&window);

    let _ = window.unminimize();
    let _ = window.show();
    if focus {
        let _ = window.set_focus();
    }

    #[cfg(debug_assertions)]
    if focus {
        let _ = window.open_devtools();
    }

    Ok(window)
}

fn place_initial_position(app: &AppHandle, window: &WebviewWindow) {
    let monitor = match crate::utils::screen::ScreenUtils::get_monitor_at_cursor(app) {
        Ok(monitor) => monitor,
        Err(_) => return,
    };
    let work_area = monitor.work_area();

    let outer_size = match window.outer_size() {
        Ok(size) => size,
        Err(_) => return,
    };

    let (cursor_x, cursor_y) = crate::mouse::get_cursor_position();
    let target_x = position_axis_at_cursor(
        cursor_x,
        outer_size.width as i32,
        work_area.position.x,
        work_area.size.width as i32,
    );
    let target_y = position_axis_at_cursor(
        cursor_y,
        outer_size.height as i32,
        work_area.position.y,
        work_area.size.height as i32,
    );

    let _ = window.set_position(PhysicalPosition::new(target_x, target_y));
}

fn position_axis_at_cursor(cursor: i32, window_size: i32, area_start: i32, area_size: i32) -> i32 {
    let area_end = area_start.saturating_add(area_size);
    let mut value = cursor.saturating_add(CURSOR_MARGIN);

    if value.saturating_add(window_size) > area_end {
        value = cursor
            .saturating_sub(window_size)
            .saturating_sub(CURSOR_MARGIN);
    }

    clamp_window_axis(value, area_start, area_size, window_size)
}

fn clamp_window_axis(value: i32, area_start: i32, area_size: i32, window_size: i32) -> i32 {
    let max_start = area_start
        .saturating_add(area_size)
        .saturating_sub(window_size);
    if max_start <= area_start {
        return area_start;
    }
    value.max(area_start).min(max_start)
}

fn bind_window_events(window: &WebviewWindow) {
    window.on_window_event(|event| {
        if let WindowEvent::Focused(false) = event {
            crate::services::memory::schedule_cleanup_after_window_inactive();
        }
    });
}
