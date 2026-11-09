use std::io;

use chrono::Local;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Clear, HighlightSpacing, Paragraph, Row, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Table, TableState, Wrap,
    },
    DefaultTerminal, Frame,
};

pub enum Tab {
    Clipboard,
    Favorites,
    Groups,
}

impl Tab {
    fn name(&self) -> &'static str {
        match self {
            Tab::Clipboard => "剪贴板",
            Tab::Favorites => "收藏",
            Tab::Groups => "分组",
        }
    }

    fn next(&self) -> Self {
        match self {
            Tab::Clipboard => Tab::Favorites,
            Tab::Favorites => Tab::Groups,
            Tab::Groups => Tab::Clipboard,
        }
    }

    fn prev(&self) -> Self {
        match self {
            Tab::Clipboard => Tab::Groups,
            Tab::Favorites => Tab::Clipboard,
            Tab::Groups => Tab::Favorites,
        }
    }
}

pub enum Screen {
    List,
    Detail(usize),
    ConfirmDelete(usize),
}

pub struct App {
    pub db: rusqlite::Connection,
    pub items: Vec<ClipboardRow>,
    pub total_count: i64,
    pub current_page: usize,
    pub page_size: usize,
    pub search_query: String,
    pub is_searching: bool,
    pub fav_items: Vec<FavoriteRow>,
    pub fav_total: i64,
    pub fav_page: usize,
    pub fav_search: String,
    pub fav_is_searching: bool,
    pub groups: Vec<GroupRow>,
    pub current_tab: Tab,
    pub table_state: TableState,
    pub scroll_state: ScrollbarState,
    pub screen: Screen,
    pub status_message: String,
    pub should_quit: bool,
}

pub struct ClipboardRow {
    pub id: i64,
    pub content_preview: String,
    pub content_full: String,
    pub html_content: Option<String>,
    pub content_type: String,
    pub is_pinned: bool,
    pub paste_count: i64,
    pub source_app: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub image_id: Option<String>,
}

pub struct FavoriteRow {
    pub id: String,
    pub title: String,
    pub content: String,
    pub html_content: Option<String>,
    pub content_type: String,
    pub group_name: String,
    pub image_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

pub struct GroupRow {
    pub name: String,
    pub item_count: i32,
    pub order: i32,
}

fn human_timestamp(ts: i64) -> String {
    if let Some(dt) = chrono::TimeZone::timestamp_opt(&Local, ts, 0).single() {
        dt.format("%m-%d %H:%M").to_string()
    } else {
        "-".into()
    }
}

fn human_timestamp_full(ts: i64) -> String {
    if let Some(dt) = chrono::TimeZone::timestamp_opt(&Local, ts, 0).single() {
        dt.format("%Y-%m-%d %H:%M:%S").to_string()
    } else {
        "-".into()
    }
}

fn content_preview(s: &str, max_len: usize) -> String {
    let s = s.replace('\n', " ").replace('\r', "");
    if s.chars().count() > max_len {
        let truncated: String = s.chars().take(max_len).collect();
        format!("{}...", truncated)
    } else {
        s
    }
}

fn type_label(content_type: &str, _image_id: Option<&str>) -> &'static str {
    if content_type.contains("image") {
        "image"
    } else if content_type.contains("file") {
        "file"
    } else if content_type.contains("rich_text") {
        "rich_text"
    } else if content_type.contains("link") {
        "link"
    } else {
        "text"
    }
}

fn header_style() -> Style {
    Style::new()
        .bg(Color::DarkGray)
        .add_modifier(Modifier::BOLD)
}

fn selected_style() -> Style {
    Style::new()
        .bg(Color::Rgb(70, 70, 100))
        .add_modifier(Modifier::BOLD)
}

fn pinned_style() -> Style {
    Style::new().fg(Color::Yellow)
}
impl App {
    fn item_count(&self) -> usize {
        match self.current_tab {
            Tab::Clipboard => self.items.len(),
            Tab::Favorites => self.fav_items.len(),
            Tab::Groups => self.groups.len(),
        }
    }

    fn tab_total(&self) -> i64 {
        match self.current_tab {
            Tab::Clipboard => self.total_count,
            Tab::Favorites => self.fav_total,
            Tab::Groups => self.groups.len() as i64,
        }
    }

    fn tab_page(&self) -> usize {
        match self.current_tab {
            Tab::Clipboard => self.current_page,
            Tab::Favorites => self.fav_page,
            Tab::Groups => 0,
        }
    }

    fn tab_total_pages(&self) -> usize {
        match self.current_tab {
            Tab::Clipboard => ((self.total_count as usize).saturating_sub(1) / self.page_size) + 1,
            Tab::Favorites => ((self.fav_total as usize).saturating_sub(1) / self.page_size) + 1,
            Tab::Groups => 1,
        }
    }
}
impl App {
    pub fn load_clipboard(&mut self) {
        let offset = (self.current_page * self.page_size) as i64;
        let limit = self.page_size as i64;

        let (rows, total) = if self.search_query.trim().is_empty() {
            let total: i64 = self
                .db
                .query_row("SELECT COUNT(*) FROM clipboard", [], |r| r.get(0))
                .unwrap_or(0);

            let mut stmt = self
                .db
                .prepare(
                    "SELECT id, content, html_content, content_type, is_pinned, paste_count,
                            source_app, created_at, updated_at, image_id
                     FROM clipboard
                     ORDER BY is_pinned DESC, item_order DESC, updated_at DESC
                     LIMIT ?1 OFFSET ?2",
                )
                .unwrap();

            let rows: Vec<ClipboardRow> = stmt
                .query_map(rusqlite::params![limit, offset], |row| {
                    let content: String = row.get(1)?;
                    Ok(ClipboardRow {
                        id: row.get(0)?,
                        content_preview: content_preview(&content, 60),
                        content_full: content,
                        html_content: row.get(2)?,
                        content_type: row.get(3)?,
                        is_pinned: row.get::<_, i64>(4)? != 0,
                        paste_count: row.get(5)?,
                        source_app: row.get(6)?,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                        image_id: row.get(9)?,
                    })
                })
                .unwrap()
                .filter_map(|r| r.ok())
                .collect();

            (rows, total)
        } else {
            let pattern = format!("%{}%", self.search_query);
            let total: i64 = self
                .db
                .query_row(
                    "SELECT COUNT(*) FROM clipboard WHERE content LIKE ?1",
                    rusqlite::params![&pattern],
                    |r| r.get(0),
                )
                .unwrap_or(0);

            let mut stmt = self
                .db
                .prepare(
                    "SELECT id, content, html_content, content_type, is_pinned, paste_count,
                            source_app, created_at, updated_at, image_id
                     FROM clipboard
                     WHERE content LIKE ?3
                     ORDER BY is_pinned DESC, item_order DESC, updated_at DESC
                     LIMIT ?1 OFFSET ?2",
                )
                .unwrap();

            let rows: Vec<ClipboardRow> = stmt
                .query_map(rusqlite::params![limit, offset, &pattern], |row| {
                    let content: String = row.get(1)?;
                    Ok(ClipboardRow {
                        id: row.get(0)?,
                        content_preview: content_preview(&content, 60),
                        content_full: content,
                        html_content: row.get(2)?,
                        content_type: row.get(3)?,
                        is_pinned: row.get::<_, i64>(4)? != 0,
                        paste_count: row.get(5)?,
                        source_app: row.get(6)?,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                        image_id: row.get(9)?,
                    })
                })
                .unwrap()
                .filter_map(|r| r.ok())
                .collect();

            (rows, total)
        };

        self.items = rows;
        self.total_count = total;
        self.scroll_state = ScrollbarState::new(self.items.len().max(1)).position(0);

        if !self.items.is_empty() && self.table_state.selected().is_none() {
            self.table_state.select(Some(0));
        }
        if self.items.is_empty() {
            self.table_state.select(None);
        }
    }

    pub fn delete_clipboard(&mut self, index: usize) -> bool {
        if let Some(item) = self.items.get(index) {
            let id = item.id;
            let _ = self.db.execute(
                "DELETE FROM clipboard_data WHERE target_kind = 'clipboard' AND target_id = ?1",
                rusqlite::params![id.to_string()],
            );
            match self
                .db
                .execute("DELETE FROM clipboard WHERE id = ?1", rusqlite::params![id])
            {
                Ok(n) if n > 0 => {
                    self.status_message = format!("已删除剪贴板项 #{}", id);
                    self.load_clipboard();
                    true
                }
                Ok(_) => {
                    self.status_message = format!("未找到剪贴板项 #{}", id);
                    false
                }
                Err(e) => {
                    self.status_message = format!("删除失败: {}", e);
                    false
                }
            }
        } else {
            false
        }
    }
    pub fn load_favorites(&mut self) {
        let offset = (self.fav_page * self.page_size) as i64;
        let limit = self.page_size as i64;

        let (rows, total) = if self.fav_search.trim().is_empty() {
            let total: i64 = self
                .db
                .query_row("SELECT COUNT(*) FROM favorites", [], |r| r.get(0))
                .unwrap_or(0);

            let mut stmt = self
                .db
                .prepare(
                    "SELECT id, title, content, html_content, content_type, image_id,
                            group_name, created_at, updated_at
                     FROM favorites
                     ORDER BY item_order DESC, updated_at DESC
                     LIMIT ?1 OFFSET ?2",
                )
                .unwrap();

            let rows: Vec<FavoriteRow> = stmt
                .query_map(rusqlite::params![limit, offset], |row| {
                    Ok(FavoriteRow {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        content: row.get(2)?,
                        html_content: row.get(3)?,
                        content_type: row.get(4)?,
                        image_id: row.get(5)?,
                        group_name: row.get(6)?,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                    })
                })
                .unwrap()
                .filter_map(|r| r.ok())
                .collect();

            (rows, total)
        } else {
            let pattern = format!("%{}%", self.fav_search);
            let total: i64 = self
                .db
                .query_row(
                    "SELECT COUNT(*) FROM favorites WHERE content LIKE ?1 OR title LIKE ?1",
                    rusqlite::params![&pattern],
                    |r| r.get(0),
                )
                .unwrap_or(0);

            let mut stmt = self
                .db
                .prepare(
                    "SELECT id, title, content, html_content, content_type, image_id,
                            group_name, created_at, updated_at
                     FROM favorites
                     WHERE content LIKE ?3 OR title LIKE ?3
                     ORDER BY item_order DESC, updated_at DESC
                     LIMIT ?1 OFFSET ?2",
                )
                .unwrap();

            let rows: Vec<FavoriteRow> = stmt
                .query_map(rusqlite::params![limit, offset, &pattern], |row| {
                    Ok(FavoriteRow {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        content: row.get(2)?,
                        html_content: row.get(3)?,
                        content_type: row.get(4)?,
                        image_id: row.get(5)?,
                        group_name: row.get(6)?,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                    })
                })
                .unwrap()
                .filter_map(|r| r.ok())
                .collect();

            (rows, total)
        };

        self.fav_items = rows;
        self.fav_total = total;
        self.scroll_state = ScrollbarState::new(self.fav_items.len().max(1)).position(0);

        if !self.fav_items.is_empty() && self.table_state.selected().is_none() {
            self.table_state.select(Some(0));
        }
        if self.fav_items.is_empty() {
            self.table_state.select(None);
        }
    }

    pub fn delete_favorite(&mut self, index: usize) -> bool {
        if let Some(item) = self.fav_items.get(index) {
            let id = &item.id;
            match self
                .db
                .execute("DELETE FROM favorites WHERE id = ?1", rusqlite::params![id])
            {
                Ok(n) if n > 0 => {
                    self.status_message = format!("已删除收藏项 {}", id);
                    self.load_favorites();
                    true
                }
                Ok(_) => {
                    self.status_message = format!("未找到收藏项 {}", id);
                    false
                }
                Err(e) => {
                    self.status_message = format!("删除失败: {}", e);
                    false
                }
            }
        } else {
            false
        }
    }
    pub fn load_groups(&mut self) {
        let mut stmt = self
            .db
            .prepare("SELECT name, order_index FROM groups ORDER BY order_index, name")
            .unwrap();

        let group_rows: Vec<(String, i32)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        drop(stmt);

        let mut groups = Vec::new();
        for (name, order) in group_rows {
            let count: i32 = self
                .db
                .query_row(
                    "SELECT COUNT(*) FROM favorites WHERE group_name = ?1",
                    rusqlite::params![&name],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            groups.push(GroupRow {
                name,
                item_count: count,
                order,
            });
        }

        self.groups = groups;
        if !self.groups.is_empty() && self.table_state.selected().is_none() {
            self.table_state.select(Some(0));
        }
        if self.groups.is_empty() {
            self.table_state.select(None);
        }
    }
    pub fn next_page(&mut self) {
        let total_pages = self.tab_total_pages().max(1);
        let page = self.tab_page();
        if page + 1 < total_pages {
            match self.current_tab {
                Tab::Clipboard => {
                    self.current_page += 1;
                    self.table_state.select(Some(0));
                    self.load_clipboard();
                }
                Tab::Favorites => {
                    self.fav_page += 1;
                    self.table_state.select(Some(0));
                    self.load_favorites();
                }
                Tab::Groups => {}
            }
        }
    }

    pub fn prev_page(&mut self) {
        let page = self.tab_page();
        if page > 0 {
            match self.current_tab {
                Tab::Clipboard => {
                    self.current_page -= 1;
                    self.load_clipboard();
                    let last = self.items.len().saturating_sub(1);
                    self.table_state.select(Some(last));
                }
                Tab::Favorites => {
                    self.fav_page -= 1;
                    self.load_favorites();
                    let last = self.fav_items.len().saturating_sub(1);
                    self.table_state.select(Some(last));
                }
                Tab::Groups => {}
            }
        }
    }

    pub fn select_next(&mut self) {
        let count = self.item_count();
        if count == 0 {
            return;
        }
        let i = self.table_state.selected().unwrap_or(0);
        if i + 1 < count {
            self.table_state.select(Some(i + 1));
            self.scroll_state = self.scroll_state.position(i + 1);
        }
    }

    pub fn select_prev(&mut self) {
        let i = self.table_state.selected().unwrap_or(0);
        if i > 0 {
            self.table_state.select(Some(i - 1));
            self.scroll_state = self.scroll_state.position(i - 1);
        }
    }
}
pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();

    let outer = Block::bordered()
        .title(" QuickClipboard 维护模式 ")
        .title_alignment(Alignment::Center)
        .border_style(Style::new().fg(Color::Cyan));
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .split(inner);

    match app.screen {
        Screen::List | Screen::ConfirmDelete(_) | Screen::Detail(_) => {
            draw_tab_bar(f, app, chunks[0]);

            match app.current_tab {
                Tab::Clipboard | Tab::Favorites => {
                    draw_search_bar(f, app, chunks[1]);
                    match app.current_tab {
                        Tab::Clipboard => draw_clipboard_table(f, app, chunks[2]),
                        Tab::Favorites => draw_favorites_table(f, app, chunks[2]),
                        _ => {}
                    }
                }
                Tab::Groups => {
                    draw_groups_info(f, app, chunks[1]);
                    draw_groups_table(f, app, chunks[2]);
                }
            }

            let (help_text, help_color): (&str, Color) = match &app.screen {
                Screen::List => match app.current_tab {
                    Tab::Clipboard => {
                        if app.is_searching {
                            ("输入关键字后按 Enter 搜索, Tab/Esc 取消搜索 | ←→ 切换面板 | ↑↓ 导航 | PgUp/PgDn 翻页 | D 删除 | Q 退出", Color::Gray)
                        } else {
                            ("←→ 切换面板 | ↑↓ 导航 | Enter 查看详情 | PgUp/PgDn 翻页 | Tab 搜索 | D 删除 | R 刷新 | Q 退出", Color::Gray)
                        }
                    }
                    Tab::Favorites => {
                        if app.fav_is_searching {
                            ("输入关键字后按 Enter 搜索, Tab/Esc 取消搜索 | ←→ 切换面板 | ↑↓ 导航 | PgUp/PgDn 翻页 | D 删除 | Q 退出", Color::Gray)
                        } else {
                            ("←→ 切换面板 | ↑↓ 导航 | Enter 查看详情 | PgUp/PgDn 翻页 | Tab 搜索 | D 删除 | R 刷新 | Q 退出", Color::Gray)
                        }
                    }
                    Tab::Groups => ("←→ 切换面板 | ↑↓ 导航 | Q 退出", Color::Gray),
                },
                Screen::Detail(_) => ("Esc 返回 | D 删除此项", Color::Gray),
                Screen::ConfirmDelete(_) => ("Enter / Y 确认删除 | Esc / N 取消", Color::Red),
            };
            let help = Paragraph::new(help_text)
                .style(Style::new().fg(help_color))
                .alignment(Alignment::Center);
            f.render_widget(help, chunks[3]);
        }
    }
    match (&app.screen, &app.current_tab) {
        (Screen::Detail(idx), Tab::Clipboard) => {
            if let Some(item) = app.items.get(*idx) {
                draw_clipboard_detail_popup(f, item, area);
            }
        }
        (Screen::ConfirmDelete(idx), Tab::Clipboard) => {
            if let Some(item) = app.items.get(*idx) {
                draw_clipboard_confirm_popup(f, item, area);
            }
        }
        (Screen::Detail(idx), Tab::Favorites) => {
            if let Some(item) = app.fav_items.get(*idx) {
                draw_favorite_detail_popup(f, item, area);
            }
        }
        (Screen::ConfirmDelete(idx), Tab::Favorites) => {
            if let Some(item) = app.fav_items.get(*idx) {
                draw_favorite_confirm_popup(f, item, area);
            }
        }
        _ => {}
    }
    if !app.status_message.is_empty() {
        let len = app.status_message.chars().count() as u16 + 4;
        let toast_area = Rect {
            x: area.x + area.width.saturating_sub(len + 2),
            y: area.y + 1,
            width: len.min(area.width),
            height: 1,
        };
        let toast = Paragraph::new(app.status_message.as_str())
            .style(Style::new().bg(Color::Rgb(50, 80, 50)).fg(Color::White));
        f.render_widget(Clear, toast_area);
        f.render_widget(toast, toast_area);
    }
}

fn draw_tab_bar(f: &mut Frame, app: &App, area: Rect) {
    let tabs = [Tab::Clipboard, Tab::Favorites, Tab::Groups];
    let spans: Vec<Span> = tabs
        .iter()
        .flat_map(|t| {
            let name = t.name();
            let style = if app.current_tab.name() == name {
                Style::new()
                    .fg(Color::Yellow)
                    .bg(Color::Rgb(60, 60, 80))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(Color::Gray)
            };
            vec![
                Span::raw("  "),
                Span::styled(name, style),
                Span::raw("  "),
                Span::raw("│"),
            ]
        })
        .take(tabs.len() * 4 - 1)
        .collect();

    let line = Line::from(spans);
    f.render_widget(Paragraph::new(line), area);
}

fn draw_search_bar(f: &mut Frame, app: &App, area: Rect) {
    let (is_searching, query) = match app.current_tab {
        Tab::Clipboard => (app.is_searching, &app.search_query),
        Tab::Favorites => (app.fav_is_searching, &app.fav_search),
        _ => return,
    };

    let label = if is_searching {
        Span::styled(
            "搜索: ",
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled("搜索: ", Style::new().fg(Color::Gray))
    };

    let cursor = if is_searching { "█" } else { "" };
    let query_text = format!("{}{}", query, cursor);

    let mut spans: Vec<Span> = vec![label, Span::raw(&query_text)];

    if !query.is_empty() && !is_searching {
        spans.push(Span::styled(
            format!("  (共 {} 条结果)", app.tab_total()),
            Style::new().fg(Color::Cyan),
        ));
    }

    if !is_searching {
        spans.push(Span::styled(
            "  按 Tab 开始搜索",
            Style::new().fg(Color::DarkGray),
        ));
    }

    let line = Line::from(spans);
    let p = Paragraph::new(line).block(Block::new().borders(Borders::NONE));
    f.render_widget(p, area);
}

fn draw_groups_info(f: &mut Frame, app: &App, area: Rect) {
    let info = format!("共 {} 个分组", app.groups.len());
    let p = Paragraph::new(Span::styled(info, Style::new().fg(Color::Gray)));
    f.render_widget(p, area);
}
fn draw_clipboard_table(f: &mut Frame, app: &mut App, area: Rect) {
    let header = Row::new(vec![
        Cell::from("ID"),
        Cell::from("类型"),
        Cell::from("内容预览"),
        Cell::from("时间"),
        Cell::from("来源"),
    ])
    .style(header_style())
    .height(1);

    let widths = [
        Constraint::Length(6),
        Constraint::Length(10),
        Constraint::Percentage(50),
        Constraint::Length(12),
        Constraint::Length(12),
    ];

    let rows: Vec<Row> = app
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_sel = app.table_state.selected() == Some(i);
            let mut row = Row::new(vec![
                Cell::from(format!("{}", item.id)),
                Cell::from(type_label(&item.content_type, item.image_id.as_deref())),
                Cell::from(item.content_preview.as_str()),
                Cell::from(human_timestamp(item.created_at)),
                Cell::from(item.source_app.as_deref().unwrap_or("-")),
            ]);

            if item.is_pinned {
                row = row.style(if is_sel {
                    selected_style().add_modifier(Modifier::BOLD)
                } else {
                    pinned_style()
                });
            } else if is_sel {
                row = row.style(selected_style());
            }
            row
        })
        .collect();

    let total_pages = app.tab_total_pages().max(1);
    let footer_text = format!(
        " 第 {}/{} 页 (共 {} 条) ",
        app.current_page + 1,
        total_pages,
        app.total_count
    );

    let table = Table::new(rows, widths)
        .header(header)
        .column_spacing(1)
        .block(
            Block::new()
                .borders(Borders::NONE)
                .title_bottom(footer_text)
                .title_alignment(Alignment::Left),
        )
        .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_spacing(HighlightSpacing::WhenSelected);

    f.render_stateful_widget(table, area, &mut app.table_state);

    if !app.items.is_empty() {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"));
        let mut scroll_state =
            ScrollbarState::new(app.items.len()).position(app.table_state.selected().unwrap_or(0));
        f.render_stateful_widget(scrollbar, area, &mut scroll_state);
    }
}
fn draw_favorites_table(f: &mut Frame, app: &mut App, area: Rect) {
    let header = Row::new(vec![
        Cell::from("ID"),
        Cell::from("类型"),
        Cell::from("标题"),
        Cell::from("内容预览"),
        Cell::from("分组"),
        Cell::from("时间"),
    ])
    .style(header_style())
    .height(1);

    let widths = [
        Constraint::Length(10),
        Constraint::Length(8),
        Constraint::Length(14),
        Constraint::Percentage(40),
        Constraint::Length(10),
        Constraint::Length(12),
    ];

    let rows: Vec<Row> = app
        .fav_items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_sel = app.table_state.selected() == Some(i);
            let style = if is_sel {
                selected_style()
            } else {
                Style::new()
            };
            let preview = content_preview(&item.content, 60);
            Row::new(vec![
                Cell::from(item.id.as_str()),
                Cell::from(type_label(&item.content_type, item.image_id.as_deref())),
                Cell::from(if item.title.is_empty() {
                    "未命名"
                } else {
                    &item.title
                }),
                Cell::from(preview),
                Cell::from(item.group_name.as_str()),
                Cell::from(human_timestamp(item.created_at)),
            ])
            .style(style)
        })
        .collect();

    let total_pages = app.tab_total_pages().max(1);
    let footer_text = format!(
        " 第 {}/{} 页 (共 {} 条) ",
        app.fav_page + 1,
        total_pages,
        app.fav_total
    );

    let table = Table::new(rows, widths)
        .header(header)
        .column_spacing(1)
        .block(
            Block::new()
                .borders(Borders::NONE)
                .title_bottom(footer_text)
                .title_alignment(Alignment::Left),
        )
        .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_spacing(HighlightSpacing::WhenSelected);

    f.render_stateful_widget(table, area, &mut app.table_state);

    if !app.fav_items.is_empty() {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"));
        let mut scroll_state = ScrollbarState::new(app.fav_items.len())
            .position(app.table_state.selected().unwrap_or(0));
        f.render_stateful_widget(scrollbar, area, &mut scroll_state);
    }
}
fn draw_groups_table(f: &mut Frame, app: &mut App, area: Rect) {
    let header = Row::new(vec![
        Cell::from("分组名称"),
        Cell::from("条目数"),
        Cell::from("排序"),
    ])
    .style(header_style())
    .height(1);

    let widths = [
        Constraint::Percentage(50),
        Constraint::Length(10),
        Constraint::Length(8),
    ];

    let rows: Vec<Row> = app
        .groups
        .iter()
        .enumerate()
        .map(|(i, group)| {
            let is_sel = app.table_state.selected() == Some(i);
            let style = if is_sel {
                selected_style()
            } else {
                Style::new()
            };
            Row::new(vec![
                Cell::from(group.name.as_str()),
                Cell::from(format!("{}", group.item_count)),
                Cell::from(format!("{}", group.order)),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(rows, widths)
        .header(header)
        .column_spacing(1)
        .block(Block::new().borders(Borders::NONE))
        .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_spacing(HighlightSpacing::WhenSelected);

    f.render_stateful_widget(table, area, &mut app.table_state);
}
fn draw_clipboard_detail_popup(f: &mut Frame, item: &ClipboardRow, area: Rect) {
    let popup_width = area.width.saturating_sub(12).min(80);
    let popup_height = area.height.saturating_sub(6).min(20);
    let popup_area = Rect {
        x: area.x + (area.width.saturating_sub(popup_width)) / 2,
        y: area.y + (area.height.saturating_sub(popup_height)) / 2,
        width: popup_width,
        height: popup_height,
    };

    f.render_widget(Clear, popup_area);

    let block = Block::bordered()
        .title(format!(" 剪贴板项详情 (#{}) ", item.id))
        .title_alignment(Alignment::Center)
        .border_style(Style::new().fg(Color::Cyan))
        .style(Style::new().bg(Color::Rgb(30, 30, 40)));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let meta = vec![
        Line::from(vec![
            Span::styled("类型: ", Style::new().fg(Color::Gray)),
            Span::raw(type_label(&item.content_type, item.image_id.as_deref())),
            Span::raw("    "),
            Span::styled("来源: ", Style::new().fg(Color::Gray)),
            Span::raw(item.source_app.as_deref().unwrap_or("-")),
        ]),
        Line::from(vec![
            Span::styled("粘贴次数: ", Style::new().fg(Color::Gray)),
            Span::raw(format!("{}", item.paste_count)),
            Span::raw("    "),
            Span::styled("置顶: ", Style::new().fg(Color::Gray)),
            Span::raw(if item.is_pinned { "是" } else { "否" }),
            Span::raw("    "),
            Span::styled("时间: ", Style::new().fg(Color::Gray)),
            Span::raw(human_timestamp_full(item.created_at)),
        ]),
    ];

    let meta_height = meta.len() as u16 + 1;
    for (i, line) in meta.iter().enumerate() {
        let pos = Rect {
            x: inner.x + 1,
            y: inner.y + i as u16,
            width: inner.width.saturating_sub(2),
            height: 1,
        };
        f.render_widget(Paragraph::new(line.clone()), pos);
    }

    let content_block = Block::bordered()
        .title(" 内容 ")
        .border_style(Style::new().fg(Color::DarkGray));

    let content_area = Rect {
        x: inner.x + 1,
        y: inner.y + meta_height,
        width: inner.width.saturating_sub(2),
        height: inner.height.saturating_sub(meta_height + 2),
    };

    let body_text = if item.content_full.chars().count() > 2000 {
        let truncated: String = item.content_full.chars().take(2000).collect();
        format!("{}\n\n... (内容过长，已截断显示)", truncated)
    } else {
        item.content_full.clone()
    };

    let content_para = Paragraph::new(body_text)
        .wrap(Wrap { trim: false })
        .scroll((0, 0));

    f.render_widget(content_block, content_area);
    f.render_widget(
        content_para,
        Rect {
            x: content_area.x + 1,
            y: content_area.y + 1,
            width: content_area.width.saturating_sub(2),
            height: content_area.height.saturating_sub(2),
        },
    );

    let hint = Paragraph::new(" Esc 返回  |  D 删除此项 ")
        .alignment(Alignment::Center)
        .style(Style::new().fg(Color::Gray));
    let hint_area = Rect {
        x: popup_area.x,
        y: popup_area.y + popup_area.height.saturating_sub(1),
        width: popup_area.width,
        height: 1,
    };
    f.render_widget(hint, hint_area);
}

fn draw_clipboard_confirm_popup(f: &mut Frame, item: &ClipboardRow, area: Rect) {
    let popup_width = 50u16;
    let popup_height = 9u16;
    let popup_area = Rect {
        x: area.x + (area.width.saturating_sub(popup_width)) / 2,
        y: area.y + (area.height.saturating_sub(popup_height)) / 2,
        width: popup_width,
        height: popup_height,
    };

    f.render_widget(Clear, popup_area);

    let block = Block::bordered()
        .title(" 确认删除 ")
        .title_alignment(Alignment::Center)
        .border_style(Style::new().fg(Color::Red))
        .style(Style::new().bg(Color::Rgb(40, 30, 30)));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let text = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            format!("  确定要删除剪贴板项 #{} 吗？", item.id),
            Style::new().fg(Color::White).add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            "  此操作不可撤销！",
            Style::new().fg(Color::Red),
        )]),
        Line::from(vec![Span::raw(format!(
            "  内容预览: {}",
            content_preview(&item.content_full, 30)
        ))
        .style(Style::new().fg(Color::Gray))]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  [ Enter / Y 确认 ]    [ Esc / N 取消 ]",
            Style::new().fg(Color::Yellow),
        )]),
    ];

    let p = Paragraph::new(text);
    f.render_widget(p, inner);
}
fn draw_favorite_detail_popup(f: &mut Frame, item: &FavoriteRow, area: Rect) {
    let popup_width = area.width.saturating_sub(12).min(80);
    let popup_height = area.height.saturating_sub(6).min(20);
    let popup_area = Rect {
        x: area.x + (area.width.saturating_sub(popup_width)) / 2,
        y: area.y + (area.height.saturating_sub(popup_height)) / 2,
        width: popup_width,
        height: popup_height,
    };

    f.render_widget(Clear, popup_area);

    let block = Block::bordered()
        .title(format!(" 收藏项详情 ({}) ", item.id))
        .title_alignment(Alignment::Center)
        .border_style(Style::new().fg(Color::Cyan))
        .style(Style::new().bg(Color::Rgb(30, 30, 40)));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let title_display = if item.title.is_empty() {
        "未命名"
    } else {
        &item.title
    };
    let meta = vec![
        Line::from(vec![
            Span::styled("标题: ", Style::new().fg(Color::Gray)),
            Span::raw(title_display),
        ]),
        Line::from(vec![
            Span::styled("分组: ", Style::new().fg(Color::Gray)),
            Span::raw(&item.group_name),
            Span::raw("    "),
            Span::styled("类型: ", Style::new().fg(Color::Gray)),
            Span::raw(type_label(&item.content_type, item.image_id.as_deref())),
        ]),
        Line::from(vec![
            Span::styled("创建: ", Style::new().fg(Color::Gray)),
            Span::raw(human_timestamp_full(item.created_at)),
            Span::raw("    "),
            Span::styled("更新: ", Style::new().fg(Color::Gray)),
            Span::raw(human_timestamp_full(item.updated_at)),
        ]),
    ];

    let meta_height = meta.len() as u16 + 1;
    for (i, line) in meta.iter().enumerate() {
        let pos = Rect {
            x: inner.x + 1,
            y: inner.y + i as u16,
            width: inner.width.saturating_sub(2),
            height: 1,
        };
        f.render_widget(Paragraph::new(line.clone()), pos);
    }

    let content_block = Block::bordered()
        .title(" 内容 ")
        .border_style(Style::new().fg(Color::DarkGray));

    let content_area = Rect {
        x: inner.x + 1,
        y: inner.y + meta_height,
        width: inner.width.saturating_sub(2),
        height: inner.height.saturating_sub(meta_height + 2),
    };

    let body_text = if item.content.chars().count() > 2000 {
        let truncated: String = item.content.chars().take(2000).collect();
        format!("{}\n\n... (内容过长，已截断显示)", truncated)
    } else {
        item.content.clone()
    };

    let content_para = Paragraph::new(body_text)
        .wrap(Wrap { trim: false })
        .scroll((0, 0));

    f.render_widget(content_block, content_area);
    f.render_widget(
        content_para,
        Rect {
            x: content_area.x + 1,
            y: content_area.y + 1,
            width: content_area.width.saturating_sub(2),
            height: content_area.height.saturating_sub(2),
        },
    );

    let hint = Paragraph::new(" Esc 返回  |  D 删除此项 ")
        .alignment(Alignment::Center)
        .style(Style::new().fg(Color::Gray));
    let hint_area = Rect {
        x: popup_area.x,
        y: popup_area.y + popup_area.height.saturating_sub(1),
        width: popup_area.width,
        height: 1,
    };
    f.render_widget(hint, hint_area);
}

fn draw_favorite_confirm_popup(f: &mut Frame, item: &FavoriteRow, area: Rect) {
    let popup_width = 50u16;
    let popup_height = 9u16;
    let popup_area = Rect {
        x: area.x + (area.width.saturating_sub(popup_width)) / 2,
        y: area.y + (area.height.saturating_sub(popup_height)) / 2,
        width: popup_width,
        height: popup_height,
    };

    f.render_widget(Clear, popup_area);

    let block = Block::bordered()
        .title(" 确认删除 ")
        .title_alignment(Alignment::Center)
        .border_style(Style::new().fg(Color::Red))
        .style(Style::new().bg(Color::Rgb(40, 30, 30)));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let title_display = if item.title.is_empty() {
        "未命名"
    } else {
        &item.title
    };
    let text = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            format!("  确定要删除收藏项「{}」吗？", title_display),
            Style::new().fg(Color::White).add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            "  此操作不可撤销！",
            Style::new().fg(Color::Red),
        )]),
        Line::from(vec![Span::raw(format!(
            "  ID: {}    分组: {}",
            item.id, item.group_name
        ))
        .style(Style::new().fg(Color::Gray))]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  [ Enter / Y 确认 ]    [ Esc / N 取消 ]",
            Style::new().fg(Color::Yellow),
        )]),
    ];

    let p = Paragraph::new(text);
    f.render_widget(p, inner);
}
pub fn run_tui(mut app: App) -> io::Result<()> {
    let mut terminal = ratatui::init();
    app.load_clipboard();
    app.load_favorites();
    app.load_groups();
    let result = event_loop(&mut terminal, &mut app);
    ratatui::restore();
    result
}

fn event_loop(terminal: &mut DefaultTerminal, app: &mut App) -> io::Result<()> {
    while !app.should_quit {
        terminal.draw(|f| draw(f, app))?;

        app.status_message.clear();

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Release {
                continue;
            }

            if key.code == KeyCode::Char('c')
                && key.modifiers.contains(event::KeyModifiers::CONTROL)
            {
                app.should_quit = true;
                continue;
            }
            if let Screen::List = &app.screen {
                match key.code {
                    KeyCode::Right => {
                        app.current_tab = app.current_tab.next();
                        app.table_state.select(Some(0));
                        app.scroll_state = ScrollbarState::new(1);
                        continue;
                    }
                    KeyCode::Left => {
                        app.current_tab = app.current_tab.prev();
                        app.table_state.select(Some(0));
                        app.scroll_state = ScrollbarState::new(1);
                        continue;
                    }
                    _ => {}
                }
            }

            match &app.screen {
                Screen::List => handle_list_input(app, key),
                Screen::Detail(_) => handle_detail_input(app, key),
                Screen::ConfirmDelete(idx) => handle_confirm_input(app, key, *idx),
            }
        }
    }
    Ok(())
}

fn handle_list_input(app: &mut App, key: event::KeyEvent) {
    let is_searching = match app.current_tab {
        Tab::Clipboard => app.is_searching,
        Tab::Favorites => app.fav_is_searching,
        Tab::Groups => false,
    };

    if is_searching {
        match key.code {
            KeyCode::Esc | KeyCode::Tab => match app.current_tab {
                Tab::Clipboard => {
                    app.search_query.clear();
                    app.is_searching = false;
                    app.current_page = 0;
                    app.load_clipboard();
                }
                Tab::Favorites => {
                    app.fav_search.clear();
                    app.fav_is_searching = false;
                    app.fav_page = 0;
                    app.load_favorites();
                }
                _ => {}
            },
            KeyCode::Enter => match app.current_tab {
                Tab::Clipboard => {
                    app.is_searching = false;
                    app.current_page = 0;
                    app.load_clipboard();
                }
                Tab::Favorites => {
                    app.fav_is_searching = false;
                    app.fav_page = 0;
                    app.load_favorites();
                }
                _ => {}
            },
            KeyCode::Backspace => {
                match app.current_tab {
                    Tab::Clipboard => app.search_query.pop(),
                    Tab::Favorites => app.fav_search.pop(),
                    _ => None,
                };
            }
            KeyCode::Char(c) => {
                match app.current_tab {
                    Tab::Clipboard => app.search_query.push(c),
                    Tab::Favorites => app.fav_search.push(c),
                    _ => {}
                };
            }
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => app.should_quit = true,

        KeyCode::Tab => match app.current_tab {
            Tab::Clipboard => {
                app.is_searching = true;
                app.search_query.clear();
            }
            Tab::Favorites => {
                app.fav_is_searching = true;
                app.fav_search.clear();
            }
            Tab::Groups => {
                app.status_message = "分组列表不支持搜索".into();
            }
        },

        KeyCode::Char('r') | KeyCode::Char('R') => {
            match app.current_tab {
                Tab::Clipboard => app.load_clipboard(),
                Tab::Favorites => app.load_favorites(),
                Tab::Groups => app.load_groups(),
            }
            app.status_message = "已刷新".into();
        }

        KeyCode::Down | KeyCode::Char('j') => app.select_next(),
        KeyCode::Up | KeyCode::Char('k') => app.select_prev(),
        KeyCode::PageDown => app.next_page(),
        KeyCode::PageUp => app.prev_page(),
        KeyCode::Home => {
            app.table_state.select(Some(0));
        }
        KeyCode::End => {
            let last = app.item_count().saturating_sub(1);
            app.table_state.select(Some(last));
        }

        KeyCode::Enter => {
            let idx = match app.table_state.selected() {
                Some(i) => i,
                None => return,
            };
            match app.current_tab {
                Tab::Clipboard if idx < app.items.len() => app.screen = Screen::Detail(idx),
                Tab::Favorites if idx < app.fav_items.len() => app.screen = Screen::Detail(idx),
                Tab::Groups => {}
                _ => {}
            }
        }

        KeyCode::Char('d') | KeyCode::Char('D') => {
            let idx = match app.table_state.selected() {
                Some(i) => i,
                None => return,
            };
            match app.current_tab {
                Tab::Clipboard if idx < app.items.len() => {
                    app.screen = Screen::ConfirmDelete(idx);
                }
                Tab::Favorites if idx < app.fav_items.len() => {
                    app.screen = Screen::ConfirmDelete(idx);
                }
                Tab::Groups => {
                    app.status_message = "分组不支持在维护模式下删除".into();
                }
                _ => {}
            }
        }

        KeyCode::Esc => match app.current_tab {
            Tab::Clipboard if !app.search_query.is_empty() => {
                app.search_query.clear();
                app.current_page = 0;
                app.load_clipboard();
                app.status_message = "已清除搜索".into();
            }
            Tab::Favorites if !app.fav_search.is_empty() => {
                app.fav_search.clear();
                app.fav_page = 0;
                app.load_favorites();
                app.status_message = "已清除搜索".into();
            }
            _ => {}
        },

        _ => {}
    }
}

fn handle_detail_input(app: &mut App, key: event::KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Enter => {
            app.screen = Screen::List;
        }
        KeyCode::Char('d') | KeyCode::Char('D') => {
            if let Screen::Detail(idx) = app.screen {
                app.screen = Screen::ConfirmDelete(idx);
            }
        }
        _ => {}
    }
}

fn handle_confirm_input(app: &mut App, key: event::KeyEvent, idx: usize) {
    match key.code {
        KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
            match app.current_tab {
                Tab::Clipboard => {
                    app.delete_clipboard(idx);
                }
                Tab::Favorites => {
                    app.delete_favorite(idx);
                }
                _ => {}
            }
            app.screen = Screen::List;
        }
        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
            app.screen = Screen::List;
        }
        _ => {}
    }
}
