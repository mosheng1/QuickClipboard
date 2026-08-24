import { invoke } from '@tauri-apps/api/core'
import { restoreLastFocus } from '@shared/api/window'
import { getOneTimePasteEnabled } from '@shared/services/oneTimePaste'

// 分页查询收藏列表
export async function getFavoritesHistory(params = {}) {
  const { offset = 0, limit = 50, groupName, search, contentType, pasteStatus } = params

  const invokeParams = { offset, limit }
  if (groupName && groupName !== '全部') invokeParams.groupName = groupName
  if (search) invokeParams.search = search
  if (contentType) invokeParams.contentType = contentType
  if (pasteStatus && pasteStatus !== 'all') invokeParams.pasteStatus = pasteStatus

  return await invoke('get_favorites_history', invokeParams)
}

// 获取收藏总数
export async function getFavoritesTotalCount(groupName = null) {
  return await invoke('get_favorites_total_count', {
    groupName: groupName === '全部' ? null : groupName,
  })
}

// 添加收藏
export async function addFavorite(title, content, groupName = '全部') {
  const result = await invoke('add_quick_text', { title, content, groupName })
  const item = await invoke('get_favorite_item_by_id_cmd', { id: result.id })
  await invoke('emit_quick_texts_updated', {
    payload: {
      kind: 'created',
      item,
      insert_index: 0,
    },
  })
  return result
}

// 更新收藏
export async function updateFavorite(id, title, content, groupName, htmlContent = undefined) {
  const params = { id, title, content, groupName }
  if (htmlContent !== undefined) {
    params.htmlContent = htmlContent
  }
  const result = await invoke('update_quick_text', params)
  await invoke('emit_quick_texts_updated', {
    payload: {
      kind: 'updated',
    },
  })
  return result
}

// 删除收藏
export async function deleteFavorite(id) {
  const result = await invoke('delete_quick_text', { id })
  const { clipboardStore } = await import('@shared/store/clipboardStore')
  clipboardStore.clearFavoriteIds([id])
  return result
}

export async function deleteFavoriteItems(ids) {
  const result = await invoke('delete_favorite_items', { ids })
  const { clipboardStore } = await import('@shared/store/clipboardStore')
  clipboardStore.clearFavoriteIds(ids)
  return result
}

// 移动收藏项位置（拖拽排序）
export async function moveFavoriteItem(fromId, toId) {
  return await invoke('move_favorite_item_cmd', {
    fromId,
    toId,
  })
}

// 粘贴收藏内容
export async function pasteFavorite(id, action = null) {
  try {
    await restoreLastFocus()
    const params = { favorite_id: id }
    if (action) {
      params.action = action
    }

    await invoke('paste_content', { params })

    // 一次性粘贴：粘贴后删除当前收藏
    if (getOneTimePasteEnabled()) {
      try {
        await deleteFavorite(id)
        const { refreshFavorites } = await import('@shared/store/favoritesStore')
        await refreshFavorites()
      } catch (deleteError) {
        console.error('一次性粘贴删除收藏失败:', deleteError)
      }
    }

    return true
  } catch (error) {
    console.error('粘贴收藏内容失败:', error)
    throw error
  }
}

// 复制收藏项内容（不记录到历史）
export async function copyFavoriteItem(id) {
  return await invoke('copy_favorite_item', { id })
}

export async function getFavoriteItemPasteOptions(id) {
  return await invoke('get_favorite_item_paste_options_cmd', { id })
}

export async function mergeCopyFavoriteItems(ids) {
  return await invoke('merge_copy_favorite_items', { ids })
}

export async function mergePasteFavoriteItems(ids) {
  return await invoke('merge_paste_favorite_items', { ids })
}
