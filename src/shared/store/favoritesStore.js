import { proxy } from 'valtio'
import { listen } from '@tauri-apps/api/event'
import { 
  getFavoritesHistory,
  getFavoritesTotalCount,
  deleteFavorite as apiDeleteFavorite,
  pasteFavorite as apiPasteFavorite
} from '@shared/api/favorites'

listen('favorite-paste-count-updated', (event) => {
  const id = event.payload
  for (const key of Object.keys(favoritesStore.items)) {
    const item = favoritesStore.items[key]
    if (item && item.id === id) {
      favoritesStore.items[key] = { ...item, paste_count: (item.paste_count || 0) + 1 }
      break
    }
  }
})

const CACHE_WINDOW_SIZE = 120
const CACHE_BUFFER = 40
let favoritesRequestVersion = 0
let favoritesActiveGroupName = '全部'

function normalizeFavoritesGroupName(groupName) {
  return groupName || '全部'
}

function nextFavoritesRequestVersion(groupName = favoritesActiveGroupName) {
  favoritesRequestVersion += 1
  favoritesActiveGroupName = normalizeFavoritesGroupName(groupName)
  return favoritesRequestVersion
}

function isFavoritesRequestCurrent(version, filter, contentType, pasteStatus, groupName) {
  return version === favoritesRequestVersion
    && favoritesStore.filter === filter
    && favoritesStore.contentType === contentType
    && favoritesStore.pasteStatus === pasteStatus
    && favoritesActiveGroupName === normalizeFavoritesGroupName(groupName)
}

// 收藏 Store
export const favoritesStore = proxy({
  items: {},
  totalCount: 0,
  filter: '',
  contentType: 'all',
  pasteStatus: 'all',
  selectedIds: new Set(),
  selectedEntries: [],
  isMultiSelectMode: false,
  selectionAnchorIndex: null,
  loading: false,
  error: null,
  loadingRanges: new Set(),
  currentViewRange: { start: 0, end: 50 }, 

  // 设置指定范围的数据
  setItemsInRange(startIndex, items) {
    items.forEach((item, offset) => {
      this.items[startIndex + offset] = item
    })
    this.trimCache()
  },
  
  updateViewRange(startIndex, endIndex) {
    const prev = this.currentViewRange
    if (Math.abs(prev.start - startIndex) > 10 || Math.abs(prev.end - endIndex) > 10) {
      this.currentViewRange = { start: startIndex, end: endIndex }
      this.trimCache()
    }
  },
  
  trimCache() {
    const itemCount = Object.keys(this.items).length
    if (itemCount <= CACHE_WINDOW_SIZE) return
    
    const { start, end } = this.currentViewRange
    const center = Math.floor((start + end) / 2)
    const keepStart = Math.max(0, center - CACHE_BUFFER)
    const keepEnd = Math.min(this.totalCount - 1, center + CACHE_BUFFER)
    
    for (const key of Object.keys(this.items)) {
      const index = parseInt(key, 10)
      if (index < keepStart || index > keepEnd) {
        delete this.items[key]
      }
    }
  },
  
  // 获取指定索引的项
  getItem(index) {
    return this.items[index]
  },
  
  // 检查指定索引是否已加载
  hasItem(index) {
    return index in this.items
  },
  
  // 添加新项到开头（新收藏内容）
  addItem(item) {
    this.items = {}
  },
  
  // 删除项
  removeItem(id) {
    this.removeItems([id])
  },

  removeItems(ids) {
    const removeIdSet = new Set(ids.filter(Boolean))
    if (!removeIdSet.size) {
      return
    }

    const entries = Object.entries(this.items)
      .map(([key, item]) => [parseInt(key, 10), item])
      .filter(([, item]) => item)
      .sort((a, b) => a[0] - b[0])

    let removedCount = 0
    const nextItems = {}
    for (const [index, item] of entries) {
      if (removeIdSet.has(item.id)) {
        removedCount += 1
        continue
      }
      nextItems[index - removedCount] = item
    }

    if (removedCount === 0) {
      return
    }

    this.items = nextItems
    this.totalCount = Math.max(0, this.totalCount - removedCount)

    if (this.selectionAnchorIndex != null) {
      this.selectionAnchorIndex = Math.max(0, this.selectionAnchorIndex - removedCount)
    }
  },

  moveLoadedItem(fromIndex, toIndex) {
    if (fromIndex === toIndex) {
      return true
    }

    const start = Math.min(fromIndex, toIndex)
    const end = Math.max(fromIndex, toIndex)
    const rangeItems = []

    for (let index = start; index <= end; index += 1) {
      if (!this.hasItem(index)) {
        return false
      }
      rangeItems.push(this.items[index])
    }

    const movedItem = rangeItems[fromIndex - start]
    if (!movedItem) {
      return false
    }

    rangeItems.splice(fromIndex - start, 1)
    rangeItems.splice(toIndex - start, 0, movedItem)

    rangeItems.forEach((item, offset) => {
      this.items[start + offset] = item
    })

    if (this.selectionAnchorIndex != null && this.selectionAnchorIndex >= start && this.selectionAnchorIndex <= end) {
      if (this.selectionAnchorIndex === fromIndex) {
        this.selectionAnchorIndex = toIndex
      } else if (fromIndex < toIndex && this.selectionAnchorIndex > fromIndex && this.selectionAnchorIndex <= toIndex) {
        this.selectionAnchorIndex -= 1
      } else if (fromIndex > toIndex && this.selectionAnchorIndex >= toIndex && this.selectionAnchorIndex < fromIndex) {
        this.selectionAnchorIndex += 1
      }
    }

    if (this.selectedEntries.length > 0) {
      this.selectedEntries = this.selectedEntries
        .map(entry => {
          if (entry.index === fromIndex) {
            return { ...entry, index: toIndex }
          }
          if (fromIndex < toIndex && entry.index > fromIndex && entry.index <= toIndex) {
            return { ...entry, index: entry.index - 1 }
          }
          if (fromIndex > toIndex && entry.index >= toIndex && entry.index < fromIndex) {
            return { ...entry, index: entry.index + 1 }
          }
          return entry
        })
    }

    return true
  },

  insertLoadedItemAt(item, insertIndex, totalCount) {
    if (!item || !Number.isInteger(insertIndex) || insertIndex < 0 || insertIndex >= totalCount) {
      return false
    }

    const nextItems = {}
    const entries = Object.entries(this.items)
      .map(([key, value]) => [parseInt(key, 10), value])
      .filter(([, value]) => value)
      .sort((a, b) => b[0] - a[0])

    for (const [index, value] of entries) {
      const nextIndex = index >= insertIndex ? index + 1 : index
      if (nextIndex < totalCount) {
        nextItems[nextIndex] = value
      }
    }

    nextItems[insertIndex] = item
    this.items = nextItems
    this.totalCount = totalCount

    if (this.selectionAnchorIndex != null && this.selectionAnchorIndex >= insertIndex) {
      this.selectionAnchorIndex += 1
    }

    if (this.selectedEntries.length > 0) {
      this.selectedEntries = this.selectedEntries
        .map(entry => (
          entry.index >= insertIndex
            ? { ...entry, index: entry.index + 1 }
            : entry
        ))
    }

    const { start, end } = this.currentViewRange
    if (insertIndex <= start) {
      this.currentViewRange = {
        start: start + 1,
        end: end + 1,
      }
    } else if (insertIndex <= end) {
      this.currentViewRange = {
        start,
        end: end + 1,
      }
    }

    return true
  },
  
  setFilter(value) {
    if (this.filter !== value) {
      nextFavoritesRequestVersion()
      this.filter = value
      this.items = {}
      this.loadingRanges = new Set()
      this.exitMultiSelectMode()
    }
  },
  
  setContentType(value) {
    if (this.contentType !== value) {
      nextFavoritesRequestVersion()
      this.contentType = value
      this.items = {}
      this.loadingRanges = new Set()
      this.exitMultiSelectMode()
    }
  },
  
  enterMultiSelectMode(entry = null) {
    if (!entry?.id) return false
    this.isMultiSelectMode = true
    this.replaceSelection([entry])
    this.selectionAnchorIndex = entry.index
    return true
  },

  exitMultiSelectMode() {
    this.isMultiSelectMode = false
    this.selectedEntries = []
    this.selectedIds = new Set()
    this.selectionAnchorIndex = null
  },

  setSelectionAnchorIndex(index) {
    this.selectionAnchorIndex = typeof index === 'number' ? index : null
  },

  normalizeSelectedEntry(entry) {
    return {
      id: entry.id,
      index: entry.index,
      contentType: entry.contentType,
    }
  },

  hasSelectedId(id) {
    return this.selectedEntries.some(entry => entry.id === id)
  },

  replaceSelection(entries) {
    const uniqueEntries = []
    const seenIds = new Set()

    for (const entry of entries) {
      if (!entry?.id || seenIds.has(entry.id)) continue
      seenIds.add(entry.id)
      uniqueEntries.push(this.normalizeSelectedEntry(entry))
    }

    this.selectedEntries = uniqueEntries
    this.selectedIds = new Set(uniqueEntries.map(entry => entry.id))
  },

  toggleSelectedEntry(entry) {
    const normalizedEntry = this.normalizeSelectedEntry(entry)
    const exists = this.selectedEntries.some(selected => selected.id === normalizedEntry.id)
    if (exists) {
      if (this.selectedEntries.length === 1) {
        this.exitMultiSelectMode()
      } else {
        this.replaceSelection(this.selectedEntries.filter(selected => selected.id !== normalizedEntry.id))
      }
      return
    }

    this.replaceSelection([...this.selectedEntries, normalizedEntry])
  },

  selectRange(entries) {
    this.replaceSelection(entries)
  },

  setPasteStatus(value) {
    if (this.pasteStatus !== value) {
      nextFavoritesRequestVersion()
      this.pasteStatus = value
      this.items = {}
      this.loadingRanges = new Set()
      this.exitMultiSelectMode()
    }
  },

  getSelectedIds() {
    return this.selectedEntries.map(entry => entry.id)
  },

  toggleSelect(id) {
    if (!id) return
    this.toggleSelectedEntry({
      id,
      index: Number.MAX_SAFE_INTEGER,
      contentType: 'text',
    })
  },
  
  clearSelection() {
    this.selectedEntries = []
    this.selectedIds = new Set()
    this.selectionAnchorIndex = null
  },
  
  clearAll() {
    this.items = {}
    this.totalCount = 0
    this.currentViewRange = { start: 0, end: 50 }
    this.exitMultiSelectMode()
  },
  
  // 记录正在加载的范围
  addLoadingRange(start, end) {
    this.loadingRanges.add(`${start}-${end}`)
  },
  
  // 移除加载中的范围
  removeLoadingRange(start, end) {
    this.loadingRanges.delete(`${start}-${end}`)
  },
  
  // 检查范围是否正在加载
  isRangeLoading(start, end) {
    return this.loadingRanges.has(`${start}-${end}`)
  },

  hasOverlappingLoadingRange(start, end) {
    for (const range of this.loadingRanges) {
      const [loadStart, loadEnd] = range.split('-').map(Number);
      if (start <= loadEnd && end >= loadStart) {
        return true;
      }
    }
    return false;
  }
})

// 加载指定范围的数据
export async function loadFavoritesRange(startIndex, endIndex, groupName = null, requestContext = null) {
  if (!groupName) {
    const { groupsStore } = await import('./groupsStore')
    groupName = groupsStore.currentGroup
  }

  const requestVersion = requestContext?.version ?? favoritesRequestVersion
  const requestFilter = requestContext?.filter ?? favoritesStore.filter
  const requestContentType = requestContext?.contentType ?? favoritesStore.contentType
  const requestPasteStatus = requestContext?.pasteStatus ?? favoritesStore.pasteStatus
  const requestGroupName = normalizeFavoritesGroupName(requestContext?.groupName ?? groupName)

  if (!isFavoritesRequestCurrent(requestVersion, requestFilter, requestContentType, requestPasteStatus, requestGroupName)) {
    return
  }

  if (favoritesStore.loading && !requestContext) {
    return
  }

  if (favoritesStore.isRangeLoading(startIndex, endIndex) || 
      favoritesStore.hasOverlappingLoadingRange(startIndex, endIndex)) {
    return
  }
  
  // 检查是否所有数据都已加载
  let allLoaded = true
  for (let i = startIndex; i <= endIndex; i++) {
    if (!favoritesStore.hasItem(i)) {
      allLoaded = false
      break
    }
  }
  
  if (allLoaded) {
    return
  }
  
  favoritesStore.addLoadingRange(startIndex, endIndex)
  
  try {
    const limit = endIndex - startIndex + 1
    const result = await getFavoritesHistory({
      offset: startIndex,
      limit,
      groupName: requestGroupName,
      contentType: requestContentType !== 'all' ? requestContentType : undefined,
      pasteStatus: requestPasteStatus,
      search: requestFilter || undefined
    })

    if (!isFavoritesRequestCurrent(requestVersion, requestFilter, requestContentType, requestPasteStatus, requestGroupName)) {
      return
    }
    
    // 将数据按索引存储
    favoritesStore.setItemsInRange(startIndex, result.items)
    
    // 更新总数
    if (result.total_count !== undefined) {
      favoritesStore.totalCount = result.total_count
    }
  } catch (err) {
    if (isFavoritesRequestCurrent(requestVersion, requestFilter, requestContentType, requestPasteStatus, requestGroupName)) {
      console.error(`加载范围 ${startIndex}-${endIndex} 失败:`, err)
      favoritesStore.error = err.message || '加载失败'
    }
  } finally {
    if (isFavoritesRequestCurrent(requestVersion, requestFilter, requestContentType, requestPasteStatus, requestGroupName)) {
      favoritesStore.removeLoadingRange(startIndex, endIndex)
    }
  }
}

// 初始化加载
export async function initFavorites(groupName = null) {
  if (!groupName) {
    const { groupsStore } = await import('./groupsStore')
    groupName = groupsStore.currentGroup
  }

  const requestGroupName = normalizeFavoritesGroupName(groupName)
  const requestVersion = nextFavoritesRequestVersion(requestGroupName)
  const requestFilter = favoritesStore.filter
  const requestContentType = favoritesStore.contentType
  const requestPasteStatus = favoritesStore.pasteStatus

  favoritesStore.loading = true
  favoritesStore.error = null
  
  try {
    favoritesStore.items = {}
    favoritesStore.loadingRanges = new Set()
    
    if (requestContentType !== 'all' || requestFilter || requestPasteStatus !== 'all') {
      const result = await getFavoritesHistory({
        offset: 0,
        limit: 50,
        groupName: requestGroupName,
        contentType: requestContentType !== 'all' ? requestContentType : undefined,
        pasteStatus: requestPasteStatus,
        search: requestFilter || undefined
      })

      if (!isFavoritesRequestCurrent(requestVersion, requestFilter, requestContentType, requestPasteStatus, requestGroupName)) {
        return
      }
      
      favoritesStore.totalCount = result.total_count
      favoritesStore.setItemsInRange(0, result.items)
    } else {
      const totalCount = await getFavoritesTotalCount(requestGroupName)

      if (!isFavoritesRequestCurrent(requestVersion, requestFilter, requestContentType, requestPasteStatus, requestGroupName)) {
        return
      }

      favoritesStore.totalCount = totalCount
      
      if (totalCount > 0) {
        const endIndex = Math.min(49, totalCount - 1)
        await loadFavoritesRange(0, endIndex, requestGroupName, {
          version: requestVersion,
          filter: requestFilter,
          contentType: requestContentType,
          pasteStatus: requestPasteStatus,
          groupName: requestGroupName,
        })
      }
    }
  } catch (err) {
    if (isFavoritesRequestCurrent(requestVersion, requestFilter, requestContentType, requestPasteStatus, requestGroupName)) {
      console.error('初始化收藏列表失败:', err)
      favoritesStore.error = err.message || '加载失败'
    }
  } finally {
    if (isFavoritesRequestCurrent(requestVersion, requestFilter, requestContentType, requestPasteStatus, requestGroupName)) {
      favoritesStore.loading = false
    }
  }
}

// 刷新收藏列表
export async function refreshFavorites(groupName = null) {
  return await initFavorites(groupName)
}

// 删除收藏项
export async function deleteFavorite(id) {
  try {
    const { showConfirm } = await import('@shared/utils/dialog')
    const i18n = (await import('@shared/i18n')).default
    const confirmed = await showConfirm(
      i18n.t('favorites.confirmDelete'),
      i18n.t('favorites.confirmDeleteTitle')
    )
    if (!confirmed) return false

    await apiDeleteFavorite(id)
    favoritesStore.removeItem(id)
    return true
  } catch (err) {
    console.error('删除收藏项失败:', err)
    throw err
  }
}

// 粘贴收藏项
export async function pasteFavorite(id) {
  try {
    await apiPasteFavorite(id)
    return true
  } catch (err) {
    console.error('粘贴收藏项失败:', err)
    throw err
  }
}

