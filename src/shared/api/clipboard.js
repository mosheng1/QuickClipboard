import { invoke } from '@tauri-apps/api/core'
import { restoreLastFocus } from '@shared/api/window'
import { getOneTimePasteEnabled } from '@shared/services/oneTimePaste'
// 获取剪贴板历史列表
export async function getClipboardHistory(params = {}) {
  try {
    const { offset = 0, limit = 50, search, contentType } = params

    const invokeParams = { offset, limit }
    if (search) invokeParams.search = search
    if (contentType) invokeParams.contentType = contentType

    return await invoke('get_clipboard_history', invokeParams)
  } catch (error) {
    console.error('获取剪贴板历史失败:', error)
    return {
      total_count: 0,
      items: [],
      offset: 0,
      limit: 50,
      has_more: false
    }
  }
}

// 获取剪贴板总数
export async function getClipboardTotalCount() {
  try {
    return await invoke('get_clipboard_total_count')
  } catch (error) {
    console.error('获取剪贴板总数失败:', error)
    return 0
  }
}

// 粘贴剪贴板项
export async function pasteClipboardItem(clipboardId, action = null) {
  try {
    await restoreLastFocus()
    const params = { clipboard_id: clipboardId }
    if (action) {
      params.action = action
    }

    await invoke('paste_content', { params })

    // 一次性粘贴：粘贴后删除当前记录
    if (getOneTimePasteEnabled()) {
      try {
        await deleteClipboardItem(clipboardId)
        const { loadClipboardItems } = await import('@shared/store/clipboardStore')
        await loadClipboardItems()
      } catch (deleteError) {
        console.error('一次性粘贴删除剪贴板记录失败:', deleteError)
      }
    }

    return true
  } catch (error) {
    console.error('粘贴失败:', error)
    throw error
  }
}

// 删除剪贴板项
export async function deleteClipboardItem(id) {
  try {
    await invoke('delete_clipboard_item', { id })
    return true
  } catch (error) {
    console.error('删除剪贴板项失败:', error)
    throw error
  }
}

export async function deleteClipboardItems(ids) {
  try {
    await invoke('delete_clipboard_items', { ids })
    return true
  } catch (error) {
    console.error('批量删除剪贴板项失败:', error)
    throw error
  }
}

// 清空剪贴板历史
export async function clearClipboardHistory() {
  try {
    await invoke('clear_clipboard_history')
    return true
  } catch (error) {
    console.error('清空剪贴板历史失败:', error)
    throw error
  }
}

// 移动剪贴板项到顶部
export async function moveClipboardItemToTop(id) {
  try {
    await invoke('move_clipboard_item', { id })
    return true
  } catch (error) {
    console.error('移动剪贴板项到顶部失败:', error)
    throw error
  }
}

// 移动剪贴板项（按 ID，用于搜索/筛选时）
export async function moveClipboardItemById(fromId, toId) {
  try {
    await invoke('move_clipboard_item_by_id', { fromId, toId })
    return true
  } catch (error) {
    console.error('移动剪贴板项失败:', error)
    throw error
  }
}

// 添加到常用文本
export async function addToFavorites(id) {
  try {
    const result = await invoke('add_clipboard_to_favorites', { id })
    const favoriteItem = await getFavoriteItemById(result.id)
    await invoke('emit_quick_texts_updated', {
      payload: {
        kind: 'created',
        item: favoriteItem,
        insert_index: 0,
      },
    })
    return true
  } catch (error) {
    console.error('添加到常用文本失败:', error)
    throw error
  }
}


// 检查文件是否存在
export async function checkFileExists(path) {
  try {
    return await invoke('file_exists', { path })
  } catch (error) {
    console.warn(`检查文件是否存在失败: ${path}`, error)
    return false
  }
}

// 打开文本编辑器
export async function openTextEditor() {
  try {
    await invoke('open_text_editor_window')
    return true
  } catch (error) {
    console.error('打开文本编辑器失败:', error)
    throw error
  }
}

// 贴图片到屏幕
export async function pinImageToScreen(filePath) {
  try {
    await invoke('create_native_pin_from_file', { filePath })
    return true
  } catch (error) {
    console.error('原生贴图失败，回退到 Tauri WebView 版:', error)
    try {
      await invoke('pin_image_from_file', { filePath })
      return true
    } catch (fallbackError) {
      console.error('贴图到屏幕失败（原生版和Tauri版均失败）:', error, fallbackError)
      const combinedError = new Error(`贴图到屏幕失败: 原生版 - ${error?.message || error}; Tauri WebView 版 - ${fallbackError?.message || fallbackError}`)
      throw combinedError
    }
  }
}

// 保存图片到文件
export async function saveImageToFile(content, filePath) {
  try {
    await invoke('save_image_to_file', { content, filePath })
    return true
  } catch (error) {
    console.error('保存图片失败:', error)
    throw error
  }
}


// 获取单个剪贴板项
export async function getClipboardItemById(id, maxLength = null) {
  const params = { id };
  if (maxLength !== null) {
    params.max_length = maxLength;
  }
  return await invoke('get_clipboard_item_by_id_cmd', params);
}

export async function getClipboardItemPasteOptions(id) {
  return await invoke('get_clipboard_item_paste_options_cmd', { id })
}

// 更新剪贴板项
export async function updateClipboardItem(id, content, htmlContent = undefined) {
  const params = { id, content }
  if (htmlContent !== undefined) {
    params.htmlContent = htmlContent
  }
  await invoke('update_clipboard_item_cmd', params)
  await invoke('emit_clipboard_updated', {
    payload: {
      kind: 'updated',
    },
  })
}

// 获取单个收藏项
export async function getFavoriteItemById(id, maxLength = null) {
  const params = { id };
  if (maxLength !== null) {
    params.max_length = maxLength;
  }
  return await invoke('get_favorite_item_by_id_cmd', params);
}

// 添加剪贴板项到收藏
export async function addClipboardToFavorites(id, groupName) {
  const result = await invoke('add_clipboard_to_favorites', { id, groupName })
  const favoriteItem = await getFavoriteItemById(result.id)
  await invoke('emit_quick_texts_updated', {
    payload: {
      kind: 'created',
      item: favoriteItem,
      insert_index: 0,
    },
  })
}


// 另存图片
export async function saveImageFromPath(filePath) {
  return await invoke('save_image_from_path', { filePath })
}

// 复制剪贴板项内容（不记录到历史）
export async function copyClipboardItem(id) {
  return await invoke('copy_clipboard_item', { id })
}

export async function mergeCopyClipboardItems(ids) {
  return await invoke('merge_copy_clipboard_items', { ids })
}

export async function mergePasteClipboardItems(ids) {
  return await invoke('merge_paste_clipboard_items', { ids })
}

// 切换剪贴板项置顶状态
export async function togglePinClipboardItem(id) {
  const isPinned = await invoke('toggle_pin_clipboard_item', { id })
  await invoke('emit_clipboard_updated', {
    payload: {
      kind: 'updated',
    },
  })
  return isPinned
}
