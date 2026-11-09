pub mod commands;
pub mod window;

use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Mutex,
};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MenuRegion {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Default)]
struct MenuRegions {
    main: Option<MenuRegion>,
    subs: Vec<MenuRegion>,
}

impl MenuRegions {
    fn contains(&self, x: i32, y: i32) -> bool {
        let hit =
            |r: &MenuRegion| x >= r.x && x <= r.x + r.width && y >= r.y && y <= r.y + r.height;
        self.main.as_ref().map_or(false, hit) || self.subs.iter().any(hit)
    }

    fn update(&mut self, main: MenuRegion, subs: Vec<MenuRegion>) {
        self.main = Some(main);
        self.subs = subs;
    }

    fn clear(&mut self) {
        self.main = None;
        self.subs.clear();
    }
}

struct MenuState {
    result: Mutex<Option<String>>,
    options: Mutex<Option<window::ContextMenuRequest>>,
    regions: Mutex<MenuRegions>,
    visible: AtomicBool,
    active_session: AtomicU64,
    session_counter: AtomicU64,
}

impl MenuState {
    fn new() -> Self {
        Self {
            result: Mutex::new(None),
            options: Mutex::new(None),
            regions: Mutex::new(MenuRegions::default()),
            visible: AtomicBool::new(false),
            active_session: AtomicU64::new(0),
            session_counter: AtomicU64::new(1),
        }
    }
}

static STATE: OnceCell<MenuState> = OnceCell::new();

fn state() -> &'static MenuState {
    STATE.get_or_init(MenuState::new)
}

#[allow(dead_code)]
pub fn init() {
    let _ = state();
}

pub(crate) fn get_result() -> Option<String> {
    state().result.lock().ok()?.clone()
}

pub(crate) fn set_result(v: Option<String>) {
    if let Ok(mut result) = state().result.lock() {
        *result = v;
    }
}

pub(crate) fn clear_result() {
    set_result(None);
}

pub(crate) fn has_result() -> bool {
    state().result.lock().map(|r| r.is_some()).unwrap_or(false)
}

pub(crate) fn set_options(opt: window::ContextMenuRequest) {
    if let Ok(mut options) = state().options.lock() {
        *options = Some(opt);
    }
}

pub(crate) fn get_options() -> Option<window::ContextMenuRequest> {
    state().options.lock().ok()?.clone()
}

pub(crate) fn clear_options() {
    if let Ok(mut options) = state().options.lock() {
        *options = None;
    }
}

pub fn clear_options_for_session(sid: u64) {
    if let Ok(mut options) = state().options.lock() {
        if options.as_ref().map_or(false, |c| c.session_id == sid) {
            *options = None;
        }
    }
}

pub fn set_active_menu_session(sid: u64) {
    let state = state();
    state.active_session.store(sid, Ordering::Relaxed);
    state.visible.store(true, Ordering::Relaxed);
}

pub fn clear_active_menu_session(sid: u64) {
    let state = state();
    if state.active_session.load(Ordering::Relaxed) == sid {
        state.active_session.store(0, Ordering::Relaxed);
        state.visible.store(false, Ordering::Relaxed);
    }
}

pub fn get_active_menu_session() -> u64 {
    state().active_session.load(Ordering::Relaxed)
}

pub fn next_menu_session_id() -> u64 {
    state().session_counter.fetch_add(1, Ordering::Relaxed)
}

pub fn is_menu_visible() -> bool {
    state().visible.load(Ordering::Relaxed)
}

pub fn update_menu_regions(main: MenuRegion, subs: Vec<MenuRegion>) {
    if let Ok(mut regions) = state().regions.lock() {
        regions.update(main, subs);
    }
}

pub fn is_point_in_menu_region(x: i32, y: i32) -> bool {
    state()
        .regions
        .lock()
        .map(|r| r.contains(x, y))
        .unwrap_or(false)
}

pub fn clear_menu_regions() {
    if let Ok(mut regions) = state().regions.lock() {
        regions.clear();
    }
}

pub fn is_context_menu_visible() -> bool {
    is_menu_visible()
}
