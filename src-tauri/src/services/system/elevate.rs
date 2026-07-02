use crate::services::settings::AppSettings;

const TASK_NAME: &str = "QuickClipboardAdmin";

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

// 判断当前设置是否需要维护计划任务
// 统一启动流程和设置变更流程的门控条件，避免分散硬编码
pub fn should_maintain_scheduled_task(settings: &AppSettings) -> bool {
    settings.run_as_admin && settings.auto_start
}

// 同步计划任务：根据设置创建任务（失败仅日志，非致命）
// 供设置变更时调用，封装门控判断 + 创建 + 非致命错误处理
pub fn sync_scheduled_task(settings: &AppSettings) {
    if should_maintain_scheduled_task(settings) {
        if let Err(e) = create_scheduled_task() {
            eprintln!("同步计划任务失败: {}", e);
        }
    }
}

// 检查当前是否以管理员权限运行
#[cfg(windows)]
pub fn is_running_as_admin() -> bool {
    use ::windows::Win32::Foundation::HANDLE;
    use ::windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use ::windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token_handle: HANDLE = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token_handle).is_err() {
            return false;
        }

        let mut elevation = TOKEN_ELEVATION::default();
        let mut return_length: u32 = 0;

        let result = GetTokenInformation(
            token_handle,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut return_length,
        );

        let _ = ::windows::Win32::Foundation::CloseHandle(token_handle);

        if result.is_ok() {
            return elevation.TokenIsElevated != 0;
        }
    }
    false
}

#[cfg(not(windows))]
pub fn is_running_as_admin() -> bool {
    false
}


// 检查计划任务是否存在
#[cfg(windows)]
pub fn is_scheduled_task_exists() -> bool {
    use std::process::Command;

    crate::startup_diagnostics::set_startup_stage_if_starting("检查管理员启动：查询计划任务是否存在");

    let output = Command::new("schtasks")
        .args(["/Query", "/TN", TASK_NAME])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    matches!(output, Ok(o) if o.status.success())
}

#[cfg(not(windows))]
pub fn is_scheduled_task_exists() -> bool {
    false
}

// 检查计划任务的路径是否与当前程序路径匹配
#[cfg(windows)]
pub fn is_scheduled_task_path_valid() -> bool {
    use std::process::Command;

    crate::startup_diagnostics::set_startup_stage_if_starting("检查管理员启动：校验计划任务目标路径");

    let current_exe = match std::env::current_exe() {
        Ok(p) => p.to_string_lossy().to_lowercase(),
        Err(_) => return false,
    };

    let output = Command::new("schtasks")
        .args(["/Query", "/TN", TASK_NAME, "/FO", "LIST", "/V"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    if let Ok(o) = output {
        if o.status.success() {
            let stdout = decode_ansi(&o.stdout).to_lowercase();
            return stdout.contains(&current_exe);
        }
    }
    false
}

#[cfg(not(windows))]
pub fn is_scheduled_task_path_valid() -> bool {
    false
}

// 构建计划任务的 XML（纯函数，可单元测试）
// xmlns 属性不可省略（Task Scheduler Schema v1.2 要求）
fn build_task_xml(exe_path: &str, username: &str, allow_on_battery: bool) -> String {
    let disallow = if allow_on_battery { "false" } else { "true" };
    let exe_path = xml_escape(exe_path);
    let username = xml_escape(username);
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <Principals>
    <Principal>
      <UserId>{username}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <DisallowStartIfOnBatteries>{disallow}</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
  </Settings>
  <Actions>
    <Exec>
      <Command>{exe_path}</Command>
    </Exec>
  </Actions>
</Task>"#
    )
}

// 转义 XML 特殊字符，防止用户名或路径含 & < > " ' 导致 XML 损坏
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

// 将字符串编码为 UTF-16 LE with BOM
// schtasks.exe 在中文 Windows(CP936) 上会将 UTF-8 文件按 GBK 解码导致 XML 损坏
// UTF-16 LE with BOM 是 Windows Task Scheduler 原生编码，schtasks 通过 BOM 正确识别
fn encode_utf16_le_with_bom(s: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(2 + s.len() * 2);
    bytes.extend_from_slice(&[0xFF, 0xFE]); // UTF-16 LE BOM
    for unit in s.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes
}

// 将 schtasks.exe 输出的 ANSI（中文 Windows 为 GBK）字节解码为 UTF-8
#[cfg(windows)]
fn decode_ansi(bytes: &[u8]) -> String {
    use std::os::windows::ffi::OsStringExt;
    extern "system" {
        fn MultiByteToWideChar(c: u32, f: u32, m: *const u8, b: i32, w: *mut u16, n: i32) -> i32;
    }
    // unsafe 仅包裹 FFI 调用，vec! 与 OsString 转换在块外
    let len = unsafe {
        MultiByteToWideChar(0, 0, bytes.as_ptr(), bytes.len() as i32, std::ptr::null_mut(), 0)
    };
    if len <= 0 {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    let mut buf = vec![0u16; len as usize];
    unsafe {
        MultiByteToWideChar(0, 0, bytes.as_ptr(), bytes.len() as i32, buf.as_mut_ptr(), len)
    };
    std::ffi::OsString::from_wide(&buf).to_string_lossy().into_owned()
}

// 创建计划任务
// 使用 schtasks /Create /XML 导入，XML 文件以 UTF-16 LE BOM 编码写入
// 解决中文 Windows 上 schtasks.exe 将 UTF-8 按 GBK 解码导致的 "无法切换命名空间" 错误
#[cfg(windows)]
pub fn create_scheduled_task() -> Result<(), String> {
    use std::io::Write;
    use std::process::Command;

    crate::startup_diagnostics::set_startup_stage_if_starting("检查管理员启动：准备创建计划任务");

    let exe_path = std::env::current_exe()
        .map_err(|e| format!("获取程序路径失败: {}", e))?;
    let exe_path_str = exe_path.to_string_lossy();

    let username = std::env::var("USERNAME").unwrap_or_default();
    let allow_on_battery = crate::services::settings::get_settings().auto_start_on_battery;
    let xml = build_task_xml(&exe_path_str, &username, allow_on_battery);

    // 写入临时 XML 文件（UTF-16 LE with BOM）
    // 优先使用用户临时目录；若路径含非 ASCII 字符（如中文用户名），改用 C:\Windows\Temp
    // 避免 schtasks.exe 的 ANSI 入口点损坏命令行参数中的路径
    let user_temp = std::env::temp_dir();
    let temp_path = if user_temp.to_string_lossy().is_ascii() {
        user_temp.join("qc_task.xml")
    } else {
        std::path::PathBuf::from(r"C:\Windows\Temp\qc_task.xml")
    };
    let temp_path_str = temp_path.to_string_lossy();

    crate::startup_diagnostics::set_startup_stage_if_starting("检查管理员启动：写入 XML 文件");
    {
        let mut file = std::fs::File::create(&temp_path)
            .map_err(|e| format!("创建临时文件失败: {}", e))?;
        file.write_all(&encode_utf16_le_with_bom(&xml))
            .map_err(|e| format!("写入临时文件失败: {}", e))?;
        file.flush()
            .map_err(|e| format!("刷新临时文件失败: {}", e))?;
    }

    // 通过 XML 导入创建新任务（/F 强制覆盖已有任务，无需先删除）
    crate::startup_diagnostics::set_startup_stage_if_starting("检查管理员启动：创建新的计划任务");
    let output = Command::new("schtasks")
        .args(["/Create", "/XML", &temp_path_str, "/TN", TASK_NAME, "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    // 无论成功失败都清理临时文件
    let _ = std::fs::remove_file(&temp_path);

    let output = output.map_err(|e| format!("执行 schtasks 失败: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = decode_ansi(&output.stderr);
        Err(format!("创建计划任务失败: {}", stderr))
    }
}

#[cfg(not(windows))]
pub fn create_scheduled_task() -> Result<(), String> {
    Err("仅支持 Windows".to_string())
}


// 删除计划任务
#[cfg(windows)]
pub fn delete_scheduled_task() -> Result<(), String> {
    use std::process::Command;

    crate::startup_diagnostics::set_startup_stage_if_starting("检查管理员启动：删除计划任务");

    let _ = Command::new("schtasks")
        .args(["/Delete", "/TN", TASK_NAME, "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    Ok(())
}

#[cfg(not(windows))]
pub fn delete_scheduled_task() -> Result<(), String> {
    Ok(())
}

// 通过计划任务启动程序
#[cfg(windows)]
pub fn run_via_scheduled_task() -> bool {
    use std::process::Command;

    crate::startup_diagnostics::set_startup_stage_if_starting("检查管理员启动：通过计划任务启动提权实例");

    let output = Command::new("schtasks")
        .args(["/Run", "/TN", TASK_NAME])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    matches!(output, Ok(o) if o.status.success())
}

#[cfg(not(windows))]
pub fn run_via_scheduled_task() -> bool {
    false
}

// 尝试以管理员权限重启程序（优先使用计划任务）
#[cfg(windows)]
pub fn try_elevate_and_restart() -> bool {
    crate::startup_diagnostics::set_startup_stage_if_starting("检查管理员启动：尝试复用已有计划任务");
    if is_scheduled_task_exists() && is_scheduled_task_path_valid() && run_via_scheduled_task() {
        return true;
    }
    
    crate::startup_diagnostics::set_startup_stage_if_starting("检查管理员启动：回退到 UAC 提权");
    try_elevate_with_uac()
}

#[cfg(not(windows))]
pub fn try_elevate_and_restart() -> bool {
    false
}

// 使用 UAC 提权重启
#[cfg(windows)]
pub fn try_elevate_with_uac() -> bool {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use ::windows::Win32::UI::Shell::ShellExecuteW;
    use ::windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use ::windows::core::PCWSTR;

    crate::startup_diagnostics::set_startup_stage_if_starting("检查管理员启动：拉起 UAC 提权窗口");

    if let Ok(exe_path) = std::env::current_exe() {
        let operation: Vec<u16> = OsStr::new("runas")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let file: Vec<u16> = exe_path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            let result = ShellExecuteW(
                None,
                PCWSTR(operation.as_ptr()),
                PCWSTR(file.as_ptr()),
                PCWSTR(std::ptr::null()),
                PCWSTR(std::ptr::null()),
                SW_SHOWNORMAL,
            );

            return result.0 as usize > 32;
        }
    }
    false
}

#[cfg(not(windows))]
pub fn try_elevate_with_uac() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_task_xml_battery_flag() {
        // allow_on_battery=true → DisallowStartIfOnBatteries=false
        let xml = build_task_xml("C:\\test\\app.exe", "TestUser", true);
        assert!(xml.contains("<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>"));
        assert!(xml.contains("<StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>"));
        assert!(xml.contains("<RunLevel>HighestAvailable</RunLevel>"));
        assert!(xml.contains("<Command>C:\\test\\app.exe</Command>"));
        assert!(xml.contains("<UserId>TestUser</UserId>"));
        assert!(xml.contains("xmlns=\"http://schemas.microsoft.com/windows/2004/02/mit/task\""));

        // allow_on_battery=false → DisallowStartIfOnBatteries=true
        let xml_disallow = build_task_xml("C:\\test\\app.exe", "TestUser", false);
        assert!(xml_disallow.contains("<DisallowStartIfOnBatteries>true</DisallowStartIfOnBatteries>"));
    }

    #[test]
    fn build_task_xml_escapes_special_chars() {
        // 用户名和路径含 & < > " ' 五个特殊字符，应全部转义，不能原样出现在 XML 中
        let xml = build_task_xml("C:\\a&b<c>\"d'\\app.exe", "U&V<W>\"X'", true);
        assert!(xml.contains("<UserId>U&amp;V&lt;W&gt;&quot;X&apos;</UserId>"));
        assert!(xml.contains("<Command>C:\\a&amp;b&lt;c&gt;&quot;d&apos;\\app.exe</Command>"));
        // 确保未转义的 & 不存在
        assert!(!xml.contains("U&V"));
    }

    #[test]
    fn encode_utf16_le_with_bom_correct() {
        // BOM (FF FE) + "A" in UTF-16 LE (41 00)
        assert_eq!(encode_utf16_le_with_bom("A"), vec![0xFF, 0xFE, 0x41, 0x00]);
    }

    #[cfg(windows)]
    #[test]
    fn decode_ansi_correct() {
        // 空输入
        assert_eq!(decode_ansi(b""), "");
        // ASCII 在所有 ANSI 代码页中编码一致
        assert_eq!(decode_ansi(b"error: access denied"), "error: access denied");
    }
}
