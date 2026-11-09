use fltk::{
    app,
    app::MouseWheel,
    draw,
    enums::Damage,
    enums::{Align, Color, Event, Font, FrameType, Key},
    frame::Frame,
    menu::{MenuButton, MenuButtonType},
    prelude::*,
    table::{TableContext, TableRow},
    window::DoubleWindow,
};
use once_cell::sync::Lazy;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    time::Duration,
};

#[derive(Debug, Clone)]
pub struct ListItem {
    pub id: i64,
    pub label: String,
    pub kind_label: String,
    pub is_pinned: bool,
}

#[derive(Debug, Clone)]
pub struct ShowOptions {
    pub items: Vec<ListItem>,
    pub footer_text: String,
    pub page_items: Vec<PageItem>,
    pub current_page: i64,
    pub theme: ThemeColors,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub physical_x: i32,
    pub physical_y: i32,
    pub physical_width: i32,
    pub physical_height: i32,
}

#[derive(Debug, Clone)]
pub struct PageItem {
    pub page_index: i64,
    pub label: String,
}

#[derive(Debug, Clone, Copy)]
pub struct ThemeColors {
    pub window_bg: (u8, u8, u8),
    pub panel_bg: (u8, u8, u8),
    pub footer_bg: (u8, u8, u8),
    pub hover_bg: (u8, u8, u8),
    pub border: (u8, u8, u8),
    pub accent: (u8, u8, u8),
    pub window_border: (u8, u8, u8),
    pub text: (u8, u8, u8),
    pub footer_text: (u8, u8, u8),
}

#[derive(Debug, Clone)]
pub enum UiEvent {
    ItemActivated(i64),
    PageScroll(i32),
    PageSelected(i64),
    Hidden,
}

#[derive(Debug, Clone)]
enum Command {
    Show(ShowOptions),
    Hide,
}

#[derive(Debug, Default)]
struct TableState {
    items: Vec<ListItem>,
}

#[derive(Debug, Default)]
struct PagePopupState {
    items: Vec<PageItem>,
    current_page: i64,
}

type UiCallback = Arc<dyn Fn(UiEvent) + Send + Sync + 'static>;

static COMMAND_SENDER: Lazy<Mutex<Option<app::Sender<Command>>>> = Lazy::new(|| Mutex::new(None));
static UI_CALLBACK: Lazy<Mutex<Option<UiCallback>>> = Lazy::new(|| Mutex::new(None));
static UI_STARTED: AtomicBool = AtomicBool::new(false);
static UI_VISIBLE: AtomicBool = AtomicBool::new(false);
static PANEL_BOUNDS: Lazy<Mutex<Option<(i32, i32, i32, i32)>>> = Lazy::new(|| Mutex::new(None));

const DEFAULT_WIDTH: i32 = 420;
const ROW_HEIGHT: i32 = 18;
const MAX_VISIBLE_ROWS: i32 = 25;
const MIN_VISIBLE_ROWS: i32 = 1;
const FOOTER_HEIGHT: i32 = 20;
const WINDOW_BORDER_WIDTH: i32 = 2;
const CONTENT_TOP_PADDING: i32 = 4;
const IDLE_LOOP_SLEEP_MS: u64 = 16;
const CONTENT_FONT_SIZE: i32 = 14;
const BADGE_FONT_SIZE: i32 = 11;
const PAGE_POPUP_MARGIN: i32 = 6;

pub fn preferred_height(item_count: usize) -> i32 {
    item_count.clamp(MIN_VISIBLE_ROWS as usize, MAX_VISIBLE_ROWS as usize) as i32 * ROW_HEIGHT
        + FOOTER_HEIGHT
        + CONTENT_TOP_PADDING
        + WINDOW_BORDER_WIDTH * 2
}

fn truncate_text_to_width(text: &str, max_width: i32) -> String {
    if max_width <= 0 || text.is_empty() {
        return String::new();
    }

    if draw::measure(text, false).0 <= max_width {
        return text.to_string();
    }

    let ellipsis = "…";
    let ellipsis_width = draw::measure(ellipsis, false).0;
    if ellipsis_width >= max_width {
        return ellipsis.to_string();
    }

    let mut result = String::new();
    for ch in text.chars() {
        let mut candidate = result.clone();
        candidate.push(ch);
        candidate.push('…');
        if draw::measure(&candidate, false).0 > max_width {
            break;
        }
        result.push(ch);
    }

    if result.is_empty() {
        ellipsis.to_string()
    } else {
        format!("{}{}", result, ellipsis)
    }
}

fn stripe_row_bg(theme: ThemeColors, row: i32) -> Color {
    if row % 2 == 0 {
        return color_from(theme.panel_bg);
    }

    let blend_channel = |base: u8, target: u8, weight: u32| -> u8 {
        let remain = 255_u32.saturating_sub(weight);
        (((base as u32 * remain) + (target as u32 * weight)) / 255) as u8
    };

    let target = if luminance(theme.panel_bg) >= 140 {
        (0, 0, 0)
    } else {
        (255, 255, 255)
    };

    Color::from_rgb(
        blend_channel(theme.panel_bg.0, target.0, 10),
        blend_channel(theme.panel_bg.1, target.1, 10),
        blend_channel(theme.panel_bg.2, target.2, 10),
    )
}

fn luminance(color: (u8, u8, u8)) -> u32 {
    ((color.0 as u32 * 299) + (color.1 as u32 * 587) + (color.2 as u32 * 114)) / 1000
}

fn lock_or_recover<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn ui_callback() -> Option<UiCallback> {
    lock_or_recover(&UI_CALLBACK).clone()
}

pub fn init<F>(callback: F) -> Result<(), String>
where
    F: Fn(UiEvent) + Send + Sync + 'static,
{
    let callback: UiCallback = Arc::new(callback);
    *lock_or_recover(&UI_CALLBACK) = Some(callback);

    if UI_STARTED.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    let (ready_tx, ready_rx) = mpsc::channel();

    std::thread::spawn(move || {
        let app = app::App::default();
        app::set_visible_focus(false);
        app::background(18, 20, 25);
        app::background2(27, 30, 37);
        app::foreground(234, 238, 244);
        let (sender, receiver) = app::channel::<Command>();
        let _ = ready_tx.send(sender);

        let table_state: Rc<RefCell<TableState>> = Rc::new(RefCell::new(TableState::default()));
        let page_popup_state: Rc<RefCell<PagePopupState>> =
            Rc::new(RefCell::new(PagePopupState::default()));
        let theme_state: Rc<RefCell<ThemeColors>> = Rc::new(RefCell::new(ThemeColors {
            window_bg: (255, 255, 255),
            panel_bg: (243, 244, 246),
            footer_bg: (229, 231, 235),
            hover_bg: (221, 232, 247),
            border: (199, 204, 212),
            accent: (59, 130, 246),
            window_border: (17, 24, 39),
            text: (17, 24, 39),
            footer_text: (107, 114, 128),
        }));

        let mut window = DoubleWindow::new(
            0,
            0,
            DEFAULT_WIDTH,
            preferred_height(MIN_VISIBLE_ROWS as usize),
            "",
        );
        window.set_border(false);
        window.set_override();
        window.set_frame(FrameType::BorderBox);
        window.set_color(Color::from_rgb(17, 24, 39));
        window.make_resizable(false);
        window.clear_visible_focus();

        let mut list = TableRow::new(
            WINDOW_BORDER_WIDTH,
            WINDOW_BORDER_WIDTH + CONTENT_TOP_PADDING,
            window.width() - WINDOW_BORDER_WIDTH * 2,
            window.height() - FOOTER_HEIGHT - WINDOW_BORDER_WIDTH * 2 - CONTENT_TOP_PADDING,
            "",
        );
        list.set_frame(FrameType::FlatBox);
        list.set_color(Color::from_rgb(243, 244, 246));
        list.set_rows(0);
        list.set_cols(1);
        list.set_col_header(false);
        list.set_row_header(false);
        list.set_col_resize(false);
        list.set_row_resize(false);
        list.set_col_width_all((window.width() - WINDOW_BORDER_WIDTH * 2).max(1));
        list.set_row_height_all(ROW_HEIGHT);
        list.set_scrollbar_size(0);
        list.end();

        let mut top_inset = Frame::new(
            WINDOW_BORDER_WIDTH,
            WINDOW_BORDER_WIDTH,
            window.width() - WINDOW_BORDER_WIDTH * 2,
            CONTENT_TOP_PADDING,
            "",
        );
        top_inset.set_frame(FrameType::FlatBox);
        top_inset.set_color(Color::from_rgb(243, 244, 246));

        let table_hover_row = Rc::new(Cell::new(None::<i32>));

        {
            let table_state = Rc::clone(&table_state);
            let table_hover_row = Rc::clone(&table_hover_row);
            let theme_state = Rc::clone(&theme_state);
            list.draw_cell(move |_, ctx, row, _col, x, y, w, h| match ctx {
                TableContext::StartPage => {
                    draw::set_font(Font::Helvetica, CONTENT_FONT_SIZE);
                }
                TableContext::Cell => {
                    let state = table_state.borrow();
                    let theme = *theme_state.borrow();
                    let item = state.items.get(row as usize);
                    let is_hover = table_hover_row.get() == Some(row);

                    let background = if is_hover {
                        color_from(theme.hover_bg)
                    } else {
                        stripe_row_bg(theme, row)
                    };

                    draw::push_clip(x, y, w, h);
                    draw::set_draw_color(background);
                    draw::draw_rectf(x, y, w, h);

                    draw::set_draw_color(color_from(theme.border));
                    draw::draw_line(x, y + h - 1, x + w, y + h - 1);

                    if let Some(item) = item {
                        let text_left = if item.is_pinned {
                            let dot_size = 6;
                            let dot_x = x + 8;
                            let dot_y = y + (h - dot_size) / 2;
                            draw::set_draw_color(color_from(theme.accent));
                            draw::draw_rectf(dot_x, dot_y, dot_size, dot_size);
                            x + 20
                        } else {
                            x + 10
                        };

                        let badge_gap = 8;
                        let badge_padding_x = 6;
                        let badge_height = 14;
                        let badge_width = if item.kind_label.is_empty() {
                            0
                        } else {
                            draw::set_font(Font::Helvetica, BADGE_FONT_SIZE);
                            draw::measure(&item.kind_label, false).0 + badge_padding_x * 2
                        };
                        let text_right = if badge_width > 0 {
                            x + w - badge_width - badge_gap - 10
                        } else {
                            x + w - 10
                        };
                        let text_width = (text_right - text_left).max(0);

                        draw::set_font(Font::Helvetica, CONTENT_FONT_SIZE);
                        let visible_label = truncate_text_to_width(&item.label, text_width);
                        draw::set_draw_color(color_from(theme.text));
                        draw::draw_text2(
                            &visible_label,
                            text_left,
                            y,
                            text_width,
                            h,
                            Align::Left | Align::Inside,
                        );

                        if badge_width > 0 {
                            let badge_x = x + w - badge_width - 10;
                            let badge_y = y + (h - badge_height) / 2;
                            let badge_bg = if is_hover {
                                color_from(theme.footer_bg)
                            } else {
                                color_from(theme.window_bg)
                            };

                            draw::set_draw_color(badge_bg);
                            draw::draw_rectf(badge_x, badge_y, badge_width, badge_height);
                            draw::set_draw_color(color_from(theme.border));
                            draw::draw_rect(badge_x, badge_y, badge_width, badge_height);

                            draw::set_font(Font::Helvetica, BADGE_FONT_SIZE);
                            draw::set_draw_color(color_from(theme.footer_text));
                            draw::draw_text2(
                                &item.kind_label,
                                badge_x,
                                badge_y,
                                badge_width,
                                badge_height,
                                Align::Center | Align::Inside,
                            );
                            draw::set_font(Font::Helvetica, CONTENT_FONT_SIZE);
                        }
                    }

                    draw::pop_clip();
                }
                _ => {}
            });
        }

        {
            let table_state = Rc::clone(&table_state);
            let table_hover_row = Rc::clone(&table_hover_row);
            list.handle(move |table, event| match event {
                Event::Move => {
                    let item_count = table_state.borrow().items.len() as i32;
                    if item_count <= 0 {
                        return false;
                    }

                    let relative_y = app::event_y() - table.y();
                    if relative_y < 0 || relative_y >= table.h() {
                        return false;
                    }

                    let hover_row = (relative_y / ROW_HEIGHT).clamp(0, item_count - 1);
                    let previous_row = table_hover_row.replace(Some(hover_row));
                    if previous_row == Some(hover_row) {
                        return true;
                    }
                    if previous_row != Some(hover_row) {
                        redraw_row(table, previous_row);
                        redraw_row(table, Some(hover_row));
                    }
                    true
                }
                Event::Released => {
                    let item_count = table_state.borrow().items.len() as i32;
                    if item_count <= 0 {
                        return false;
                    }

                    let relative_y = app::event_y() - table.y();
                    if relative_y < 0 || relative_y >= table.h() {
                        return false;
                    }

                    let row = (relative_y / ROW_HEIGHT).clamp(0, item_count - 1);
                    let item = {
                        let state = table_state.borrow();
                        state.items.get(row as usize).cloned()
                    };

                    if let Some(item) = item {
                        if let Some(callback) = ui_callback() {
                            callback(UiEvent::ItemActivated(item.id));
                        }
                    }
                    true
                }
                Event::MouseWheel => {
                    let delta = match app::event_dy() {
                        MouseWheel::Up => -1,
                        MouseWheel::Down => 1,
                        _ => 0,
                    };

                    if delta == 0 {
                        return false;
                    }

                    if let Some(callback) = ui_callback() {
                        callback(UiEvent::PageScroll(delta));
                    }
                    true
                }
                Event::KeyDown if app::event_key() == Key::Enter => {
                    let item = {
                        let state = table_state.borrow();
                        let row = table_hover_row.get().unwrap_or(0);
                        state.items.get(row as usize).cloned()
                    };

                    if let Some(item) = item {
                        if let Some(callback) = ui_callback() {
                            callback(UiEvent::ItemActivated(item.id));
                        }
                        return true;
                    }

                    false
                }
                Event::Leave => {
                    let previous_row = table_hover_row.take();
                    if previous_row.is_some() {
                        redraw_row(table, previous_row);
                    }
                    false
                }
                _ => false,
            });
        }

        let mut footer = Frame::new(
            WINDOW_BORDER_WIDTH,
            window.height() - FOOTER_HEIGHT - WINDOW_BORDER_WIDTH,
            window.width() - WINDOW_BORDER_WIDTH * 2,
            FOOTER_HEIGHT,
            "",
        );
        footer.set_frame(FrameType::FlatBox);
        footer.set_color(Color::from_rgb(229, 231, 235));
        footer.set_label_color(Color::from_rgb(107, 114, 128));
        footer.set_label_size(12);
        footer.set_align(Align::Inside | Align::Center);

        let mut footer_separator = Frame::new(
            WINDOW_BORDER_WIDTH,
            window.height() - FOOTER_HEIGHT - WINDOW_BORDER_WIDTH,
            window.width() - WINDOW_BORDER_WIDTH * 2,
            1,
            "",
        );
        footer_separator.set_frame(FrameType::FlatBox);
        footer_separator.set_color(Color::from_rgb(199, 204, 212));

        let mut page_menu = MenuButton::new(0, 0, 0, 0, "");
        page_menu.set_type(MenuButtonType::Popup3);
        page_menu.set_frame(FrameType::NoBox);
        page_menu.clear_visible_focus();

        {
            let mut list = list.clone();
            let mut top_inset = top_inset.clone();
            let mut footer = footer.clone();
            let mut footer_separator = footer_separator.clone();
            let mut page_menu = page_menu.clone();
            let page_popup_state = Rc::clone(&page_popup_state);
            window.handle(move |win, event| match event {
                Event::Unfocus | Event::Deactivate | Event::Close | Event::Hide => {
                    if win.shown() {
                        win.hide();
                    }
                    UI_VISIBLE.store(false, Ordering::SeqCst);
                    if let Some(callback) = ui_callback() {
                        callback(UiEvent::Hidden);
                    }
                    true
                }
                Event::KeyDown if app::event_key() == Key::Escape => {
                    win.hide();
                    UI_VISIBLE.store(false, Ordering::SeqCst);
                    if let Some(callback) = ui_callback() {
                        callback(UiEvent::Hidden);
                    }
                    true
                }
                Event::Resize => {
                    let inner_width = (win.width() - WINDOW_BORDER_WIDTH * 2).max(1);
                    let inner_height = (win.height() - WINDOW_BORDER_WIDTH * 2).max(1);
                    list.resize(
                        WINDOW_BORDER_WIDTH,
                        WINDOW_BORDER_WIDTH + CONTENT_TOP_PADDING,
                        inner_width,
                        (inner_height - FOOTER_HEIGHT - CONTENT_TOP_PADDING).max(1),
                    );
                    top_inset.resize(
                        WINDOW_BORDER_WIDTH,
                        WINDOW_BORDER_WIDTH,
                        inner_width,
                        CONTENT_TOP_PADDING,
                    );
                    footer.resize(
                        WINDOW_BORDER_WIDTH,
                        win.height() - FOOTER_HEIGHT - WINDOW_BORDER_WIDTH,
                        inner_width,
                        FOOTER_HEIGHT,
                    );
                    footer_separator.resize(
                        WINDOW_BORDER_WIDTH,
                        win.height() - FOOTER_HEIGHT - WINDOW_BORDER_WIDTH,
                        inner_width,
                        1,
                    );
                    true
                }
                Event::Released => {
                    let x = app::event_x();
                    let y = app::event_y();
                    let in_footer = point_in_widget(&footer, x, y);

                    if in_footer {
                        let item_count = page_popup_state.borrow().items.len();
                        if item_count <= 1 {
                            return false;
                        }

                        page_menu.clear();
                        {
                            let state = page_popup_state.borrow();
                            for item in &state.items {
                                page_menu.add_choice(&item.label);
                            }
                        }

                        let anchor_x = (footer.x() + footer.w() - PAGE_POPUP_MARGIN).max(0);
                        let anchor_y = footer.y().max(0);
                        page_menu.resize(anchor_x, anchor_y, 1, footer.h().max(1));

                        if let Some(selected) = page_menu.popup() {
                            if let Some(label) = selected.label() {
                                let page_index = {
                                    let state = page_popup_state.borrow();
                                    state
                                        .items
                                        .iter()
                                        .find(|item| item.label == label)
                                        .map(|item| item.page_index)
                                };

                                if let Some(page_index) = page_index {
                                    if let Some(callback) = ui_callback() {
                                        callback(UiEvent::PageSelected(page_index));
                                    }
                                    return true;
                                }
                            }
                        }

                        page_menu.resize(0, 0, 0, 0);

                        return true;
                    }

                    false
                }
                _ => false,
            });
        }

        window.end();

        loop {
            let _ = app.wait();

            if let Some(command) = receiver.recv() {
                match command {
                    Command::Show(options) => {
                        {
                            let mut state = table_state.borrow_mut();
                            state.items = options.items;
                        }
                        {
                            let mut state = page_popup_state.borrow_mut();
                            state.items = options.page_items;
                            state.current_page = options.current_page;
                        }
                        *theme_state.borrow_mut() = options.theme;
                        footer.set_label(&options.footer_text);
                        window.set_color(color_from(options.theme.window_border));
                        list.set_color(color_from(options.theme.panel_bg));
                        top_inset.set_color(color_from(options.theme.panel_bg));
                        footer.set_color(color_from(options.theme.footer_bg));
                        footer.set_label_color(color_from(options.theme.footer_text));
                        footer_separator.set_color(color_from(options.theme.border));
                        page_menu.set_color(color_from(options.theme.window_bg));
                        page_menu.set_text_color(color_from(options.theme.text));
                        page_menu.set_selection_color(color_from(options.theme.hover_bg));
                        window.resize(
                            options.x,
                            options.y,
                            options.width.max(280),
                            options.height.max(ROW_HEIGHT * MIN_VISIBLE_ROWS),
                        );
                        let inner_width = (window.width() - WINDOW_BORDER_WIDTH * 2).max(1);
                        let inner_height = (window.height() - WINDOW_BORDER_WIDTH * 2).max(1);
                        list.resize(
                            WINDOW_BORDER_WIDTH,
                            WINDOW_BORDER_WIDTH + CONTENT_TOP_PADDING,
                            inner_width,
                            (inner_height - FOOTER_HEIGHT - CONTENT_TOP_PADDING).max(1),
                        );
                        top_inset.resize(
                            WINDOW_BORDER_WIDTH,
                            WINDOW_BORDER_WIDTH,
                            inner_width,
                            CONTENT_TOP_PADDING,
                        );
                        list.set_rows(table_state.borrow().items.len() as i32);
                        list.set_cols(1);
                        list.set_col_width_all(inner_width);
                        list.set_row_height_all(ROW_HEIGHT);
                        footer.resize(
                            WINDOW_BORDER_WIDTH,
                            window.height() - FOOTER_HEIGHT - WINDOW_BORDER_WIDTH,
                            inner_width,
                            FOOTER_HEIGHT,
                        );
                        footer_separator.resize(
                            WINDOW_BORDER_WIDTH,
                            window.height() - FOOTER_HEIGHT - WINDOW_BORDER_WIDTH,
                            inner_width,
                            1,
                        );
                        top_inset.redraw();
                        list.redraw();
                        footer_separator.redraw();
                        footer.redraw();
                        window.show();
                        apply_physical_bounds(
                            &window,
                            options.physical_x,
                            options.physical_y,
                            options.physical_width,
                            options.physical_height,
                        );
                        *lock_or_recover(&PANEL_BOUNDS) = Some((
                            options.physical_x,
                            options.physical_y,
                            options.physical_width,
                            options.physical_height,
                        ));
                        UI_VISIBLE.store(true, Ordering::SeqCst);
                    }
                    Command::Hide => {
                        if window.shown() {
                            window.hide();
                        }
                        *lock_or_recover(&PANEL_BOUNDS) = None;
                        UI_VISIBLE.store(false, Ordering::SeqCst);
                    }
                }
            }

            std::thread::sleep(Duration::from_millis(IDLE_LOOP_SLEEP_MS));
        }
    });

    let sender = ready_rx
        .recv_timeout(Duration::from_secs(5))
        .map_err(|_| "FLTK 低占用列表初始化超时".to_string())?;
    *lock_or_recover(&COMMAND_SENDER) = Some(sender);

    Ok(())
}

pub fn show(options: ShowOptions) -> Result<(), String> {
    let sender = COMMAND_SENDER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
        .ok_or_else(|| "FLTK 低占用列表未初始化".to_string())?;
    sender.send(Command::Show(options));
    Ok(())
}

pub fn hide() -> Result<(), String> {
    let sender = COMMAND_SENDER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
        .ok_or_else(|| "FLTK 低占用列表未初始化".to_string())?;
    sender.send(Command::Hide);
    Ok(())
}

pub fn is_visible() -> bool {
    UI_VISIBLE.load(Ordering::SeqCst)
}

pub fn contains_point(x: i32, y: i32) -> bool {
    let bounds = lock_or_recover(&PANEL_BOUNDS);
    if let Some((left, top, width, height)) = *bounds {
        x >= left && x <= left + width && y >= top && y <= top + height
    } else {
        false
    }
}

#[cfg(target_os = "windows")]
fn apply_physical_bounds(window: &DoubleWindow, x: i32, y: i32, width: i32, height: i32) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWM_WINDOW_CORNER_PREFERENCE,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        MoveWindow, SetWindowPos, HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE,
        SWP_NOSIZE, SWP_SHOWWINDOW,
    };

    let hwnd = HWND(window.raw_handle());
    if hwnd.0.is_null() {
        return;
    }

    unsafe {
        let _ = MoveWindow(hwnd, x, y, width.max(1), height.max(1), true);
        let corner_preference = DWM_WINDOW_CORNER_PREFERENCE(2);
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner_preference as *const _ as _,
            std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        );
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_NOTOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn apply_physical_bounds(_window: &DoubleWindow, _x: i32, _y: i32, _width: i32, _height: i32) {}

fn redraw_row(table: &mut TableRow, row: Option<i32>) {
    let Some(row) = row else {
        return;
    };

    if let Some((x, y, w, h)) = table.find_cell(TableContext::Cell, row, 0) {
        table.set_damage_area(Damage::All, x, y, w, h);
    }
}

fn point_in_widget<W: WidgetExt>(widget: &W, x: i32, y: i32) -> bool {
    x >= widget.x()
        && x <= widget.x() + widget.w()
        && y >= widget.y()
        && y <= widget.y() + widget.h()
}

fn color_from(rgb: (u8, u8, u8)) -> Color {
    Color::from_rgb(rgb.0, rgb.1, rgb.2)
}
