import { proxy } from 'valtio'
import { 
  defaultSettings, 
  loadSettingsFromBackend, 
  saveSettingsToBackend 
} from '@shared/services/settingsService'

const storeDefaultSettings = {
  ...defaultSettings,
  adaptiveMaxHeight: 350,
  fileIconSize: 'large'
}

// 设置 Store
export const settingsStore = proxy({
  ...storeDefaultSettings,
  // UI 专属设置（localStorage）
  fontSize: 14,
  footerLeftRatio: 0.5,
  
  // 系统主题状态
  systemIsDark: typeof window !== 'undefined' && window.matchMedia
    ? window.matchMedia('(prefers-color-scheme: dark)').matches
    : false,

  // 自定义字体加载状态
  customFontStatus: 'idle',
  
  // 加载设置
  async loadSettings() {
    const settings = await loadSettingsFromBackend()
    const loadedSettings = {
      ...storeDefaultSettings,
      ...settings,
      adaptiveMaxHeight: settings.adaptiveMaxHeight ?? settings.adaptive_max_height ?? storeDefaultSettings.adaptiveMaxHeight,
      fileIconSize: settings.fileIconSize ?? settings.file_icon_size ?? storeDefaultSettings.fileIconSize
    }
    
    // 更新所有设置到 store
    Object.keys(loadedSettings).forEach(key => {
      if (key in this && key !== 'loadSettings' && key !== 'saveSetting' && key !== 'saveSettings' && key !== 'saveAllSettings' && key !== 'updateSettings') {
        this[key] = loadedSettings[key]
      }
    })
    
    return loadedSettings
  },
  
  // 保存单个设置项
  async saveSetting(key, value, options = {}) {
    this[key] = value
    
    // 收集当前所有设置
    const currentSettings = this.getAllSettings()
    const result = await saveSettingsToBackend(currentSettings, options)
    
    return result
  },

  // 批量保存多个设置项
  async saveSettings(updates, options = {}) {
    this.updateSettings(updates)

    const currentSettings = this.getAllSettings()
    const result = await saveSettingsToBackend(currentSettings, options)

    return result
  },
  
  // 保存所有设置
  async saveAllSettings() {
    const currentSettings = this.getAllSettings()
    return await saveSettingsToBackend(currentSettings)
  },
  
  // 批量更新设置（不保存）
  updateSettings(updates) {
    Object.keys(updates).forEach(key => {
      if (key in this && key !== 'loadSettings' && key !== 'saveSetting' && key !== 'saveSettings' && key !== 'saveAllSettings' && key !== 'updateSettings' && key !== 'getAllSettings') {
        this[key] = updates[key]
      }
    })
  },
  
  // 获取所有设置（排除方法）
  getAllSettings() {
    const settings = {}
    Object.keys(storeDefaultSettings).forEach(key => {
      if (key in this) {
        settings[key] = this[key]
      }
    })
    return settings
  },
  
  // 主题设置
  setTheme(theme) {
    this.saveSetting('theme', theme)
  },

  // 亮色主题风格设置
  setLightThemeStyle(style) {
    this.saveSetting('lightThemeStyle', style)
  },
  
  // 暗色主题风格设置
  setDarkThemeStyle(style) {
    this.saveSetting('darkThemeStyle', style)
  },
  
  // 语言设置
  async setLanguage(lang) {
    await this.saveSetting('language', lang)
  },
  
  // 字体大小
  setFontSize(size) {
    this.fontSize = size
    localStorage.setItem('fontSize', String(size))
  },
  
  // 底部栏左侧占比
  setFooterLeftRatio(ratio) {
    this.footerLeftRatio = ratio
    localStorage.setItem('footerLeftRatio', String(ratio))
  },
  
  // 粘贴格式
  setPasteWithFormat(withFormat) {
    this.saveSetting('pasteWithFormat', withFormat)
  }
})

// 初始化设置
export async function initSettings() {
  // 从 localStorage 恢复 UI 设置
  const fontSize = localStorage.getItem('fontSize')
  const footerLeftRatio = localStorage.getItem('footerLeftRatio')

  const legacyRowHeight = localStorage.getItem('rowHeight')
  const legacyFileDisplayMode = localStorage.getItem('fileDisplayMode')
  const legacyListStyle = localStorage.getItem('listStyle')
  
  if (fontSize) settingsStore.fontSize = parseInt(fontSize)
  if (footerLeftRatio) settingsStore.footerLeftRatio = parseFloat(footerLeftRatio)
  
  // 从后端加载所有配置
  await settingsStore.loadSettings()

  // 把旧 localStorage 值迁移到配置文件（仅当后端还是默认值时）
  const migrateIfNeeded = async (key, legacyValue) => {
    if (!legacyValue) return
    if (settingsStore[key] !== storeDefaultSettings[key]) return
    await settingsStore.saveSetting(key, legacyValue, { showToast: false })
    try { localStorage.removeItem(key) } catch (_) {}
  }
  await migrateIfNeeded('rowHeight', legacyRowHeight)
  await migrateIfNeeded('fileDisplayMode', legacyFileDisplayMode)
  await migrateIfNeeded('listStyle', legacyListStyle)
 
  if (settingsStore.language) {
    const i18n = (await import('@shared/i18n')).default
    await i18n.changeLanguage(settingsStore.language)
  }
}
