#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "linux")]
    {
        if std::env::var("WAYLAND_DISPLAY").is_ok()
            && std::env::var("GDK_BACKEND").is_err()
        {
            std::env::set_var("GDK_BACKEND", "x11");
        }

        // 如果已有实例运行，通过 Unix socket 发送命令并退出
        let args: Vec<String> = std::env::args().collect();
        let has_action = args.iter().any(|a|
            a == "--toggle" || a == "--quickpaste" || a == "--settings"
        );
        if has_action && quickclipboard_lib::is_server_running() {
            let cmd = if args.contains(&"--quickpaste".to_string()) {
                "quickpaste"
            } else if args.contains(&"--settings".to_string()) {
                "settings"
            } else {
                "toggle"
            };
            match quickclipboard_lib::send_command(cmd) {
                Ok(()) => return,
                Err(e) => eprintln!("IPC 发送失败: {}，启动新实例", e),
            }
        }
    }
    quickclipboard_lib::install_startup_panic_hook();
    quickclipboard_lib::run();
}
