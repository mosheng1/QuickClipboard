import {
  reloadSettings,
  saveSettings as saveSettingsApi,
  setEdgeHideEnabled as setEdgeHideEnabledApi,
  getAllWindowsInfo,
  hideMainWindowIfAutoShown,
  getAppVersion as getAppVersionApi,
  isPortableMode as isPortableModeApi
} from '@shared/api'
import { emit } from '@tauri-apps/api/event'
import { toast } from '@shared/store/toastStore'
import i18n from '@shared/i18n'

export const defaultSettings = {
  // 常规设置
  autoStart: false,
  runAsAdmin: false,
  startHidden: false,
  showTrayIcon: true,
  showStartupNotification: true,
  autoLowMemoryEnabled: false,
  autoLowMemoryIdleMinutes: 15,
  autoExitLowMemoryMode: false,
  historyLimit: 100,
  language: 'zh-CN',
  
  // 外观设置
  tooltipsEnabled: true,
  theme: 'light',
  lightThemeStyle: 'modern',
  darkThemeStyle: 'classic',
  customFontEnabled: false,
  customFontType: 'file',
  customFontPath: '',
  customFontUrl: '',
  customFontFamily: '',
  visibleOptionalTabs: ['favorites', 'emoji'],
  opacity: 0.9,
  backgroundImagePath: '',
  superBackgroundBlurScale: 1,
  clipboardAnimationEnabled: true,
  uiAnimationEnabled: true,

  // 列表外观
  listStyle: 'card', 
  rowHeight: 'medium',
  autoRowMaxLines: 18,
  fileDisplayMode: 'detailed', 
  cardSpacing: 8, 
  
  // 快捷键设置
  toggleShortcut: 'Shift+Space',
  openSettingsShortcut: '',
  quickpasteShortcut: 'Ctrl+`',
  transferShelfCreateShortcut: '',
  screenshotShortcut: 'Ctrl+Shift+A',
  numberShortcuts: true,
  numberShortcutsModifier: 'Ctrl',
  
  // 剪贴板窗口快捷键
  navigateUpShortcut: 'ArrowUp',
  navigateDownShortcut: 'ArrowDown',
  tabLeftShortcut: 'ArrowLeft',
  tabRightShortcut: 'ArrowRight',
  focusSearchShortcut: 'Tab',
  hideWindowShortcut: 'Escape',
  pasteItemShortcut: 'Enter',
  previousGroupShortcut: 'Ctrl+ArrowUp',
  nextGroupShortcut: 'Ctrl+ArrowDown',
  togglePinShortcut: 'Ctrl+P',
  toggleClipboardMonitorShortcut: '',
  togglePasteWithFormatShortcut: '',
  toggleLowMemoryModeShortcut: '',
  pastePlainTextShortcut: '',
  
  // 剪贴板设置
  clipboardMonitor: true,
  ignoreDuplicates: true,
  saveImages: true,
  imagePreview: true,
  textPreview: true,
  filePreview: true,
  settingsMigrationVersion: 1,
  displayPriorityOrder: 'text,html,image',
  autoScrollToTopOnShow: false,
  autoClearSearch: false,
  windowPositionMode: 'smart',
  rememberWindowSize: true,
  titleBarPosition: 'top',
  edgeHideEnabled: true,
  edgeSnapPosition: null,
  edgeHideOffset: 3,
  autoFocusSearch: false,
  pasteWithFormat: true,
  pasteShortcutMode: 'ctrl_v',
  modifierClickMultiSelect: true,
  pasteToTop: false,
  showListShortcuts: true,
  showListIndex: true,
  showBadges: true,
  showSourceIcon: true,
  updateCheckInterval: 'daily',
  disableUpdatePopup: false,
  includeBetaUpdates: null,
  
  // 音效设置
  soundEnabled: true,
  soundVolume: 50,
  copySoundPath: '',
  pasteSoundPath: '',
  copySoundTiming: 'success',  
  pasteSoundTiming: 'success', 

  // 图片显示限制
  imageMaxSizeMb: 15,
  imageMaxWidth: 4096,
  imageMaxHeight: 4096,
  
  // 便捷粘贴设置
  quickpasteEnabled: true,
  quickpastePasteOnModifierRelease: true,
  quickpasteScrollSound: true,
  quickpasteScrollSoundPath: 'sounds/roll.mp3',
  
  // 截屏设置
  screenshotEnabled: true,
  screenshotShortcut: 'Ctrl+Shift+A',
  screenshotQuickSaveShortcut: '',
  screenshotQuickPinShortcut: '',
  screenshotQuickOcrShortcut: '',
  screenshotQuality: 85,
  screenshotAutoSave: false,
  screenshotShowHints: true,
  screenshotElementDetection: 'all',
  screenshotMagnifierEnabled: true,
  screenshotHintsEnabled: true,
  screenshotColorIncludeFormat: true,
  screenshotWindowLifecycleMode: 'quick',
  screenshotAutoDisposeMinutes: 10,
  
  // AI 配置
  aiTranslationEnabled: false,
  aiApiKey: '',
  aiModel: 'Qwen/Qwen2-7B-Instruct',
  aiBaseUrl: 'https://api.siliconflow.cn/v1',
  aiTargetLanguage: 'auto',
  aiTranslateOnCopy: false,
  aiTranslateOnPaste: true,
  aiTranslationPrompt: '请将以下文本翻译成{target_language}，严格保持原文的所有格式、换行符、段落结构和空白字符，只返回翻译结果，不要添加任何解释或修改格式：',
  aiInputSpeed: 50,
  aiNewlineMode: 'auto',
  aiOutputMode: 'stream',
  
  // 鼠标设置
  mouseMiddleButtonEnabled: false,
  mouseMiddleButtonModifier: 'None',
  mouseMiddleButtonTrigger: 'short_press',
  mouseMiddleButtonLongPressMs: 300,
  
  // 应用过滤
  appFilterEnabled: false,
  appFilterBlocklist: [],
  appFilterEffect: 'clipboard_only',
  
  // 保存的窗口状态
  savedWindowPosition: null,
  savedWindowSize: null,
  
  // 数据存储设置
  customStoragePath: null,
  useCustomStorage: false,

  // WebDAV Sync 设置
  webdavEnabled: false,
  webdavUrl: '',
  webdavUsername: '',
  webdavPassword: '',
  webdavRootPath: 'quickclipboard',
  webdavAutoPush: false,
  webdavPushDelaySecs: 10,
  webdavAutoPull: false,
  webdavAutoPullOnWindowShow: false,
  webdavPullIntervalSecs: 30,
  webdavPushShortcut: '',
  webdavPullShortcut: '',
  webdavSyncClipboard: true,
  webdavSyncFavorites: true,
  webdavSyncImages: false,
  syncTransferActiveMode: 'webdav',

}

// 加载设置
export async function loadSettingsFromBackend() {
  try {
    const savedSettings = await reloadSettings()

    const mergedSettings = { ...defaultSettings, ...savedSettings }
    
    return mergedSettings
  } catch (error) {
    console.error('加载设置失败:', error)
    return { ...defaultSettings }
  }
}

// 保存设置
export async function saveSettingsToBackend(settings, options = {}) {
  const { showToast = true } = options
  
  try {
    await saveSettingsApi(settings)
    
    await emit('settings-changed', settings)
    
    if (showToast) {
      toast.success(i18n.t('settings.saved'))
    }
    return { success: true }
  } catch (error) {
    console.error('保存设置失败:', error)
    if (showToast) {
      toast.error(i18n.t('settings.saveFailed'))
    }
    return { success: false, error: error.message }
  }
}

// 保存单个设置项
export async function saveSingleSetting(key, value, allSettings) {
  const updatedSettings = { ...allSettings, [key]: value }
  return await saveSettingsToBackend(updatedSettings)
}

// 获取应用版本
export async function getAppVersion() {
  try {
    const versionInfo = await getAppVersionApi()
    return versionInfo
  } catch (error) {
    console.error('获取版本信息失败:', error)
    return { version: '未知' }
  }
}


// 检查是否为便携版模式
export async function isPortableMode() {
  try {
    return await isPortableModeApi()
  } catch (error) {
    console.error('检查便携版模式失败:', error)
    return false
  }
}

// 设置贴边隐藏
export async function setEdgeHideEnabled(enabled) {
  try {
    await setEdgeHideEnabledApi(enabled)
    return { success: true }
  } catch (error) {
    console.error('更新贴边隐藏设置失败:', error)
    return { success: false, error: error.message }
  }
}

// 获取所有窗口信息（用于应用过滤）
export async function getAllWindowsInfoService() {
  try {
    return await getAllWindowsInfo()
  } catch (error) {
    console.error('获取应用列表失败:', error)
    return []
  }
}

// 隐藏主窗口
export async function hideMainWindowIfAutoShownService() {
  try {
    await hideMainWindowIfAutoShown()
  } catch (error) {
    console.error('隐藏主窗口失败:', error)
  }
}

