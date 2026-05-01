# QuickClipboard Linux 安装与配置指南

适用于 Ubuntu 26.04 LTS (Wayland) 及类似发行版。

## 系统要求

- OS: Ubuntu 26.04 LTS 或其他基于 GNOME + Wayland 的 Linux 发行版
- Rust: 1.94.0+
- Node.js: 24.14.0+
- 显示服务器: Wayland（自动使用 XWayland 兼容层）

## 1. 安装系统依赖

```bash
sudo apt update
sudo apt install -y \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  libxdo-dev \
  libssl-dev \
  libasound2-dev \
  librsvg2-dev \
  libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev \
  xdotool \
  build-essential \
  pkg-config
```

关键依赖说明：

| 依赖包 | 用途 |
|--------|------|
| `libwebkit2gtk-4.1-dev` | Tauri WebView 渲染引擎 |
| `libgtk-3-dev` | GTK3 窗口管理 |
| `libayatana-appindicator3-dev` | 系统托盘图标 |
| `libxdo-dev` / `xdotool` | 粘贴模拟（通过 XWayland） |
| `libssl-dev` | 网络通信 |
| `libasound2-dev` | 音频播放 |

## 2. 构建项目

### 克隆仓库

```bash
git clone https://github.com/mosheng1/QuickClipboard.git
cd QuickClipboard
```

### 安装前端依赖

```bash
npm install
```

### 社区版构建（推荐）

社区版排除了 Windows 专属的截图和 GPU 图片查看插件：

```bash
node scripts/community-build.js
```

构建产物位于：
- 二进制文件: `src-tauri/target/release/QuickClipboard`
- DEB 安装包: `src-tauri/target/release/bundle/deb/QuickClipboard_*_amd64.deb`

### 完整版构建

```bash
node scripts/community-build.js --full
```

> 注意：完整版需要 `screenshot-suite` 和 `gpu-image-viewer` 私有插件源码。

## 3. 安装

### 方式一：DEB 包安装（推荐）

```bash
sudo dpkg -i src-tauri/target/release/bundle/deb/QuickClipboard_*_amd64.deb
```

### 方式二：手动安装

```bash
sudo cp src-tauri/target/release/QuickClipboard /usr/bin/
```

> **重要**：手动安装不会包含图标和 desktop 文件，推荐使用 DEB 包。

## 4. 运行

### 从应用菜单启动

在 GNOME 应用菜单中搜索 "QuickClipboard" 启动。

### 从终端启动

```bash
QuickClipboard
```

### 命令行参数

```bash
QuickClipboard --toggle      # 切换主窗口显示/隐藏
QuickClipboard --quickpaste  # 打开快速粘贴窗口
QuickClipboard --settings    # 打开设置窗口
```

## 5. 快捷键配置

QuickClipboard 在 Wayland 下通过 GNOME 自定义快捷键实现全局热键。

### 默认快捷键

| 快捷键 | 功能 |
|--------|------|
| `Shift+Space` | 切换主窗口 |
| `` Ctrl+` `` | 快速粘贴 |

### 修改快捷键

1. 在 QuickClipboard 设置 → 快捷键中修改
2. 或通过 GNOME 设置 → 键盘 → 自定义快捷键 手动修改

### 快捷键原理

Wayland 出于安全考虑阻止应用直接抓取全局按键。QuickClipboard 使用 `gsettings` 注册 GNOME 自定义快捷键，快捷键路径：

```
/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom0/  → toggle
/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom1/  → quickpaste
/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom2/  → settings
```

## 6. 架构说明

### Wayland 兼容策略

```
QuickClipboard (Wayland)
├── GDK_BACKEND=x11          → 强制 GTK 使用 XWayland
├── 粘贴模拟: xdotool         → 通过 XWayland 模拟按键
├── 全局快捷键: gsettings     → GNOME 自定义快捷键
├── 单实例 IPC: Unix socket   → /tmp/quickclipboard.sock
└── 剪贴板: clipboard-rs      → x11rb X11 剪贴板
```

### 单实例机制

应用启动时检测 `/tmp/quickclipboard.sock` 是否存在：
- **已存在**：通过 Unix socket 发送命令给已有实例，然后退出
- **不存在**：创建 socket 并启动 IPC 服务器

## 7. 常见问题

### Q: 启动时显示 "Connection refused"

构建时未启用 `custom-protocol` 特性，web 资源没有内嵌到二进制文件。确保使用 `scripts/community-build.js` 构建或手动添加 `--features custom-protocol`。

### Q: 快捷键不工作

1. 确认使用 Wayland：`echo $XDG_SESSION_TYPE` 应输出 `wayland`
2. 检查 GNOME 快捷键是否注册：`gsettings get org.gnome.settings-daemon.plugins.media-keys custom-keybindings`
3. 手动重新注册：重启 QuickClipboard

### Q: 粘贴没有反应

1. 确认 `xdotool` 已安装：`which xdotool`
2. 确认 XWayland 可用：`echo $WAYLAND_DISPLAY` 和 `echo $GDK_BACKEND`

### Q: 系统托盘图标不显示

安装 libayatana-appindicator 扩展：

```bash
sudo apt install gnome-shell-extension-appindicator
```

然后注销重新登录。

## 8. Cargo 网络配置（国内用户）

如 crates.io 下载缓慢，配置 USTC 镜像：

```bash
mkdir -p ~/.cargo
cat > ~/.cargo/config.toml << 'EOF'
[source.crates-io]
replace-with = 'ustc'

[source.ustc]
registry = "sparse+https://mirrors.ustc.edu.cn/crates.io-index/"
EOF
```

## 9. 版本历史

详见 [CHANGELOG.md](../CHANGELOG.md)。

| 版本 | 日期 | 说明 |
|------|------|------|
| 0.4.0 | 2026-05-01 | Linux/Wayland 平台支持 |
| 0.3.0 | 2026-04-30 | Windows 版本基础功能 |
