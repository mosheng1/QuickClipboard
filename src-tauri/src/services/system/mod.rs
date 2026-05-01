pub mod hotkey;
pub mod input_common;
pub mod input_monitor;
pub mod display_change_monitor;
pub mod raw_input;
pub mod focus;
pub mod app_filter;
pub mod win_v_hotkey;
pub mod elevate;
#[cfg(target_os = "linux")]
pub mod wayland_shortcuts;
#[cfg(target_os = "linux")]
pub mod ipc_socket;

pub use focus::{focus_clipboard_window, restore_last_focus, save_current_focus};
pub use app_filter::{
    AppInfo,
    get_all_windows_info,
    is_current_app_allowed,
    get_clipboard_source,
    is_front_app_globally_disabled,
    is_front_app_globally_disabled_from_settings,
};
#[cfg(target_os = "windows")]
pub use app_filter::{start_clipboard_source_monitor, stop_clipboard_source_monitor};
pub use elevate::{
    is_running_as_admin, 
    try_elevate_and_restart,
    is_scheduled_task_exists,
    create_scheduled_task,
    delete_scheduled_task,
};
