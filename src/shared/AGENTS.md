# src/shared/ — 共享模块

跨窗口复用的前端代码。本项目所有共享逻辑在此。

## 结构

```
shared/
├── api/              # 前后端通信封装（invoke 包装器）
├── components/       # 共享 UI 组件
├── config/           # 全局配置常量
├── constants/        # 常量定义（如 tabVisibility）
├── hooks/            # 共享 React hooks
├── i18n.js           # i18next 初始化（fallback: zh-CN）
├── locales/          # zh-CN.json + en-US.json（两个文件必须同步）
├── services/         # 前端业务逻辑
├── store/            # Valtio proxy 状态
├── styles/           # UnoCSS 全局样式
└── utils/            # 工具函数
```

## i18n 协议（强制）

```jsx
// 组件中使用
import { useTranslation } from 'react-i18next'
const { t } = useTranslation()
;<span>{t('key.nested.path')}</span>
```

- **fallback**: zh-CN
- **新增文本**: 同时在 `locales/zh-CN.json` 和 `locales/en-US.json` 中添加
- **禁止**: 只在单一 locale 中添加 key

## Valtio store

```js
// store/clipboardStore.js
import { proxy } from 'valtio'
export const clipboardStore = proxy({ items: [], activeId: null })
```

所有全局状态用 Valtio `proxy`。禁止 Redux / Zustand / MobX。

## Import alias

- `@shared/*` → `src/shared/*`
- 窗口代码可 import 此目录下的模块

## 反模式

- ❌ 在此处使用 TypeScript
- ❌ 使用 Redux / Zustand 替代 Valtio
- ❌ 在 i18n 中忽略 zh-CN fallback 规则
