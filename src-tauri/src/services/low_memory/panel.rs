use low_memory_fltk::{ListItem, PageItem, ShowOptions, ThemeColors, UiEvent};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use tauri::AppHandle;

use crate::services::database::{query_clipboard_items, QueryParams};

const PANEL_WIDTH_LOGICAL: i32 = 420;
const PANEL_PAGE_SIZE: i64 = 25;
const MAX_PREVIEW_CHARS: usize = 120;

#[derive(Debug, Clone, Copy, Default)]
struct PanelPosition {
    logical_x: i32,
    logical_y: i32,
    physical_x: i32,
    physical_y: i32,
    physical_width: i32,
    physical_height: i32,
}

#[derive(Debug, Default)]
struct PanelPageState {
    current_page: i64,
    last_position: Option<PanelPosition>,
}

static PANEL_PAGE_STATE: Lazy<Mutex<PanelPageState>> =
    Lazy::new(|| Mutex::new(PanelPageState::default()));

pub fn init_panel(app: AppHandle) -> Result<(), String> {
    low_memory_fltk::init(move |event| match event {
        UiEvent::ItemActivated(item_id) => {
            let app = app.clone();
            std::thread::spawn(move || {
                handle_item_activated(&app, item_id);
            });
        }
        UiEvent::PageScroll(delta) => {
            std::thread::spawn(move || {
                if let Err(error) = scroll_panel_page(delta) {
                    eprintln!("低占用模式列表滚动翻页失败: {}", error);
                }
            });
        }
        UiEvent::PageSelected(page) => {
            std::thread::spawn(move || {
                if let Err(error) = jump_panel_page(page) {
                    eprintln!("低占用模式列表跳转页失败: {}", error);
                }
            });
        }
        UiEvent::Hidden => {}
    })
}

pub fn show_panel() -> Result<(), String> {
    {
        let mut state = PANEL_PAGE_STATE.lock();
        state.current_page = 0;
        state.last_position = None;
    }
    show_panel_at_current_page(None)
}

fn scroll_panel_page(delta: i32) -> Result<bool, String> {
    let (current_page, last_position) = {
        let state = PANEL_PAGE_STATE.lock();
        (state.current_page, state.last_position)
    };

    let page = load_page(current_page)?;
    if page.total_pages <= 1 {
        return Ok(false);
    }

    let next_page = if delta > 0 {
        if current_page >= page.total_pages - 1 {
            0
        } else {
            current_page + 1
        }
    } else if delta < 0 {
        if current_page <= 0 {
            page.total_pages - 1
        } else {
            current_page - 1
        }
    } else {
        current_page
    };

    if next_page == current_page {
        return Ok(false);
    }

    {
        let mut state = PANEL_PAGE_STATE.lock();
        state.current_page = next_page;
    }

    show_panel_at_current_page(last_position)?;
    Ok(true)
}

fn jump_panel_page(page: i64) -> Result<bool, String> {
    let last_position = PANEL_PAGE_STATE.lock().last_position;
    let page_data = load_page(page)?;
    let target_page = page_data.current_page;

    {
        let mut state = PANEL_PAGE_STATE.lock();
        if state.current_page == target_page {
            return Ok(false);
        }
        state.current_page = target_page;
    }

    show_panel_at_current_page(last_position)?;
    Ok(true)
}

fn show_panel_at_current_page(position_override: Option<PanelPosition>) -> Result<(), String> {
    let current_page = PANEL_PAGE_STATE.lock().current_page;
    let page = load_page(current_page)?;
    let height_logical = low_memory_fltk::preferred_height(page.items.len());
    let position = if let Some(position) = position_override {
        rebuild_position_from_existing(position, height_logical)?
    } else {
        build_position(height_logical)?
    };
    let theme = resolve_panel_theme();

    {
        let mut state = PANEL_PAGE_STATE.lock();
        state.last_position = Some(position);
    }

    low_memory_fltk::show(ShowOptions {
        items: page.items,
        footer_text: format!(
            "第 {}/{} 页 · {}-{} / {} 条",
            page.current_page + 1,
            page.total_pages.max(1),
            page.range_start,
            page.range_end,
            page.total_count
        ),
        page_items: build_page_items(page.total_pages, page.total_count),
        current_page: page.current_page,
        theme,
        x: position.logical_x,
        y: position.logical_y,
        width: PANEL_WIDTH_LOGICAL,
        height: height_logical,
        physical_x: position.physical_x,
        physical_y: position.physical_y,
        physical_width: position.physical_width,
        physical_height: position.physical_height,
    })
}

fn rebuild_position_from_existing(
    previous: PanelPosition,
    height_logical: i32,
) -> Result<PanelPosition, String> {
    let app = crate::utils::screen::get_app_handle().ok_or("APP_HANDLE 未初始化")?;
    let scale_factor = crate::screen::ScreenUtils::get_scale_factor_at_point(
        app,
        previous.physical_x,
        previous.physical_y,
    );
    let physical_width = (PANEL_WIDTH_LOGICAL as f64 * scale_factor).round() as i32;
    let physical_height = (height_logical as f64 * scale_factor).round() as i32;
    let (physical_x, physical_y) = crate::screen::ScreenUtils::constrain_to_physical_bounds(
        app,
        previous.physical_x,
        previous.physical_y,
        physical_width,
        physical_height,
    )?;

    Ok(PanelPosition {
        logical_x: (physical_x as f64 / scale_factor).round() as i32,
        logical_y: (physical_y as f64 / scale_factor).round() as i32,
        physical_x,
        physical_y,
        physical_width,
        physical_height,
    })
}

fn build_position(height_logical: i32) -> Result<PanelPosition, String> {
    let (cursor_x, cursor_y) = crate::mouse::get_cursor_position();
    let monitor = crate::screen::ScreenUtils::get_monitor_at_cursor_global()?;
    let scale_factor = monitor.scale_factor();
    let width_physical = (PANEL_WIDTH_LOGICAL as f64 * scale_factor).round() as i32;
    let height_physical = (height_logical as f64 * scale_factor).round() as i32;
    let position = crate::utils::positioning::calculate_popup_position(
        cursor_x,
        cursor_y,
        width_physical,
        height_physical,
        &monitor,
    );
    let logical_x = (position.x as f64 / scale_factor).round() as i32;
    let logical_y = (position.y as f64 / scale_factor).round() as i32;

    Ok(PanelPosition {
        logical_x,
        logical_y,
        physical_x: position.x,
        physical_y: position.y,
        physical_width: width_physical,
        physical_height: height_physical,
    })
}

pub fn hide_panel() -> Result<(), String> {
    PANEL_PAGE_STATE.lock().last_position = None;
    low_memory_fltk::hide()
}

pub fn toggle_panel() -> Result<(), String> {
    if low_memory_fltk::is_visible() {
        hide_panel()
    } else {
        show_panel()
    }
}

pub fn is_panel_visible() -> bool {
    low_memory_fltk::is_visible()
}

pub fn is_point_in_panel(x: i32, y: i32) -> bool {
    low_memory_fltk::contains_point(x, y)
}

fn handle_item_activated(app: &AppHandle, item_id: i64) {
    use crate::services::database::get_clipboard_item_by_id;
    use crate::services::paste::paste_handler::paste_clipboard_item_with_update;
    use crate::services::system::restore_last_focus;

    if item_id <= 0 {
        return;
    }

    let _ = hide_panel();
    let _ = restore_last_focus();

    std::thread::sleep(std::time::Duration::from_millis(80));

    if let Ok(Some(item)) = get_clipboard_item_by_id(item_id) {
        if let Err(error) = paste_clipboard_item_with_update(&item) {
            eprintln!("低占用模式列表粘贴失败: {}", error);
            let _ = crate::services::notification::show_notification(
                app,
                "低占用模式",
                "列表项粘贴失败，请重试。",
            );
        }
    }
}

#[derive(Debug)]
struct PanelPage {
    items: Vec<ListItem>,
    total_count: i64,
    total_pages: i64,
    current_page: i64,
    range_start: i64,
    range_end: i64,
}

fn load_page(page: i64) -> Result<PanelPage, String> {
    let offset = page.max(0) * PANEL_PAGE_SIZE;
    let result = query_clipboard_items(QueryParams {
        offset,
        limit: PANEL_PAGE_SIZE,
        search: None,
        content_type: None,
    })?;

    let total_pages = if result.total_count == 0 {
        1
    } else {
        ((result.total_count + PANEL_PAGE_SIZE - 1) / PANEL_PAGE_SIZE).max(1)
    };

    let mut items: Vec<ListItem> = result
        .items
        .into_iter()
        .map(|item| ListItem {
            id: item.id,
            label: format_item_label(&item),
            kind_label: item_kind_label(&item.content_type).to_string(),
            is_pinned: item.is_pinned,
        })
        .collect();

    if items.is_empty() {
        items.push(ListItem {
            id: 0,
            label: "(暂无记录)".to_string(),
            kind_label: String::new(),
            is_pinned: false,
        });
    }

    let safe_total_count = result.total_count.max(0);
    let clamped_page = page.max(0).min(total_pages - 1);
    let range_start = if safe_total_count == 0 {
        0
    } else {
        clamped_page * PANEL_PAGE_SIZE + 1
    };
    let range_end = if safe_total_count == 0 {
        0
    } else {
        ((clamped_page + 1) * PANEL_PAGE_SIZE).min(safe_total_count)
    };

    Ok(PanelPage {
        items,
        total_count: safe_total_count,
        total_pages,
        current_page: clamped_page,
        range_start,
        range_end,
    })
}

fn build_page_items(total_pages: i64, total_count: i64) -> Vec<PageItem> {
    let total_pages = total_pages.max(1);
    let total_count = total_count.max(0);
    let mut items = Vec::with_capacity(total_pages as usize);

    for page_index in 0..total_pages {
        let range_start = if total_count == 0 {
            0
        } else {
            page_index * PANEL_PAGE_SIZE + 1
        };
        let range_end = if total_count == 0 {
            0
        } else {
            ((page_index + 1) * PANEL_PAGE_SIZE).min(total_count)
        };

        items.push(PageItem {
            page_index,
            label: format!(
                "第 {} 页 · {}-{} 条",
                page_index + 1,
                range_start,
                range_end
            ),
        });
    }

    items
}

fn normalize_text(text: &str) -> String {
    let mut result = String::new();
    let mut last_was_space = false;

    for c in text.chars() {
        if c.is_whitespace() {
            if !last_was_space {
                result.push(' ');
                last_was_space = true;
            }
        } else {
            result.push(c);
            last_was_space = false;
        }
    }

    result.trim().to_string()
}

fn summarize_text(text: &str) -> String {
    let text = normalize_text(text);
    if text.is_empty() {
        return "(空内容)".to_string();
    }

    let mut result = String::new();
    for (index, ch) in text.chars().enumerate() {
        if index >= MAX_PREVIEW_CHARS {
            result.push('…');
            break;
        }
        result.push(ch);
    }
    result
}

fn parse_files_content(content: &str) -> Option<Vec<String>> {
    if !content.starts_with("files:") {
        return None;
    }

    let json_str = &content[6..];
    let parsed: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let files = parsed.get("files")?.as_array()?;

    let names: Vec<String> = files
        .iter()
        .filter_map(|file| {
            file.get("name")
                .and_then(|name| name.as_str())
                .map(|name| name.to_string())
        })
        .collect();

    if names.is_empty() {
        None
    } else {
        Some(names)
    }
}

fn summarize_named_items(names: &[String], unit: &str) -> String {
    if names.is_empty() {
        return format!("(空{})", unit);
    }

    if names.len() == 1 {
        return summarize_text(&names[0]);
    }

    let first = summarize_text(&names[0]);
    let second = summarize_text(&names[1]);
    if names.len() == 2 {
        return format!("{} · {}", first, second);
    }

    format!("{} · {} 等 {} 个{}", first, second, names.len(), unit)
}

fn format_item_label(item: &crate::services::database::ClipboardItem) -> String {
    match item.content_type.as_str() {
        "text" | "link" | "rich_text" => summarize_text(&item.content),
        "image" => {
            if let Some(names) = parse_files_content(&item.content) {
                summarize_named_items(&names, "张图片")
            } else {
                "图片".to_string()
            }
        }
        "file" => {
            if let Some(names) = parse_files_content(&item.content) {
                summarize_named_items(&names, "个文件")
            } else {
                let filename = item.content.split(['/', '\\']).last().unwrap_or("文件");
                summarize_text(filename)
            }
        }
        _ => summarize_text(&item.content),
    }
}

fn item_kind_label(content_type: &str) -> &'static str {
    match content_type {
        "text" => "文本",
        "link" => "链接",
        "rich_text" => "富文",
        "image" => "图片",
        "file" => "文件",
        _ => "其他",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PanelThemeKind {
    Light,
    DarkModern,
    DarkClassic,
}

fn resolve_panel_theme() -> ThemeColors {
    match resolve_panel_theme_kind() {
        PanelThemeKind::Light => ThemeColors {
            window_bg: (255, 255, 255),
            panel_bg: (243, 244, 246),
            footer_bg: (229, 231, 235),
            hover_bg: blend_rgba((243, 244, 246), (59, 130, 246, 31)),
            border: blend_rgba((243, 244, 246), (17, 24, 39, 56)),
            accent: (59, 130, 246),
            window_border: (71, 85, 105),
            text: (17, 24, 39),
            footer_text: (107, 114, 128),
        },
        PanelThemeKind::DarkModern => ThemeColors {
            window_bg: (17, 24, 39),
            panel_bg: (31, 41, 55),
            footer_bg: (45, 51, 66),
            hover_bg: blend_rgba((31, 41, 55), (96, 165, 250, 46)),
            border: blend_rgba((31, 41, 55), (255, 255, 255, 56)),
            accent: (59, 130, 246),
            window_border: (229, 231, 235),
            text: (229, 231, 235),
            footer_text: (203, 213, 225),
        },
        PanelThemeKind::DarkClassic => ThemeColors {
            window_bg: (30, 30, 30),
            panel_bg: (42, 42, 42),
            footer_bg: (51, 51, 51),
            hover_bg: blend_rgba((42, 42, 42), (74, 137, 220, 46)),
            border: blend_rgba((42, 42, 42), (255, 255, 255, 56)),
            accent: (74, 137, 220),
            window_border: (224, 224, 224),
            text: (224, 224, 224),
            footer_text: (199, 199, 199),
        },
    }
}

fn resolve_panel_theme_kind() -> PanelThemeKind {
    let settings = crate::get_settings();
    let theme = settings.theme.trim();

    if theme == "dark" {
        return if settings.dark_theme_style == "modern" {
            PanelThemeKind::DarkModern
        } else {
            PanelThemeKind::DarkClassic
        };
    }

    if theme == "auto" && is_system_dark_mode() {
        return if settings.dark_theme_style == "modern" {
            PanelThemeKind::DarkModern
        } else {
            PanelThemeKind::DarkClassic
        };
    }

    PanelThemeKind::Light
}

fn blend_rgba(base: (u8, u8, u8), overlay: (u8, u8, u8, u8)) -> (u8, u8, u8) {
    let alpha = overlay.3 as f32 / 255.0;
    let blend_channel = |base_channel: u8, overlay_channel: u8| -> u8 {
        ((base_channel as f32 * (1.0 - alpha)) + (overlay_channel as f32 * alpha)).round() as u8
    };

    (
        blend_channel(base.0, overlay.0),
        blend_channel(base.1, overlay.1),
        blend_channel(base.2, overlay.2),
    )
}

#[cfg(target_os = "windows")]
fn is_system_dark_mode() -> bool {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let personalize =
        hkcu.open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
    let Ok(key) = personalize else {
        return false;
    };

    let value: Result<u32, _> = key.get_value("AppsUseLightTheme");
    matches!(value, Ok(0))
}

#[cfg(not(target_os = "windows"))]
fn is_system_dark_mode() -> bool {
    false
}
