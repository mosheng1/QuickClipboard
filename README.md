<h1 align="center">QuickClipboard</h1>

<p align="center">
  <strong>重新定义你的复制粘贴体验</strong><br>
  轻量 · 快速 · 智能 · 可定制
</p>

<div align="center">
  <img src="readme-assets/logo.png" alt="QuickClipboard Logo" width="120">
  <br><br>
  <a href="https://github.com/mosheng1/QuickClipboard/stargazers">
    <img src="https://img.shields.io/github/stars/mosheng1/QuickClipboard?style=for-the-badge&logo=github&color=yellow" alt="Stars">
  </a>
  <a href="https://github.com/mosheng1/QuickClipboard/releases">
    <img src="https://img.shields.io/github/v/release/mosheng1/QuickClipboard?style=for-the-badge&label=Release&color=brightgreen" alt="Release">
  </a>
  <a href="https://github.com/mosheng1/QuickClipboard/releases">
    <img src="https://img.shields.io/github/downloads/mosheng1/QuickClipboard/total.svg?style=for-the-badge&color=blueviolet" alt="Downloads">
  </a>
  <a href="https://github.com/mosheng1/QuickClipboard/blob/main/LICENSE">
    <img src="https://img.shields.io/github/license/mosheng1/QuickClipboard.svg?style=for-the-badge&color=orange" alt="License">
  </a>
</div>

<div align="center">
  <b>简体中文</b> · <a href="i18n/README.en.md">English</a> · <a href="i18n/README.zh-TW.md">繁體中文</a> · <a href="i18n/README.ja.md">日本語</a> · <a href="i18n/README.ko.md">한국어</a>
</div>

---

## 简介

**QuickClipboard** 是一款跨平台剪贴板增强工具（现支持Windows，Android），基于 Tauri 2 + Rust + React 构建。它在你复制的那一刻就开始工作——自动记录文本、图片、富文本、文件，让你随时找回曾经复制过的任何内容。不只是记录，QuickClipboard 还集成了截图、贴图、OCR、WebDAV 同步与局域网同步/传输等能力，是日常办公效率的全面提升。

> 原生性能，内存占用低，启动即用，常驻系统托盘。

---

## 核心功能

| 模块                  | 功能                                                                                                                |
| --------------------- | ------------------------------------------------------------------------------------------------------------------- |
| 剪贴板管理          | 全类型记录（文本 / HTML / 图片 / 文件） · 智能去重 · 搜索筛选 · 虚拟列表 · 拖拽排序 / 置顶 · SQLite 持久化          |
| 内容预览            | 悬停预览文本 / HTML / 图片 / 文件列表 · Ctrl+滚轮滚动 / 缩放 · 多格式内容切换预览                                   |
| 快速粘贴            | 列表粘贴 · 数字快捷键 1-9 粘贴 · 纯文本 / 带格式粘贴 · 合并复制 / 合并粘贴 · 一次性粘贴 · 便捷粘贴窗口 · Win+V 支持 |
| 收藏与分组          | 收藏常用内容 · 自定义分组 / 图标 / 颜色 · 分组排序 · 批量移动到分组 · 快捷键切换分组                                |
| Emoji / 符号 / 图库 | Emoji 大全 · 符号大全 · 自定义图片 / GIF 图库 · 最近使用 · 拖拽 / 点击即用                                          |
| 贴图到屏幕          | 桌面置顶贴图 · GPU 加速渲染 · 拖拽缩放 / 置顶 · 复制 / 另存为 · 截图后直接贴图                                      |
| 内置截图            | 普通截图 · 快速截屏 / 快速贴图 / 快速 OCR · 多屏支持 · 长截图 · 自动选区 · 屏幕取色 · 标注编辑                      |
| OCR 识别            | 图片 OCR · 截图 OCR · 一键提取并复制文字                                                                            |
| 同步 / 传输         | WebDAV 全同步 · 局域网 HTTP 直连 · 配对码连接 · 自动推拉 · 文件发送                                                |
| 边缘吸附与窗口      | 屏幕边缘自动隐藏 · 跟随光标唤出 · 窗口置顶 · 记忆位置 / 尺寸 · 标题栏方向切换                                       |
| 个性化              | 跟随系统 / 明暗主题 / 超级背景 · 多套主题风格 · 自定义背景 / 模糊 · 自定义字体 · 动画开关                           |
| 低内存模式          | 自动或手动切换轻量模式 · 即时恢复完整界面 · 告别内存焦虑（低占用模式占用仅~10MB）                                   |
| 专项后台优化        | 进入后台自动清理内存占用 · 暂停前端更新 · 降低系统资源占用（后台时内存占用~50MB）                                   |
| 数据管理            | ZIP 导入导出 · 备份恢复 · 自定义存储路径 · 数据迁移 / 合并 · 清空历史 · 便携模式                                    |
| 应用过滤            | 已过滤应用列表 · 剪贴板监听过滤 · 前台禁用所有功能 · 进程级规则                                                     |
| 系统集成            | 托盘常驻 · 开机自启动 · 自动更新 · 管理员权限运行 · 启动通知                                                        |

---

## 界面预览

<div align="center">

<table>
  <tr align="center">
    <td>
      <a href="readme-assets/display/浅色.png" target="_blank">
        <img src="readme-assets/display/浅色.png" alt="浅色主题">
      </a>
      <div><strong>浅色</strong></div>
    </td>
    <td>
      <a href="readme-assets/display/浅色手绘.png" target="_blank">
        <img src="readme-assets/display/浅色手绘.png" alt="浅色手绘主题">
      </a>
      <div><strong>浅色手绘</strong></div>
    </td>
    <td>
      <a href="readme-assets/display/暗色.png" target="_blank">
        <img src="readme-assets/display/暗色.png" alt="暗色主题">
      </a>
      <div><strong>暗色</strong></div>
    </td>
    <td>
      <a href="readme-assets/display/暗色经典.png" target="_blank">
        <img src="readme-assets/display/暗色经典.png" alt="暗色经典主题">
      </a>
      <div><strong>暗色经典</strong></div>
    </td>
    <td>
      <a href="readme-assets/display/暗色手绘.png" target="_blank">
        <img src="readme-assets/display/暗色手绘.png" alt="暗色手绘主题">
      </a>
      <div><strong>暗色手绘</strong></div>
    </td>
    <td>
      <a href="readme-assets/display/自定义背景.png" target="_blank">
        <img src="readme-assets/display/自定义背景.png" alt="自定义背景">
      </a>
      <div><strong>自定义背景</strong></div>
    </td>
  </tr>
</table>

<table>
  <tr align="center">
    <td>
      <a href="readme-assets/display/设置.png" target="_blank">
        <img src="readme-assets/display/设置.png" alt="设置界面">
      </a>
      <div><strong>设置</strong></div>
    </td>
    <td>
      <a href="readme-assets/display/表情符号页.png" target="_blank">
        <img src="readme-assets/display/表情符号页.png" alt="表情符号页">
      </a>
      <div><strong>表情符号页</strong></div>
    </td>
    <td>
      <a href="readme-assets/display/图库页.png" target="_blank">
        <img src="readme-assets/display/图库页.png" alt="图库页">
      </a>
      <div><strong>图库页</strong></div>
    </td>
  </tr>
</table>

<table>
  <tr align="center">
    <td>
      <a href="readme-assets/display/内容预览.gif" target="_blank">
        <img src="readme-assets/display/内容预览.gif" alt="内容预览">
      </a>
      <div><strong>内容预览</strong></div>
    </td>
  </tr>
</table>

</div>

---

## 系统要求

- Windows 10 / 11 (x64)  

---

## 下载方式（v0.4.0）

| 类型                                                      |                       说明 |                                                        下载量                                                        | 链接                                                                                                                                                                                            |
| --------------------------------------------------------- | -------------------------: | :------------------------------------------------------------------------------------------------------------------: | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **NSIS 安装包**<br>`QuickClipboard_0.4.0_x64-setup.exe` | 推荐安装方式，支持自动卸载 | ![下载量](https://img.shields.io/github/downloads/mosheng1/QuickClipboard/QuickClipboard_0.4.0_x64-setup.exe?label=) | [![下载 NSIS](https://img.shields.io/badge/下载-NSIS安装包-blue?style=for-the-badge)](https://github.com/mosheng1/QuickClipboard/releases/download/v0.4.0/QuickClipboard_0.4.0_x64-setup.exe) |
| **绿色版**<br>`QuickClipboard_0.4.0.exe`                |       免安装，双击即可使用 |      ![下载量](https://img.shields.io/github/downloads/mosheng1/QuickClipboard/QuickClipboard_0.4.0.exe?label=)      | [![下载绿色版](https://img.shields.io/badge/下载-绿色版-orange?style=for-the-badge)](https://github.com/mosheng1/QuickClipboard/releases/download/v0.4.0/QuickClipboard_0.4.0.exe)            |
| **便携版**<br>`QuickClipboard_0.4.0_portable.exe`       |      更适合放U盘或移动使用 | ![下载量](https://img.shields.io/github/downloads/mosheng1/QuickClipboard/QuickClipboard_0.4.0_portable.exe?label=)  | [![下载便携版](https://img.shields.io/badge/下载-便携版-green?style=for-the-badge)](https://github.com/mosheng1/QuickClipboard/releases/download/v0.4.0/QuickClipboard_0.4.0_portable.exe)    |
| **安卓版**<br>`QuickClipboard_Android_v1.0.4.apk`       |    适用于 Android 设备安装 | ![下载量](https://img.shields.io/github/downloads/mosheng1/QuickClipboard/QuickClipboard_Android_v1.0.4.apk?label=)  | [![下载安卓版](https://img.shields.io/badge/下载-安卓版-success?style=for-the-badge)](https://github.com/mosheng1/QuickClipboard/releases/download/v0.4.0/QuickClipboard_Android_v1.0.4.apk)  |
| **网盘下载**                                            |    GitHub 较慢时的备用渠道 |                                                          —                                                           | [![网盘下载](https://img.shields.io/badge/网盘下载-点击进入-red?style=for-the-badge)](https://www.123912.com/s/A9Ckjv-Vu75v?pwd=UhWA#)                                                        |

---

## 官方网站 · 视频教程 · 社群交流

<div align="center">

<a href="https://space.bilibili.com/438982697" target="_blank">
  <img src="https://img.shields.io/badge/Bilibili-功能演示视频-00A1D6?style=for-the-badge&logo=bilibili" alt="Bilibili">
</a>

<p style="margin-top:6px; margin-bottom:18px;">
  含功能演示、使用教程、安装说明与常见问题
</p>

<a href="https://quickclipboard.cn/" target="_blank">
  <img src="https://img.shields.io/badge/官方网站-quickclipboard.cn-blue?style=for-the-badge&logo=firefox-browser" alt="官网">
</a>

<p style="margin-top:6px; margin-bottom:24px;">
  获取最新版本、下载镜像、文档资料与更多内容
</p>

<p style="margin-top:10px; margin-bottom:12px;">
  扫码或搜索号码加入：
</p>

<table>
  <tr>
    <td align="center" width="33%">
      <a href="https://pd.qq.com/s/blp3j847c" target="_blank">
        <img src="src/assets/pD_1.png" alt="频道二维码" width="170" />
      </a>
      <div style="margin-top:8px;"><strong>频道：</strong>pd80680380</div>
      <div style="margin-top:10px;">
        <a href="https://pd.qq.com/s/blp3j847c" target="_blank">
          <img src="https://img.shields.io/badge/立即加入-3b82f6?style=for-the-badge" alt="立即加入" />
        </a>
      </div>
    </td>
    <td align="center" width="33%">
      <a href="https://qm.qq.com/q/nUCO76MX9C" target="_blank">
        <img src="src/assets/qG_1.png" alt="群聊1二维码" width="170" />
      </a>
      <div style="margin-top:8px;"><strong>群聊1：</strong>725313287</div>
      <div style="margin-top:10px;">
        <a href="https://qm.qq.com/q/nUCO76MX9C" target="_blank">
          <img src="https://img.shields.io/badge/立即加入-3b82f6?style=for-the-badge" alt="立即加入" />
        </a>
      </div>
    </td>
    <td align="center" width="33%">
      <a href="https://qm.qq.com/q/O5zOi3uTuy" target="_blank">
        <img src="src/assets/qG_2.png" alt="群聊2二维码" width="170" />
      </a>
      <div style="margin-top:8px;"><strong>群聊2：</strong>1033556729</div>
      <div style="margin-top:10px;">
        <a href="https://qm.qq.com/q/O5zOi3uTuy" target="_blank">
          <img src="https://img.shields.io/badge/立即加入-3b82f6?style=for-the-badge" alt="立即加入" />
        </a>
      </div>
    </td>
  </tr>
</table>

</div>

---

## 支持与赞助

<div align="center">
  <p>如果你觉得这个项目有帮助，欢迎 Star、Fork 或通过赞赏支持开发。</p>
  <img src="src/assets/wxzsm.png" alt="赞赏码" width="240">
</div>


---

## 开发与构建

### 环境依赖

- Node.js ≥ 16  
- Rust ≥ 1.70  
- Tauri CLI ≥ 2.0

### 常用命令

```bash
# 安装依赖
npm install

# 开发模式
npm run tauri dev

# 构建发行版
npm run tauri:build

# 社区版开发模式（不含私有插件）
npm run tauri:dev:community

# 社区版构建（不含私有插件）
npm run tauri:build:community

# 代码检查与测试（社区版，无需私有插件）
npm run check:community       # Rust 编译检查
npm run clippy:community      # Rust lint
npm run test:rust             # Rust 单元测试
```

### 关于私有组件

本项目的**官方发布版**包含以下私有插件（不在开源范围内）：

- `gpu-image-viewer`（GPU 加速图片窗口）：提升贴图和图片预览性能，在拥有多个贴图窗口情况下显著降低内存占用。
- `screenshot-suite`（截屏套件）：包含自由截屏、截屏贴图、截屏OCR、长截图等相关能力。

---

## 许可证

 本项目基于 [Apache License 2.0](LICENSE) 开源。

> 私有插件 `gpu-image-viewer`、`screenshot-suite` 不在开源范围内，仅官方发布版包含。
