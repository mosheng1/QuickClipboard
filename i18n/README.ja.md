<h1 align="center">QuickClipboard</h1>

<p align="center">
  <strong>コピー＆ペーストの体験を再定義</strong><br>
  軽量 · 高速 · スマート · カスタマイズ可能
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
  <a href="../README.md">简体中文</a> · <a href="README.en.md">English</a> · <a href="README.zh-TW.md">繁體中文</a> · <b>日本語</b> · <a href="README.ko.md">한국어</a>
</div>

---

## 紹介

**QuickClipboard** は、Tauri 2 + Rust + React で構築されたクロスプラットフォームのクリップボード強化ツールです（現在 Windows と Android に対応）。コピーした瞬間から自動的にテキスト、画像、リッチテキスト、ファイルを記録し、過去にコピーした内容をいつでも取り戻せます。記録だけでなく、画像ピン留め、OCR、WebDAV 同期、LAN 同期/転送などの機能も統合しており、日常業務の生産性を大幅に向上させます。

> ネイティブパフォーマンス、低メモリ使用量、起動後すぐに使用可能、システムトレイ常駐。

---

## 核心機能

| モジュール                  | 機能                                                                                                                |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| クリップボード管理          | 全タイプ記録（テキスト / HTML / 画像 / ファイル）· スマート重複排除 · 検索フィルター · 仮想リスト · ドラッグ並べ替え / 固定 · SQLite 永続化 |
| コンテンツプレビュー        | ホバープレビュー（テキスト / HTML / 画像 / ファイルリスト）· Ctrl+スクロール / ズーム · マルチフォーマット切り替えプレビュー |
| クイックペースト            | リストからペースト · 数字キー 1-9 でペースト · プレーンテキスト / 書式付きペースト · マージコピー / マージペースト · ワンショットペースト · クイックペーストウィンドウ · Win+V 対応 |
| お気に入りとグループ        | よく使う内容をお気に入り登録 · カスタムグループ / アイコン / カラー · グループ並べ替え · グループへ一括移動 · ショートカットキーでグループ切替 |
| Emoji / 記号 / ギャラリー   | 絵文字大全 · 記号大全 · カスタム画像 / GIF ギャラリー · 最近使用 · ドラッグまたはクリックで使用 |
| 画面にピン留め              | デスクトップ固定表示 · ドラッグでリサイズ / 固定 · コピー / 名前を付けて保存 |
| OCR 認識                    | 画像 OCR · ワンクリックでテキスト抽出とコピー                                                                        |
| 同期 / 転送                 | WebDAV フル同期 · LAN HTTP 直接接続 · ペアリングコード接続 · 自動プッシュ/プル · ファイル送信                           |
| エッジスナップとウィンドウ  | 画面端で自動非表示 · カーソル追従で呼び出し · ウィンドウ固定 · 位置 / サイズを記憶 · タイトルバー方向切替                |
| パーソナライズ              | システム追従 / ライト・ダークテーマ / スーパー背景 · 複数のテーマスタイル · カスタム背景 / ぼかし · カスタムフォント · アニメーション切替 |
| 低メモリモード              | 自動または手動で軽量モードに切替 · 即時に完全 UI 復元 · メモリ不足の心配無用（低メモリモードで約 10MB）                   |
| バックグラウンド最適化      | バックグラウンド時に自動メモリ解放 · フロントエンド更新停止 · システムリソース使用量削減（バックグラウンドで約 50MB）       |
| データ管理                  | ZIP インポート / エクスポート · バックアップと復元 · カスタム保存先 · データ移行 / マージ · 履歴クリア · ポータブルモード    |
| アプリフィルタリング        | フィルター対象アプリ一覧 · クリップボード監視フィルター · フォアグラウンド時に全機能を無効化 · プロセスレベルルール |
| システム連携                | トレイ常駐 · 自動起動 · 自動更新 · 管理者権限で実行 · 起動通知                                                        |

---

## UI プレビュー

<div align="center">

<table>
  <tr align="center">
    <td>
      <a href="../readme-assets/display/浅色.png" target="_blank">
        <img src="../readme-assets/display/浅色.png" alt="浅色主题">
      </a>
      <div><strong>ライト</strong></div>
    </td>
    <td>
      <a href="../readme-assets/display/浅色手绘.png" target="_blank">
        <img src="../readme-assets/display/浅色手绘.png" alt="浅色手绘主题">
      </a>
      <div><strong>ライトスケッチ</strong></div>
    </td>
    <td>
      <a href="../readme-assets/display/暗色.png" target="_blank">
        <img src="../readme-assets/display/暗色.png" alt="暗色主题">
      </a>
      <div><strong>ダーク</strong></div>
    </td>
    <td>
      <a href="../readme-assets/display/暗色经典.png" target="_blank">
        <img src="../readme-assets/display/暗色经典.png" alt="暗色经典主题">
      </a>
      <div><strong>ダーククラシック</strong></div>
    </td>
    <td>
      <a href="../readme-assets/display/暗色手绘.png" target="_blank">
        <img src="../readme-assets/display/暗色手绘.png" alt="暗色手绘主题">
      </a>
      <div><strong>ダークスケッチ</strong></div>
    </td>
    <td>
      <a href="../readme-assets/display/自定义背景.png" target="_blank">
        <img src="../readme-assets/display/自定义背景.png" alt="自定义背景">
      </a>
      <div><strong>カスタム背景</strong></div>
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
      <div><strong>絵文字ピッカー</strong></div>
    </td>
    <td>
      <a href="../readme-assets/display/图库页.png" target="_blank">
        <img src="../readme-assets/display/图库页.png" alt="图库页">
      </a>
      <div><strong>ギャラリー</strong></div>
    </td>
  </tr>
</table>

<table>
  <tr align="center">
    <td>
      <a href="../readme-assets/display/内容预览.gif" target="_blank">
        <img src="../readme-assets/display/内容预览.gif" alt="内容预览">
      </a>
      <div><strong>コンテンツプレビュー</strong></div>
    </td>
  </tr>
</table>

</div>

---

## システム要件

- Windows 10 / 11 (x64)

---

## ダウンロード（v0.5.0）

| タイプ                                                      |                       説明 |                                                        ダウンロード数                                                        | リンク                                                                                                                                                                                              |
| ----------------------------------------------------------- | -------------------------: | :--------------------------------------------------------------------------------------------------------------------------: | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **NSIS インストーラ**<br>`QuickClipboard_0.5.0_x64-setup.exe` | 推奨；自動アンインストール対応 | ![ダウンロード](https://img.shields.io/github/downloads/mosheng1/QuickClipboard/QuickClipboard_0.5.0_x64-setup.exe?label=) | [![ダウンロード NSIS](https://img.shields.io/badge/ダウンロード-NSISインストーラ-blue?style=for-the-badge)](https://github.com/mosheng1/QuickClipboard/releases/download/v0.5.0/QuickClipboard_0.5.0_x64-setup.exe) |
| **ポータブル版**<br>`QuickClipboard_0.5.0.exe`              |       インストール不要、ダブルクリックで使用 | ![ダウンロード](https://img.shields.io/github/downloads/mosheng1/QuickClipboard/QuickClipboard_0.5.0.exe?label=)            | [![ダウンロード ポータブル](https://img.shields.io/badge/ダウンロード-ポータブル版-orange?style=for-the-badge)](https://github.com/mosheng1/QuickClipboard/releases/download/v0.5.0/QuickClipboard_0.5.0.exe)    |
| **USB ポータブル版**<br>`QuickClipboard_0.5.0_portable.exe` |       USB メモリやモバイル利用に最適 | ![ダウンロード](https://img.shields.io/github/downloads/mosheng1/QuickClipboard/QuickClipboard_0.5.0_portable.exe?label=)    | [![ダウンロード USB](https://img.shields.io/badge/ダウンロード-USBポータブル版-green?style=for-the-badge)](https://github.com/mosheng1/QuickClipboard/releases/download/v0.5.0/QuickClipboard_0.5.0_portable.exe) |
| **Android 版**<br>`QuickClipboard_Android_v1.0.4.apk`       |     Android 端末向け | ![ダウンロード](https://img.shields.io/github/downloads/mosheng1/QuickClipboard/QuickClipboard_Android_v1.0.4.apk?label=)    | [![ダウンロード Android](https://img.shields.io/badge/ダウンロード-Android-success?style=for-the-badge)](https://github.com/mosheng1/QuickClipboard/releases/download/v0.4.0/QuickClipboard_Android_v1.0.4.apk)  |
| **クラウドドライブ**                                          |    GitHub が遅い場合の代替手段 |                                                          —                                                                    | [![クラウドドライブ](https://img.shields.io/badge/クラウドドライブ-クリック-red?style=for-the-badge)](https://www.123912.com/s/A9Ckjv-Vu75v?pwd=UhWA#)                                              |

---

## 公式サイト · チュートリアル · コミュニティ

<div align="center">

<a href="https://space.bilibili.com/438982697" target="_blank">
  <img src="https://img.shields.io/badge/Bilibili-デモ動画-00A1D6?style=for-the-badge&logo=bilibili" alt="Bilibili">
</a>

<p style="margin-top:6px; margin-bottom:18px;">
  機能デモ、使い方チュートリアル、インストールガイド、よくある質問
</p>

<a href="https://quickclipboard.cn/" target="_blank">
  <img src="https://img.shields.io/badge/公式サイト-quickclipboard.cn-blue?style=for-the-badge&logo=firefox-browser" alt="公式サイト">
</a>

<p style="margin-top:6px; margin-bottom:24px;">
  最新バージョン、ダウンロードミラー、ドキュメントなどを入手
</p>

<p style="margin-top:10px; margin-bottom:12px;">
  QR コードをスキャンするか、グループ番号を検索して参加：
</p>

<table>
  <tr>
    <td align="center" width="33%">
      <a href="https://pd.qq.com/s/blp3j847c" target="_blank">
        <img src="../src/assets/pD_1.png" alt="チャンネルQRコード" width="170" />
      </a>
      <div style="margin-top:8px;"><strong>チャンネル：</strong>pd80680380</div>
      <div style="margin-top:10px;">
        <a href="https://pd.qq.com/s/blp3j847c" target="_blank">
          <img src="https://img.shields.io/badge/今すぐ参加-3b82f6?style=for-the-badge" alt="今すぐ参加" />
        </a>
      </div>
    </td>
    <td align="center" width="33%">
      <a href="https://qm.qq.com/q/nUCO76MX9C" target="_blank">
        <img src="../src/assets/qG_1.png" alt="グループ1QRコード" width="170" />
      </a>
      <div style="margin-top:8px;"><strong>グループ1：</strong>725313287</div>
      <div style="margin-top:10px;">
        <a href="https://qm.qq.com/q/nUCO76MX9C" target="_blank">
          <img src="https://img.shields.io/badge/今すぐ参加-3b82f6?style=for-the-badge" alt="今すぐ参加" />
        </a>
      </div>
    </td>
    <td align="center" width="33%">
      <a href="https://qm.qq.com/q/O5zOi3uTuy" target="_blank">
        <img src="../src/assets/qG_2.png" alt="グループ2QRコード" width="170" />
      </a>
      <div style="margin-top:8px;"><strong>グループ2：</strong>1033556729</div>
      <div style="margin-top:10px;">
        <a href="https://qm.qq.com/q/O5zOi3uTuy" target="_blank">
          <img src="https://img.shields.io/badge/今すぐ参加-3b82f6?style=for-the-badge" alt="今すぐ参加" />
        </a>
      </div>
    </td>
  </tr>
</table>

</div>

---

## サポートとスポンサー

<div align="center">
  <p>このプロジェクトが役に立ったと思われたら、Star、Fork、または寄付で開発を支援してください。</p>
  <img src="../src/assets/wxzsm.png" alt="寄付QRコード" width="240">
</div>


---

## 開発とビルド

### 環境要件

- Node.js ≥ 16  
- Rust ≥ 1.70  
- Tauri CLI ≥ 2.0

### よく使うコマンド

```bash
# 依存関係のインストール
npm install

# 開発モード
npm run tauri dev

# リリースビルド
npm run tauri:build
```

## ライセンス

このプロジェクトは [Apache License 2.0](LICENSE) の下で公開されています。
