use std::process::Command;

const GSCHEMA_BASE: &str = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings";

fn gsettings_get(args: &[&str]) -> Result<String, String> {
    let output = Command::new("gsettings")
        .args(args)
        .output()
        .map_err(|e| format!("gsettings 执行失败: {}", e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

fn gsettings_set(schema_path: &str, key: &str, value: &str) -> Result<(), String> {
    let schema = format!(
        "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:{}",
        schema_path
    );
    let output = Command::new("gsettings")
        .args(["set", &schema, key, value])
        .output()
        .map_err(|e| format!("gsettings set 失败: {}", e))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    Ok(())
}

fn shortcut_binding(shortcut_str: &str) -> String {
    shortcut_str
        .replace("Shift+", "<Shift>")
        .replace("Ctrl+", "<Control>")
        .replace("Alt+", "<Alt>")
        .replace("Super+", "<Super>")
        .replace("Win+", "<Super>")
        .replace("Space", "space")
        .replace("Backquote", "grave")
}

pub fn register_wayland_shortcut(
    id: &str,
    shortcut_str: &str,
    action_arg: &str,
) -> Result<(), String> {
    let idx = match id {
        "toggle" => 0,
        "quickpaste" => 1,
        "settings" => 2,
        _ => return Ok(()),
    };
    let path = format!("{}/custom{}/", GSCHEMA_BASE, idx);

    gsettings_set(&path, "name", &format!("QuickClipboard {}", id))?;
    gsettings_set(
        &path,
        "command",
        &format!("/usr/bin/QuickClipboard {}", action_arg),
    )?;
    gsettings_set(&path, "binding", &shortcut_binding(shortcut_str))?;

    let current = gsettings_get(&[
        "get",
        "org.gnome.settings-daemon.plugins.media-keys",
        "custom-keybindings",
    ])?;

    let entry = format!("{}/custom{}/", GSCHEMA_BASE, idx);
    if !current.contains(&entry) {
        let new_list = if current == "@as []" || current.is_empty() {
            format!("[ '{}']", entry)
        } else {
            let trimmed = current.trim_start_matches('[').trim_end_matches(']');
            format!("[ {}, '{}' ]", trimmed, entry)
        };
        let output = Command::new("gsettings")
            .args([
                "set",
                "org.gnome.settings-daemon.plugins.media-keys",
                "custom-keybindings",
                &new_list,
            ])
            .output()
            .map_err(|e| format!("gsettings set custom-keybindings 失败: {}", e))?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).to_string());
        }
    }

    println!("已注册 Wayland 快捷键 [{}]: {} → {}", id, shortcut_str, action_arg);
    Ok(())
}

pub fn unregister_all_wayland_shortcuts() -> Result<(), String> {
    let output = Command::new("gsettings")
        .args([
            "set",
            "org.gnome.settings-daemon.plugins.media-keys",
            "custom-keybindings",
            "[]",
        ])
        .output()
        .map_err(|e| format!("清除快捷键失败: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    println!("已清除所有 Wayland 快捷键");
    Ok(())
}

pub fn is_wayland_session() -> bool {
    std::env::var("XDG_SESSION_TYPE").map(|v| v == "wayland").unwrap_or(false)
}
