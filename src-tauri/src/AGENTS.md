# src-tauri/src/ — Rust 后端核心

Tauri 2 应用的 Rust 代码入口。`lib.rs` 定义 `quickclipboard_lib` crate，`main.rs` 调用 `run()`。

## 结构

```
src/
├── lib.rs            # crate root，模块声明 + pub use 导出
├── main.rs           # 二进制入口，调用 lib::run()
├── startup_diagnostics.rs  # 启动诊断（崩溃检测、旧进程检查）
├── commands/         # Tauri invoke 命令（13 个模块）
│   ├── mod.rs        #   pub mod 声明 + pub use 导出
│   ├── clipboard.rs  #   剪贴板操作命令
│   ├── settings.rs   #   设置命令
│   ├── window.rs     #   窗口控制命令
│   └── ...           #   app_links, data_management, favorites 等
├── services/         # 业务逻辑层（19 个模块）
│   ├── clipboard/    #   剪贴板监听 + 持久化
│   ├── database/     #   SQLite 连接管理（单连接 + Mutex）
│   ├── ai/           #   AI 翻译服务
│   ├── lan_sync/     #   局域网同步
│   ├── settings/     #   设置读写
│   └── ...           #   image, paste, sound, memory 等
├── windows/          # 窗口管理（10 个子模块）
│   ├── main_window/  #   主窗口显示/隐藏/吸附
│   ├── quickpaste/   #   快速粘贴窗口
│   ├── tray/         #   系统托盘
│   └── ...           #   settings_window, pin_image 等
├── security/         # WebView 安全检查
├── state/            # Tauri 全局状态管理
└── utils/            # mouse, screen, positioning 工具
```

## Feature gate

截图功能通过虚拟 feature `screenshot-suite` 统一控制。`screenshot-suite-oss`（社区版）
和 `screenshot-suite`（完整版）各自激活该虚拟 feature，互斥编译。

OSS 插件运行时注册为 `screenshot-suite` 命名空间（方案 B），权限由插件自身 `build.rs` 注册。
`capabilities/screenshot.json` 中直接使用 `screenshot-suite:` 前缀，无需构建时替换。

```rust
// lib.rs 已包含互斥保护：
#[cfg(all(feature = "screenshot-suite-private", feature = "screenshot-suite-oss"))]
compile_error!("screenshot-suite and screenshot-suite-oss cannot coexist");
```

## 新增 Tauri 命令（三步法）

1. **创建命令模块** — `commands/<name>.rs`，实现 `#[tauri::command]` 函数
2. **注册模块** — `commands/mod.rs` 中 `pub mod <name>` + `pub use <name>::*`
3. **声明权限** — 插件 `build.rs` 的 `tauri_plugin::Builder::new(&[...]).build()` + `capabilities/*.json`

遗漏任一步 → 前端 `invoke()` 静默失败（无编译错误）。

## SQLite 约束

- 单连接 + `Mutex`，WAL 模式
- **不支持并发写入** — 避免多线程同时写库
- 连接获取：`services/database/` 模块管理

## 反模式

- ❌ 在社区版中启用私有 feature：`screenshot-suite-private`、`gpu-image-viewer`
- ❌ 在 `lib.rs` 中直接引用私有插件类型（应在 feature gate 后使用）
- ❌ 跳过 capabilities 权限声明
- ❌ 并发写入 SQLite
