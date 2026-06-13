# src/ — 前端

Vite 根目录。纯 JavaScript/JSX，禁用 TypeScript。

## 结构

```
src/
├── windows/          # 独立 mini-app（每个窗口一个目录）
│   ├── main/         #   主窗口（剪贴板历史、收藏、分组）
│   ├── quickpaste/   #   快速粘贴浮窗
│   ├── settings/     #   设置窗口
│   ├── preview/      #   内容预览窗口
│   ├── community/    #   社区版专属窗口
│   ├── textEditor/   #   文本编辑器窗口
│   ├── pinImage/     #   图片钉窗口
│   ├── updater/      #   更新器窗口
│   ├── receiveBox/   #   接收文件窗口
│   ├── transferShelf/ #  传输文件架窗口
│   └── screenshot/   #   截图窗口（私有插件，不在仓库中）
├── shared/           # 跨窗口共享模块（详见 shared/AGENTS.md）
├── plugins/          # 前端插件（context_menu, input_dialog）
└── assets/           # 静态资源
```

## 每个窗口的标准文件

```
<窗口>/
├── index.html       # Vite 入口 HTML（被 rollupOptions.input 引用）
├── index.jsx        # React 挂载点，import './index.html'
└── App.jsx          # 根组件
```

> 部分既有窗口偏离此模式：
> - `community/` 使用 `index.js`（非 `index.jsx`），且缺少 `App.jsx`
> - `pinImage/` 使用 `pinImage.html`（非 `index.html`）+ `index.js`，且缺少 `App.jsx`
> - `updater/` 缺少 `App.jsx`
>
> 新增窗口时遵循标准模式即可。

## 新增窗口步骤

1. 创建 `src/windows/<name>/` 目录，含标准三文件
2. 在 `vite.config.js` → `rollupOptions.input` 中添加条目
3. 在 `src-tauri/src/windows/` 中创建 Rust 窗口管理器（`WebviewWindowBuilder` 动态创建窗口）
4. 在 `src-tauri/capabilities/<name>.json` 中添加 capabilities 文件
5. 在 `src-tauri/tauri.conf.json` → `app.security.capabilities` 中注册 capability 名称

> ⚠️ 多数窗口在 Rust 端通过 `WebviewWindowBuilder` **动态创建**，不会出现在 `tauri.conf.json` 的 `app.windows` 数组中（只有 `main` 窗口例外）。

## 关键约定

- **纯 JS/JSX** — 无 `.ts`/`.tsx`
- **UnoCSS global 模式** — 使用 utility class，不用 CSS Modules
- **i18n 强制** — `useTranslation()`，双 locale 同步
- **Valtio** — 唯一状态管理方案
- **dompurify** — 用户 HTML 必须先净化
- **Vite alias**: `@` → `src/`、`@shared` → `src/shared/`、`@windows` → `src/windows/`

## 反模式

- ❌ 在截图窗口直接 import `undo`/`redo`（用 `useShapeHistory()`）
- ❌ 忽略任一 locale 的 i18n 更新
