<h1 align="center">QuickClipboard</h1>

<p align="center">
  <strong>重新定義你的複製貼上體驗</strong><br>
  輕量 · 快速 · 智慧 · 可自訂
</p>

<div align="center">
  <img src="../readme-assets/logo.png" alt="QuickClipboard Logo" width="120">
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
  <a href="../README.md">简体中文</a> · <a href="README.en.md">English</a> · <b>繁體中文</b> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a>
</div>

---

## 簡介

**QuickClipboard** 是一款跨平台剪貼簿增強工具（現支援 Windows、Android），基於 Tauri 2 + Rust + React 構建。它在你複製的那一刻就開始工作——自動記錄文字、圖片、富文本、檔案，讓你隨時找回曾經複製過的任何內容。不只是記錄，QuickClipboard 還整合了貼圖、OCR、WebDAV 同步與區域網路同步/傳輸等能力，是日常辦公效率的全面提升。

> 原生效能，記憶體佔用低，啟動即用，常駐系統托盤。

---

## 核心功能

| 模組                  | 功能                                                                                                                |
| --------------------- | ------------------------------------------------------------------------------------------------------------------- |
| 剪貼簿管理          | 全類型記錄（文字 / HTML / 圖片 / 檔案）· 智慧去重 · 搜尋篩選 · 虛擬列表 · 拖曳排序 / 置頂 · SQLite 持久化           |
| 內容預覽            | 懸停預覽文字 / HTML / 圖片 / 檔案列表 · Ctrl+滾輪捲動 / 縮放 · 多格式內容切換預覽                                   |
| 快速貼上            | 列表貼上 · 數字快捷鍵 1-9 貼上 · 純文字 / 帶格式貼上 · 合併複製 / 合併貼上 · 一次性貼上 · 便捷貼上視窗 · Win+V 支援 |
| 收藏與分組          | 收藏常用內容 · 自訂分組 / 圖示 / 顏色 · 分組排序 · 批次移動到分組 · 快捷鍵切換分組                                  |
| Emoji / 符號 / 圖庫 | Emoji 大全 · 符號大全 · 自訂圖片 / GIF 圖庫 · 最近使用 · 拖曳 / 點擊即用                                            |
| 貼圖到螢幕          | 桌面置頂貼圖 · 拖曳縮放 / 置頂 · 複製 / 另存新檔                                                                      |
| OCR 辨識            | 圖片 OCR · 一鍵提取並複製文字                                                                                        |
| 同步 / 傳輸         | WebDAV 完整同步 · 區域網路 HTTP 直連 · 配對碼連接 · 自動推拉 · 檔案傳送                                              |
| 邊緣吸附與視窗      | 螢幕邊緣自動隱藏 · 跟隨游標喚出 · 視窗置頂 · 記憶位置 / 尺寸 · 標題列方向切換                                       |
| 個人化              | 跟隨系統 / 明暗主題 / 超級背景 · 多套主題風格 · 自訂背景 / 模糊 · 自訂字型 · 動畫開關                               |
| 低記憶體模式        | 自動或手動切換輕量模式 · 即時恢復完整界面 · 告別記憶體焦慮（低佔用模式僅約 10MB）                                   |
| 專項背景最佳化      | 進入背景自動清理記憶體佔用 · 暫停前端更新 · 降低系統資源佔用（背景時記憶體約 50MB）                                  |
| 資料管理            | ZIP 匯入匯出 · 備份還原 · 自訂儲存路徑 · 資料遷移 / 合併 · 清空歷史 · 可攜模式                                      |
| 應用過濾            | 已過濾應用列表 · 剪貼簿監聽過濾 · 前景停用所有功能 · 程序級規則                                                     |
| 系統整合            | 托盤常駐 · 開機自動啟動 · 自動更新 · 管理員權限執行 · 啟動通知                                                       |

---

## 界面預覽

<div align="center">

<table>
  <tr align="center">
    <td>
      <a href="../readme-assets/display/浅色.png" target="_blank">
        <img src="../readme-assets/display/浅色.png" alt="浅色主题">
      </a>
      <div><strong>淺色</strong></div>
    </td>
    <td>
      <a href="../readme-assets/display/浅色手绘.png" target="_blank">
        <img src="../readme-assets/display/浅色手绘.png" alt="浅色手绘主题">
      </a>
      <div><strong>淺色手繪</strong></div>
    </td>
    <td>
      <a href="../readme-assets/display/暗色.png" target="_blank">
        <img src="../readme-assets/display/暗色.png" alt="暗色主题">
      </a>
      <div><strong>暗色</strong></div>
    </td>
    <td>
      <a href="../readme-assets/display/暗色经典.png" target="_blank">
        <img src="../readme-assets/display/暗色经典.png" alt="暗色经典主题">
      </a>
      <div><strong>暗色經典</strong></div>
    </td>
    <td>
      <a href="../readme-assets/display/暗色手绘.png" target="_blank">
        <img src="../readme-assets/display/暗色手绘.png" alt="暗色手绘主题">
      </a>
      <div><strong>暗色手繪</strong></div>
    </td>
    <td>
      <a href="../readme-assets/display/自定义背景.png" target="_blank">
        <img src="../readme-assets/display/自定义背景.png" alt="自定义背景">
      </a>
      <div><strong>自訂背景</strong></div>
    </td>
  </tr>
</table>

<table>
  <tr align="center">
    <td>
      <a href="../readme-assets/display/设置.png" target="_blank">
        <img src="../readme-assets/display/设置.png" alt="设置界面">
      </a>
      <div><strong>設定</strong></div>
    </td>
    <td>
      <a href="../readme-assets/display/表情符号页.png" target="_blank">
        <img src="../readme-assets/display/表情符号页.png" alt="表情符号页">
      </a>
      <div><strong>表情符號頁</strong></div>
    </td>
    <td>
      <a href="../readme-assets/display/图库页.png" target="_blank">
        <img src="../readme-assets/display/图库页.png" alt="图库页">
      </a>
      <div><strong>圖庫頁</strong></div>
    </td>
  </tr>
</table>

<table>
  <tr align="center">
    <td>
      <a href="../readme-assets/display/内容预览.gif" target="_blank">
        <img src="../readme-assets/display/内容预览.gif" alt="内容预览">
      </a>
      <div><strong>內容預覽</strong></div>
    </td>
  </tr>
</table>

</div>

---

## 系統需求

- Windows 10 / 11 (x64)

---

## 下載方式（v0.5.0）

| 類型                                                      |                       說明 |                                                        下載量                                                        | 連結                                                                                                                                                                                            |
| --------------------------------------------------------- | -------------------------: | :------------------------------------------------------------------------------------------------------------------: | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **NSIS 安裝包**<br>`QuickClipboard_0.5.0_x64-setup.exe` | 推薦安裝方式，支援自動卸載 | ![下載量](https://img.shields.io/github/downloads/mosheng1/QuickClipboard/QuickClipboard_0.5.0_x64-setup.exe?label=) | [![下載 NSIS](https://img.shields.io/badge/下載-NSIS安裝包-blue?style=for-the-badge)](https://github.com/mosheng1/QuickClipboard/releases/download/v0.5.0/QuickClipboard_0.5.0_x64-setup.exe) |
| **綠色版**<br>`QuickClipboard_0.5.0.exe`                |       免安裝，雙擊即可使用 |      ![下載量](https://img.shields.io/github/downloads/mosheng1/QuickClipboard/QuickClipboard_0.5.0.exe?label=)      | [![下載綠色版](https://img.shields.io/badge/下載-綠色版-orange?style=for-the-badge)](https://github.com/mosheng1/QuickClipboard/releases/download/v0.5.0/QuickClipboard_0.5.0.exe)            |
| **可攜版**<br>`QuickClipboard_0.5.0_portable.exe`       |      更適合放隨身碟或行動使用 | ![下載量](https://img.shields.io/github/downloads/mosheng1/QuickClipboard/QuickClipboard_0.5.0_portable.exe?label=)  | [![下載可攜版](https://img.shields.io/badge/下載-可攜版-green?style=for-the-badge)](https://github.com/mosheng1/QuickClipboard/releases/download/v0.5.0/QuickClipboard_0.5.0_portable.exe)    |
| **Android 版**<br>`QuickClipboard_Android_v1.0.4.apk`    |    適用於 Android 裝置安裝 | ![下載量](https://img.shields.io/github/downloads/mosheng1/QuickClipboard/QuickClipboard_Android_v1.0.4.apk?label=)  | [![下載 Android 版](https://img.shields.io/badge/下載-Android版-success?style=for-the-badge)](https://github.com/mosheng1/QuickClipboard/releases/download/v0.4.0/QuickClipboard_Android_v1.0.4.apk)  |
| **雲端下載**                                            |    GitHub 較慢時的備用管道 |                                                          —                                                           | [![雲端下載](https://img.shields.io/badge/雲端下載-點擊進入-red?style=for-the-badge)](https://www.123912.com/s/A9Ckjv-Vu75v?pwd=UhWA#)                                                        |

---

## 官方網站 · 影片教學 · 社群交流

<div align="center">

<a href="https://space.bilibili.com/438982697" target="_blank">
  <img src="https://img.shields.io/badge/Bilibili-功能演示影片-00A1D6?style=for-the-badge&logo=bilibili" alt="Bilibili">
</a>

<p style="margin-top:6px; margin-bottom:18px;">
  含功能演示、使用教學、安裝說明與常見問題
</p>

<a href="https://quickclipboard.cn/" target="_blank">
  <img src="https://img.shields.io/badge/官方網站-quickclipboard.cn-blue?style=for-the-badge&logo=firefox-browser" alt="官網">
</a>

<p style="margin-top:6px; margin-bottom:24px;">
  取得最新版本、下載鏡像、文件資料與更多內容
</p>

<p style="margin-top:10px; margin-bottom:12px;">
  掃碼或搜尋號碼加入：
</p>

<table>
  <tr>
    <td align="center" width="33%">
      <a href="https://pd.qq.com/s/blp3j847c" target="_blank">
        <img src="../src/assets/pD_1.png" alt="频道二维码" width="170" />
      </a>
      <div style="margin-top:8px;"><strong>頻道：</strong>pd80680380</div>
      <div style="margin-top:10px;">
        <a href="https://pd.qq.com/s/blp3j847c" target="_blank">
          <img src="https://img.shields.io/badge/立即加入-3b82f6?style=for-the-badge" alt="立即加入" />
        </a>
      </div>
    </td>
    <td align="center" width="33%">
      <a href="https://qm.qq.com/q/nUCO76MX9C" target="_blank">
        <img src="../src/assets/qG_1.png" alt="群聊1二维码" width="170" />
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
        <img src="../src/assets/qG_2.png" alt="群聊2二维码" width="170" />
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

## 支援與贊助

<div align="center">
  <p>如果你覺得這個專案有幫助，歡迎 Star、Fork 或透過贊賞支援開發。</p>
  <img src="../src/assets/wxzsm.png" alt="贊賞碼" width="240">
</div>


---

## 開發與建置

### 環境依賴

- Node.js ≥ 16  
- Rust ≥ 1.70  
- Tauri CLI ≥ 2.0

### 常用指令

```bash
# 安裝依賴
npm install

# 開發模式
npm run tauri dev

# 建置發行版
npm run tauri:build
```

## 授權條款

 本專案基於 [Apache License 2.0](LICENSE) 開源。
