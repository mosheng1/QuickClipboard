﻿// 主入口文件 - 协调各个模块

// =================== 启动横幅 ===================
function printStartupBanner() {
  console.log('');
  console.log('███╗   ███╗ ██████╗ ███████╗██╗  ██╗███████╗███╗   ██╗ ██████╗ ');
  console.log('████╗ ████║██╔═══██╗██╔════╝██║  ██║██╔════╝████╗  ██║██╔════╝ ');
  console.log('██╔████╔██║██║   ██║███████╗███████║█████╗  ██╔██╗ ██║██║  ███╗');
  console.log('██║╚██╔╝██║██║   ██║╚════██║██╔══██║██╔══╝  ██║╚██╗██║██║   ██║');
  console.log('██║ ╚═╝ ██║╚██████╔╝███████║██║  ██║███████╗██║ ╚████║╚██████╔╝');
  console.log('╚═╝     ╚═╝ ╚═════╝ ╚══════╝╚═╝  ╚═╝╚══════╝╚═╝  ╚═══╝ ╚═════╝ ');
  console.log('');
  console.log('QuickClipboard v1.0.0 - 快速剪贴板管理工具');
  console.log('Author: MoSheng | Frontend: JavaScript + Vite');
  console.log('Main window initializing...');
  console.log('');
}

import { initThemeManager } from './js/themeManager.js';

import {
  initDOMReferences,
  setCurrentFilter,
  setCurrentQuickTextsFilter,
  setIsOneTimePaste,
  searchInput,
  contentFilter,
  quickTextsSearch,
  quickTextsFilter,
  oneTimePasteSwitch
} from './js/config.js';

import {
  initAiTranslation
} from './js/aiTranslation.js';

import {
  refreshClipboardHistory,
  filterClipboardItems
} from './js/clipboard.js';

import {
  refreshQuickTexts,
  filterQuickTexts,
  setupQuickTexts
} from './js/quickTexts.js';



import {
  setupTabSwitching,
  setupConfirmModal,
  setupAlertModal
} from './js/ui.js';

import {
  setupClipboardEventListener,
  setupTrayEventListeners,
  setupKeyboardShortcuts,
  setupContextMenuDisable
} from './js/events.js';

import { initSortable } from './js/sortable.js';
import { initInputFocusManagement } from './js/focus.js';
import { setupWindowControls } from './js/window.js';
import { initGroups } from './js/groups.js';
import { initScreenshot } from './js/screenshot.js';
import {
  initializeSettingsManager,
  initializeTheme,
  setupThemeListener,
  updateShortcutDisplay
} from './js/settingsManager.js';
document.addEventListener('contextmenu', function (e) {
  e.preventDefault();
});
// 等待后端初始化完成
async function waitForBackendInitialization() {
  let attempts = 0;
  const maxAttempts = 50; // 最多等待5秒

  while (attempts < maxAttempts) {
    try {
      const isInitialized = await invoke('is_backend_initialized');
      if (isInitialized) {
        return;
      }
    } catch (error) {
      // 静默处理错误
    }

    // 等待100ms后重试
    await new Promise(resolve => setTimeout(resolve, 100));
    attempts++;
  }
}

// 初始化应用
async function initApp() {

  // 等待后端初始化完成，然后获取数据
  await waitForBackendInitialization();

  // 输出启动横幅
  printStartupBanner();

  // 初始化DOM元素引用
  initDOMReferences();

  // 初始化设置管理器
  await initializeSettingsManager();

  // 更新快捷键显示
  updateShortcutDisplay();

  // 初始化主题管理器（必须等待完成）
  await initThemeManager();

  // 初始化主题（同步主题管理器的状态）
  initializeTheme();

  // 设置主题监听器
  setupThemeListener();

  // 初始化分组功能（必须在常用文本之前）
  await initGroups();

  // 获取剪贴板历史
  await refreshClipboardHistory();
  // 获取常用文本
  await refreshQuickTexts();

  // 设置搜索功能
  searchInput.addEventListener('input', filterClipboardItems);
  quickTextsSearch.addEventListener('input', filterQuickTexts);

  // 设置筛选功能
  contentFilter.addEventListener('change', (e) => {
    setCurrentFilter(e.target.value);
    filterClipboardItems();
  });

  quickTextsFilter.addEventListener('change', (e) => {
    setCurrentQuickTextsFilter(e.target.value);
    filterQuickTexts();
  });

  // 设置一次性粘贴开关
  if (oneTimePasteSwitch) {
    oneTimePasteSwitch.addEventListener('change', (e) => {
      setIsOneTimePaste(e.target.checked);
    });
  }

  // 初始化AI翻译功能
  await initAiTranslation();

  // 设置标签页切换
  setupTabSwitching();

  // 设置常用文本功能
  setupQuickTexts();

  // 设置UI模态框
  setupConfirmModal();
  setupAlertModal();



  // 设置窗口控制按钮
  setupWindowControls();

  // 监听剪贴板变化事件
  setupClipboardEventListener();

  // 监听托盘事件
  setupTrayEventListeners();

  // 设置键盘快捷键
  // setupKeyboardShortcuts();

  // 初始化拖拽排序
  initSortable();

  // 初始化输入框焦点管理
  initInputFocusManagement();

  // 初始化截屏功能
  initScreenshot();

  // 设置右键菜单禁用
  setupContextMenuDisable();

  // 监听常用文本刷新事件
  window.addEventListener('refreshQuickTexts', refreshQuickTexts);

  // 监听分组变化事件
  window.addEventListener('groupChanged', refreshQuickTexts);

  // 设置窗口可见性监听器
  setupWindowVisibilityListener();
}

// 设置窗口可见性监听器
function setupWindowVisibilityListener() {
  // 监听页面可见性变化
  document.addEventListener('visibilitychange', () => {
    updateShortcutDisplay();
    if (!document.hidden) {
      // 页面变为可见时，更新快捷键显示
      updateShortcutDisplay();
    }
  });

  // 监听窗口焦点事件
  window.addEventListener('focus', () => {
    // 窗口获得焦点时，更新快捷键显示
    updateShortcutDisplay();
  });
}

// 页面加载完成后初始化
window.addEventListener('DOMContentLoaded', () => {
  // 初始化应用
  initApp();
});

