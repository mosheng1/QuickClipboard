import { invoke } from '@tauri-apps/api/core'

// 获取应用版本信息
export async function getAppVersion() {
  return await invoke('get_app_version')
}

// 检查是否为便携模式
export async function isPortableMode() {
  return await invoke('is_portable_mode')
}

// 复制文本
export async function copyTextToClipboard(text) {
  return await invoke('copy_text_to_clipboard', { text })
}

// OCR识别图片文件
export async function recognizeImageOcr(filePath, language = null) {
  return await invoke('recognize_file_ocr', { filePath, language })
}

export async function promptDisableWinVHotkeyIfNeeded() {
  return await invoke('prompt_disable_win_v_hotkey_if_needed')
}

export async function promptEnableWinVHotkey() {
  return await invoke('prompt_enable_win_v_hotkey')
}

