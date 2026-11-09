#[cfg(target_os = "windows")]
mod windows_display_change_monitor {
    use super::super::input_common;
    use once_cell::sync::Lazy;
    use parking_lot::Mutex;
    use std::mem::size_of;
    use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, PeekMessageW,
        RegisterClassExW, TranslateMessage, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, MSG,
        PM_NOREMOVE, WM_DESTROY, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_SETTINGCHANGE, WNDCLASSEXW,
        WS_OVERLAPPEDWINDOW,
    };

    static DISPLAY_MONITOR_ACTIVE: AtomicBool = AtomicBool::new(false);
    static DISPLAY_MONITOR_THREAD_ID: AtomicU32 = AtomicU32::new(0);
    static DISPLAY_REFRESH_VERSION: AtomicU64 = AtomicU64::new(0);
    static DISPLAY_REFRESH_LAST_RUN_MS: Lazy<Mutex<u64>> = Lazy::new(|| Mutex::new(0));

    const DISPLAY_REFRESH_DELAY_MS: u64 = 450;
    const DISPLAY_REFRESH_THROTTLE_MS: u64 = 150;

    pub(crate) fn start_display_change_monitor_if_needed() {
        if DISPLAY_MONITOR_ACTIVE.swap(true, Ordering::SeqCst) {
            return;
        }

        thread::spawn(move || unsafe {
            let tid = GetCurrentThreadId();
            DISPLAY_MONITOR_THREAD_ID.store(tid, Ordering::SeqCst);

            let h_module = match GetModuleHandleW(PCWSTR::null()) {
                Ok(h) => h,
                Err(_) => {
                    eprintln!("[DisplayChange] GetModuleHandleW 失败");
                    DISPLAY_MONITOR_ACTIVE.store(false, Ordering::SeqCst);
                    DISPLAY_MONITOR_THREAD_ID.store(0, Ordering::SeqCst);
                    return;
                }
            };

            let mut init_msg = MSG::default();
            let _ = PeekMessageW(&mut init_msg, None, 0, 0, PM_NOREMOVE);

            let class_name = w!("QuickClipboardDisplayChangeSink");
            let wnd_class = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(display_change_wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: h_module.into(),
                hIcon: Default::default(),
                hCursor: Default::default(),
                hbrBackground: Default::default(),
                lpszMenuName: PCWSTR::null(),
                lpszClassName: class_name,
                hIconSm: Default::default(),
            };

            if RegisterClassExW(&wnd_class) == 0 {
                let err = windows::Win32::Foundation::GetLastError();
                if err != windows::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS {
                    eprintln!("[DisplayChange] RegisterClassExW 失败：{:?}", err);
                    DISPLAY_MONITOR_ACTIVE.store(false, Ordering::SeqCst);
                    DISPLAY_MONITOR_THREAD_ID.store(0, Ordering::SeqCst);
                    return;
                }
            }

            let _hwnd = match CreateWindowExW(
                Default::default(),
                class_name,
                w!(""),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                None,
                None,
                Some(h_module.into()),
                None,
            ) {
                Ok(hwnd) => hwnd,
                Err(_) => {
                    eprintln!("[DisplayChange] CreateWindowExW 失败");
                    DISPLAY_MONITOR_ACTIVE.store(false, Ordering::SeqCst);
                    DISPLAY_MONITOR_THREAD_ID.store(0, Ordering::SeqCst);
                    return;
                }
            };

            println!("[DisplayChange] 显示器变化监听已启动");

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            println!("[DisplayChange] 显示器变化监听线程退出");
            DISPLAY_MONITOR_ACTIVE.store(false, Ordering::SeqCst);
            DISPLAY_MONITOR_THREAD_ID.store(0, Ordering::SeqCst);
        });
    }

    unsafe extern "system" fn display_change_wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_DISPLAYCHANGE | WM_DPICHANGED | WM_SETTINGCHANGE => {
                schedule_hidden_snap_refresh();
                LRESULT(0)
            }
            WM_DESTROY => LRESULT(0),
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    fn schedule_hidden_snap_refresh() {
        let now_ms = current_unix_ms();
        {
            let last_run_ms = DISPLAY_REFRESH_LAST_RUN_MS.lock();
            if now_ms.saturating_sub(*last_run_ms) < DISPLAY_REFRESH_THROTTLE_MS {
                return;
            }
        }

        let version = DISPLAY_REFRESH_VERSION.fetch_add(1, Ordering::SeqCst) + 1;

        thread::spawn(move || {
            thread::sleep(Duration::from_millis(DISPLAY_REFRESH_DELAY_MS));

            if DISPLAY_REFRESH_VERSION.load(Ordering::SeqCst) != version {
                return;
            }

            input_common::run_on_main_thread(|| {
                handle_display_change_impl();
                *DISPLAY_REFRESH_LAST_RUN_MS.lock() = current_unix_ms();
            });
        });
    }

    fn handle_display_change_impl() {
        let Some(window) = input_common::try_get_main_window() else {
            return;
        };

        match crate::windows::main_window::needs_hidden_snap_refresh(&window) {
            Ok(true) => {
                let _ = crate::windows::main_window::refresh_hidden_snapped_window(&window);
            }
            Ok(false) => {}
            Err(_) => {}
        }
    }

    fn current_unix_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

#[cfg(target_os = "windows")]
pub(crate) use windows_display_change_monitor::start_display_change_monitor_if_needed;

#[cfg(not(target_os = "windows"))]
pub(crate) fn start_display_change_monitor_if_needed() {}
