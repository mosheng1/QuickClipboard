import { invoke } from '@tauri-apps/api/core'

// 播放复制音效
export async function playCopySound() {
  return await invoke('play_copy_sound')
}

// 播放粘贴音效
export async function playPasteSound() {
  return await invoke('play_paste_sound')
}

// 播放滚动音效
export async function playScrollSound() {
  return await invoke('play_scroll_sound')
}

