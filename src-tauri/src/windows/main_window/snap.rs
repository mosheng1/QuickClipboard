use tauri::{WebviewWindow, Manager, Emitter};
use super::state::{SnapEdge, set_snap_edge, set_hidden, clear_snap, is_snapped};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

const SNAP_THRESHOLD: i32 = 30;
const FRONTEND_CONTENT_INSET_LOGICAL: f64 = 5.0;

static ANIMATION_VERSION: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
struct MonitorEdgeContext {
    id: String,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    scale_factor: f64,
    left_edge: bool,
    right_edge: bool,
    top_edge: bool,
    bottom_edge: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct ResolvedSnapPosition {
    pub x: i32,
    pub y: i32,
    pub scale_factor: f64,
    pub edge: SnapEdge,
    pub monitor_id: String,
}

fn get_content_inset(scale_factor: f64) -> i32 {
    (FRONTEND_CONTENT_INSET_LOGICAL * scale_factor) as i32
}

fn clamp_ratio(ratio: f64) -> f64 {
    if ratio.is_finite() {
        ratio.clamp(0.0, 1.0)
    } else {
        0.5
    }
}

fn build_monitor_identifier(monitor: &tauri::Monitor) -> String {
    if let Some(name) = monitor.name() {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    let pos = monitor.position();
    let size = monitor.size();
    format!(
        "monitor:{}:{}:{}:{}:{:.3}",
        pos.x,
        pos.y,
        size.width,
        size.height,
        monitor.scale_factor()
    )
}

fn edge_to_setting_value(edge: SnapEdge) -> Option<String> {
    match edge {
        SnapEdge::Left => Some("left".to_string()),
        SnapEdge::Right => Some("right".to_string()),
        SnapEdge::Top => Some("top".to_string()),
        SnapEdge::Bottom => Some("bottom".to_string()),
        SnapEdge::None => None,
    }
}

fn edge_from_setting_value(value: &str) -> Option<SnapEdge> {
    match value {
        "left" => Some(SnapEdge::Left),
        "right" => Some(SnapEdge::Right),
        "top" => Some(SnapEdge::Top),
        "bottom" => Some(SnapEdge::Bottom),
        _ => None,
    }
}

fn build_monitor_contexts(app: &tauri::AppHandle) -> Result<Vec<MonitorEdgeContext>, String> {
    let monitors = app
        .available_monitors()
        .map_err(|e| format!("获取显示器列表失败: {}", e))?;
    let monitor_edges = crate::utils::screen::ScreenUtils::get_all_monitors_with_edges(app)?;
    let contexts = monitors
        .into_iter()
        .filter_map(|monitor| {
            let pos = monitor.position();
            let size = monitor.size();
            let x = pos.x;
            let y = pos.y;
            let width = size.width as i32;
            let height = size.height as i32;
            let scale_factor = monitor.scale_factor();
            monitor_edges
                .iter()
                .find(|(mx, my, mw, mh, _, _, _, _)| {
                    *mx == x && *my == y && *mw == width && *mh == height
                })
                .map(|(_, _, _, _, left_edge, right_edge, top_edge, bottom_edge)| {
                    MonitorEdgeContext {
                        id: build_monitor_identifier(&monitor),
                        x,
                        y,
                        width,
                        height,
                        scale_factor,
                        left_edge: *left_edge,
                        right_edge: *right_edge,
                        top_edge: *top_edge,
                        bottom_edge: *bottom_edge,
                    }
                })
        })
        .collect();
    Ok(contexts)
}

fn monitor_supports_edge(monitor: &MonitorEdgeContext, edge: SnapEdge) -> bool {
    match edge {
        SnapEdge::Left => monitor.left_edge,
        SnapEdge::Right => monitor.right_edge,
        SnapEdge::Top => monitor.top_edge,
        SnapEdge::Bottom => monitor.bottom_edge,
        SnapEdge::None => false,
    }
}

fn find_monitor_for_window(
    app: &tauri::AppHandle,
    edge: SnapEdge,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Result<MonitorEdgeContext, String> {
    let contexts = build_monitor_contexts(app)?;
    let anchor_x = x + width / 2;
    let anchor_y = y + height / 2;

    contexts
        .iter()
        .cloned()
        .filter(|monitor| monitor_supports_edge(monitor, edge))
        .filter(|monitor| {
            anchor_x >= monitor.x
                && anchor_x < monitor.x + monitor.width
                && anchor_y >= monitor.y
                && anchor_y < monitor.y + monitor.height
        })
        .next()
        .or_else(|| {
            contexts
                .iter()
                .cloned()
                .filter(|monitor| monitor_supports_edge(monitor, edge))
                .min_by_key(|monitor| {
                    let center_x = monitor.x + monitor.width / 2;
                    let center_y = monitor.y + monitor.height / 2;
                    let dx = center_x - anchor_x;
                    let dy = center_y - anchor_y;
                    dx * dx + dy * dy
                })
        })
        .ok_or_else(|| "未找到可用的贴边显示器".to_string())
}

fn resolve_usable_edge_on_monitor(
    monitor: &MonitorEdgeContext,
    preferred_edge: SnapEdge,
) -> Option<SnapEdge> {
    let ordered_edges = match preferred_edge {
        SnapEdge::Top => [SnapEdge::Top, SnapEdge::Left, SnapEdge::Right, SnapEdge::Bottom],
        SnapEdge::Bottom => [SnapEdge::Bottom, SnapEdge::Left, SnapEdge::Right, SnapEdge::Top],
        SnapEdge::Left => [SnapEdge::Left, SnapEdge::Top, SnapEdge::Bottom, SnapEdge::Right],
        SnapEdge::Right => [SnapEdge::Right, SnapEdge::Top, SnapEdge::Bottom, SnapEdge::Left],
        SnapEdge::None => [SnapEdge::None, SnapEdge::Top, SnapEdge::Bottom, SnapEdge::Left],
    };

    ordered_edges
        .into_iter()
        .find(|edge| monitor_supports_edge(monitor, *edge))
}

fn compute_local_ratio_in_monitor(
    monitor: &MonitorEdgeContext,
    edge: SnapEdge,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> f64 {
    match edge {
        SnapEdge::Top | SnapEdge::Bottom => {
            let span = (monitor.width - width).max(0) as f64;
            if span <= 0.0 {
                0.5
            } else {
                clamp_ratio((x - monitor.x) as f64 / span)
            }
        }
        SnapEdge::Left | SnapEdge::Right => {
            let span = (monitor.height - height).max(0) as f64;
            if span <= 0.0 {
                0.5
            } else {
                clamp_ratio((y - monitor.y) as f64 / span)
            }
        }
        SnapEdge::None => 0.5,
    }
}

fn resolve_position_in_monitor(
    monitor: &MonitorEdgeContext,
    edge: SnapEdge,
    ratio: f64,
    width: i32,
    height: i32,
) -> (i32, i32) {
    let ratio = clamp_ratio(ratio);
    match edge {
        SnapEdge::Top | SnapEdge::Bottom => {
            let span = (monitor.width - width).max(0);
            let x = if span > 0 {
                monitor.x + (span as f64 * ratio).round() as i32
            } else {
                monitor.x
            };
            (x, monitor.y)
        }
        SnapEdge::Left | SnapEdge::Right => {
            let span = (monitor.height - height).max(0);
            let y = if span > 0 {
                monitor.y + (span as f64 * ratio).round() as i32
            } else {
                monitor.y
            };
            (monitor.x, y)
        }
        SnapEdge::None => (monitor.x, monitor.y),
    }
}

fn resolve_saved_monitor_and_edge(
    app: &tauri::AppHandle,
    preferred_edge: SnapEdge,
    monitor_id: Option<&str>,
) -> Result<(MonitorEdgeContext, SnapEdge), String> {
    let contexts = build_monitor_contexts(app)?;

    if let Some(monitor_id) = monitor_id {
        if let Some(monitor) = contexts.iter().find(|monitor| monitor.id == monitor_id) {
            if let Some(edge) = resolve_usable_edge_on_monitor(monitor, preferred_edge) {
                return Ok((monitor.clone(), edge));
            }
        }
    }

    if let Some(monitor) = contexts
        .iter()
        .find(|monitor| monitor_supports_edge(monitor, preferred_edge))
    {
        return Ok((monitor.clone(), preferred_edge));
    }

    contexts
        .iter()
        .find_map(|monitor| {
            resolve_usable_edge_on_monitor(monitor, preferred_edge)
                .map(|edge| (monitor.clone(), edge))
        })
        .ok_or_else(|| "未找到可用的贴边显示器".to_string())
}

fn normalize_saved_logical_size_for_monitor(
    width: u32,
    height: u32,
) -> (f64, f64) {
    // 保存时已经是逻辑像素，直接应用最小尺寸限制即可
    (width.max(350) as f64, height.max(500) as f64)
}

fn resolve_startup_restore_window_size(
    window: &WebviewWindow,
    edge: SnapEdge,
    monitor_id: Option<&str>,
    settings: &crate::services::AppSettings,
) -> Result<(i32, i32), String> {
    let (monitor, _) = resolve_saved_monitor_and_edge(window.app_handle(), edge, monitor_id)?;

    if settings.remember_window_size {
        if let Some((saved_width, saved_height)) = settings.saved_window_size {
            let (logical_width, logical_height) = normalize_saved_logical_size_for_monitor(
                saved_width,
                saved_height,
            );
            return Ok((
                (logical_width * monitor.scale_factor).round().max(1.0) as i32,
                (logical_height * monitor.scale_factor).round().max(1.0) as i32,
            ));
        }
    }

    let size = window.outer_size().map_err(|e| e.to_string())?;
    Ok((size.width as i32, size.height as i32))
}

pub(crate) fn compute_snap_layout(
    app: &tauri::AppHandle,
    edge: SnapEdge,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Result<(String, f64), String> {
    let monitor = find_monitor_for_window(app, edge, x, y, width, height)?;
    let ratio = compute_local_ratio_in_monitor(&monitor, edge, x, y, width, height);
    Ok((monitor.id.clone(), ratio))
}

pub(crate) fn compute_snap_ratio(
    app: &tauri::AppHandle,
    edge: SnapEdge,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Result<f64, String> {
    Ok(compute_snap_layout(app, edge, x, y, width, height)?.1)
}

pub(crate) fn resolve_snapped_position(
    app: &tauri::AppHandle,
    edge: SnapEdge,
    monitor_id: Option<&str>,
    ratio: f64,
    width: i32,
    height: i32,
) -> Result<ResolvedSnapPosition, String> {
    let (monitor, actual_edge) = resolve_saved_monitor_and_edge(app, edge, monitor_id)?;
    let content_inset = get_content_inset(monitor.scale_factor);
    let (base_x, base_y) =
        resolve_position_in_monitor(&monitor, actual_edge, ratio, width, height);

    let position = match actual_edge {
        SnapEdge::Left => (monitor.x - content_inset, base_y),
        SnapEdge::Right => (monitor.x + monitor.width - width + content_inset, base_y),
        SnapEdge::Top => (base_x, monitor.y - content_inset),
        SnapEdge::Bottom => (base_x, monitor.y + monitor.height - height + content_inset),
        SnapEdge::None => (base_x, base_y),
    };
    Ok(ResolvedSnapPosition {
        x: position.0,
        y: position.1,
        scale_factor: monitor.scale_factor,
        edge: actual_edge,
        monitor_id: monitor.id.clone(),
    })
}

fn resolve_hidden_position(
    app: &tauri::AppHandle,
    edge: SnapEdge,
    monitor_id: Option<&str>,
    ratio: f64,
    width: i32,
    height: i32,
    edge_hide_offset: i32,
) -> Result<ResolvedSnapPosition, String> {
    let (monitor, actual_edge) = resolve_saved_monitor_and_edge(app, edge, monitor_id)?;
    let content_inset = get_content_inset(monitor.scale_factor);
    let hide_offset = if edge_hide_offset == 0 {
        0
    } else {
        content_inset + edge_hide_offset
    };
    let (base_x, base_y) =
        resolve_position_in_monitor(&monitor, actual_edge, ratio, width, height);

    let position = match actual_edge {
        SnapEdge::Left => (monitor.x - width + hide_offset, base_y),
        SnapEdge::Right => (monitor.x + monitor.width - hide_offset, base_y),
        SnapEdge::Top => (base_x, monitor.y - height + hide_offset),
        SnapEdge::Bottom => (base_x, monitor.y + monitor.height - hide_offset),
        SnapEdge::None => (base_x, base_y),
    };
    Ok(ResolvedSnapPosition {
        x: position.0,
        y: position.1,
        scale_factor: monitor.scale_factor,
        edge: actual_edge,
        monitor_id: monitor.id.clone(),
    })
}

fn save_snap_layout(edge: SnapEdge, ratio: f64, monitor_id: Option<String>) {
    let edge_value = edge_to_setting_value(edge);
    let normalized_ratio = clamp_ratio(ratio);
    let _ = crate::services::settings::update_with(|settings| {
        settings.edge_snap_edge = edge_value.clone();
        settings.edge_snap_ratio = Some(normalized_ratio);
        settings.edge_snap_monitor_id = monitor_id.clone();
    });
}

fn clear_saved_snap_layout() {
    let _ = crate::services::settings::update_with(|settings| {
        settings.edge_snap_position = None;
        settings.edge_snap_edge = None;
        settings.edge_snap_ratio = None;
        settings.edge_snap_monitor_id = None;
    });
}

pub fn check_snap(window: &WebviewWindow) -> Result<(), String> {
    let settings = crate::get_settings();
    if !settings.edge_hide_enabled {
        return Ok(());
    }
    
    let (x, y, w, h) = crate::utils::positioning::get_window_bounds(window)?;
    
    let app = window.app_handle();
    let (monitor_x, monitor_y, monitor_w, monitor_h) = 
        crate::utils::screen::ScreenUtils::get_monitor_at_point(app, x, y)?;
    let monitor_right = monitor_x + monitor_w;
    let monitor_bottom = monitor_y + monitor_h;
    
    let (left_is_edge, right_is_edge, top_is_edge, bottom_is_edge) = 
        crate::utils::screen::ScreenUtils::get_real_edges_at_point(app, x, y)?;
    
    let edge = if left_is_edge && (x - monitor_x).abs() <= SNAP_THRESHOLD {
        Some(SnapEdge::Left)
    } else if right_is_edge && (monitor_right - (x + w as i32)).abs() <= SNAP_THRESHOLD {
        Some(SnapEdge::Right)
    } else if top_is_edge && (y - monitor_y).abs() <= SNAP_THRESHOLD {
        Some(SnapEdge::Top)
    } else if bottom_is_edge && (monitor_bottom - (y + h as i32)).abs() <= SNAP_THRESHOLD {
        Some(SnapEdge::Bottom)
    } else {
        None
    };
    
    if let Some(edge) = edge {
        let (monitor_id, ratio) = compute_snap_layout(app, edge, x, y, w as i32, h as i32)?;
        set_snap_edge(edge, Some((x, y)), Some(monitor_id.clone()), Some(ratio));
        save_snap_layout(edge, ratio, Some(monitor_id));
        snap_to_edge(window, edge)?;
        super::edge_monitor::start_edge_monitoring();
    } else {
        clear_snap();
        clear_saved_snap_layout();
        super::edge_monitor::stop_edge_monitoring();
    }
    
    Ok(())
}

pub fn snap_to_edge(window: &WebviewWindow, edge: SnapEdge) -> Result<(), String> {
    let size = window.outer_size().map_err(|e| e.to_string())?;
    let (x, y, _, _) = crate::utils::positioning::get_window_bounds(window)?;
    let settings = crate::get_settings();
    let ratio = compute_snap_ratio(
        window.app_handle(),
        edge,
        x,
        y,
        size.width as i32,
        size.height as i32,
    )?;
    
    let resolved = resolve_snapped_position(
        window.app_handle(),
        edge,
        settings.edge_snap_monitor_id.as_deref(),
        ratio,
        size.width as i32,
        size.height as i32,
    )?;
    
    window.set_position(tauri::PhysicalPosition::new(resolved.x, resolved.y))
        .map_err(|e| e.to_string())?;
    
    Ok(())
}

pub fn hide_snapped_window(window: &WebviewWindow) -> Result<(), String> {
    use tauri::Manager;
    
    let state = super::state::get_window_state();
    
    if !state.is_snapped || state.is_hidden {
        return Ok(());
    }
    
    if crate::is_context_menu_visible() {
        return Ok(());
    }

    crate::windows::preview_window::suppress_preview_for_main_window_hide(&window.app_handle());
    let _ = crate::windows::pin_image_window::close_image_preview(window.app_handle().clone());
    #[cfg(feature = "gpu-image-viewer")]
    let _ = crate::windows::native_pin_window::close_native_image_preview();
    let _ = crate::windows::preview_window::close_preview_window(window.app_handle().clone());

    let size = window.outer_size().map_err(|e| e.to_string())?;
    let (x, y, _, _) = crate::utils::positioning::get_window_bounds(window)?;
    let settings = crate::get_settings();
    let ratio = state
        .snap_ratio
        .or(settings.edge_snap_ratio)
        .unwrap_or(compute_snap_ratio(
            window.app_handle(),
            state.snap_edge,
            x,
            y,
            size.width as i32,
            size.height as i32,
        )?);
    let resolved = resolve_hidden_position(
        window.app_handle(),
        state.snap_edge,
        state
            .snap_monitor_id
            .as_deref()
            .or(settings.edge_snap_monitor_id.as_deref()),
        ratio,
        size.width as i32,
        size.height as i32,
        settings.edge_hide_offset,
    )?;
    
    // 根据动画配置决定是否使用过渡
    if settings.clipboard_animation_enabled {
        animate_window_position(window, x, y, resolved.x, resolved.y, 200)?;
    } else {
        window.set_position(tauri::PhysicalPosition::new(resolved.x, resolved.y))
            .map_err(|e| e.to_string())?;
    }
    set_snap_edge(
        resolved.edge,
        Some((resolved.x, resolved.y)),
        Some(resolved.monitor_id.clone()),
        Some(ratio),
    );
    set_hidden(true);
    save_snap_layout(resolved.edge, ratio, Some(resolved.monitor_id));
    
    super::state::set_window_state(super::state::WindowState::Hidden);
    crate::services::memory::schedule_cleanup_after_main_window_hide();
    
    crate::input_monitor::disable_mouse_monitoring();
    crate::input_monitor::disable_navigation_keys();
    
    Ok(())
}

pub fn refresh_hidden_snapped_window(window: &WebviewWindow) -> Result<(), String> {
    let state = super::state::get_window_state();

    if !state.is_snapped || !state.is_hidden {
        return Ok(());
    }

    let settings = crate::get_settings();
    let size = window.outer_size().map_err(|e| e.to_string())?;
    let (x, y, _, _) = crate::utils::positioning::get_window_bounds(window)?;
    let ratio = state
        .snap_ratio
        .or(settings.edge_snap_ratio)
        .unwrap_or(compute_snap_ratio(
            window.app_handle(),
            state.snap_edge,
            x,
            y,
            size.width as i32,
            size.height as i32,
        )?);
    let resolved = resolve_hidden_position(
        window.app_handle(),
        state.snap_edge,
        state
            .snap_monitor_id
            .as_deref()
            .or(settings.edge_snap_monitor_id.as_deref()),
        ratio,
        size.width as i32,
        size.height as i32,
        settings.edge_hide_offset,
    )?;

    window
        .set_position(tauri::PhysicalPosition::new(resolved.x, resolved.y))
        .map_err(|e| e.to_string())?;
    set_snap_edge(
        resolved.edge,
        Some((resolved.x, resolved.y)),
        Some(resolved.monitor_id.clone()),
        Some(ratio),
    );
    set_hidden(true);
    save_snap_layout(resolved.edge, ratio, Some(resolved.monitor_id));

    Ok(())
}

pub fn needs_hidden_snap_refresh(window: &WebviewWindow) -> Result<bool, String> {
    let state = super::state::get_window_state();

    if !state.is_snapped || !state.is_hidden {
        return Ok(false);
    }

    let settings = crate::get_settings();
    let size = window.outer_size().map_err(|e| e.to_string())?;
    let (x, y, _, _) = crate::utils::positioning::get_window_bounds(window)?;
    let ratio = state
        .snap_ratio
        .or(settings.edge_snap_ratio)
        .unwrap_or(compute_snap_ratio(
            window.app_handle(),
            state.snap_edge,
            x,
            y,
            size.width as i32,
            size.height as i32,
        )?);
    let resolved = resolve_hidden_position(
        window.app_handle(),
        state.snap_edge,
        state
            .snap_monitor_id
            .as_deref()
            .or(settings.edge_snap_monitor_id.as_deref()),
        ratio,
        size.width as i32,
        size.height as i32,
        settings.edge_hide_offset,
    )?;

    const POSITION_TOLERANCE: i32 = 2;
    Ok(
        (x - resolved.x).abs() > POSITION_TOLERANCE
            || (y - resolved.y).abs() > POSITION_TOLERANCE,
    )
}

pub fn show_snapped_window(window: &WebviewWindow) -> Result<(), String> {
    crate::windows::preview_window::resume_preview_after_main_window_show();

    let state = super::state::get_window_state();
    
    if !state.is_snapped || !state.is_hidden {
        return Ok(());
    }
    
    let size = window.outer_size().map_err(|e| e.to_string())?;
    let (x, y, _, _) = crate::utils::positioning::get_window_bounds(window)?;
    let settings = crate::get_settings();
    let ratio = state
        .snap_ratio
        .or(settings.edge_snap_ratio)
        .unwrap_or(compute_snap_ratio(
            window.app_handle(),
            state.snap_edge,
            x,
            y,
            size.width as i32,
            size.height as i32,
        )?);
    let resolved = resolve_snapped_position(
        window.app_handle(),
        state.snap_edge,
        state
            .snap_monitor_id
            .as_deref()
            .or(settings.edge_snap_monitor_id.as_deref()),
        ratio,
        size.width as i32,
        size.height as i32,
    )?;
    
    if !window.is_visible().unwrap_or(false) {
        let _ = window.show();
    }
    let _ = window.emit("edge-snap-show", ());
    let _ = crate::commands::window::emit_main_window_refresh_needed_event(&window.app_handle());
    
    // 根据动画配置决定是否使用过渡
    if settings.clipboard_animation_enabled {
        animate_window_position(window, x, y, resolved.x, resolved.y, 200)?;
    } else {
        window.set_position(tauri::PhysicalPosition::new(resolved.x, resolved.y))
            .map_err(|e| e.to_string())?;
    }
    set_snap_edge(
        resolved.edge,
        Some((resolved.x, resolved.y)),
        Some(resolved.monitor_id.clone()),
        Some(ratio),
    );
    set_hidden(false);
    save_snap_layout(resolved.edge, ratio, Some(resolved.monitor_id));
    
    super::state::set_window_state(super::state::WindowState::Visible);
    let _ = super::refresh_always_on_top(window);

    crate::input_monitor::enable_mouse_monitoring();
    crate::input_monitor::enable_navigation_keys();
    
    Ok(())
}

fn animate_window_position(
    window: &WebviewWindow,
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
    duration_ms: u64,
) -> Result<(), String> {
    let version = ANIMATION_VERSION.fetch_add(1, Ordering::SeqCst) + 1;
    let window_clone = window.clone();
    
    std::thread::spawn(move || {
        let frame_duration = Duration::from_millis(16);
        let total_frames = duration_ms / 16;
        
        if total_frames == 0 {
            let _ = window_clone.set_position(tauri::PhysicalPosition::new(end_x, end_y));
            return;
        }
        
        let dx = end_x - start_x;
        let dy = end_y - start_y;
        
        for frame in 0..=total_frames {
            if ANIMATION_VERSION.load(Ordering::SeqCst) != version {
                return;
            }
            
            let progress = frame as f32 / total_frames as f32;
            let eased_progress = 1.0 - (1.0 - progress).powi(2);
            
            let current_x = start_x + (dx as f32 * eased_progress) as i32;
            let current_y = start_y + (dy as f32 * eased_progress) as i32;
            
            let _ = window_clone.set_position(tauri::PhysicalPosition::new(current_x, current_y));
            
            if frame < total_frames {
                std::thread::sleep(frame_duration);
            }
        }
        
        if ANIMATION_VERSION.load(Ordering::SeqCst) == version {
            let _ = window_clone.set_position(tauri::PhysicalPosition::new(end_x, end_y));
        }
    });
    
    Ok(())
}

pub fn restore_from_snap(window: &WebviewWindow) -> Result<(), String> {
    let state = super::state::get_window_state();
    
    if let Some(pos) = state.snap_position {
        window.set_position(tauri::PhysicalPosition::new(pos.0, pos.1))
            .map_err(|e| e.to_string())?;
    }
    
    clear_snap();
    Ok(())
}

pub fn is_window_snapped() -> bool {
    is_snapped()
}

// 启动时恢复贴边隐藏状态
pub fn restore_edge_snap_on_startup(window: &WebviewWindow) -> Result<(), String> {
    let settings = crate::get_settings();

    if !settings.edge_hide_enabled {
        return Ok(());
    }

    let snapped_edge = settings
        .edge_snap_edge
        .as_deref()
        .and_then(edge_from_setting_value);
    let snapped_ratio = settings.edge_snap_ratio.map(clamp_ratio);
    let (snapped_edge, snapped_ratio) = match (snapped_edge, snapped_ratio) {
        (Some(edge), Some(ratio)) => (edge, ratio),
        _ => return Ok(()),
    };

    let (restore_width, restore_height) = resolve_startup_restore_window_size(
        window,
        snapped_edge,
        settings.edge_snap_monitor_id.as_deref(),
        &settings,
    )?;
    let resolved = resolve_hidden_position(
        window.app_handle(),
        snapped_edge,
        settings.edge_snap_monitor_id.as_deref(),
        snapped_ratio,
        restore_width,
        restore_height,
        settings.edge_hide_offset,
    )?;

    set_snap_edge(
        resolved.edge,
        Some((resolved.x, resolved.y)),
        Some(resolved.monitor_id.clone()),
        Some(snapped_ratio),
    );
    set_hidden(true);
    
    window.set_position(tauri::PhysicalPosition::new(resolved.x, resolved.y))
        .map_err(|e| e.to_string())?;
    save_snap_layout(resolved.edge, snapped_ratio, Some(resolved.monitor_id));
    
    let _ = window.show();
    let _ = window.set_always_on_top(true);
    
    super::state::set_window_state(super::state::WindowState::Hidden);
    
    crate::input_monitor::disable_mouse_monitoring();
    crate::input_monitor::disable_navigation_keys();
    
    super::edge_monitor::start_edge_monitoring();
    
    Ok(())
}
