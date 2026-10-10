mod global;
mod navigation;

pub use global::{
    get_shortcut_status, get_shortcut_statuses, init_hotkey_manager, is_hotkeys_enabled,
    ShortcutStatus,
};

pub fn reload_from_settings() -> Result<(), String> {
    let result = global::reload_from_settings();
    navigation::reload_navigation_hotkeys_from_settings();
    result
}

pub fn enable_hotkeys() -> Result<(), String> {
    let result = global::enable_hotkeys();
    navigation::sync_navigation_hotkeys_for_foreground();
    result
}

pub fn disable_hotkeys() {
    global::disable_hotkeys();
    navigation::sync_navigation_hotkeys_for_foreground();
}

pub fn unregister_all() {
    global::unregister_all();
    navigation::sync_navigation_hotkeys_for_foreground();
}

pub fn sync_hotkeys_for_foreground() {
    global::sync_hotkeys_for_foreground();
    navigation::sync_navigation_hotkeys_for_foreground();
}

pub fn enable_navigation_hotkeys() {
    navigation::enable_navigation_hotkeys();
}

pub fn disable_navigation_hotkeys() {
    navigation::disable_navigation_hotkeys();
}

pub fn suspend_execute_item_hotkey() {
    navigation::suspend_execute_item_hotkey();
}

pub fn resume_execute_item_hotkey() {
    navigation::resume_execute_item_hotkey();
}
