use serde::{Deserialize, Serialize};

#[cfg(not(target_os = "windows"))]
use active_win_pos_rs::get_active_window;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInfo {
    pub name: String,
    pub process: String,
    pub path: String,
    pub icon: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ClipboardSourceInfo {
    pub process_name: String,
    pub process_path: String,
    pub window_title: String,
    pub source_type: ClipboardSourceType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClipboardSourceType {
    ClipboardOwner,
    ForegroundWindow,
    Unknown,
}

fn matches_filter_rule_text(
    process_name: &str,
    window_title: &str,
    process_path: &str,
    filter: &str,
) -> bool {
    let filter = filter.trim();
    if filter.is_empty() {
        return false;
    }

    if filter.contains('*') || filter.contains('?') {
        wildcard_match(filter, process_name)
            || wildcard_match(filter, window_title)
            || wildcard_match(filter, process_path)
    } else {
        let f = filter.to_lowercase();
        process_name.to_lowercase().contains(&f)
            || window_title.to_lowercase().contains(&f)
            || process_path.to_lowercase().contains(&f)
    }
}

// Windows 实现
#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use once_cell::sync::Lazy;
    use parking_lot::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    static CLIPBOARD_SOURCE_CACHE: Lazy<Mutex<Option<ClipboardSourceInfo>>> =
        Lazy::new(|| Mutex::new(None));

    static SOURCE_MONITOR_RUNNING: AtomicBool = AtomicBool::new(false);
    static SOURCE_MONITOR_THREAD_ID: AtomicU32 = AtomicU32::new(0);

    // 启动剪贴板来源监控
    pub fn start_clipboard_source_monitor() {
        use std::thread;
        use windows::core::w;
        use windows::Win32::Foundation::{
            GetLastError, ERROR_CLASS_ALREADY_EXISTS, HWND, LPARAM, LRESULT, WPARAM,
        };
        use windows::Win32::System::DataExchange::{
            AddClipboardFormatListener, RemoveClipboardFormatListener,
        };
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
            RegisterClassW, TranslateMessage, MSG, WINDOW_EX_STYLE, WM_CLIPBOARDUPDATE, WNDCLASSW,
            WS_OVERLAPPED,
        };

        if SOURCE_MONITOR_RUNNING
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        thread::spawn(move || unsafe {
            SOURCE_MONITOR_THREAD_ID.store(
                windows::Win32::System::Threading::GetCurrentThreadId(),
                Ordering::SeqCst,
            );
            unsafe extern "system" fn wnd_proc(
                hwnd: HWND,
                msg: u32,
                wparam: WPARAM,
                lparam: LPARAM,
            ) -> LRESULT {
                if msg == WM_CLIPBOARDUPDATE {
                    let source = get_clipboard_source_internal();
                    *CLIPBOARD_SOURCE_CACHE.lock() = Some(source);
                    return LRESULT(0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }

            let class_name = w!("KiroClipboardMonitor");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wnd_proc),
                lpszClassName: class_name,
                ..Default::default()
            };

            if RegisterClassW(&wc) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
                SOURCE_MONITOR_RUNNING.store(false, Ordering::SeqCst);
                SOURCE_MONITOR_THREAD_ID.store(0, Ordering::SeqCst);
                return;
            }

            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class_name,
                w!("Clipboard Monitor"),
                WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                None,
                None,
                None,
                None,
            );

            let Ok(hwnd) = hwnd else {
                SOURCE_MONITOR_RUNNING.store(false, Ordering::SeqCst);
                SOURCE_MONITOR_THREAD_ID.store(0, Ordering::SeqCst);
                return;
            };

            if AddClipboardFormatListener(hwnd).is_err() {
                SOURCE_MONITOR_RUNNING.store(false, Ordering::SeqCst);
                SOURCE_MONITOR_THREAD_ID.store(0, Ordering::SeqCst);
                let _ = DestroyWindow(hwnd);
                return;
            }

            let mut msg = MSG::default();
            while SOURCE_MONITOR_RUNNING.load(Ordering::Relaxed) {
                if GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                } else {
                    break;
                }
            }

            let _ = RemoveClipboardFormatListener(hwnd);
            let _ = DestroyWindow(hwnd);
            SOURCE_MONITOR_THREAD_ID.store(0, Ordering::SeqCst);
        });
    }

    // 停止剪贴板来源监控
    pub fn stop_clipboard_source_monitor() {
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};

        SOURCE_MONITOR_RUNNING.store(false, Ordering::SeqCst);
        let thread_id = SOURCE_MONITOR_THREAD_ID.load(Ordering::SeqCst);
        if thread_id != 0 {
            unsafe {
                let _ = PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
    }

    // 获取剪贴板来源
    pub fn get_clipboard_source() -> ClipboardSourceInfo {
        if let Some(cached) = CLIPBOARD_SOURCE_CACHE.lock().clone() {
            if !cached.process_name.is_empty() {
                return cached;
            }
        }
        get_clipboard_source_internal()
    }

    // 获取剪贴板来源
    fn get_clipboard_source_internal() -> ClipboardSourceInfo {
        use windows::Win32::System::DataExchange::GetClipboardOwner;
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

        unsafe {
            // 首选：剪贴板所有者
            if let Ok(owner) = GetClipboardOwner() {
                if !owner.is_invalid() && owner.0 as usize != 0 {
                    if let Some(info) =
                        get_process_info_from_hwnd(owner, ClipboardSourceType::ClipboardOwner)
                    {
                        if !info.process_name.is_empty() {
                            return info;
                        }
                    }
                }
            }

            // 备用：前台窗口
            let foreground = GetForegroundWindow();
            if !foreground.is_invalid() && foreground.0 as usize != 0 {
                if let Some(info) =
                    get_process_info_from_hwnd(foreground, ClipboardSourceType::ForegroundWindow)
                {
                    return info;
                }
            }

            ClipboardSourceInfo {
                process_name: String::new(),
                process_path: String::new(),
                window_title: String::new(),
                source_type: ClipboardSourceType::Unknown,
            }
        }
    }

    // 从窗口句柄获取进程信息
    unsafe fn get_process_info_from_hwnd(
        hwnd: windows::Win32::Foundation::HWND,
        source_type: ClipboardSourceType,
    ) -> Option<ClipboardSourceInfo> {
        use windows::Win32::UI::WindowsAndMessaging::{GetWindowTextW, GetWindowThreadProcessId};

        let mut process_id: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));

        if process_id == 0 {
            return None;
        }

        let mut title_buffer = [0u16; 512];
        let title_len = GetWindowTextW(hwnd, &mut title_buffer);
        let window_title = if title_len > 0 {
            String::from_utf16_lossy(&title_buffer[..title_len as usize])
        } else {
            String::new()
        };

        let (process_path, process_name) = get_process_name_by_id(process_id);

        // UWP 应用特殊处理
        let final_name = if process_name.to_lowercase() == "applicationframehost.exe" {
            get_uwp_app_name(hwnd).unwrap_or(process_name)
        } else {
            process_name
        };

        Some(ClipboardSourceInfo {
            process_name: final_name,
            process_path,
            window_title,
            source_type,
        })
    }

    // 通过进程ID获取进程名称
    fn get_process_name_by_id(process_id: u32) -> (String, String) {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
        use windows::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION,
            PROCESS_VM_READ,
        };

        unsafe {
            if let Ok(handle) = OpenProcess(
                PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
                false,
                process_id,
            ) {
                let mut buffer = [0u16; 260];
                let len = GetModuleFileNameExW(Some(handle), None, &mut buffer);
                let result = if len > 0 {
                    let path = String::from_utf16_lossy(&buffer[..len as usize]);
                    let name = path.split('\\').last().unwrap_or(&path).to_string();
                    Some((path, name))
                } else {
                    None
                };
                let _ = CloseHandle(handle);
                if let Some(result) = result {
                    return result;
                }
            }

            if let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) {
                let mut buffer = [0u16; 260];
                let len = GetModuleFileNameExW(Some(handle), None, &mut buffer);
                let result = if len > 0 {
                    let path = String::from_utf16_lossy(&buffer[..len as usize]);
                    let name = path.split('\\').last().unwrap_or(&path).to_string();
                    Some((path, name))
                } else {
                    None
                };
                let _ = CloseHandle(handle);
                if let Some(result) = result {
                    return result;
                }
            }

            (String::new(), String::new())
        }
    }

    // 获取 UWP 应用真实名称
    fn get_uwp_app_name(hwnd: windows::Win32::Foundation::HWND) -> Option<String> {
        use windows::core::BOOL;
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::Foundation::LPARAM;
        use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
        use windows::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
        };
        use windows::Win32::UI::WindowsAndMessaging::{EnumChildWindows, GetWindowThreadProcessId};

        struct Context {
            result: Option<String>,
            parent_pid: u32,
        }

        unsafe {
            let mut parent_pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut parent_pid));

            let mut ctx = Context {
                result: None,
                parent_pid,
            };

            unsafe extern "system" fn callback(
                child: windows::Win32::Foundation::HWND,
                lparam: LPARAM,
            ) -> BOOL {
                let ctx = &mut *(lparam.0 as *mut Context);
                let mut child_pid: u32 = 0;
                GetWindowThreadProcessId(child, Some(&mut child_pid));

                if child_pid > 0 && child_pid != ctx.parent_pid {
                    if let Ok(handle) = OpenProcess(
                        PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
                        false,
                        child_pid,
                    ) {
                        let mut buffer = [0u16; 260];
                        let len = GetModuleFileNameExW(Some(handle), None, &mut buffer);
                        let result = if len > 0 {
                            let path = String::from_utf16_lossy(&buffer[..len as usize]);
                            let name = path.split('\\').last().unwrap_or(&path).to_string();
                            if !name.is_empty() && name.to_lowercase() != "applicationframehost.exe"
                            {
                                Some(name)
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        let _ = CloseHandle(handle);
                        if let Some(name) = result {
                            ctx.result = Some(name);
                            return BOOL(0);
                        }
                    }
                }
                BOOL(1)
            }

            let _ = EnumChildWindows(
                Some(hwnd),
                Some(callback),
                LPARAM(&mut ctx as *mut _ as isize),
            );
            ctx.result
        }
    }

    // 获取所有可见窗口信息
    pub fn get_all_windows_info() -> Vec<AppInfo> {
        use windows::core::BOOL;
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::Foundation::{HWND, LPARAM};
        use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
        use windows::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
        };
        use windows::Win32::UI::WindowsAndMessaging::{
            EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
        };

        let mut windows: Vec<AppInfo> = Vec::new();

        unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let windows = &mut *(lparam.0 as *mut Vec<AppInfo>);

            if IsWindowVisible(hwnd).as_bool() {
                let mut pid: u32 = 0;
                GetWindowThreadProcessId(hwnd, Some(&mut pid));

                if pid > 0 {
                    let mut title_buf = [0u16; 512];
                    let title_len = GetWindowTextW(hwnd, &mut title_buf);
                    if title_len > 0 {
                        let title = String::from_utf16_lossy(&title_buf[..title_len as usize]);
                        if !title.trim().is_empty() && title != "Program Manager" {
                            if let Ok(handle) =
                                OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid)
                            {
                                let mut buf = [0u16; 260];
                                let len = GetModuleFileNameExW(Some(handle), None, &mut buf);
                                let result = if len > 0 {
                                    let path = String::from_utf16_lossy(&buf[..len as usize]);
                                    let name = path.split('\\').last().unwrap_or(&path).to_string();
                                    Some((name, path))
                                } else {
                                    None
                                };
                                let _ = CloseHandle(handle);
                                if let Some((name, path)) = result {
                                    windows.push(AppInfo {
                                        name: title,
                                        process: name,
                                        path: path.clone(),
                                        icon: crate::utils::icon::get_file_icon_base64(&path),
                                    });
                                }
                            }
                        }
                    }
                }
            }
            BOOL(1)
        }

        unsafe {
            let _ = EnumWindows(Some(enum_proc), LPARAM(&mut windows as *mut _ as isize));
        }

        windows.sort_by(|a, b| a.process.cmp(&b.process));
        windows.dedup_by(|a, b| a.process == b.process && a.name == b.name);
        windows
    }
}

// 非 Windows 实现
#[cfg(not(target_os = "windows"))]
pub fn get_clipboard_source() -> ClipboardSourceInfo {
    match get_active_window() {
        Ok(win) => {
            let path = win.process_path.to_string_lossy().to_string();
            let name = win
                .process_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            ClipboardSourceInfo {
                process_name: name,
                process_path: path,
                window_title: win.title,
                source_type: ClipboardSourceType::ForegroundWindow,
            }
        }
        Err(_) => ClipboardSourceInfo {
            process_name: String::new(),
            process_path: String::new(),
            window_title: String::new(),
            source_type: ClipboardSourceType::Unknown,
        },
    }
}

#[cfg(not(target_os = "windows"))]
pub fn get_all_windows_info() -> Vec<AppInfo> {
    Vec::new()
}

#[cfg(target_os = "windows")]
pub use windows_impl::{
    get_all_windows_info, get_clipboard_source, start_clipboard_source_monitor,
    stop_clipboard_source_monitor,
};

// 通配符匹配（支持 * 和 ?）
fn wildcard_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.to_lowercase().chars().collect();
    let text: Vec<char> = text.to_lowercase().chars().collect();

    let mut dp = vec![vec![false; text.len() + 1]; pattern.len() + 1];
    dp[0][0] = true;

    for (i, &pc) in pattern.iter().enumerate() {
        if pc == '*' {
            dp[i + 1][0] = dp[i][0];
        } else {
            break;
        }
    }

    for (i, &pc) in pattern.iter().enumerate() {
        for (j, &tc) in text.iter().enumerate() {
            if pc == '*' {
                dp[i + 1][j + 1] = dp[i][j + 1] || dp[i + 1][j];
            } else if pc == '?' || pc == tc {
                dp[i + 1][j + 1] = dp[i][j];
            }
        }
    }

    dp[pattern.len()][text.len()]
}

// 检查来源是否匹配过滤规则
fn matches_filter_rule(source: &ClipboardSourceInfo, filter: &str) -> bool {
    let filter = filter.trim();
    if filter.is_empty() {
        return false;
    }

    if filter.contains('*') || filter.contains('?') {
        wildcard_match(filter, &source.process_name)
            || wildcard_match(filter, &source.window_title)
            || wildcard_match(filter, &source.process_path)
    } else {
        let f = filter.to_lowercase();
        source.process_name.to_lowercase().contains(&f)
            || source.window_title.to_lowercase().contains(&f)
            || source.process_path.to_lowercase().contains(&f)
    }
}

pub fn is_front_app_globally_disabled(
    app_filter_enabled: bool,
    app_filter_blocklist: &[String],
    app_filter_effect: &str,
) -> bool {
    if !app_filter_enabled {
        return false;
    }

    if app_filter_effect != "global_disable" {
        return false;
    }

    let Some(info) = crate::services::system::focus::get_foreground_app_info() else {
        return false;
    };

    app_filter_blocklist.iter().any(|f| {
        matches_filter_rule_text(
            &info.process_name,
            &info.window_title,
            &info.process_path,
            f,
        )
    })
}

pub fn is_front_app_globally_disabled_from_settings() -> bool {
    let settings = crate::services::get_settings();
    is_front_app_globally_disabled(
        settings.app_filter_enabled,
        &settings.app_filter_blocklist,
        &settings.app_filter_effect,
    )
}

// 检查当前应用是否允许记录剪贴板
pub fn is_current_app_allowed(app_filter_enabled: bool, app_filter_blocklist: &[String]) -> bool {
    if !app_filter_enabled {
        return true;
    }

    let source = get_clipboard_source();

    if source.source_type == ClipboardSourceType::Unknown {
        return true;
    }

    !app_filter_blocklist
        .iter()
        .any(|f| matches_filter_rule(&source, f))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_rule_matches_process_name_case_insensitively() {
        assert!(matches_filter_rule_text(
            "Chrome.exe",
            "",
            "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
            "chrome.exe",
        ));
    }

    #[test]
    fn filter_rule_supports_wildcards() {
        assert!(matches_filter_rule_text(
            "Code.exe",
            "Visual Studio Code",
            "C:\\Users\\test\\AppData\\Local\\Programs\\Microsoft VS Code\\Code.exe",
            "*code.exe",
        ));
    }
}
