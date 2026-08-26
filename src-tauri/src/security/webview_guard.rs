// WebView2 环境变量安全检查

// 危险参数列表：(参数模式, 描述)
const DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    // DevTools 相关
    ("--auto-open-devtools-for-tabs", "自动打开开发者工具"),
    ("--remote-debugging-port", "远程调试端口"),
    ("--remote-debugging-pipe", "远程调试管道"),
    ("--remote-debugging-address", "远程调试地址"),
    // 安全策略绕过
    ("--disable-web-security", "禁用网页安全策略"),
    ("--disable-site-isolation-trials", "禁用站点隔离"),
    ("--allow-running-insecure-content", "允许运行不安全内容"),
    ("--disable-features=IsolateOrigins", "禁用源隔离"),
    // 扩展注入
    ("--load-extension", "加载外部扩展"),
    ("--disable-extensions-except", "扩展白名单绕过"),
    // 用户数据篡改
    ("--user-data-dir", "自定义用户数据目录"),
    // 沙箱绕过
    ("--disable-gpu-sandbox", "禁用 GPU 沙箱"),
    ("--no-sandbox", "禁用沙箱"),
    ("--disable-setuid-sandbox", "禁用 setuid 沙箱"),
];

// 检查 WebView2 环境变量中是否包含危险参数
fn check_dangerous_webview2_args() -> Option<String> {
    let args = std::env::var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").ok()?;
    let args_lower = args.to_lowercase();

    let detected: Vec<String> = DANGEROUS_PATTERNS
        .iter()
        .filter(|(pattern, _)| args_lower.contains(&pattern.to_lowercase()))
        .map(|(pattern, desc)| format!("• {} ({})", pattern, desc))
        .collect();

    if detected.is_empty() {
        return None;
    }

    Some(format!(
        "环境变量: WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS\n\n检测到的危险参数:\n{}",
        detected.join("\n")
    ))
}

// 显示安全警告对话框
#[cfg(windows)]
fn show_security_warning(warning: &str) {
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONWARNING, MB_OK};

    let title: Vec<u16> = "安全警告 - QuickClipboard\0".encode_utf16().collect();
    let message: Vec<u16> = format!(
        "检测到可能影响应用安全的环境变量配置：\n\n{}\n\n\
        为保护您的数据安全，应用将退出。\n\n\
        如需正常使用，请移除相关环境变量后重新启动。\0",
        warning
    )
    .encode_utf16()
    .collect();

    unsafe {
        MessageBoxW(
            None,
            PCWSTR(message.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONWARNING,
        );
    }
}

// 执行 WebView2 安全检查
// 如果检测到危险参数，显示警告对话框并退出程序
pub fn check_webview_security() {
    #[cfg(all(not(debug_assertions), windows))]
    {
        if let Some(warning) = check_dangerous_webview2_args() {
            show_security_warning(&warning);
            std::process::exit(1);
        }
    }
}
