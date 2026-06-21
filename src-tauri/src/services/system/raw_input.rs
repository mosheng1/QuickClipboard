use super::input_common;

#[cfg(target_os = "windows")]
mod windows_raw_input {
    use super::input_common;
    use once_cell::sync::Lazy;
    use parking_lot::Mutex;
    use std::mem::size_of;
    use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use windows::core::w;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::Input::{
        GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER,
        RID_INPUT, RIDEV_INPUTSINK,
    };
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClassNameW, GetMessageW, GetParent,
        PeekMessageW, RegisterClassExW, TranslateMessage, WindowFromPoint, CS_HREDRAW, CS_VREDRAW,
        CW_USEDEFAULT, MSG, PM_NOREMOVE, WM_DESTROY, WM_INPUT, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN,
        WM_SYSKEYUP, WNDCLASSEXW, WS_OVERLAPPEDWINDOW,
    };

    use crate::services::sound::AppSounds;

    static RAW_INPUT_ACTIVE: AtomicBool = AtomicBool::new(false);
    static RAW_INPUT_THREAD_ID: AtomicU32 = AtomicU32::new(0);
    static CTRL_DOWN: AtomicBool = AtomicBool::new(false);
    static SHIFT_DOWN: AtomicBool = AtomicBool::new(false);
    static MIDDLE_BUTTON_DOWN: AtomicBool = AtomicBool::new(false);
    static MIDDLE_BUTTON_PRESS_ID: AtomicU64 = AtomicU64::new(0);
    static PREVIEW_GUARD_PENDING: AtomicBool = AtomicBool::new(false);
    static PREVIEW_GUARD_LAST_RUN_MS: Lazy<Mutex<u64>> = Lazy::new(|| Mutex::new(0));
    static QUICKPASTE_KEYBOARD_MODE_ENABLED: AtomicBool = AtomicBool::new(false);
    static QUICKPASTE_HIDE_TRIGGERED: AtomicBool = AtomicBool::new(false);
    static QUICKPASTE_REQUIRED_MODIFIER_MASK: AtomicU8 = AtomicU8::new(0);
    static QUICKPASTE_SECONDARY_KEY_VK: AtomicU32 = AtomicU32::new(0);
    static QUICKPASTE_SECONDARY_KEY_DOWN: AtomicBool = AtomicBool::new(false);
    static QUICKPASTE_SECONDARY_KEY_PRESS_ID: AtomicU64 = AtomicU64::new(0);
    const VK_CONTROL_CODE: u32 = 0x11;
    const VK_LCONTROL_CODE: u32 = 0xA2;
    const VK_RCONTROL_CODE: u32 = 0xA3;
    const VK_SHIFT_CODE: u32 = 0x10;
    const VK_LSHIFT_CODE: u32 = 0xA0;
    const VK_RSHIFT_CODE: u32 = 0xA1;
    const VK_INSERT_CODE: u32 = 0x2D;
    const PREVIEW_GUARD_THROTTLE_MS: u64 = 50;
    const QUICKPASTE_REPEAT_INITIAL_DELAY_MS: u64 = 300;
    const QUICKPASTE_REPEAT_INTERVAL_MS: u64 = 120;

    pub(crate) fn start_raw_input_if_needed() {
        if RAW_INPUT_ACTIVE.swap(true, Ordering::SeqCst) {
            return;
        }

        thread::spawn(move || unsafe {
            let tid = GetCurrentThreadId();
            RAW_INPUT_THREAD_ID.store(tid, Ordering::SeqCst);

            let h_module = match GetModuleHandleW(PCWSTR::null()) {
                Ok(h) => h,
                Err(_) => {
                    eprintln!("[RawInput] GetModuleHandleW 失败");
                    RAW_INPUT_ACTIVE.store(false, Ordering::SeqCst);
                    RAW_INPUT_THREAD_ID.store(0, Ordering::SeqCst);
                    return;
                }
            };

            // 初始化线程消息队列（Windows 的消息队列是惰性创建的）
            let mut init_msg = MSG::default();
            let _ = PeekMessageW(&mut init_msg, None, 0, 0, PM_NOREMOVE);

            let class_name = w!("QuickClipboardRawInputSink");

            let wnd_class = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(raw_input_wnd_proc),
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
                    eprintln!("[RawInput] RegisterClassExW 失败：{:?}", err);
                    RAW_INPUT_ACTIVE.store(false, Ordering::SeqCst);
                    RAW_INPUT_THREAD_ID.store(0, Ordering::SeqCst);
                    return;
                }
            }

            let hwnd = match CreateWindowExW(
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
                Ok(h) => h,
                Err(_) => {
                    eprintln!("[RawInput] CreateWindowExW 失败");
                    RAW_INPUT_ACTIVE.store(false, Ordering::SeqCst);
                    RAW_INPUT_THREAD_ID.store(0, Ordering::SeqCst);
                    return;
                }
            };

            let rid = [
                RAWINPUTDEVICE {
                    usUsagePage: 0x01,
                    usUsage: 0x06,
                    dwFlags: RIDEV_INPUTSINK,
                    hwndTarget: hwnd,
                },
                RAWINPUTDEVICE {
                    usUsagePage: 0x01,
                    usUsage: 0x02,
                    dwFlags: RIDEV_INPUTSINK,
                    hwndTarget: hwnd,
                },
            ];

            if let Err(_) = RegisterRawInputDevices(&rid, size_of::<RAWINPUTDEVICE>() as u32) {
                let err = windows::Win32::Foundation::GetLastError();
                eprintln!("[RawInput] RegisterRawInputDevices 失败：{:?}", err);
                RAW_INPUT_ACTIVE.store(false, Ordering::SeqCst);
                RAW_INPUT_THREAD_ID.store(0, Ordering::SeqCst);
                return;
            }

            println!("[RawInput] Raw Input 已启动");

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            println!("[RawInput] Raw Input 线程退出");
            RAW_INPUT_ACTIVE.store(false, Ordering::SeqCst);
            RAW_INPUT_THREAD_ID.store(0, Ordering::SeqCst);
        });
    }

    unsafe extern "system" fn raw_input_wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_INPUT => {
                handle_raw_input(lparam);
                LRESULT(0)
            }
            WM_DESTROY => LRESULT(0),
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    unsafe fn handle_raw_input(lparam: LPARAM) {
        let mut size: u32 = 0;

        let res = GetRawInputData(
            HRAWINPUT(lparam.0 as *mut _),
            RID_INPUT,
            None,
            &mut size,
            size_of::<RAWINPUTHEADER>() as u32,
        );

        if res == u32::MAX || size == 0 {
            return;
        }

        let mut buf = vec![0u8; size as usize];

        let res = GetRawInputData(
            HRAWINPUT(lparam.0 as *mut _),
            RID_INPUT,
            Some(buf.as_mut_ptr() as *mut _),
            &mut size,
            size_of::<RAWINPUTHEADER>() as u32,
        );

        if res == u32::MAX {
            return;
        }

        let raw: &RAWINPUT = &*(buf.as_ptr() as *const RAWINPUT);

        match raw.header.dwType {
            // 鼠标
            0 => {
                let mouse = raw.data.mouse;
                let button_flags = mouse.Anonymous.Anonymous.usButtonFlags;

                const RI_MOUSE_LEFT_BUTTON_DOWN: u16 = 0x0001;
                const RI_MOUSE_RIGHT_BUTTON_DOWN: u16 = 0x0004;
                const RI_MOUSE_MIDDLE_BUTTON_DOWN: u16 = 0x0010;
                const RI_MOUSE_MIDDLE_BUTTON_UP: u16 = 0x0020;
                const RI_MOUSE_WHEEL: u16 = 0x0400;

                if (button_flags & (RI_MOUSE_LEFT_BUTTON_DOWN | RI_MOUSE_RIGHT_BUTTON_DOWN)) != 0 {
                    input_common::run_on_main_thread(|| {
                        handle_click_outside_impl();
                    });
                }

                schedule_preview_guard_check();

                if (button_flags & RI_MOUSE_MIDDLE_BUTTON_UP) != 0 {
                    MIDDLE_BUTTON_DOWN.store(false, Ordering::SeqCst);
                    MIDDLE_BUTTON_PRESS_ID.fetch_add(1, Ordering::SeqCst);
                }

                if (button_flags & RI_MOUSE_MIDDLE_BUTTON_DOWN) != 0 {
                    handle_middle_button_down_impl();
                }

                if (button_flags & RI_MOUSE_WHEEL) != 0 {
                    let delta = mouse.Anonymous.Anonymous.usButtonData as i16 as i32;
                    if delta != 0 {
                        let dy = delta as i64;
                        input_common::run_on_main_thread(move || {
                            handle_wheel_event_impl(dy);
                        });
                    }
                }
            }
            // 键盘
            1 => {
                let kb = raw.data.keyboard;
                let vkey = kb.VKey as u32;
                let message = kb.Message;

                let is_keydown = message == WM_KEYDOWN || message == WM_SYSKEYDOWN;
                let is_keyup = message == WM_KEYUP || message == WM_SYSKEYUP;

                handle_quickpaste_keyboard_event(vkey, is_keydown, is_keyup);

                if vkey == VK_CONTROL_CODE || vkey == VK_LCONTROL_CODE || vkey == VK_RCONTROL_CODE {
                    if is_keydown {
                        CTRL_DOWN.store(true, Ordering::Relaxed);
                    } else if is_keyup {
                        CTRL_DOWN.store(false, Ordering::Relaxed);
                    }
                    return;
                }

                if vkey == VK_SHIFT_CODE || vkey == VK_LSHIFT_CODE || vkey == VK_RSHIFT_CODE {
                    if is_keydown {
                        SHIFT_DOWN.store(true, Ordering::Relaxed);
                    } else if is_keyup {
                        SHIFT_DOWN.store(false, Ordering::Relaxed);
                    }
                    return;
                }

                if is_keydown {
                    let is_ctrl_v = CTRL_DOWN.load(Ordering::Relaxed)
                        && (vkey == b'V' as u32 || vkey == b'v' as u32);
                    let is_shift_insert = SHIFT_DOWN.load(Ordering::Relaxed) && vkey == VK_INSERT_CODE;

                    if is_ctrl_v || is_shift_insert {
                        AppSounds::play_paste_immediate();
                    }
                }
            }
            _ => {}
        }
    }

    use tauri::{Emitter, Manager, WebviewWindow};
    use tauri_plugin_global_shortcut::{Code, Shortcut};

    #[cfg(target_os = "windows")]
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_CONTROL, VK_LCONTROL, VK_RCONTROL, VK_MENU, VK_LMENU, VK_RMENU,
        VK_SHIFT, VK_LSHIFT, VK_RSHIFT, VK_LWIN, VK_RWIN,
    };

    pub(crate) fn enable_quickpaste_keyboard_mode() {
        let settings = crate::get_settings();
        if !settings.quickpaste_enabled || !settings.quickpaste_paste_on_modifier_release {
            disable_quickpaste_keyboard_mode();
            return;
        }

        QUICKPASTE_REQUIRED_MODIFIER_MASK.store(
            quickpaste_modifier_mask_from_shortcut(&settings.quickpaste_shortcut),
            Ordering::Relaxed,
        );
        QUICKPASTE_SECONDARY_KEY_VK.store(
            quickpaste_secondary_key_vk_from_shortcut(&settings.quickpaste_shortcut).unwrap_or(0),
            Ordering::Relaxed,
        );
        QUICKPASTE_SECONDARY_KEY_DOWN.store(false, Ordering::SeqCst);
        QUICKPASTE_SECONDARY_KEY_PRESS_ID.fetch_add(1, Ordering::SeqCst);
        QUICKPASTE_HIDE_TRIGGERED.store(false, Ordering::SeqCst);
        QUICKPASTE_KEYBOARD_MODE_ENABLED.store(true, Ordering::SeqCst);
    }

    pub(crate) fn disable_quickpaste_keyboard_mode() {
        QUICKPASTE_KEYBOARD_MODE_ENABLED.store(false, Ordering::SeqCst);
        QUICKPASTE_REQUIRED_MODIFIER_MASK.store(0, Ordering::Relaxed);
        QUICKPASTE_SECONDARY_KEY_VK.store(0, Ordering::Relaxed);
        QUICKPASTE_SECONDARY_KEY_DOWN.store(false, Ordering::SeqCst);
        QUICKPASTE_SECONDARY_KEY_PRESS_ID.fetch_add(1, Ordering::SeqCst);
        QUICKPASTE_HIDE_TRIGGERED.store(false, Ordering::SeqCst);
    }

    pub(crate) fn start_quickpaste_secondary_key_hold() {
        if !QUICKPASTE_KEYBOARD_MODE_ENABLED.load(Ordering::SeqCst) {
            return;
        }

        let secondary_vk = QUICKPASTE_SECONDARY_KEY_VK.load(Ordering::Relaxed);
        if secondary_vk != 0 && is_vk_pressed(secondary_vk) {
            start_quickpaste_secondary_repeat(secondary_vk);
        }
    }

    fn quickpaste_modifier_mask_from_shortcut(shortcut: &str) -> u8 {
        let mut mask = 0;
        for part in shortcut.split('+') {
            match part.trim() {
                "Ctrl" | "Control" => mask |= 0x01,
                "Alt" => mask |= 0x02,
                "Shift" => mask |= 0x04,
                "Win" | "Super" | "Meta" | "Cmd" | "Command" => mask |= 0x08,
                _ => {}
            }
        }
        mask
    }

    fn quickpaste_modifier_mask_from_vk(vk: u32) -> u8 {
        if vk == VK_CONTROL.0 as u32 || vk == VK_LCONTROL.0 as u32 || vk == VK_RCONTROL.0 as u32 {
            return 0x01;
        }
        if vk == VK_MENU.0 as u32 || vk == VK_LMENU.0 as u32 || vk == VK_RMENU.0 as u32 {
            return 0x02;
        }
        if vk == VK_SHIFT.0 as u32 || vk == VK_LSHIFT.0 as u32 || vk == VK_RSHIFT.0 as u32 {
            return 0x04;
        }
        if vk == VK_LWIN.0 as u32 || vk == VK_RWIN.0 as u32 {
            return 0x08;
        }
        0
    }

    fn is_quickpaste_secondary_key_candidate(vk: u32) -> bool {
        vk > 0
            && quickpaste_modifier_mask_from_vk(vk) == 0
            && !matches!(vk, 0x01..=0x06)
    }

    fn quickpaste_secondary_key_vk_from_shortcut(shortcut: &str) -> Option<u32> {
        let shortcut = shortcut
            .replace("Win+", "Super+")
            .parse::<Shortcut>()
            .ok()?;
        quickpaste_code_to_vk(shortcut.key)
    }

    fn quickpaste_code_to_vk(code: Code) -> Option<u32> {
        let key = code.to_string();

        if let Some(value) = key.strip_prefix("Key") {
            if value.len() == 1 && value.as_bytes()[0].is_ascii_alphabetic() {
                return Some(value.as_bytes()[0].to_ascii_uppercase() as u32);
            }
        }

        if let Some(value) = key.strip_prefix("Digit") {
            if value.len() == 1 && value.as_bytes()[0].is_ascii_digit() {
                return Some(value.as_bytes()[0] as u32);
            }
        }

        if let Some(num) = key.strip_prefix('F').and_then(|n| n.parse::<u32>().ok()) {
            if (1..=24).contains(&num) {
                return Some(0x6F + num);
            }
        }

        if let Some(num) = key.strip_prefix("Numpad").and_then(|n| n.parse::<u32>().ok()) {
            if num <= 9 {
                return Some(0x60 + num);
            }
        }

        const NAMED_KEY_VKS: &[(&str, u32)] = &[
            ("Backquote", 0xC0), ("Minus", 0xBD), ("Equal", 0xBB), ("BracketLeft", 0xDB),
            ("BracketRight", 0xDD), ("Backslash", 0xDC), ("Semicolon", 0xBA), ("Quote", 0xDE),
            ("Comma", 0xBC), ("Period", 0xBE), ("Slash", 0xBF), ("Backspace", 0x08),
            ("Insert", 0x2D), ("Delete", 0x2E), ("Home", 0x24), ("End", 0x23),
            ("PageUp", 0x21), ("PageDown", 0x22), ("Space", 0x20), ("Tab", 0x09),
            ("Enter", 0x0D), ("Escape", 0x1B), ("ArrowUp", 0x26), ("ArrowDown", 0x28),
            ("ArrowLeft", 0x25), ("ArrowRight", 0x27),
        ];

        NAMED_KEY_VKS
            .iter()
            .find_map(|(name, vk)| (*name == key).then_some(*vk))
    }

    fn is_vk_pressed(vk: u32) -> bool {
        unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 }
    }

    fn handle_quickpaste_keyboard_event(vk: u32, is_keydown: bool, is_keyup: bool) {
        if !QUICKPASTE_KEYBOARD_MODE_ENABLED.load(Ordering::SeqCst) || (!is_keydown && !is_keyup) {
            return;
        }

        if is_keydown {
            handle_quickpaste_secondary_key_down(vk);
            return;
        }

        handle_quickpaste_secondary_key_up(vk);
        handle_quickpaste_modifier_release(vk);
    }

    fn handle_quickpaste_secondary_key_down(vk: u32) {
        if !is_quickpaste_secondary_key_candidate(vk) {
            return;
        }

        let secondary_vk = QUICKPASTE_SECONDARY_KEY_VK.load(Ordering::Relaxed);
        if secondary_vk == 0 {
            QUICKPASTE_SECONDARY_KEY_VK.store(vk, Ordering::Relaxed);
        } else if vk != secondary_vk {
            return;
        }

        start_quickpaste_secondary_repeat(vk);
    }

    fn start_quickpaste_secondary_repeat(vk: u32) {
        if !crate::windows::quickpaste::is_visible() {
            return;
        }

        if QUICKPASTE_SECONDARY_KEY_DOWN.swap(true, Ordering::SeqCst) {
            return;
        }

        let press_id = QUICKPASTE_SECONDARY_KEY_PRESS_ID
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1);

        thread::spawn(move || {
            thread::sleep(Duration::from_millis(QUICKPASTE_REPEAT_INITIAL_DELAY_MS));

            while QUICKPASTE_KEYBOARD_MODE_ENABLED.load(Ordering::SeqCst)
                && QUICKPASTE_SECONDARY_KEY_DOWN.load(Ordering::SeqCst)
                && QUICKPASTE_SECONDARY_KEY_PRESS_ID.load(Ordering::SeqCst) == press_id
                && crate::windows::quickpaste::is_visible()
                && is_vk_pressed(vk)
            {
                handle_quickpaste_next_request_impl();
                thread::sleep(Duration::from_millis(QUICKPASTE_REPEAT_INTERVAL_MS));
            }

            if QUICKPASTE_SECONDARY_KEY_PRESS_ID.load(Ordering::SeqCst) == press_id {
                QUICKPASTE_SECONDARY_KEY_DOWN.store(false, Ordering::SeqCst);
            }
        });
    }

    fn handle_quickpaste_secondary_key_up(vk: u32) {
        let secondary_vk = QUICKPASTE_SECONDARY_KEY_VK.load(Ordering::Relaxed);
        if secondary_vk != 0 && vk == secondary_vk {
            QUICKPASTE_SECONDARY_KEY_DOWN.store(false, Ordering::SeqCst);
            QUICKPASTE_SECONDARY_KEY_PRESS_ID.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn handle_quickpaste_next_request_impl() {
        input_common::run_on_main_thread(|| {
            let settings = crate::get_settings();
            if !settings.quickpaste_enabled || !settings.quickpaste_paste_on_modifier_release {
                return;
            }

            if !crate::windows::quickpaste::is_visible() {
                return;
            }

            if let Some(app) = input_common::try_get_app_handle() {
                if let Some(window) = app.get_webview_window("quickpaste") {
                    let _ = window.emit("quickpaste-next", ());
                }
            }
        });
    }

    fn handle_quickpaste_modifier_release(vk: u32) {
        let required = QUICKPASTE_REQUIRED_MODIFIER_MASK.load(Ordering::Relaxed);
        let released_mask = quickpaste_modifier_mask_from_vk(vk);

        if required == 0 || (released_mask & required) == 0 {
            return;
        }

        let (ctrl, alt, shift, meta) = get_modifier_keys_state_impl();
        let all_released = ((required & 0x01) == 0 || !ctrl)
            && ((required & 0x02) == 0 || !alt)
            && ((required & 0x04) == 0 || !shift)
            && ((required & 0x08) == 0 || !meta);

        if all_released && !QUICKPASTE_HIDE_TRIGGERED.swap(true, Ordering::SeqCst) {
            handle_quickpaste_hide_request_impl();
        }
    }

    fn handle_quickpaste_hide_request_impl() {
        input_common::run_on_main_thread(|| {
            let settings = crate::get_settings();
            if !settings.quickpaste_enabled || !settings.quickpaste_paste_on_modifier_release {
                return;
            }

            if !crate::windows::quickpaste::is_visible() {
                return;
            }

            if let Some(app) = input_common::try_get_app_handle() {
                if let Some(window) = app.get_webview_window("quickpaste") {
                    let _ = window.emit("quickpaste-hide", ());
                }
            }

            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_millis(50));
                input_common::run_on_main_thread(|| {
                    if let Some(app) = input_common::try_get_app_handle() {
                        let _ = crate::windows::quickpaste::hide_quickpaste_window(&app);
                    }
                });
            });
        });
    }

    fn should_handle_click_outside_impl() -> bool {
        if input_common::is_mouse_monitoring_enabled() {
            return true;
        }

        if crate::services::low_memory::is_low_memory_mode()
            && crate::services::low_memory::is_panel_visible()
        {
            return true;
        }

        #[cfg(feature = "screenshot-suite")]
        {
            if let Some(app) = input_common::try_get_app_handle() {
                if let Some(win) = app.get_webview_window("screenshot") {
                    return win.is_visible().unwrap_or(false);
                }
            }
        }

        false
    }

    fn handle_wheel_event_impl(_delta_y: i64) {
    }

    fn current_unix_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    fn schedule_preview_guard_check() {
        if !input_common::is_mouse_monitoring_enabled() {
            return;
        }

        let now_ms = current_unix_ms();
        {
            let last_run_ms = PREVIEW_GUARD_LAST_RUN_MS.lock();
            if now_ms.saturating_sub(*last_run_ms) < PREVIEW_GUARD_THROTTLE_MS {
                return;
            }
        }

        if PREVIEW_GUARD_PENDING.swap(true, Ordering::SeqCst) {
            return;
        }

        input_common::run_on_main_thread(|| {
            handle_preview_guard_check_impl();
            *PREVIEW_GUARD_LAST_RUN_MS.lock() = current_unix_ms();
            PREVIEW_GUARD_PENDING.store(false, Ordering::SeqCst);
        });
    }

    fn check_modifier_requirement_impl(required: &str) -> bool {
        let (ctrl, alt, shift, _meta) = get_modifier_keys_state_impl();

        if required == "None" || required.is_empty() {
            return true;
        }

        let parts: Vec<&str> = required.split('+').collect();
        let need_ctrl = parts.contains(&"Ctrl");
        let need_alt = parts.contains(&"Alt");
        let need_shift = parts.contains(&"Shift");

        (!need_ctrl || ctrl)
            && (!need_alt || alt)
            && (!need_shift || shift)
            && (need_ctrl || !ctrl)
            && (need_alt || !alt)
            && (need_shift || !shift)
    }

    fn get_modifier_keys_state_impl() -> (bool, bool, bool, bool) {
        unsafe {
            let ctrl = (GetAsyncKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0;
            let alt = (GetAsyncKeyState(VK_MENU.0 as i32) as u16 & 0x8000) != 0;
            let shift = (GetAsyncKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0;
            let meta = (GetAsyncKeyState(VK_LWIN.0 as i32) as u16 & 0x8000) != 0
                || (GetAsyncKeyState(VK_RWIN.0 as i32) as u16 & 0x8000) != 0;
            (ctrl, alt, shift, meta)
        }
    }

    fn handle_middle_button_action_impl() {
        let settings = crate::get_settings();
        if !settings.mouse_middle_button_enabled {
            return;
        }

        if crate::services::system::is_front_app_globally_disabled_from_settings() {
            return;
        }

        if !check_modifier_requirement_impl(&settings.mouse_middle_button_modifier) {
            return;
        }

        if let Some(app) = input_common::try_get_app_handle() {
            crate::toggle_main_window_visibility(&app);
        }
    }

    fn handle_middle_button_down_impl() {
        let settings = crate::get_settings();
        if !settings.mouse_middle_button_enabled {
            return;
        }

        if crate::services::system::is_front_app_globally_disabled_from_settings() {
            return;
        }

        if !check_modifier_requirement_impl(&settings.mouse_middle_button_modifier) {
            return;
        }

        if settings.mouse_middle_button_modifier != "None" {
            input_common::run_on_main_thread(|| {
                handle_middle_button_action_impl();
            });
            return;
        }

        if settings.mouse_middle_button_trigger != "long_press" {
            input_common::run_on_main_thread(|| {
                handle_middle_button_action_impl();
            });
            return;
        }

        let threshold_ms = settings.mouse_middle_button_long_press_ms.max(1) as u64;
        let press_id = MIDDLE_BUTTON_PRESS_ID.fetch_add(1, Ordering::SeqCst).wrapping_add(1);
        MIDDLE_BUTTON_DOWN.store(true, Ordering::SeqCst);

        thread::spawn(move || {
            thread::sleep(Duration::from_millis(threshold_ms));

            if !MIDDLE_BUTTON_DOWN.load(Ordering::SeqCst) {
                return;
            }

            if MIDDLE_BUTTON_PRESS_ID.load(Ordering::SeqCst) != press_id {
                return;
            }

            input_common::run_on_main_thread(move || {
                if !MIDDLE_BUTTON_DOWN.load(Ordering::SeqCst) {
                    return;
                }

                if MIDDLE_BUTTON_PRESS_ID.load(Ordering::SeqCst) != press_id {
                    return;
                }

                handle_middle_button_action_impl();
            });
        });
    }

    // 检测鼠标是否在系统托盘区域（含任务栏通知区域、溢出区域、本程序托盘图标），
    // 用于避免把托盘点击误判为"窗口外点击"而错误隐藏主窗口。
    //
    // 从 WindowFromPoint 返回的窗口开始，向上遍历最多 10 层父窗口链，
    // 若任一级别窗口类名命中预定义列表，则视为托盘区域点击。
    //
    // 需要遍历父窗口链的原因：通知区域图标嵌入在 ToolbarWindow32 → SysPager
    // → TrayNotifyWnd → Shell_TrayWnd 层级中，WindowFromPoint 返回的是深层
    // 子窗口（如 ToolbarWindow32），不直接返回 Shell_TrayWnd。
    //
    // 注意：tray_icon_app 依赖 Tauri tray-icon 组件内部窗口类名，若未来变更需同步更新。
    fn is_cursor_on_system_tray_impl() -> bool {
        const TRAY_CLASSES: &[&str] = &[
            "Shell_TrayWnd",
            "Shell_SecondaryTrayWnd",
            "NotifyIconOverflowWindow",
            "TopLevelWindowForOverflowXamlIsland",
            "tray_icon_app",
        ];

        let (x, y) = crate::mouse::get_cursor_position();
        let mut hwnd = unsafe { WindowFromPoint(POINT { x, y }) };
        if hwnd.0.is_null() {
            return false;
        }

        fn get_class(hwnd: HWND) -> Option<String> {
            let mut buf = [0u16; 64];
            let len = unsafe { GetClassNameW(hwnd, &mut buf) };
            (len > 0).then(|| String::from_utf16_lossy(&buf[..len as usize]))
        }

        for _ in 0..10 {
            if let Some(class) = get_class(hwnd) {
                if TRAY_CLASSES.iter().any(|&c| c == class.as_str()) {
                    return true;
                }
            }
            hwnd = match unsafe { GetParent(hwnd) } {
                Ok(h) if !h.0.is_null() => h,
                _ => break,
            };
        }

        false
    }

    fn is_mouse_outside_window_impl(window: &WebviewWindow) -> bool {
        let (cursor_x, cursor_y) = crate::mouse::get_cursor_position();

        let (win_x, win_y, win_width, win_height) = match crate::get_window_bounds(window) {
            Ok(bounds) => bounds,
            Err(_) => return false,
        };

        cursor_x < win_x || cursor_x > win_x + win_width as i32
            || cursor_y < win_y || cursor_y > win_y + win_height as i32
    }

    fn handle_preview_guard_check_impl() {
        let Some(app) = input_common::try_get_app_handle() else {
            return;
        };

        if app.get_webview_window("preview-window").is_none() {
            return;
        }

        let Some(main_window) = input_common::try_get_main_window() else {
            crate::windows::preview_window::force_close_preview_window(&app);
            return;
        };

        let state = crate::get_window_state();
        if state.state != crate::WindowState::Visible || state.is_hidden {
            crate::windows::preview_window::force_close_preview_window(&app);
            return;
        }

        let (cursor_x, cursor_y) = crate::mouse::get_cursor_position();
        let in_menu_region = crate::is_context_menu_visible()
            && crate::windows::plugins::context_menu::is_point_in_menu_region(cursor_x, cursor_y);

        if in_menu_region {
            crate::windows::preview_window::force_close_preview_window(&app);
            return;
        }

        if is_mouse_outside_window_impl(&main_window) {
            crate::windows::preview_window::force_close_preview_window(&app);
        }
    }

    fn handle_click_outside_impl() {
        if crate::services::low_memory::is_low_memory_mode()
            && crate::services::low_memory::is_panel_visible()
        {
            let (cursor_x, cursor_y) = crate::mouse::get_cursor_position();
            if !crate::services::low_memory::is_point_in_panel(cursor_x, cursor_y) {
                let _ = crate::services::low_memory::hide_panel();
            }
            return;
        }

        if crate::is_context_menu_visible() {
            if let Some(main_window) = input_common::try_get_main_window() {
                if let Some(menu_window) = main_window.app_handle().get_webview_window("context-menu") {
                    let (cursor_x, cursor_y) = crate::mouse::get_cursor_position();
                    if menu_window.is_visible().unwrap_or(false)
                        && !crate::windows::plugins::context_menu::is_point_in_menu_region(cursor_x, cursor_y)
                    {
                        let _ = menu_window.emit("close-context-menu", ());
                    }
                }
            }
            return;
        }

        if !should_handle_click_outside_impl() {
            return;
        }

        // 托盘区域点击（任务栏通知区域 / 溢出区域 / 托盘图标）不触发主窗口隐藏，
        // 避免与托盘图标的 click 事件产生 show/hide 竞争导致动画闪跳。
        if is_cursor_on_system_tray_impl() {
            return;
        }

        if let Some(window) = input_common::try_get_main_window() {
            let state = crate::get_window_state();

            if state.is_hidden {
                return;
            }

            if state.is_pinned {
                return;
            }

            if window.is_visible().unwrap_or(false) && is_mouse_outside_window_impl(&window) {
                let _ = crate::check_snap(&window);
                crate::hide_main_window(&window);
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub(crate) use windows_raw_input::{
    disable_quickpaste_keyboard_mode,
    enable_quickpaste_keyboard_mode,
    start_quickpaste_secondary_key_hold,
    start_raw_input_if_needed,
};

#[cfg(not(target_os = "windows"))]
pub(crate) fn start_raw_input_if_needed() {}

#[cfg(not(target_os = "windows"))]
pub(crate) fn enable_quickpaste_keyboard_mode() {}

#[cfg(not(target_os = "windows"))]
pub(crate) fn disable_quickpaste_keyboard_mode() {}

#[cfg(not(target_os = "windows"))]
pub(crate) fn start_quickpaste_secondary_key_hold() {}
