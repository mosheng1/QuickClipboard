mod manager;
mod panel;
mod state;

pub use manager::{
    ensure_main_window, enter_low_memory_mode, exit_low_memory_mode, init_auto_low_memory_manager,
};
pub use panel::{
    hide_panel, init_panel, is_panel_visible, is_point_in_panel, show_panel, toggle_panel,
};
pub use state::{
    finish_exit_low_memory, init_window_activity_timestamp, is_low_memory_mode,
    is_user_requested_exit, set_user_requested_exit, try_start_exit_low_memory,
};
