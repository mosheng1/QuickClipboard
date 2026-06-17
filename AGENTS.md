# QuickClipboard

跨平台剪贴板管理工具。Tauri 2 + Rust 后端，React/Vite 前端。支持 Windows + Android。

## 架构概览

```
src/              ← Vite 根目录（纯 JS/JSX，禁用 TypeScript）
  windows/        ← 独立 mini-app（每个对应一个 Tauri 窗口）
  shared/         ← 跨窗口共享代码（i18n、store、hooks、utils）
  plugins/        ← 前端插件（context_menu、input_dialog）
src-tauri/        ← Rust 后端（Tauri 2）
  src/
    commands/     ← Tauri invoke 命令模块
    services/     ← 业务逻辑（SQLite、clipboard、sync 等）
    windows/      ← 窗口生命周期（tray、main、quickpaste 等）
  plugins/        ← Rust 插件（部分为私有：gpu-image-viewer、screenshot-suite）
```

**各层详细规范：** 参见 `src/AGENTS.md`、`src/shared/AGENTS.md`、`src-tauri/src/AGENTS.md`。  
**扩展文档：** 参见 `docs/常用命令.md`、`docs/工作树配置.md`（注：`docs/` 在 `.gitignore` 中，需本地生成，不在 git 中分发）。

## 环境要求

- Node.js ≥ 16（`package.json` 中未声明 `engines` 字段，此为推荐值）
- Rust ≥ 1.70 + `tauri-cli`
- Git + SSH 密钥（拉取私有子模块时需要）

## 常用命令

```bash
npm install                        # 安装依赖
git submodule update --init --recursive  # 初始化子模块（含私有插件，需要 SSH 密钥）

npm run tauri dev                  # 开发模式（完整版，含私有插件）
npm run tauri:dev:community        # 开发模式（社区版，不含私有插件）
npm run tauri:dev:no-watch         # 开发模式（禁用文件监听）

npm run tauri:build                # 正式构建（完整版）
npm run tauri:build:community      # 正式构建（社区版）
```

`npm run dev` / `npm run build` 仅运行 Vite（不含 Tauri）。除非只需要前端打包，否则使用 `tauri dev` / `tauri build`。

开发时也可用 `bun` 代替 `npm`（`bun install` / `bun run tauri dev`），速度更快。但**正式构建必须用 `npm`** — bun 的 symlink 扁平化结构会导致 Vite 构建产物在 transparent WebView2 下触发渲染异常。

## 私有版与社区版

私有插件（`gpu-image-viewer`、`screenshot-suite`）不在仓库中。社区版构建脚本（`scripts/community-build.js`）会临时修改 `src-tauri/Cargo.toml` 移除私有依赖行，构建后自动恢复。关键环境变量：`QC_COMMUNITY=1`、`QC_NO_SCREENSHOT=1`。

`Cargo.toml` 中的 feature 标志：
- `default = ["gpu-image-viewer", "screenshot-suite"]` — 完整构建
- 社区构建通过 `--no-default-features` 禁用私有 feature（由 `scripts/community-build.js` 执行）
- `gpu-image-viewer = ["dep:gpu-image-viewer"]` — 私有 GPU 图片查看器
- `screenshot-suite = ["dep:screenshot-suite"]` — 私有截图插件
- `screenshot-suite` 在 `lib.rs` 的 `#[cfg(feature = "screenshot-suite")]` 块中统一使用

> ⚠️ `lib.rs` 中没有 `compile_error!` 来阻止 `screenshot-suite` 与社区版截图插件共存，仅靠构建流程保证互斥。

## 私有仓库拉取

`screenshot-suite` 是 git 子模块，通过 SSH 拉取（`git@github.com:mosheng1/screenshot-suite.git`）。拉取需要：

1. **SSH 密钥** — 已配置 GitHub SSH key
2. **仓库访问权限** — 对私有仓库 `mosheng1/screenshot-suite` 有读取权限

如果无法拉取私有子模块，使用社区版构建（`npm run tauri:dev:community`）即可绕过，不依赖私有插件。

`gpu-image-viewer` 同样是私有插件，不在仓库中，`Cargo.toml` 中通过 `optional = true` 声明（完整构建时启用，社区构建通过 `--no-default-features` 排除）。

## 添加功能

**新增 Tauri 窗口（前端）：**
1. 创建 `src/windows/<name>/` 目录，含 `index.html`、`index.jsx`、`App.jsx`
2. 在 `vite.config.js` → `rollupOptions.input` 中添加条目
3. 在 `src-tauri/src/windows/` 中创建 Rust 窗口管理器，使用 `WebviewWindowBuilder` 动态创建窗口
4. 在 `src-tauri/capabilities/<name>.json` 中添加 capabilities 文件
5. 在 `src-tauri/tauri.conf.json` → `app.security.capabilities` 中注册 capability 名称

> ⚠️ 多数窗口由 Rust 代码**动态创建**（`WebviewWindowBuilder::new()`），不出现在 `tauri.conf.json` 的 `app.windows` 数组中。`app.windows` 中仅 `main` 窗口（因其需要特殊的窗口属性如透明、无边框、总在最上等）。

**新增 Tauri 命令（后端）：**
1. 创建 `commands/<name>.rs`，实现 `#[tauri::command]` 函数
2. 在 `commands/mod.rs` 中注册：`pub mod <name>` + `pub use <name>::*`
3. 添加到 `lib.rs` 的 `generate_handler` 列表中
4. 添加到对应的 capabilities JSON

**漏掉任何一步 → `invoke()` 静默失败（无编译错误）。**

## 关键约束

- **禁止 TypeScript** — `src/` 下只能使用纯 JS/JSX
- **UnoCSS global 模式** — 使用 utility class，不用 CSS Modules
- **Valtio** — 唯一的状态管理方案（禁止 Redux/Zustand/MobX）
- **i18n** — `zh-CN.json` 和 `en-US.json` 必须同步更新
- **dompurify** — 所有用户 HTML 在渲染前必须净化
- **SQLite** — 单连接 + `Mutex`，WAL 模式，不支持并发写入
- **React Compiler** — 通过 `babel-plugin-react-compiler` 激活，遵循 Rules of React
- **Vite alias** — `@` → `src/`、`@shared` → `src/shared/`、`@windows` → `src/windows/`
- `.gitignore` 排除项：`src/windows/screenshot/`、`src-tauri/plugins/gpu-image-viewer/`、`docs/`、`.tauri/`

## 测试与验证

仓库根 `package.json` 无测试脚本。截图插件（`src-tauri/plugins/screenshot-suite/`，私有子模块）内含 Rust 测试和前端测试（Vitest + Playwright），需拉取子模块后可用。

验证方式：
- **Rust 编译**: `cargo check` / `cargo build`
- **前端编译**: `npm run build`（仅 Vite）
- **截图插件测试**: `cd src-tauri/plugins/screenshot-suite && cargo test`（需子模块已拉取且初始化）
