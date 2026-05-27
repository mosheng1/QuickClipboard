import js from '@eslint/js'
import reactPlugin from 'eslint-plugin-react'
import globals from 'globals'

export default [
  // 基础推荐规则
  js.configs.recommended,

  // React 插件配置
  {
    plugins: {
      react: reactPlugin,
    },
    rules: {
      ...reactPlugin.configs.recommended.rules,
      'react/react-in-jsx-scope': 'off',   // React 19 JSX Transform 不需要 import React
      'react/prop-types': 'off',            // 项目无 TypeScript/PropTypes，暂时关闭
    },
    settings: {
      react: {
        version: '19.0',
      },
    },
  },

  // 全局配置：文件匹配 + 语言环境
  {
    files: ['**/*.{js,jsx}'],
    languageOptions: {
      globals: {
        ...globals.browser,
        ...globals.node,
        ...globals.es2024,
      },
      parserOptions: {
        ecmaVersion: 'latest',
        sourceType: 'module',
        ecmaFeatures: { jsx: true },
      },
    },
    rules: {
      'no-undef': 'error',              // 项目规范：禁止未定义变量
      'no-unused-vars': ['warn', { argsIgnorePattern: '^_', varsIgnorePattern: '^_', caughtErrorsIgnorePattern: '^_' }],
      'no-console': 'off',              // Tauri 项目允许 console
    },
  },

  // 测试文件特殊规则
  {
    files: ['**/*.{test,spec}.{js,jsx}'],
    languageOptions: {
      globals: {
        ...globals.browser,
        ...globals.node,
        ...globals.jest,
        ...globals.vitest,
      },
    },
    rules: {
      'no-undef': 'off',                // vitest globals (describe, it, expect)
    },
  },

  // 忽略目录
  {
    ignores: [
      'dist/**',
      'node_modules/**',
      'src-tauri/target/**',
      '*.config.js',                     // 构建配置文件
    ],
  },
]
