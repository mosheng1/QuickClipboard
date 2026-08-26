use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

pub fn handle_tray_click(app: &AppHandle) {
    if crate::windows::updater_window::is_force_update_mode() {
        if let Some(w) = app.get_webview_window("updater") {
            let _ = w.show();
            let _ = w.set_focus();
        }
        return;
    }
    crate::toggle_main_window_visibility(app);
}

pub fn create_click_handler(app_handle: AppHandle) -> impl Fn() + Send + 'static {
    let last_click_time = Arc::new(Mutex::new(Instant::now() - Duration::from_millis(1000)));

    move || {
        let now = Instant::now();
        let mut last_time = last_click_time.lock().unwrap();

        if now.duration_since(*last_time) < Duration::from_millis(50) {
            return;
        }

        *last_time = now;
        drop(last_time);

        handle_tray_click(&app_handle);
    }
}
