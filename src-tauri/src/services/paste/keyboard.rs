#[cfg(not(target_os = "windows"))]
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, VK_CONTROL, VK_INSERT, VK_MENU, VK_SHIFT, VK_V,
};

#[cfg(target_os = "windows")]
fn is_key_pressed(vk: u16) -> bool {
    unsafe { GetAsyncKeyState(vk as i32) < 0 }
}

#[cfg(target_os = "windows")]
fn send_key(vk: u16, up: bool) {
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS(0)
                },
                time: 0,
                dwExtraInfo: crate::services::system::raw_input::PASTE_INPUT_MARKER,
            },
        },
    };
    unsafe {
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
}

#[cfg(target_os = "windows")]
fn send_key_ex(vk: u16, up: bool, extended: bool) {
    let mut flags = if up {
        KEYEVENTF_KEYUP
    } else {
        KEYBD_EVENT_FLAGS(0)
    };
    if extended {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: crate::services::system::raw_input::PASTE_INPUT_MARKER,
            },
        },
    };
    unsafe {
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
}

#[cfg(target_os = "windows")]
use std::sync::Mutex;

#[cfg(target_os = "windows")]
static CURRENT_TRIGGER_KEY: Mutex<Option<u16>> = Mutex::new(None);

#[cfg(target_os = "windows")]
static PASTE_SIMULATION_LOCK: Mutex<()> = Mutex::new(());

#[cfg(target_os = "windows")]
pub fn set_trigger_key_from_shortcut(shortcut: &str) {
    if let Some(vk) = parse_shortcut_key_vk(shortcut) {
        *CURRENT_TRIGGER_KEY
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(vk);
    }
}

#[cfg(target_os = "windows")]
pub fn set_trigger_key_raw(vk: u16) {
    *CURRENT_TRIGGER_KEY
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = Some(vk);
}

#[cfg(target_os = "windows")]
fn take_trigger_key() -> Option<u16> {
    CURRENT_TRIGGER_KEY
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .take()
}

// 从快捷键字符串解析非修饰键虚拟键码
#[cfg(target_os = "windows")]
fn parse_shortcut_key_vk(shortcut: &str) -> Option<u16> {
    let key = shortcut.split('+').last()?.trim();
    if key.is_empty() {
        return None;
    }
    if key.len() == 1 {
        let ch = key.chars().next()?;
        if ch.is_ascii_uppercase() {
            return Some(ch as u16);
        }
        if ch.is_ascii_digit() {
            return Some(ch as u16);
        }
        return None;
    }
    match key.to_uppercase().as_str() {
        "INSERT" => Some(0x2D),
        other => {
            if let Some(num) = other.strip_prefix("F").and_then(|n| n.parse::<u16>().ok()) {
                if (1..=24).contains(&num) {
                    return Some(0x6F + num);
                }
            }
            None
        }
    }
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy)]
struct ModifierState {
    ctrl: bool,
    shift: bool,
    alt: bool,
    lwin: bool,
    rwin: bool,
}

#[cfg(target_os = "windows")]
impl ModifierState {
    fn record() -> Self {
        Self {
            ctrl: is_key_pressed(VK_CONTROL.0),
            shift: is_key_pressed(VK_SHIFT.0),
            alt: is_key_pressed(VK_MENU.0),
            lwin: is_key_pressed(0x5B),
            rwin: is_key_pressed(0x5C),
        }
    }

    fn release_conflicting(&self) {
        if self.ctrl {
            send_key(VK_CONTROL.0, true);
        }
        if self.shift {
            send_key(VK_SHIFT.0, true);
        }
        if self.lwin {
            send_key(0x5B, true);
        }
        if self.rwin {
            send_key(0x5C, true);
        }
        if self.ctrl || self.shift || self.lwin || self.rwin {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    fn physical() -> Option<Self> {
        crate::services::system::raw_input::get_physical_modifier_keys_state().map(
            |(ctrl, shift, alt, lwin, rwin)| Self {
                ctrl,
                shift,
                alt,
                lwin,
                rwin,
            },
        )
    }

    fn apply(&self) {
        Self::set_key_state(VK_CONTROL.0, self.ctrl);
        Self::set_key_state(VK_SHIFT.0, self.shift);
        Self::set_key_state(VK_MENU.0, self.alt);
        Self::set_key_state(0x5B, self.lwin);
        Self::set_key_state(0x5C, self.rwin);
    }

    fn set_key_state(vk: u16, pressed: bool) {
        if is_key_pressed(vk) != pressed {
            send_key(vk, !pressed);
        }
    }

    fn restore_current_physical(&self) {
        for pass in 0..2 {
            Self::physical().unwrap_or(*self).apply();
            if pass == 0 {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }
    }
}

// 模拟粘贴
#[cfg(target_os = "windows")]
pub fn simulate_paste() -> Result<(), String> {
    // 完整序列必须串行，避免连续粘贴互相误判对方注入的修饰键。
    let _paste_guard = PASTE_SIMULATION_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let settings = crate::get_settings();

    if settings.paste_shortcut_mode == "ctrl_v" {
        simulate_paste_ctrl_v()
    } else {
        simulate_paste_shift_insert()
    }
}

// Shift+Insert 粘贴：记录修饰键 → 释放冲突键 → 纯净粘贴 → 对齐物理状态
#[cfg(target_os = "windows")]
fn simulate_paste_shift_insert() -> Result<(), String> {
    let mods = ModifierState::record();
    mods.release_conflicting();
    if let Some(vk) = take_trigger_key() {
        send_key(vk, true);
    }
    if mods.alt {
        send_key(VK_MENU.0, true);
    }

    send_key(VK_SHIFT.0, false);
    send_key_ex(VK_INSERT.0, false, true);
    std::thread::sleep(std::time::Duration::from_millis(8));
    send_key_ex(VK_INSERT.0, true, true);
    send_key(VK_SHIFT.0, true);

    mods.restore_current_physical();
    Ok(())
}

// Ctrl+V 粘贴：记录修饰键 → 释放冲突键 → 纯净粘贴 → 对齐物理状态
#[cfg(target_os = "windows")]
fn simulate_paste_ctrl_v() -> Result<(), String> {
    let mods = ModifierState::record();
    mods.release_conflicting();
    if let Some(vk) = take_trigger_key() {
        send_key(vk, true);
    }
    if mods.alt {
        send_key(VK_MENU.0, true);
    }
    send_key(VK_CONTROL.0, false);

    send_key(VK_V.0, false);
    std::thread::sleep(std::time::Duration::from_millis(8));
    send_key(VK_V.0, true);
    send_key(VK_CONTROL.0, true);

    mods.restore_current_physical();
    Ok(())
}

// 模拟粘贴
#[cfg(not(target_os = "windows"))]
pub fn simulate_paste() -> Result<(), String> {
    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|e| format!("创建键盘模拟器失败: {}", e))?;

    enigo
        .key(Key::Control, Direction::Press)
        .map_err(|e| format!("按下Ctrl失败: {}", e))?;

    enigo
        .key(Key::Unicode('v'), Direction::Press)
        .map_err(|e| format!("按下V失败: {}", e))?;

    std::thread::sleep(std::time::Duration::from_millis(8));

    enigo
        .key(Key::Unicode('v'), Direction::Release)
        .map_err(|e| format!("释放V失败: {}", e))?;

    enigo
        .key(Key::Control, Direction::Release)
        .map_err(|e| format!("释放Ctrl失败: {}", e))?;

    Ok(())
}
