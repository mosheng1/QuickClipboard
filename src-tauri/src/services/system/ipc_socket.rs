use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};

const SOCKET_PATH: &str = "/tmp/quickclipboard.sock";

pub fn start_ipc_server(app: tauri::AppHandle) {
    let _ = std::fs::remove_file(SOCKET_PATH);
    let listener = match UnixListener::bind(SOCKET_PATH) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("IPC socket 绑定失败: {}", e);
            return;
        }
    };

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => handle_client(&app, stream),
                Err(e) => eprintln!("IPC 连接错误: {}", e),
            }
        }
    });
}

fn handle_client(app: &tauri::AppHandle, stream: UnixStream) {
    let mut reader = BufReader::new(stream);
    let mut cmd = String::new();
    if reader.read_line(&mut cmd).is_ok() {
        let cmd = cmd.trim();
        let app = app.clone();
        match cmd {
            "toggle" => {
                crate::toggle_main_window_visibility(&app);
            }
            "quickpaste" => {
                if let Err(e) = crate::windows::quickpaste::show_quickpaste_window(&app) {
                    eprintln!("IPC quickpaste 失败: {}", e);
                }
            }
            "settings" => {
                let app = app.clone();
                std::thread::spawn(move || {
                    if let Err(e) = crate::windows::settings_window::open_settings_window(&app) {
                        eprintln!("IPC settings 失败: {}", e);
                    }
                });
            }
            _ => {}
        }
        if let Ok(mut s) = reader.get_mut().try_clone() {
            let _ = s.write_all(b"ok\n");
        }
    }
}

pub fn send_command(cmd: &str) -> Result<(), String> {
    let mut stream = UnixStream::connect(SOCKET_PATH)
        .map_err(|e| format!("连接 QC 失败: {}", e))?;
    stream.write_all(format!("{}\n", cmd).as_bytes())
        .map_err(|e| format!("发送命令失败: {}", e))?;
    Ok(())
}

pub fn is_server_running() -> bool {
    UnixStream::connect(SOCKET_PATH).is_ok()
}
