use crate::services::system::input_monitor::get_modifier_keys_state;

#[cfg(target_os = "windows")]
use enigo::{Enigo, Direction, Key, Keyboard, Settings};

#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, 
    KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VK_MENU, VK_CONTROL, VK_V,
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
                dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32); }
}

// 释放所有修饰键（Alt、Ctrl、Shift、Win）
#[cfg(target_os = "windows")]
pub fn release_modifier_keys() -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| format!("创建键盘模拟器失败: {}", e))?;

    let (ctrl, shift, alt, win) = get_modifier_keys_state();

    if alt {
        enigo.key(Key::Alt, Direction::Release)
            .map_err(|e| format!("释放Alt失败: {}", e))?;
    }
    if ctrl {
        enigo.key(Key::Control, Direction::Release)
            .map_err(|e| format!("释放Ctrl失败: {}", e))?;
    }
    if shift {
        enigo.key(Key::Shift, Direction::Release)
            .map_err(|e| format!("释放Shift失败: {}", e))?;
    }
    if win {
        enigo.key(Key::Meta, Direction::Release)
            .map_err(|e| format!("释放Win失败: {}", e))?;
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn release_modifier_keys() -> Result<(), String> {
    let mut keys = Vec::new();
    let (ctrl, shift, alt, _win) = get_modifier_keys_state();
    if ctrl { keys.push("ctrl"); }
    if shift { keys.push("shift"); }
    if alt { keys.push("alt"); }
    if !keys.is_empty() {
        let args = ["keyup", &keys.join("+")];
        std::process::Command::new("xdotool")
            .args(&args)
            .output()
            .map_err(|e| format!("释放修饰键失败: {}", e))?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
struct KeyGuard {
    vk: u16,
    should_release: bool,
}

#[cfg(target_os = "windows")]
impl KeyGuard {
    fn new(vk: u16, press: bool) -> Self {
        if press {
            send_key(vk, false);
        }
        Self { vk, should_release: press }
    }
}

#[cfg(target_os = "windows")]
impl Drop for KeyGuard {
    fn drop(&mut self) {
        if self.should_release {
            send_key(self.vk, true);
        }
    }
}

// 模拟粘贴
#[cfg(target_os = "windows")]
pub fn simulate_paste() -> Result<(), String> {
    let settings = crate::get_settings();
    
    if settings.paste_shortcut_mode == "ctrl_v" {
        simulate_paste_ctrl_v()
    } else {
        simulate_paste_shift_insert()
    }
}

// Shift+Insert 粘贴
#[cfg(target_os = "windows")]
fn simulate_paste_shift_insert() -> Result<(), String> {
    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| format!("创建键盘模拟器失败: {}", e))?;

    enigo.key(Key::Shift, Direction::Press)
        .map_err(|e| format!("按下Shift失败: {}", e))?;
    enigo.key(Key::Other(0x2D), Direction::Click)
        .map_err(|e| format!("按下Insert失败: {}", e))?;
    enigo.key(Key::Shift, Direction::Release)
        .map_err(|e| format!("释放Shift失败: {}", e))?;
    
    Ok(())
}

// Ctrl+V 粘贴
#[cfg(target_os = "windows")]
fn simulate_paste_ctrl_v() -> Result<(), String> {
    let user_alt = is_key_pressed(VK_MENU.0);
    
    if user_alt {
        // 持续释放 Alt
        for _ in 0..20 {
            if !is_key_pressed(VK_MENU.0) {
                break;
            }
            send_key(VK_MENU.0, true);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    
    // 发送 Ctrl+V
    let user_ctrl = is_key_pressed(VK_CONTROL.0);
    let _ctrl_guard = KeyGuard::new(VK_CONTROL.0, !user_ctrl);
    
    send_key(VK_V.0, false);
    std::thread::sleep(std::time::Duration::from_millis(8));
    send_key(VK_V.0, true);
    
    drop(_ctrl_guard);

    if user_alt {
        send_key(VK_MENU.0, false);
        send_key(VK_CONTROL.0, false);
        send_key(VK_CONTROL.0, true);
    }
    
    Ok(())
}

// 模拟粘贴 (Linux: xdotool 通过 XWayland 工作，不触发 GNOME 远程交互权限)
#[cfg(not(target_os = "windows"))]
pub fn simulate_paste() -> Result<(), String> {
    let output = std::process::Command::new("xdotool")
        .args(["key", "--clearmodifiers", "ctrl+v"])
        .output()
        .map_err(|e| format!("执行 xdotool 粘贴失败: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "xdotool 粘贴失败: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(())
}

