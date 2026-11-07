#[cfg(not(target_os = "windows"))]
use enigo::{Enigo, Direction, Key, Keyboard, Settings};

#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, MapVirtualKeyW, SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT,
    KEYBD_EVENT_FLAGS, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, MAPVK_VK_TO_VSC, VK_INSERT,
    VK_CONTROL, VK_SHIFT, VK_V,
};

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const MODIFIER_SIDE_VKS: [u16; 6] = [0xA2, 0xA3, 0xA0, 0xA1, 0xA4, 0xA5];

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn modifier_side_states(sides: [bool; 6]) -> [(u16, bool); 6] {
    std::array::from_fn(|i| (MODIFIER_SIDE_VKS[i], sides[i]))
}

// Windows 只有在带 KEYEVENTF_EXTENDEDKEY 时才会正确处理这些扩展键的注入
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn vk_needs_extended_key(vk: u16) -> bool {
    matches!(vk, 0x5B | 0x5C | 0xA3 | 0xA5 | 0x2D)
}

// 需要释放的修饰键：通用 VK 以及左右两侧 VK
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn modifier_release_vks(ctrl: bool, shift: bool, alt: bool) -> Vec<u16> {
    let mut vks = Vec::new();
    if ctrl {
        vks.extend_from_slice(&[0x11, 0xA2, 0xA3]);
    }
    if shift {
        vks.extend_from_slice(&[0x10, 0xA0, 0xA1]);
    }
    if alt {
        vks.extend_from_slice(&[0x12, 0xA4, 0xA5]);
    }
    vks
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WinRestore {
    Hold,
    Down,
    Up,
}

// 仅当 Raw Input 确认物理 Win 仍按下时才恢复按下，否则一律抬起，避免把粘贴前的过期状态写回
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn win_restore(physical_down: Option<bool>, async_down: bool) -> WinRestore {
    match physical_down {
        Some(true) if async_down => WinRestore::Hold,
        Some(true) => WinRestore::Down,
        _ => WinRestore::Up,
    }
}

#[cfg(target_os = "windows")]
fn is_key_pressed(vk: u16) -> bool {
    unsafe { GetAsyncKeyState(vk as i32) < 0 }
}

#[cfg(target_os = "windows")]
fn send_key(vk: u16, up: bool) -> Result<(), String> {
    send_key_ex(vk, up, vk_needs_extended_key(vk))
}

#[cfg(target_os = "windows")]
fn send_key_ex(vk: u16, up: bool, extended: bool) -> Result<(), String> {
    let mut flags = if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) };
    let mut scan = 0u16;
    if extended {
        flags |= KEYEVENTF_EXTENDEDKEY;
        scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) } as u16;
    }
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(vk),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: crate::services::system::raw_input::PASTE_INPUT_MARKER,
            },
        },
    };
    if unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) } == 1 {
        Ok(())
    } else {
        Err(format!("SendInput failed: VK {vk:#04X} {}, error {:?}",
            if up { "up" } else { "down" },
            unsafe { windows::Win32::Foundation::GetLastError() }))
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
        *CURRENT_TRIGGER_KEY.lock().unwrap_or_else(|error| error.into_inner()) = Some(vk);
    }
}

#[cfg(target_os = "windows")]
pub fn set_trigger_key_raw(vk: u16) {
    *CURRENT_TRIGGER_KEY.lock().unwrap_or_else(|error| error.into_inner()) = Some(vk);
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
    let key = shortcut
        .split('+')
        .last()?
        .trim();
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
    sides: [bool; 6],
    lwin: bool,
    rwin: bool,
}

#[cfg(target_os = "windows")]
impl ModifierState {
    fn record() -> Self {
        Self {
            sides: MODIFIER_SIDE_VKS.map(is_key_pressed),
            lwin: is_key_pressed(0x5B),
            rwin: is_key_pressed(0x5C),
        }
    }

    fn release_conflicting(&self, errors: &mut Vec<String>) {
        for vk in modifier_release_vks(self.sides[0] || self.sides[1], self.sides[2] || self.sides[3], false) {
            errors.extend(send_key(vk, true).err());
        }
        if self.lwin {
            errors.extend(send_key(0x5B, true).err());
        }
        if self.rwin {
            errors.extend(send_key(0x5C, true).err());
        }
        if self.sides[..4].iter().any(|down| *down) || self.lwin || self.rwin {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    fn physical() -> Option<Self> {
        crate::services::system::raw_input::get_physical_modifier_keys_state().map(
            |(sides, lwin, rwin)| Self {
                sides,
                lwin,
                rwin,
            },
        )
    }

    fn apply(&self, physical: Option<Self>, errors: &mut Vec<String>) {
        for (vk, down) in modifier_side_states(self.sides) {
            errors.extend(Self::set_key_state(vk, down).err());
        }
        errors.extend(Self::restore_win(0x5B, physical.map(|p| p.lwin)).err());
        errors.extend(Self::restore_win(0x5C, physical.map(|p| p.rwin)).err());
    }

    fn set_key_state(vk: u16, pressed: bool) -> Result<(), String> {
        if is_key_pressed(vk) != pressed {
            send_key(vk, !pressed)?;
        }
        Ok(())
    }

    fn restore_win(vk: u16, physical_down: Option<bool>) -> Result<(), String> {
        match win_restore(physical_down, is_key_pressed(vk)) {
            WinRestore::Hold => Ok(()),
            WinRestore::Down => send_key(vk, false),
            WinRestore::Up => send_key(vk, true),
        }
    }

    fn restore_current_physical(&self, errors: &mut Vec<String>) {
        for pass in 0..2 {
            let physical = Self::physical();
            physical.unwrap_or(*self).apply(physical, errors);
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
    let mut errors = Vec::new();
    mods.release_conflicting(&mut errors);
    if let Some(vk) = take_trigger_key() {
        if let Err(error) = send_key(vk, true) {
            // Keep ownership of the trigger whose release failed for the next attempt.
            CURRENT_TRIGGER_KEY.lock().unwrap_or_else(|error| error.into_inner()).get_or_insert(vk);
            errors.push(error);
        }
    }
    if mods.sides[4] || mods.sides[5] {
        for vk in modifier_release_vks(false, false, true) {
            errors.extend(send_key(vk, true).err());
        }
    }

    if errors.is_empty() {
        errors.extend(send_key(VK_SHIFT.0, false).err());
        if errors.is_empty() {
            errors.extend(send_key(VK_INSERT.0, false).err());
            std::thread::sleep(std::time::Duration::from_millis(8));
            errors.extend(send_key(VK_INSERT.0, true).err());
        }
        errors.extend(send_key(VK_SHIFT.0, true).err());
    }
    // Always attempt every restoration, including after an injection or release failure.
    mods.restore_current_physical(&mut errors);
    if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) }
}

// Ctrl+V 粘贴
#[cfg(target_os = "windows")]
fn simulate_paste_ctrl_v() -> Result<(), String> {
    let mods = ModifierState::record();
    let mut errors = Vec::new();
    mods.release_conflicting(&mut errors);
    if let Some(vk) = take_trigger_key() {
        if let Err(error) = send_key(vk, true) {
            // Keep ownership of the trigger whose release failed for the next attempt.
            CURRENT_TRIGGER_KEY.lock().unwrap_or_else(|error| error.into_inner()).get_or_insert(vk);
            errors.push(error);
        }
    }
    if mods.sides[4] || mods.sides[5] {
        for vk in modifier_release_vks(false, false, true) {
            errors.extend(send_key(vk, true).err());
        }
    }

    if errors.is_empty() {
        errors.extend(send_key(VK_CONTROL.0, false).err());
        if errors.is_empty() {
            errors.extend(send_key(VK_V.0, false).err());
            std::thread::sleep(std::time::Duration::from_millis(8));
            errors.extend(send_key(VK_V.0, true).err());
        }
        errors.extend(send_key(VK_CONTROL.0, true).err());
    }
    // Always attempt every restoration, including after an injection or release failure.
    mods.restore_current_physical(&mut errors);
    if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) }
}

// 模拟粘贴
#[cfg(not(target_os = "windows"))]
pub fn simulate_paste() -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| format!("创建键盘模拟器失败: {}", e))?;

    enigo.key(Key::Control, Direction::Press)
        .map_err(|e| format!("按下Ctrl失败: {}", e))?;
    
    enigo.key(Key::Unicode('v'), Direction::Press)
        .map_err(|e| format!("按下V失败: {}", e))?;
    
    std::thread::sleep(std::time::Duration::from_millis(8));
    
    enigo.key(Key::Unicode('v'), Direction::Release)
        .map_err(|e| format!("释放V失败: {}", e))?;
    
    enigo.key(Key::Control, Direction::Release)
        .map_err(|e| format!("释放Ctrl失败: {}", e))?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stuck_modifier_win_restore_ignores_snapshot_when_physical_is_up() {
        assert_eq!(win_restore(None, true), WinRestore::Up);
        assert_eq!(win_restore(Some(false), true), WinRestore::Up);
        assert_eq!(win_restore(Some(true), true), WinRestore::Hold);
        assert_eq!(win_restore(Some(true), false), WinRestore::Down);
        assert!(vk_needs_extended_key(0x5B) && vk_needs_extended_key(0x5C));
    }

    #[test]
    fn stuck_modifier_restores_each_held_side_without_pressing_the_other() {
        for held in 0..6 {
            let mut sides = [false; 6];
            sides[held] = true;
            let states = modifier_side_states(sides);
            assert_eq!(states.iter().filter(|(_, down)| *down).count(), 1);
            assert_eq!(states[held], (MODIFIER_SIDE_VKS[held], true));
            assert!(!states[held ^ 1].1);
        }
        assert!(modifier_side_states([true; 6]).iter().all(|(_, down)| *down));
        assert!(modifier_side_states([false; 6]).iter().all(|(_, down)| !*down));
    }

    #[test]
    fn stuck_modifier_releases_both_sides() {
        assert_eq!(
            modifier_release_vks(true, true, false),
            vec![0x11, 0xA2, 0xA3, 0x10, 0xA0, 0xA1]
        );
        for vk in [0x5B, 0x5C, 0xA3, 0xA5, 0x2D] {
            assert!(vk_needs_extended_key(vk));
        }
        for vk in [0x11, 0x10, 0x12, 0xA2, 0xA0, 0xA4, 0x56] {
            assert!(!vk_needs_extended_key(vk));
        }
    }
}

