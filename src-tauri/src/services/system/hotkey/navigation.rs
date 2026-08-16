use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use super::global::{get_app, parse_shortcut};

static NAVIGATION_SHORTCUTS: Lazy<Mutex<Vec<NavigationShortcutRegistration>>> =
    Lazy::new(|| Mutex::new(Vec::new()));
static NAVIGATION_HOTKEYS_LIFECYCLE_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
static NAVIGATION_HOTKEYS_DESIRED: AtomicBool = AtomicBool::new(false);
static NAVIGATION_HOTKEYS_REGISTERED: AtomicBool = AtomicBool::new(false);
static NAVIGATION_REPEAT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

static NAVIGATION_THROTTLE_STATE: Lazy<Mutex<HashMap<String, Instant>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static NAVIGATION_REPEAT_TOKENS: Lazy<Mutex<HashMap<String, u64>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

const NAVIGATION_REPEAT_INITIAL_DELAY: Duration = Duration::from_millis(300);
const NAVIGATION_FAST_REPEAT_INTERVAL: Duration = Duration::from_millis(45);

#[derive(Clone)]
struct NavigationShortcutRegistration {
    id: String,
    shortcut: String,
}

#[derive(Clone)]
struct NavigationShortcutConfig {
    id: &'static str,
    action: &'static str,
    shortcut: String,
}

pub fn enable_navigation_hotkeys() {
    NAVIGATION_HOTKEYS_DESIRED.store(true, Ordering::SeqCst);
    sync_navigation_hotkeys_for_foreground();
}

pub fn disable_navigation_hotkeys() {
    NAVIGATION_HOTKEYS_DESIRED.store(false, Ordering::SeqCst);
    unregister_navigation_hotkeys();
}

pub fn sync_navigation_hotkeys_for_foreground() {
    let _lifecycle_guard = NAVIGATION_HOTKEYS_LIFECYCLE_LOCK.lock();
    sync_navigation_hotkeys_for_foreground_locked();
}

fn sync_navigation_hotkeys_for_foreground_locked() {
    if !NAVIGATION_HOTKEYS_DESIRED.load(Ordering::SeqCst) {
        unregister_navigation_hotkeys_locked();
        return;
    }

    if should_suspend_navigation_hotkeys() {
        unregister_navigation_hotkeys_locked();
        return;
    }

    if !NAVIGATION_HOTKEYS_REGISTERED.load(Ordering::SeqCst) {
        reload_navigation_hotkeys_from_settings_locked();
    }
}

pub fn reload_navigation_hotkeys_from_settings() {
    let _lifecycle_guard = NAVIGATION_HOTKEYS_LIFECYCLE_LOCK.lock();
    reload_navigation_hotkeys_from_settings_locked();
}

fn reload_navigation_hotkeys_from_settings_locked() {
    if !NAVIGATION_HOTKEYS_DESIRED.load(Ordering::SeqCst) {
        unregister_navigation_hotkeys_locked();
        return;
    }

    if should_suspend_navigation_hotkeys() {
        unregister_navigation_hotkeys_locked();
        return;
    }

    if let Err(error) = register_navigation_hotkeys_from_settings_locked() {
        eprintln!("同步导航快捷键失败: {}", error);
    }
}

fn register_navigation_hotkeys_from_settings_locked() -> Result<(), String> {
    if !unregister_navigation_hotkeys_locked() {
        return Err("原有导航快捷键尚未完全注销，已取消重新注册".to_string());
    }

    let app = get_app()?;
    let configs = navigation_shortcut_configs();
    let mut registrations = Vec::new();

    for config in configs {
        if config.shortcut.trim().is_empty() {
            continue;
        }

        let shortcut = match parse_shortcut(&config.shortcut) {
            Ok(shortcut) => shortcut,
            Err(error) => {
                eprintln!("解析导航快捷键 [{}] 失败: {}", config.id, error);
                continue;
            }
        };

        let id = config.id.to_string();
        let action = config.action.to_string();
        let shortcut_for_log = config.shortcut.clone();

        match app
            .global_shortcut()
            .on_shortcut(shortcut, move |_app, _shortcut, event| match event.state {
                ShortcutState::Pressed => handle_navigation_pressed(&id, &action),
                ShortcutState::Released => handle_navigation_released(&id),
            }) {
            Ok(_) => {
                println!("已注册导航快捷键 [{}]: {}", config.id, config.shortcut);
                registrations.push(NavigationShortcutRegistration {
                    id: config.id.to_string(),
                    shortcut: shortcut_for_log,
                });
            }
            Err(error) => {
                eprintln!(
                    "注册导航快捷键 [{}] {} 失败: {}",
                    config.id, config.shortcut, error
                );
            }
        }
    }

    let has_registration = !registrations.is_empty();
    *NAVIGATION_SHORTCUTS.lock() = registrations;
    NAVIGATION_HOTKEYS_REGISTERED.store(has_registration, Ordering::SeqCst);
    Ok(())
}

fn unregister_navigation_hotkeys() {
    let _lifecycle_guard = NAVIGATION_HOTKEYS_LIFECYCLE_LOCK.lock();
    unregister_navigation_hotkeys_locked();
}

fn unregister_navigation_hotkeys_locked() -> bool {
    let app = match get_app() {
        Ok(app) => app,
        Err(_) => return false,
    };

    let registrations = std::mem::take(&mut *NAVIGATION_SHORTCUTS.lock());
    let mut remaining_registrations = Vec::new();
    for registration in registrations {
        match parse_shortcut(&registration.shortcut) {
            Ok(shortcut) => match app.global_shortcut().unregister(shortcut) {
                Ok(_) => println!(
                    "已注销导航快捷键 [{}]: {}",
                    registration.id, registration.shortcut
                ),
                Err(error) => {
                    eprintln!(
                        "注销导航快捷键 [{}] {} 失败，将在下次同步时重试: {}",
                        registration.id, registration.shortcut, error
                    );
                    remaining_registrations.push(registration);
                }
            },
            Err(error) => {
                eprintln!(
                    "解析已注册的导航快捷键 [{}] {} 失败，将在下次同步时重试: {}",
                    registration.id, registration.shortcut, error
                );
                remaining_registrations.push(registration);
            }
        }
    }

    let fully_unregistered = remaining_registrations.is_empty();
    *NAVIGATION_SHORTCUTS.lock() = remaining_registrations;
    NAVIGATION_HOTKEYS_REGISTERED.store(!fully_unregistered, Ordering::SeqCst);
    NAVIGATION_REPEAT_SEQUENCE.fetch_add(1, Ordering::SeqCst);
    NAVIGATION_REPEAT_TOKENS.lock().clear();
    NAVIGATION_THROTTLE_STATE.lock().clear();
    fully_unregistered
}

fn navigation_shortcut_configs() -> Vec<NavigationShortcutConfig> {
    let settings = crate::get_settings();
    vec![
        NavigationShortcutConfig {
            id: "navigation_navigate_up",
            action: "navigate-up",
            shortcut: settings.navigate_up_shortcut,
        },
        NavigationShortcutConfig {
            id: "navigation_navigate_down",
            action: "navigate-down",
            shortcut: settings.navigate_down_shortcut,
        },
        NavigationShortcutConfig {
            id: "navigation_execute_item",
            action: "execute-item",
            shortcut: settings.paste_item_shortcut,
        },
        NavigationShortcutConfig {
            id: "navigation_tab_left",
            action: "tab-left",
            shortcut: settings.tab_left_shortcut,
        },
        NavigationShortcutConfig {
            id: "navigation_tab_right",
            action: "tab-right",
            shortcut: settings.tab_right_shortcut,
        },
        NavigationShortcutConfig {
            id: "navigation_previous_group",
            action: "previous-group",
            shortcut: settings.previous_group_shortcut,
        },
        NavigationShortcutConfig {
            id: "navigation_next_group",
            action: "next-group",
            shortcut: settings.next_group_shortcut,
        },
        NavigationShortcutConfig {
            id: "navigation_focus_search",
            action: "focus-search",
            shortcut: settings.focus_search_shortcut,
        },
        NavigationShortcutConfig {
            id: "navigation_hide_window",
            action: "hide-window",
            shortcut: settings.hide_window_shortcut,
        },
        NavigationShortcutConfig {
            id: "navigation_toggle_pin",
            action: "toggle-pin",
            shortcut: settings.toggle_pin_shortcut,
        },
    ]
}

fn handle_navigation_pressed(id: &str, action: &str) {
    if !NAVIGATION_HOTKEYS_DESIRED.load(Ordering::SeqCst)
        || !NAVIGATION_HOTKEYS_REGISTERED.load(Ordering::SeqCst)
        || should_suspend_navigation_hotkeys()
    {
        return;
    }

    if emit_navigation_action_if_ready(action) {
        start_navigation_repeat_if_needed(id, action);
    }
}

fn handle_navigation_released(id: &str) {
    NAVIGATION_REPEAT_TOKENS.lock().remove(id);
    NAVIGATION_REPEAT_SEQUENCE.fetch_add(1, Ordering::SeqCst);
}

fn start_navigation_repeat_if_needed(id: &str, action: &str) {
    let interval = match get_repeat_interval(action) {
        Some(interval) => interval,
        None => return,
    };

    let token = NAVIGATION_REPEAT_SEQUENCE
        .fetch_add(1, Ordering::SeqCst)
        .wrapping_add(1);
    NAVIGATION_REPEAT_TOKENS
        .lock()
        .insert(id.to_string(), token);

    let id = id.to_string();
    let action = action.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(NAVIGATION_REPEAT_INITIAL_DELAY);

        while should_continue_repeat(&id, token) {
            emit_navigation_action(&action);
            std::thread::sleep(interval);
        }
    });
}

fn should_continue_repeat(id: &str, token: u64) -> bool {
    if !NAVIGATION_HOTKEYS_DESIRED.load(Ordering::SeqCst)
        || !NAVIGATION_HOTKEYS_REGISTERED.load(Ordering::SeqCst)
        || should_suspend_navigation_hotkeys()
    {
        return false;
    }

    NAVIGATION_REPEAT_TOKENS.lock().get(id).copied() == Some(token)
}

fn should_suspend_navigation_hotkeys() -> bool {
    !super::global::is_hotkeys_enabled()
        || !crate::get_settings().hotkeys_enabled
        || crate::services::system::is_front_app_globally_disabled_from_settings()
}

fn emit_navigation_action_if_ready(action: &str) -> bool {
    if should_throttle(action) {
        return false;
    }

    emit_navigation_action(action);
    true
}

fn emit_navigation_action(action: &str) {
    let action = action.to_string();
    if let Ok(app) = get_app() {
        let app_for_task = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some(window) = app_for_task.get_webview_window("main") {
                let _ = window.emit(
                    "navigation-action",
                    serde_json::json!({
                        "action": action
                    }),
                );
            }
        });
    }
}

fn should_throttle(action: &str) -> bool {
    let delay = match get_throttle_delay(action) {
        Some(delay) => delay,
        None => return false,
    };

    let mut throttle_state = NAVIGATION_THROTTLE_STATE.lock();
    let now = Instant::now();

    if let Some(last_time) = throttle_state.get(action) {
        if now.duration_since(*last_time) < delay {
            return true;
        }
    }

    throttle_state.insert(action.to_string(), now);
    false
}

fn get_throttle_delay(action: &str) -> Option<Duration> {
    match action {
        "navigate-up" | "navigate-down" => None,
        "tab-left" | "tab-right" => Some(Duration::from_millis(150)),
        "previous-group" | "next-group" => Some(Duration::from_millis(100)),
        "execute-item" | "focus-search" | "hide-window" | "toggle-pin" => {
            Some(Duration::from_millis(200))
        }
        _ => Some(Duration::from_millis(100)),
    }
}

fn get_repeat_interval(action: &str) -> Option<Duration> {
    match action {
        "navigate-up" | "navigate-down" => Some(NAVIGATION_FAST_REPEAT_INTERVAL),
        "tab-left" | "tab-right" => Some(Duration::from_millis(150)),
        "previous-group" | "next-group" => Some(Duration::from_millis(100)),
        _ => None,
    }
}
