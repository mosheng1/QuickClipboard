pub mod app_filter;
pub mod display_change_monitor;
pub mod focus;
pub mod hotkey;
pub mod input_common;
pub mod input_monitor;
pub mod raw_input;
pub mod startup;
pub mod win_v_hotkey;

pub use app_filter::{
    get_all_windows_info, get_clipboard_source, is_current_app_allowed,
    is_front_app_globally_disabled, is_front_app_globally_disabled_from_settings, AppInfo,
};
#[cfg(target_os = "windows")]
pub use app_filter::{start_clipboard_source_monitor, stop_clipboard_source_monitor};
pub use focus::{focus_clipboard_window, restore_last_focus, save_current_focus};
pub use startup::{
    configure_auto_start, get_auto_start_status, is_admin_task_ready, is_running_as_admin,
    switch_to_standard_mode, try_elevate_and_restart,
};
