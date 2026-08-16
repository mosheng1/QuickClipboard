import { invoke } from '@tauri-apps/api/core'

// 重新加载设置
export async function reloadSettings() {
  return await invoke('reload_settings')
}

// 保存设置
export async function saveSettings(settings) {
  return await invoke('save_settings', { settings })
}

// 重置设置为默认
export async function resetSettingsToDefault() {
  return await invoke('reset_settings_to_default')
}

// 设置边缘隐藏
export async function setEdgeHideEnabled(enabled) {
  return await invoke('set_edge_hide_enabled', { enabled })
}

// 获取所有窗口信息
export async function getAllWindowsInfo() {
  return await invoke('get_all_windows_info_cmd')
}

// 设置开机自启动
export async function setAutoStart(enabled) {
  return await invoke('set_auto_start', { enabled })
}

// 获取开机自启动状态
export async function getAutoStartStatus() {
  return await invoke('get_auto_start_status')
}

// 设置管理员权限运行
export async function setRunAsAdmin(enabled) {
  return await invoke('set_run_as_admin', { enabled })
}

// 获取管理员权限运行状态
export async function getRunAsAdminStatus() {
  return await invoke('get_run_as_admin_status')
}

// 检查当前是否以管理员权限运行
export async function isRunningAsAdmin() {
  return await invoke('is_running_as_admin')
}

// 以管理员权限重启程序
export async function restartAsAdmin() {
  return await invoke('restart_as_admin')
}

// 获取所有快捷键状态
export async function getShortcutStatuses() {
  return await invoke('get_shortcut_statuses')
}

// 保存窗口位置
export async function saveWindowPosition(x, y) {
  return await invoke('save_window_position', { x, y })
}

// 保存窗口大小
export async function saveWindowSize(width, height) {
  return await invoke('save_window_size', { width, height })
}

export async function getUpdateBannerState() {
  return await invoke('get_update_banner_state')
}

export async function getOneTimePasteEnabledFromStore() {
  return await invoke('get_one_time_paste_enabled')
}

export async function setOneTimePasteEnabledToStore(enabled) {
  return await invoke('set_one_time_paste_enabled', { enabled })
}

