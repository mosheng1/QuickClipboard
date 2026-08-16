use tauri::{
    AppHandle, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};

use super::storage::ShelfGeometryPersisted;
use super::types::label_for;

pub const DEFAULT_WIDTH: u32 = 240;
pub const DEFAULT_HEIGHT: u32 = 280;
pub const MIN_WIDTH: u32 = 180;
pub const MIN_HEIGHT: u32 = 230;
pub const STAGGER_OFFSET: i32 = 24;
const CURSOR_MARGIN: i32 = 16;

#[derive(Clone, Copy)]
struct WorkArea {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    scale_factor: f64,
}

/// 创建一个 shelf 窗口实例。
///
/// `id` 由 manager 提供，最终窗口 label 为 `transfer-shelf-{id}`。
/// `title` 仅作为窗口标题展示，前端通过 url query 获取 `shelfId`。
pub fn create_shelf_window(
    app: &AppHandle,
    id: &str,
    title: &str,
    stagger_index: u32,
    geometry: Option<&ShelfGeometryPersisted>,
    focus: bool,
) -> Result<WebviewWindow, String> {
    let label = label_for(id);

    if let Some(existing) = app.get_webview_window(&label) {
        if let Some(geometry) = geometry {
            let _ = apply_shelf_geometry(app, &existing, geometry);
        }
        let _ = existing.unminimize();
        let _ = existing.show();
        if focus {
            let _ = existing.set_focus();
        }
        return Ok(existing);
    }

    let url = format!("windows/transferShelf/index.html?shelfId={}", id);

    let window = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(url.into()))
        .title(title)
        .inner_size(DEFAULT_WIDTH as f64, DEFAULT_HEIGHT as f64)
        .min_inner_size(MIN_WIDTH as f64, MIN_HEIGHT as f64)
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
        .map_err(|e| format!("创建文件盒窗口失败: {}", e))?;

    if let Some(geometry) = geometry {
        if let Err(error) = apply_shelf_geometry(app, &window, geometry) {
            eprintln!("[transfer_shelf] 应用恢复几何失败: {}", error);
            place_initial_position(app, &window, stagger_index);
        }
    } else {
        place_initial_position(app, &window, stagger_index);
    }
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

pub fn apply_shelf_geometry(
    app: &AppHandle,
    window: &WebviewWindow,
    geometry: &ShelfGeometryPersisted,
) -> Result<ShelfGeometryPersisted, String> {
    let resolved = resolve_shelf_geometry(app, geometry)?;
    let scale_factor = scale_factor_for_geometry(app, &resolved)
        .unwrap_or(1.0)
        .max(0.1);
    let logical_width = (resolved.width as f64 / scale_factor).max(1.0);
    let logical_height = (resolved.height as f64 / scale_factor).max(1.0);
    window
        .set_size(LogicalSize::new(logical_width, logical_height))
        .map_err(|e| format!("设置文件盒窗口尺寸失败: {}", e))?;
    window
        .set_position(PhysicalPosition::new(resolved.x, resolved.y))
        .map_err(|e| format!("设置文件盒窗口位置失败: {}", e))?;
    Ok(resolved)
}

pub fn resolve_shelf_geometry(
    app: &AppHandle,
    geometry: &ShelfGeometryPersisted,
) -> Result<ShelfGeometryPersisted, String> {
    let work_areas = get_work_areas(app)?;
    let Some(area) = choose_work_area(&work_areas, geometry) else {
        return Ok(ShelfGeometryPersisted {
            x: geometry.x.max(0),
            y: geometry.y.max(0),
            width: geometry.width.max(MIN_WIDTH),
            height: geometry.height.max(MIN_HEIGHT),
        });
    };

    let max_width = area.width.max(1) as u32;
    let max_height = area.height.max(1) as u32;
    let min_width = logical_to_physical(MIN_WIDTH, area.scale_factor).min(max_width);
    let min_height = logical_to_physical(MIN_HEIGHT, area.scale_factor).min(max_height);
    let width = geometry.width.max(min_width).min(max_width);
    let height = geometry.height.max(min_height).min(max_height);
    let x = clamp_axis(geometry.x, area.x, area.width, width as i32);
    let y = clamp_axis(geometry.y, area.y, area.height, height as i32);

    Ok(ShelfGeometryPersisted {
        x,
        y,
        width,
        height,
    })
}

fn get_work_areas(app: &AppHandle) -> Result<Vec<WorkArea>, String> {
    let monitors = app
        .available_monitors()
        .map_err(|e| format!("获取显示器列表失败: {}", e))?;

    Ok(monitors
        .into_iter()
        .map(|monitor| {
            let work_area = monitor.work_area();
            WorkArea {
                x: work_area.position.x,
                y: work_area.position.y,
                width: work_area.size.width as i32,
                height: work_area.size.height as i32,
                scale_factor: monitor.scale_factor(),
            }
        })
        .collect())
}

fn scale_factor_for_geometry(
    app: &AppHandle,
    geometry: &ShelfGeometryPersisted,
) -> Result<f64, String> {
    let work_areas = get_work_areas(app)?;
    Ok(choose_work_area(&work_areas, geometry)
        .map(|area| area.scale_factor)
        .unwrap_or(1.0))
}

fn logical_to_physical(value: u32, scale_factor: f64) -> u32 {
    ((value as f64) * scale_factor.max(0.1)).round().max(1.0) as u32
}

fn choose_work_area(
    work_areas: &[WorkArea],
    geometry: &ShelfGeometryPersisted,
) -> Option<WorkArea> {
    let width = geometry.width.max(MIN_WIDTH) as i32;
    let height = geometry.height.max(MIN_HEIGHT) as i32;

    work_areas
        .iter()
        .copied()
        .max_by_key(|area| intersection_area(*area, geometry.x, geometry.y, width, height))
        .filter(|area| intersection_area(*area, geometry.x, geometry.y, width, height) > 0)
        .or_else(|| {
            work_areas.iter().copied().min_by_key(|area| {
                distance_to_area_center(*area, geometry.x, geometry.y, width, height)
            })
        })
}

fn intersection_area(area: WorkArea, x: i32, y: i32, width: i32, height: i32) -> i64 {
    let left = area.x.max(x);
    let top = area.y.max(y);
    let right = area.right().min(x.saturating_add(width));
    let bottom = area.bottom().min(y.saturating_add(height));
    let width = right.saturating_sub(left).max(0) as i64;
    let height = bottom.saturating_sub(top).max(0) as i64;
    width * height
}

fn distance_to_area_center(area: WorkArea, x: i32, y: i32, width: i32, height: i32) -> i64 {
    let rect_center_x = x as i64 + width as i64 / 2;
    let rect_center_y = y as i64 + height as i64 / 2;
    let area_center_x = area.x as i64 + area.width as i64 / 2;
    let area_center_y = area.y as i64 + area.height as i64 / 2;
    let dx = rect_center_x - area_center_x;
    let dy = rect_center_y - area_center_y;
    dx * dx + dy * dy
}

fn clamp_axis(value: i32, area_start: i32, area_size: i32, window_size: i32) -> i32 {
    let max_start = area_start
        .saturating_add(area_size)
        .saturating_sub(window_size);
    if max_start <= area_start {
        return area_start;
    }
    value.max(area_start).min(max_start)
}

impl WorkArea {
    fn right(self) -> i32 {
        self.x.saturating_add(self.width)
    }

    fn bottom(self) -> i32 {
        self.y.saturating_add(self.height)
    }
}

fn place_initial_position(app: &AppHandle, window: &WebviewWindow, stagger_index: u32) {
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
    let stagger = STAGGER_OFFSET.saturating_mul(stagger_index as i32);
    let target_x = position_axis_at_cursor(
        cursor_x,
        outer_size.width as i32,
        work_area.position.x,
        work_area.size.width as i32,
        stagger,
    );
    let target_y = position_axis_at_cursor(
        cursor_y,
        outer_size.height as i32,
        work_area.position.y,
        work_area.size.height as i32,
        stagger,
    );

    let _ = window.set_position(PhysicalPosition::new(target_x, target_y));
}

fn position_axis_at_cursor(
    cursor: i32,
    window_size: i32,
    area_start: i32,
    area_size: i32,
    stagger: i32,
) -> i32 {
    let area_end = area_start.saturating_add(area_size);
    let mut value = cursor.saturating_add(CURSOR_MARGIN).saturating_add(stagger);

    if value.saturating_add(window_size) > area_end {
        value = cursor
            .saturating_sub(window_size)
            .saturating_sub(CURSOR_MARGIN)
            .saturating_sub(stagger);
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
