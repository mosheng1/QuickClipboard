import { proxy } from 'valtio'
import { listen } from '@tauri-apps/api/event'
import { 
  getClipboardHistory, 
  getClipboardTotalCount,
  deleteClipboardItem as apiDeleteItem,
  clearClipboardHistory as apiClearHistory,
  pasteClipboardItem as apiPasteClipboardItem
} from '@shared/api'

listen('paste-count-updated', (event) => {
  const id = event.payload
  for (const key of Object.keys(clipboardStore.items)) {
    const item = clipboardStore.items[key]
    if (item && item.id === id) {
      clipboardStore.items[key] = { ...item, paste_count: (item.paste_count || 0) + 1 }
      break
    }
  }
})

const CACHE_WINDOW_SIZE = 120
const CACHE_BUFFER = 40
let clipboardRequestVersion = 0
let activeClipboardLoadVersion = null

function nextClipboardRequestVersion() {
  clipboardRequestVersion += 1
  return clipboardRequestVersion
}

function createClipboardRequestContext(bumpVersion = false) {
  return {
    version: bumpVersion ? nextClipboardRequestVersion() : clipboardRequestVersion,
    filter: clipboardStore.filter,
    contentType: clipboardStore.contentType
  }
}

function isClipboardRequestCurrent(context) {
  return context?.version === clipboardRequestVersion
    && clipboardStore.filter === context.filter
    && clipboardStore.contentType === context.contentType
}

// 剪贴板 Store
export const clipboardStore = proxy({
  items: {}, 
  totalCount: 0,
  filter: '',
  contentType: 'all',
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

  getItem(index) {
    return this.items[index]
  },
  
  // 检查指定索引是否已加载
  hasItem(index) {
    return index in this.items
  },

  findLoadedItemIndex(id) {
    const entry = Object.entries(this.items)
      .map(([key, value]) => [parseInt(key, 10), value])
      .find(([, value]) => value?.id === id)
    return Number.isInteger(entry?.[0]) ? entry[0] : null
  },

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
        .sort((a, b) => a.index - b.index)
    }

    return true
  },

  insertLoadedItemAt(item, insertIndex, totalCount) {
    if (!item || !Number.isInteger(insertIndex) || insertIndex < 0 || insertIndex >= totalCount) {
      return false
    }

    const nextItems = {}
    const oldEntry = Object.entries(this.items)
      .map(([key, value]) => [parseInt(key, 10), value])
      .find(([, value]) => value?.id === item.id)
    const oldIndex = oldEntry?.[0]
    const isMovingLoadedItem = Number.isInteger(oldIndex)
    const effectiveTotalCount = isMovingLoadedItem ? this.totalCount : totalCount
    const entries = Object.entries(this.items)
      .map(([key, value]) => [parseInt(key, 10), value])
      .filter(([, value]) => value)
      .sort((a, b) => b[0] - a[0])

    for (const [index, value] of entries) {
      if (value.id === item.id) {
        continue
      }

      let nextIndex = index
      if (isMovingLoadedItem) {
        if (oldIndex < insertIndex && index > oldIndex && index <= insertIndex) {
          nextIndex = index - 1
        } else if (oldIndex > insertIndex && index >= insertIndex && index < oldIndex) {
          nextIndex = index + 1
        }
      } else if (index >= insertIndex) {
        nextIndex = index + 1
      }

      if (nextIndex < effectiveTotalCount) {
        nextItems[nextIndex] = value
      }
    }

    nextItems[insertIndex] = item
    this.items = nextItems
    this.totalCount = effectiveTotalCount

    if (this.selectionAnchorIndex != null) {
      if (isMovingLoadedItem) {
        if (this.selectionAnchorIndex === oldIndex) {
          this.selectionAnchorIndex = insertIndex
        } else if (oldIndex < insertIndex && this.selectionAnchorIndex > oldIndex && this.selectionAnchorIndex <= insertIndex) {
          this.selectionAnchorIndex -= 1
        } else if (oldIndex > insertIndex && this.selectionAnchorIndex >= insertIndex && this.selectionAnchorIndex < oldIndex) {
          this.selectionAnchorIndex += 1
        }
      } else if (this.selectionAnchorIndex >= insertIndex) {
        this.selectionAnchorIndex += 1
      }
    }

    if (this.selectedEntries.length > 0) {
      this.selectedEntries = this.selectedEntries
        .map(entry => {
          if (isMovingLoadedItem) {
            if (entry.id === item.id || entry.index === oldIndex) {
              return { ...entry, index: insertIndex }
            }
            if (oldIndex < insertIndex && entry.index > oldIndex && entry.index <= insertIndex) {
              return { ...entry, index: entry.index - 1 }
            }
            if (oldIndex > insertIndex && entry.index >= insertIndex && entry.index < oldIndex) {
              return { ...entry, index: entry.index + 1 }
            }
            return entry
          }

          return entry.index >= insertIndex
            ? { ...entry, index: entry.index + 1 }
            : entry
        })
        .sort((a, b) => a.index - b.index)
    }

    const { start, end } = this.currentViewRange
    if (isMovingLoadedItem) {
      this.currentViewRange = { start, end }
    } else if (insertIndex <= start) {
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
      nextClipboardRequestVersion()
      this.filter = value
      this.items = {}
      this.loadingRanges = new Set()
      this.exitMultiSelectMode()
    }
  },
  
  setContentType(value) {
    if (this.contentType !== value) {
      nextClipboardRequestVersion()
      this.contentType = value
      this.items = {}
      this.loadingRanges = new Set()
      this.exitMultiSelectMode()
    }
  },
  
  enterMultiSelectMode() {
    this.isMultiSelectMode = true
    this.selectedEntries = []
    this.selectedIds = new Set()
    this.selectionAnchorIndex = null
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

    uniqueEntries.sort((a, b) => a.index - b.index)
    this.selectedEntries = uniqueEntries
    this.selectedIds = new Set(uniqueEntries.map(entry => entry.id))
  },

  toggleSelectedEntry(entry) {
    const normalizedEntry = this.normalizeSelectedEntry(entry)
    const exists = this.selectedEntries.some(selected => selected.id === normalizedEntry.id)
    if (exists) {
      this.replaceSelection(this.selectedEntries.filter(selected => selected.id !== normalizedEntry.id))
      return
    }

    this.replaceSelection([...this.selectedEntries, normalizedEntry])
  },

  selectRange(entries) {
    this.replaceSelection([...this.selectedEntries, ...entries])
  },

  getSelectedIds() {
    return [...this.selectedEntries]
      .sort((a, b) => a.index - b.index)
      .map(entry => entry.id)
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
  
  removeLoadingRange(start, end) {
    this.loadingRanges.delete(`${start}-${end}`)
  },
  
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
export async function loadClipboardRange(startIndex, endIndex, requestContext = null) {
  const context = requestContext ?? createClipboardRequestContext()

  if (!isClipboardRequestCurrent(context)) {
    return
  }

  if (clipboardStore.loading && !requestContext) {
    return
  }

  // 避免重复加载
  if (clipboardStore.isRangeLoading(startIndex, endIndex) || 
      clipboardStore.hasOverlappingLoadingRange(startIndex, endIndex)) {
    return
  }
  
  // 检查是否所有数据都已加载
  let allLoaded = true
  for (let i = startIndex; i <= endIndex; i++) {
    if (!clipboardStore.hasItem(i)) {
      allLoaded = false
      break
    }
  }
  
  if (allLoaded) {
    return
  }
  
  clipboardStore.addLoadingRange(startIndex, endIndex)
  
  try {
    const limit = endIndex - startIndex + 1
    const result = await getClipboardHistory({
      offset: startIndex,
      limit,
      contentType: context.contentType !== 'all' ? context.contentType : undefined,
      search: context.filter || undefined
    })

    if (!isClipboardRequestCurrent(context)) {
      return
    }
    
    // 将数据按索引存储
    clipboardStore.setItemsInRange(startIndex, result.items)
    
    // 更新总数
    if (result.total_count !== undefined) {
      clipboardStore.totalCount = result.total_count
    }
  } catch (err) {
    if (isClipboardRequestCurrent(context)) {
      console.error(`加载范围 ${startIndex}-${endIndex} 失败:`, err)
      clipboardStore.error = err.message || '加载失败'
    }
  } finally {
    if (isClipboardRequestCurrent(context)) {
      clipboardStore.removeLoadingRange(startIndex, endIndex)
    }
  }
}

export async function loadClipboardItems() {
  return await refreshClipboardHistory()
}

// 初始化加载
export async function initClipboardItems() {
  const context = createClipboardRequestContext(true)

  activeClipboardLoadVersion = context.version
  clipboardStore.loading = true
  clipboardStore.error = null
  
  try {
    clipboardStore.items = {}
    clipboardStore.loadingRanges = new Set()
    
    if (context.contentType !== 'all' || context.filter) {
      const result = await getClipboardHistory({
        offset: 0,
        limit: 100,
        contentType: context.contentType !== 'all' ? context.contentType : undefined,
        search: context.filter || undefined
      })

      if (!isClipboardRequestCurrent(context)) {
        return
      }
      
      clipboardStore.totalCount = result.total_count
      clipboardStore.setItemsInRange(0, result.items)
    } else {
      const totalCount = await getClipboardTotalCount()

      if (!isClipboardRequestCurrent(context)) {
        return
      }

      clipboardStore.totalCount = totalCount
      
      if (totalCount > 0) {
        const endIndex = Math.min(99, totalCount - 1)
        await loadClipboardRange(0, endIndex, context)
      }
    }
  } catch (err) {
    if (isClipboardRequestCurrent(context)) {
      console.error('初始化剪贴板失败:', err)
      clipboardStore.error = err.message || '加载失败'
    }
  } finally {
    if (activeClipboardLoadVersion === context.version) {
      activeClipboardLoadVersion = null
      clipboardStore.loading = false
    }
  }
}

// 刷新剪贴板历史
export async function refreshClipboardHistory() {
  return await initClipboardItems()
}

// 删除剪贴板项
export async function deleteClipboardItem(id) {
  try {
    await apiDeleteItem(id)
    clipboardStore.removeItem(id)
    return true
  } catch (err) {
    console.error('删除剪贴板项失败:', err)
    throw err
  }
}

// 清空剪贴板历史
export async function clearClipboardHistory() {
  try {
    await apiClearHistory()
    clipboardStore.clearAll()
    return true
  } catch (err) {
    console.error('清空剪贴板历史失败:', err)
    throw err
  }
}

// 粘贴剪贴板项
export async function pasteClipboardItem(id) {
  try {
    await apiPasteClipboardItem(id)
    return true
  } catch (err) {
    console.error('粘贴剪贴板项失败:', err)
    throw err
  }
}
