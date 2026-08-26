// Windows 注册表管理，用于禁用/启用系统 Win+V 快捷键

#[cfg(windows)]
use winreg::enums::*;
#[cfg(windows)]
use winreg::RegKey;

#[cfg(windows)]
const EXPLORER_ADVANCED_PATH: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
#[cfg(windows)]
const DISABLED_HOTKEYS_VALUE: &str = "DisabledHotkeys";

#[cfg(windows)]
pub fn disable_win_v_hotkey() -> Result<(), String> {
    add_disabled_hotkey('V', true)
}

#[cfg(windows)]
pub fn disable_win_v_hotkey_silent() -> Result<(), String> {
    add_disabled_hotkey('V', false)
}

#[cfg(windows)]
pub fn enable_win_v_hotkey() -> Result<(), String> {
    remove_disabled_hotkey('V', true)
}

#[cfg(windows)]
pub fn enable_win_v_hotkey_silent() -> Result<(), String> {
    remove_disabled_hotkey('V', false)
}

#[cfg(windows)]
fn add_disabled_hotkey(key: char, restart_explorer: bool) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (reg_key, _) = hkcu
        .create_subkey(EXPLORER_ADVANCED_PATH)
        .map_err(|e| format!("无法打开注册表项: {}", e))?;

    let current_value: String = reg_key
        .get_value(DISABLED_HOTKEYS_VALUE)
        .unwrap_or_default();

    let key_upper = key.to_uppercase().to_string();
    if !current_value.contains(&key_upper) {
        let new_value = format!("{}{}", current_value, key_upper);
        reg_key
            .set_value(DISABLED_HOTKEYS_VALUE, &new_value)
            .map_err(|e| format!("无法设置注册表值: {}", e))?;
    }

    if restart_explorer {
        restart_explorer_process()?;
    }

    Ok(())
}

#[cfg(windows)]
fn remove_disabled_hotkey(key: char, restart_explorer: bool) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let reg_key = match hkcu.open_subkey_with_flags(EXPLORER_ADVANCED_PATH, KEY_READ | KEY_WRITE) {
        Ok(k) => k,
        Err(_) => return Ok(()),
    };

    let current_value: String = reg_key
        .get_value(DISABLED_HOTKEYS_VALUE)
        .unwrap_or_default();

    let key_upper = key.to_uppercase().to_string();
    let new_value = current_value.replace(&key_upper, "");

    if new_value.is_empty() {
        let _ = reg_key.delete_value(DISABLED_HOTKEYS_VALUE);
    } else if new_value != current_value {
        reg_key
            .set_value(DISABLED_HOTKEYS_VALUE, &new_value)
            .map_err(|e| format!("无法更新注册表值: {}", e))?;
    }

    if restart_explorer {
        restart_explorer_process()?;
    }

    Ok(())
}

#[cfg(windows)]
fn restart_explorer_process() -> Result<(), String> {
    use std::process::Command;

    let _ = Command::new("taskkill")
        .args(["/F", "/IM", "explorer.exe"])
        .output();

    std::thread::sleep(std::time::Duration::from_millis(1000));

    if Command::new("cmd")
        .args(["/C", "start", "explorer.exe"])
        .spawn()
        .is_err()
    {
        Command::new("explorer.exe")
            .spawn()
            .map_err(|e| format!("无法启动Explorer进程: {}", e))?;
    }

    std::thread::sleep(std::time::Duration::from_millis(1000));

    Ok(())
}

#[cfg(windows)]
pub fn is_win_v_hotkey_disabled() -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let reg_key = match hkcu.open_subkey(EXPLORER_ADVANCED_PATH) {
        Ok(k) => k,
        Err(_) => return false,
    };

    let current_value: String = reg_key
        .get_value(DISABLED_HOTKEYS_VALUE)
        .unwrap_or_default();

    current_value.contains('V')
}

#[cfg(not(windows))]
pub fn disable_win_v_hotkey() -> Result<(), String> {
    Ok(())
}
#[cfg(not(windows))]
pub fn disable_win_v_hotkey_silent() -> Result<(), String> {
    Ok(())
}
#[cfg(not(windows))]
pub fn enable_win_v_hotkey() -> Result<(), String> {
    Ok(())
}
#[cfg(not(windows))]
pub fn enable_win_v_hotkey_silent() -> Result<(), String> {
    Ok(())
}
#[cfg(not(windows))]
pub fn is_win_v_hotkey_disabled() -> bool {
    false
}
