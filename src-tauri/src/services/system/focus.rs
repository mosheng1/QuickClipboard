use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Manager, WebviewWindow};

static LAST_FOCUS_HWND: Mutex<Option<isize>> = Mutex::new(None);
static LISTENER_RUNNING: AtomicBool = AtomicBool::new(false);

static LAST_FOREGROUND_CACHE: Mutex<Option<(isize, ForegroundAppInfo)>> = Mutex::new(None);

#[derive(Debug, Clone)]
pub struct ForegroundAppInfo {
    pub process_name: String,
    pub process_path: String,
    pub window_title: String,
}

#[cfg(windows)]
static EXCLUDED_HWNDS: Mutex<Vec<isize>> = Mutex::new(Vec::new());

// 启动焦点变化监听器
pub fn start_focus_listener(app_handle: tauri::AppHandle) {
    #[cfg(windows)]
    {
        if LISTENER_RUNNING.swap(true, Ordering::SeqCst) {
            return;
        }

        let mut excluded = Vec::new();
        for label in ["main", "context-menu", "preview"] {
            if let Some(win) = app_handle.get_webview_window(label) {
                if let Ok(hwnd) = win.hwnd() {
                    excluded.push(hwnd.0 as isize);
                }
            }
        }
        *EXCLUDED_HWNDS.lock() = excluded;

        crate::services::system::hotkey::sync_hotkeys_for_foreground();

        std::thread::spawn(|| {
            start_win_event_hook();
        });
    }

    #[cfg(not(windows))]
    {
        let _ = app_handle;
    }
}

// 停止焦点变化监听器
pub fn stop_focus_listener() {
    LISTENER_RUNNING.store(false, Ordering::SeqCst);
}

#[cfg(windows)]
pub fn add_excluded_hwnd(hwnd: isize) {
    let mut excluded = EXCLUDED_HWNDS.lock();
    if !excluded.contains(&hwnd) {
        excluded.push(hwnd);
    }
}

// 聚焦剪贴板窗口
pub fn focus_clipboard_window(window: WebviewWindow) -> Result<(), String> {
    window
        .set_focus()
        .map_err(|e| format!("设置窗口焦点失败: {}", e))?;
    crate::hotkey::suspend_execute_item_hotkey();
    Ok(())
}

// 仅保存当前焦点（手动）
pub fn save_current_focus(_app_handle: tauri::AppHandle) -> Result<(), String> {
    Ok(())
}

// 恢复上次焦点窗口
pub fn restore_last_focus() -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::ffi::c_void;
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;

        if let Some(hwnd_val) = *LAST_FOCUS_HWND.lock() {
            unsafe {
                let _ = SetForegroundWindow(HWND(hwnd_val as *mut c_void));
            }
        }
        crate::hotkey::resume_execute_item_hotkey();
        Ok(())
    }

    #[cfg(not(windows))]
    {
        crate::hotkey::resume_execute_item_hotkey();
        Ok(())
    }
}

// 获取当前记录的焦点窗口句柄
pub fn get_last_focus_hwnd() -> Option<isize> {
    *LAST_FOCUS_HWND.lock()
}

pub fn get_foreground_app_info() -> Option<ForegroundAppInfo> {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
        use windows::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION,
            PROCESS_VM_READ,
        };
        use windows::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
        };

        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0.is_null() {
                return None;
            }

            let hwnd_val = hwnd.0 as isize;
            if let Some((cached_hwnd, cached_info)) = LAST_FOREGROUND_CACHE.lock().clone() {
                if cached_hwnd == hwnd_val {
                    return Some(cached_info);
                }
            }

            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 {
                return None;
            }

            let mut title_buf = [0u16; 512];
            let title_len = GetWindowTextW(hwnd, &mut title_buf);
            let window_title = if title_len > 0 {
                String::from_utf16_lossy(&title_buf[..title_len as usize])
            } else {
                String::new()
            };

            let mut process_path = String::new();
            let mut process_name = String::new();

            if let Ok(handle) = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid)
            {
                let mut buffer = [0u16; 260];
                let len = GetModuleFileNameExW(Some(handle), None, &mut buffer);
                if len > 0 {
                    process_path = String::from_utf16_lossy(&buffer[..len as usize]);
                    process_name = process_path
                        .split('\\')
                        .last()
                        .unwrap_or(&process_path)
                        .to_string();
                }
                let _ = CloseHandle(handle);
            }

            if process_name.is_empty() {
                if let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                    let mut buffer = [0u16; 260];
                    let len = GetModuleFileNameExW(Some(handle), None, &mut buffer);
                    if len > 0 {
                        process_path = String::from_utf16_lossy(&buffer[..len as usize]);
                        process_name = process_path
                            .split('\\')
                            .last()
                            .unwrap_or(&process_path)
                            .to_string();
                    }
                    let _ = CloseHandle(handle);
                }
            }

            if process_name.is_empty() {
                return None;
            }

            let info = ForegroundAppInfo {
                process_name,
                process_path,
                window_title,
            };

            *LAST_FOREGROUND_CACHE.lock() = Some((hwnd_val, info.clone()));
            Some(info)
        }
    }

    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(windows)]
fn start_win_event_hook() {
    use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent};
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, TranslateMessage, EVENT_SYSTEM_FOREGROUND, MSG,
        WINEVENT_OUTOFCONTEXT,
    };

    unsafe {
        let hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(focus_callback),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        );

        if hook.0.is_null() {
            LISTENER_RUNNING.store(false, Ordering::SeqCst);
            return;
        }

        let mut msg = MSG::default();
        while LISTENER_RUNNING.load(Ordering::SeqCst) {
            if GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        let _ = UnhookWinEvent(hook);
    }
}

#[cfg(windows)]
unsafe extern "system" fn focus_callback(
    _hook: windows::Win32::UI::Accessibility::HWINEVENTHOOK,
    _event: u32,
    _hwnd: windows::Win32::Foundation::HWND,
    _id_object: i32,
    _id_child: i32,
    _id_event_thread: u32,
    _dwms_event_time: u32,
) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetWindowTextW,
    };

    if crate::services::low_memory::is_low_memory_mode()
        && crate::services::low_memory::is_panel_visible()
    {
        return;
    }

    let hwnd = GetForegroundWindow();
    if hwnd.0.is_null() {
        return;
    }

    let hwnd_val = hwnd.0 as isize;

    if EXCLUDED_HWNDS.lock().contains(&hwnd_val) {
        return;
    }

    let mut class_buf = [0u16; 256];
    let mut name_buf = [0u16; 256];
    let class_len = GetClassNameW(hwnd, &mut class_buf);
    let name_len = GetWindowTextW(hwnd, &mut name_buf);
    let class_name = String::from_utf16_lossy(&class_buf[..class_len as usize]);
    let name = String::from_utf16_lossy(&name_buf[..name_len as usize]);

    // 过滤窗口
    if class_name == "Shell_TrayWnd"
        || class_name == "Shell_SecondaryTrayWnd"
        || class_name == "NotifyIconOverflowWindow"
        || class_name == "TopLevelWindowForOverflowXamlIsland"
        || class_name == "tray_icon_app"
        || class_name.starts_with("Windows.UI.")
        || class_name == "#32768"
        || class_name == "DropDown"
        || class_name == "Xaml_WindowedPopupClass"
        || name == "快速剪贴板"
        || name == "菜单"
    {
        return;
    }

    *LAST_FOCUS_HWND.lock() = Some(hwnd_val);

    crate::services::system::hotkey::sync_hotkeys_for_foreground();
}
