# Changelog

All notable changes to QuickClipboard will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - 2026-05-01

### Added

- **Linux/Wayland 平台支持**: QuickClipboard 现可在 Ubuntu 26.04 LTS (Wayland) 上运行
- **GNOME 自定义快捷键**: 通过 gsettings 注册全局快捷键（Shift+Space 切换、Ctrl+\` 快速粘贴）
- **Unix Socket IPC**: 替代 broken 的 tauri-plugin-single-instance TCP IPC，使用 `/tmp/quickclipboard.sock` 实现单实例通信
- **xdotool 粘贴模拟**: 替代 enigo 避免 GNOME RemoteDesktop 权限弹窗
- **Wayland 检测**: 自动检测 Wayland 会话并使用对应的后端
- **社区版构建脚本**: `scripts/community-build.js` 支持排除私有插件的社区版构建，自动启用 `custom-protocol` 特性

### Changed

- `keyboard.rs` Linux 粘贴模拟从 enigo 切换为 xdotool（通过 XWayland 工作）
- `hotkey.rs` Wayland 下使用 gsettings 注册快捷键替代 X11 XGrabKey
- `main.rs` 启动时检测已有实例并通过 IPC 转发命令
- `lib.rs` 集成 IPC server、Wayland 快捷键注册、RunEvent::Ready 处理
- `community-build.js` 构建脚本增加 `--features custom-protocol` 确保资源内嵌

### Fixed

- **"Connection refused" 错误**: 构建时启用 `custom-protocol` 特性将 web 资源内嵌到二进制文件，不再依赖外部 dev server
- **Wayland 快捷键不工作**: 使用 GNOME gsettings 自定义快捷键替代 X11 XGrabKey
- **粘贴触发 GNOME 权限弹窗**: 使用 xdotool 替代 enigo 虚拟键盘
- **IPC 路径错误**: gsettings path 格式修复 (`/custom-keybindings/custom0/`)

## [0.3.0] - 2026-04-30

### Added

- Windows 版本基础功能
- 剪贴板监听与管理
- 快速粘贴功能
- 全局快捷键支持（Windows）
- 系统托盘
- 多窗口管理（主窗口、快速粘贴、设置、预览、置顶图片）
- OCR 文字识别（Windows）
- 截图功能（Windows）
- GPU 图片查看器（Windows）
- 自动更新

[0.4.0]: https://github.com/mosheng1/QuickClipboard/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/mosheng1/QuickClipboard/releases/tag/v0.3.0
