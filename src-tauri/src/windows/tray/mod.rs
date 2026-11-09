mod events;
mod menu;
pub mod native_menu;
mod setup;

pub use events::*;
pub use native_menu::handle_native_menu_event;
pub use setup::*;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use std::process::Command;
use tauri::{tray::TrayIconId, AppHandle};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

// 切换到原生系统菜单
pub fn switch_to_native_menu(app: &AppHandle) -> Result<(), String> {
    let tray_id = TrayIconId::new("main-tray");
    if let Some(tray) = app.tray_by_id(&tray_id) {
        let menu = native_menu::create_native_menu(app)?;
        tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
        tray.set_show_menu_on_left_click(true)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

// 切换回 WebView 菜单
pub fn switch_to_webview_menu(app: &AppHandle) -> Result<(), String> {
    let tray_id = TrayIconId::new("main-tray");
    if let Some(tray) = app.tray_by_id(&tray_id) {
        tray.set_menu(None::<tauri::menu::Menu<tauri::Wry>>)
            .map_err(|e| e.to_string())?;
        tray.set_show_menu_on_left_click(false)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(windows)]
fn spawn_delayed_restart_process() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("获取当前程序路径失败: {}", e))?;
    let exe_str = exe.to_string_lossy().replace('\'', "''");

    Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &format!(
                "Start-Sleep -Milliseconds 1200; Start-Process -FilePath '{}'",
                exe_str
            ),
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("启动延迟重启进程失败: {}", e))?;

    Ok(())
}

pub fn restart_app_gracefully(app: &AppHandle) {
    if crate::services::low_memory::is_low_memory_mode() {
        crate::services::low_memory::set_user_requested_exit(true);
    }

    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        #[cfg(windows)]
        {
            match spawn_delayed_restart_process() {
                Ok(()) => app_handle.exit(0),
                Err(err) => {
                    eprintln!("托盘延迟重启失败，回退到直接重启: {}", err);
                    app_handle.restart();
                }
            }
        }

        #[cfg(not(windows))]
        {
            app_handle.restart();
        }
    });
}
