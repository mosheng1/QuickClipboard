import { Virtuoso } from 'react-virtuoso';
import { useCallback, useState, useMemo, useRef, forwardRef, useImperativeHandle, useEffect } from 'react';
import { useSnapshot } from 'valtio';
import { listen } from '@tauri-apps/api/event';
import { useCustomScrollbar } from '@shared/hooks/useCustomScrollbar';
import { useSortableList } from '@shared/hooks/useSortable';
import { useNavigation } from '@shared/hooks/useNavigation';
import { ROW_HEIGHT_CONFIG } from '@shared/hooks/useItemCommon';
import { clipboardStore, loadClipboardRange, pasteClipboardItem } from '@shared/store/clipboardStore';
import { navigationStore } from '@shared/store/navigationStore';
import { settingsStore } from '@shared/store/settingsStore';
import { getClipboardHistory, moveClipboardItemToTop, moveClipboardItemById, closePreviewWindow } from '@shared/api';
import { getOneTimePasteEnabled } from '@shared/services/oneTimePaste';
import ClipboardItem from './ClipboardItem';
import { useExternalDragSwitch } from '@shared/hooks/useExternalDragSwitch';
import ExternalDragSafeZones from '@shared/components/ExternalDragSafeZones';

const SCROLL_DEBOUNCE_DELAY = 50;
const LIST_PRELOAD_PADDING = 20;
const LIST_VIEWPORT_PADDING = 120;
const SELECTION_LOAD_PAGE_SIZE = 100;

const ClipboardList = forwardRef(({
  onScrollStateChange
}, ref) => {
  const [scrollerElement, setScrollerElement] = useState(null);
  const virtuosoRef = useRef(null);
  const currentRangeRef = useRef({
    startIndex: 0,
    endIndex: 0
  });
  const isWindowHiddenRef = useRef(false);
  const loadTimeoutRef = useRef(null);
  const loadMissingRangeRef = useRef(null);
  const selectionRequestRef = useRef(0);
  const onScrollStateChangeRef = useRef(onScrollStateChange);
  const snap = useSnapshot(navigationStore);
  const clipSnap = useSnapshot(clipboardStore);
  const isMultiSelectMode = clipSnap.isMultiSelectMode;
  const settings = useSnapshot(settingsStore);
  const showShortcut = settings.showListShortcuts !== false && !clipSnap.filter && clipSnap.contentType === 'all';
  const showIndex = settings.showListIndex !== false;
  const selectedIdSet = useMemo(() => new Set(clipSnap.selectedEntries.map(entry => entry.id)), [clipSnap.selectedEntries]);
  const selectedOrderMap = useMemo(() => new Map(clipSnap.selectedEntries.map((entry, order) => [entry.id, order + 1])), [clipSnap.selectedEntries]);
  const itemsArray = useMemo(() => {
    return Array.from({
      length: clipSnap.totalCount
    }, (_, i) => clipSnap.items[i] || null);
  }, [clipSnap.items, clipSnap.totalCount]);
  useCustomScrollbar(scrollerElement);
  const scrollerRefCallback = useCallback(element => element && setScrollerElement(element), []);
  const itemsWithId = useMemo(() => {
    return itemsArray.map((item, index) => {
      if (!item) {
        return {
          _sortId: `placeholder-${index}`,
          _isPlaceholder: true
        };
      }
      return {
        item,
        id: item.id,
        is_pinned: item.is_pinned,
        _sortId: `${item.created_at}-${index}`
      };
    });
  }, [itemsArray]);

  useEffect(() => {
    onScrollStateChangeRef.current = onScrollStateChange;
  }, [onScrollStateChange]);

  const handleDragEnd = async (oldIndex, newIndex) => {
    if (oldIndex === newIndex) return;
    const fromItem = itemsWithId[oldIndex];
    const toItem = itemsWithId[newIndex];

    if (fromItem?.is_pinned !== toItem?.is_pinned) {
      return;
    }

    try {
      if (fromItem?.id && toItem?.id) {
        await moveClipboardItemById(fromItem.id, toItem.id);
        clipboardStore.moveLoadedItem(oldIndex, newIndex);
      }
    } catch (error) {
      console.error('移动剪贴板项失败:', error);
    }
  };

  const {
    DndContext,
    SortableContext,
    DragOverlay,
    sensors,
    handleDragStart,
    handleDragEnd: onDragEnd,
    handleDragCancel,
    activeId,
    activeItem,
    strategy,
    modifiers,
    collisionDetection
  } = useSortableList({
    items: itemsWithId,
    onDragEnd: handleDragEnd,
    restrictToVertical: false,
  });

  const {
    showSafeZones,
    prepareExternalDrag,
    shouldCancelDndDrop,
    handleDndDragStart,
    handleDndDragEnd,
    handleDndDragCancel,
  } = useExternalDragSwitch({
    onDragStart: handleDragStart,
    onDragEnd: onDragEnd,
    onDragCancel: handleDragCancel,
    closePreview: () => closePreviewWindow().catch(() => { }),
  });

  const activeIndex = activeItem ? itemsWithId.findIndex(item => item._sortId === activeId || item.id === activeId) : -1;

  useEffect(() => {
    if (!activeId || !scrollerElement) return;

    const handleWheel = (e) => {
      e.preventDefault();
      scrollerElement.scrollTop += e.deltaY;
    };

    document.addEventListener('wheel', handleWheel, { passive: false });
    return () => document.removeEventListener('wheel', handleWheel);
  }, [activeId, scrollerElement]);

  useEffect(() => {
    if (!activeId) return;
    document.body.classList.add('dragging-cursor');
    return () => {
      document.body.classList.remove('dragging-cursor');
    };
  }, [activeId]);

  const {
    currentSelectedIndex,
    navigateUp,
    navigateDown,
    executeCurrentItem,
    handleItemHover,
    handleScrollStart,
    handleScrollEnd
  } = useNavigation({
    items: itemsWithId,
    virtuosoRef,
    onExecuteItem: async (item, index) => {
      try {
        await pasteClipboardItem(item.id);
        // 粘贴后置顶
        if (!getOneTimePasteEnabled() && settingsStore.pasteToTop && item.id && !item.is_pinned) {
          try {
            await moveClipboardItemToTop(item.id);
          } finally {
            clipboardStore.items = {};
          }
        }
      } catch (error) {
        console.error('粘贴失败:', error);
      }
    },
    enabled: snap.activeTab === 'clipboard' && !isMultiSelectMode
  });

  const handleVirtuosoScrollState = useCallback((scrolling) => {
    if (scrolling) {
      // 滚动时主动关闭预览，避免鼠标静止滚动导致旧预览残留
      closePreviewWindow().catch(() => { });
      handleScrollStart();
      return;
    }
    handleScrollEnd();
  }, [handleScrollEnd, handleScrollStart]);

  useEffect(() => {
    currentRangeRef.current = {
      startIndex: 0,
      endIndex: 0
    };
    clipboardStore.currentViewRange = { start: 0, end: 50 };

    if (loadTimeoutRef.current) {
      clearTimeout(loadTimeoutRef.current);
      loadTimeoutRef.current = null;
    }

    virtuosoRef.current?.scrollToIndex({
      index: 0,
      align: 'start',
      behavior: 'auto'
    });
    if (scrollerElement) {
      scrollerElement.scrollTop = 0;
    }
    navigationStore.resetNavigation();
    onScrollStateChangeRef.current?.({ atTop: true });
  }, [clipSnap.filter, clipSnap.contentType, clipSnap.pasteStatus, scrollerElement]);

  const loadMissingRange = useCallback(async (startIndex, endIndex) => {
    if (clipSnap.loading || clipSnap.totalCount <= 0) {
      return false;
    }

    let safeStart = Number.isInteger(startIndex) ? startIndex : 0;
    let safeEnd = Number.isInteger(endIndex) ? endIndex : Math.min(49, clipSnap.totalCount - 1);

    if (safeEnd < safeStart) {
      safeStart = 0;
      safeEnd = Math.min(49, clipSnap.totalCount - 1);
    }

    safeStart = Math.max(0, Math.min(safeStart, clipSnap.totalCount - 1));
    safeEnd = Math.max(safeStart, Math.min(safeEnd, clipSnap.totalCount - 1));

    let rangeStart = -1;
    let rangeEnd = -1;
    for (let index = safeStart; index <= safeEnd; index += 1) {
      if (!clipboardStore.hasItem(index)) {
        if (rangeStart === -1) rangeStart = index;
        rangeEnd = index;
      }
    }

    if (rangeStart === -1) {
      return false;
    }

    const loadStart = Math.max(0, rangeStart - LIST_PRELOAD_PADDING);
    const loadEnd = Math.min(clipSnap.totalCount - 1, rangeEnd + LIST_PRELOAD_PADDING);
    await loadClipboardRange(loadStart, loadEnd);
    return true;
  }, [clipSnap.loading, clipSnap.totalCount]);

  useEffect(() => {
    loadMissingRangeRef.current = loadMissingRange;
  }, [loadMissingRange]);

  useEffect(() => {
    const handleWindowHide = () => {
      isWindowHiddenRef.current = true;
      if (loadTimeoutRef.current) {
        clearTimeout(loadTimeoutRef.current);
        loadTimeoutRef.current = null;
      }
    };

    const handleWindowShow = () => {
      isWindowHiddenRef.current = false;
      setTimeout(() => {
        const { startIndex, endIndex } = currentRangeRef.current;
        loadMissingRangeRef.current?.(startIndex, endIndex).catch(error => {
          console.error('窗口显示后补齐剪贴板范围失败:', error);
        });
      }, 80);
    };

    const setupListeners = async () => {
      const unlisten1 = await listen('window-hide-animation', handleWindowHide);
      const unlisten2 = await listen('edge-snap-hide', handleWindowHide);
      const unlisten3 = await listen('window-show-animation', handleWindowShow);
      const unlisten4 = await listen('edge-snap-show', handleWindowShow);
      return () => {
        unlisten1();
        unlisten2();
        unlisten3();
        unlisten4();
      };
    };

    const cleanup = setupListeners();
    return () => cleanup.then(fn => fn());
  }, []);

  const loadSelectionEntries = useCallback(async (startIndex, endIndex) => {
    const entries = [];

    for (let offset = startIndex; offset <= endIndex; offset += SELECTION_LOAD_PAGE_SIZE) {
      const limit = Math.min(SELECTION_LOAD_PAGE_SIZE, endIndex - offset + 1);
      const result = await getClipboardHistory({
        offset,
        limit,
        contentType: clipSnap.contentType !== 'all' ? clipSnap.contentType : undefined,
        search: clipSnap.filter || undefined,
      });

      result.items.forEach((item, itemOffset) => {
        if (!item?.id) return;
        entries.push({
          id: item.id,
          index: offset + itemOffset,
          contentType: item.content_type,
        });
      });

      if (result.items.length < limit) {
        break;
      }
    }

    return entries;
  }, [clipSnap.contentType, clipSnap.filter]);

  const handleItemClick = useCallback(async (item, index, event) => {
    if (settings.modifierClickMultiSelect === false) {
      return false;
    }

    const isCtrlLikePressed = Boolean(event?.ctrlKey || event?.metaKey);
    const isShiftPressed = Boolean(event?.shiftKey);

    const currentlyMultiSelect = clipboardStore.isMultiSelectMode;
    if (!currentlyMultiSelect) {
      if (!isCtrlLikePressed && !isShiftPressed) {
        return false;
      }

      clipboardStore.enterMultiSelectMode({
        id: item.id,
        index,
        contentType: item.content_type,
      });

      if (isShiftPressed) {
        const anchorIndex = typeof currentSelectedIndex === 'number' && currentSelectedIndex >= 0
          ? currentSelectedIndex
          : index;
        const startIndex = Math.min(anchorIndex, index);
        const endIndex = Math.max(anchorIndex, index);
        // 先同步记录锚点，避免范围加载期间的下一次 Shift 点击被当成普通追加选择。
        clipboardStore.setSelectionAnchorIndex(anchorIndex);
        const requestId = ++selectionRequestRef.current;
        const entries = await loadSelectionEntries(startIndex, endIndex);
        if (requestId === selectionRequestRef.current) {
          clipboardStore.selectRange(anchorIndex > index ? entries.reverse() : entries);
        }
        return true;
      }

      clipboardStore.setSelectionAnchorIndex(index);
      return true;
    }

    const selectionAnchorIndex = clipboardStore.selectionAnchorIndex;
    if (isShiftPressed && typeof selectionAnchorIndex === 'number') {
      const startIndex = Math.min(selectionAnchorIndex, index);
      const endIndex = Math.max(selectionAnchorIndex, index);
      const requestId = ++selectionRequestRef.current;
      const entries = await loadSelectionEntries(startIndex, endIndex);
      if (requestId === selectionRequestRef.current) {
        clipboardStore.selectRange(selectionAnchorIndex > index ? entries.reverse() : entries);
      }
      return true;
    }

    selectionRequestRef.current += 1;

    const entry = {
      id: item.id,
      index,
      contentType: item.content_type,
    };
    if (isCtrlLikePressed) {
      clipboardStore.toggleSelectedEntry(entry);
    } else if (clipboardStore.selectedEntries.length === 1 && clipboardStore.hasSelectedId(entry.id)) {
      clipboardStore.toggleSelectedEntry(entry);
    } else {
      // 多选模式下普通单击改为单选当前项，符合常见文件管理器行为。
      clipboardStore.replaceSelection([entry]);
    }
    clipboardStore.setSelectionAnchorIndex(index);
    return true;
  }, [currentSelectedIndex, loadSelectionEntries, settings.modifierClickMultiSelect]);

  const handleRangeChanged = useCallback(({
    startIndex,
    endIndex
  }) => {
    if (isWindowHiddenRef.current || (scrollerElement && scrollerElement.clientHeight <= 1)) {
      return;
    }

    currentRangeRef.current = {
      startIndex,
      endIndex
    };

    if (loadTimeoutRef.current) {
      clearTimeout(loadTimeoutRef.current);
    }

    loadTimeoutRef.current = setTimeout(() => {
      clipboardStore.updateViewRange(startIndex, endIndex);

      loadMissingRange(startIndex, endIndex).catch(error => {
        console.error('加载当前剪贴板范围失败:', error);
      });
    }, SCROLL_DEBOUNCE_DELAY);
  }, [loadMissingRange, scrollerElement]);

  const itemsCount = Object.keys(clipSnap.items).length;

  useEffect(() => {
    if (clipSnap.loading || clipSnap.totalCount <= 0) {
      return;
    }

    const { startIndex, endIndex } = currentRangeRef.current;
    const shouldLoadInitialPage = itemsCount === 0 && startIndex === 0 && endIndex === 0;
    loadMissingRange(
      startIndex,
      shouldLoadInitialPage ? Math.min(49, clipSnap.totalCount - 1) : endIndex,
    ).catch(error => {
      console.error('补齐剪贴板可见范围失败:', error);
    });
  }, [clipSnap.loading, clipSnap.totalCount, itemsCount, loadMissingRange]);
  useImperativeHandle(ref, () => ({
    navigateUp,
    navigateDown,
    executeCurrentItem,
    executePlainTextPaste: async () => {
      if (isMultiSelectMode) {
        return;
      }
      const item = itemsWithId[currentSelectedIndex];
      if (item?.item && !item._isPlaceholder) {
        try {
          const { pasteClipboardItem } = await import('@shared/api/clipboard');
          await pasteClipboardItem(item.item.id, 'plain_text');
          if (!getOneTimePasteEnabled() && settingsStore.pasteToTop && item.item.id && !item.item.is_pinned) {
            try {
              await moveClipboardItemToTop(item.item.id);
            } finally {
              clipboardStore.items = {};
            }
          }
        } catch (error) {
          console.error('纯文本粘贴失败:', error);
        }
      }
    },
    scrollToTop: () => {
      virtuosoRef.current?.scrollToIndex({
        index: 0,
        align: 'start',
        behavior: 'auto'
      });
    }
  }), [currentSelectedIndex, executeCurrentItem, isMultiSelectMode, itemsWithId, navigateDown, navigateUp, settings.pasteToTop]);
  if (clipSnap.loading && clipSnap.filter) {
    return <div className="flex-1 bg-qc-surface overflow-hidden flex items-center justify-center transition-colors duration-500 clipboard-list" data-no-drag>
      <div className="flex flex-col items-center gap-2 text-qc-fg-subtle">
        <div className="h-5 w-5 animate-spin rounded-full border-2 border-qc-border border-t-theme-9"></div>
        <p className="text-sm">搜索中...</p>
      </div>
    </div>;
  }

  if (clipSnap.totalCount === 0) {
    return <div className="flex-1 bg-qc-surface overflow-hidden flex items-center justify-center transition-colors duration-500 clipboard-list" data-no-drag>
      <p className="text-qc-fg-subtle text-sm">
        {clipSnap.filter ? '无搜索结果' : '暂无剪贴板记录'}
      </p>
    </div>;
  }
  const rowConfig = ROW_HEIGHT_CONFIG[settings.rowHeight] || ROW_HEIGHT_CONFIG.medium;
  const isCardStyle = settings.listStyle === 'card';
  const cardSpacingPx = typeof settings.cardSpacing === 'number' ? settings.cardSpacing : 8;
  const defaultHeight = isCardStyle ? rowConfig.cardPx + cardSpacingPx : rowConfig.px;
  const heightClass = isCardStyle ? rowConfig.cardClass : rowConfig.class;
  const getCardOuterStyle = (index) => isCardStyle ? {
    paddingLeft: '0.625rem',
    paddingRight: '0.625rem',
    marginTop: index === 0 ? `${cardSpacingPx}px` : undefined,
    marginBottom: `${cardSpacingPx}px`
  } : undefined;

  return <DndContext sensors={sensors} collisionDetection={collisionDetection} onDragStart={handleDndDragStart} onDragEnd={handleDndDragEnd} onDragCancel={handleDndDragCancel} cancelDrop={shouldCancelDndDrop} modifiers={[]}>
    <ExternalDragSafeZones visible={showSafeZones} />
    <div className="flex-1 bg-qc-surface overflow-hidden custom-scrollbar-container transition-colors duration-500 clipboard-list" data-no-drag>
      <SortableContext items={itemsWithId.map(item => item._sortId)} strategy={strategy}>
        <Virtuoso ref={virtuosoRef} totalCount={clipSnap.totalCount || 0} scrollerRef={scrollerRefCallback} atTopStateChange={atTop => {
          onScrollStateChange?.({
            atTop
          });
        }} rangeChanged={handleRangeChanged} increaseViewportBy={{
          top: LIST_VIEWPORT_PADDING,
          bottom: LIST_VIEWPORT_PADDING
        }} defaultItemHeight={defaultHeight} computeItemKey={index => {
          const item = itemsWithId[index];
          return item?.id || item?._sortId || `item-${index}`;
        }} itemContent={index => {
          const entry = itemsWithId[index];
          if (!entry || entry._isPlaceholder) {
            return isCardStyle ? <div style={getCardOuterStyle(index)}>
              <div className={heightClass}>
                <div className="h-full rounded-lg border border-qc-border bg-qc-panel p-3 animate-pulse">
                  <div className="h-4 bg-qc-panel-2 rounded w-3/4 mb-2"></div>
                  <div className="h-3 bg-qc-panel-2 rounded w-1/2"></div>
                </div>
              </div>
            </div> : <div className={heightClass}>
              <div className="h-full border-b border-qc-border bg-qc-panel p-3 animate-pulse">
                <div className="h-4 bg-qc-panel-2 rounded w-3/4 mb-2"></div>
                <div className="h-3 bg-qc-panel-2 rounded w-1/2"></div>
              </div>
            </div>;
          }

          const dragActive = Boolean(activeId);
          const animationDelay = settings.uiAnimationEnabled !== false ? Math.min(index * 20, 100) : 0;
          const item = entry.item;
          return isCardStyle ? <div style={getCardOuterStyle(index)}>
            <div className={heightClass}>
              <ClipboardItem
                item={item}
                index={index}
                sortId={entry._sortId}
                isSelected={!isMultiSelectMode && currentSelectedIndex === index}
                isMultiSelected={selectedIdSet.has(item.id)}
                selectionNumber={selectedOrderMap.get(item.id)}
                isMultiSelectMode={isMultiSelectMode}
                onHover={() => handleItemHover(index)}
                onClick={handleItemClick}
                isDragActive={!isMultiSelectMode && dragActive}
                isDraggable={!isMultiSelectMode}
                showShortcut={showShortcut}
                showIndex={showIndex}
                animationDelay={animationDelay}
                leftClickAction={settings.leftClickAction}
                onPrepareExternalDrag={prepareExternalDrag}
              />
            </div>
          </div> : <div className={heightClass}>
            <ClipboardItem
              item={item}
              index={index}
              sortId={entry._sortId}
              isSelected={!isMultiSelectMode && currentSelectedIndex === index}
                isMultiSelected={selectedIdSet.has(item.id)}
                selectionNumber={selectedOrderMap.get(item.id)}
              isMultiSelectMode={isMultiSelectMode}
              onHover={() => handleItemHover(index)}
              onClick={handleItemClick}
              isDragActive={!isMultiSelectMode && dragActive}
              isDraggable={!isMultiSelectMode}
              showShortcut={showShortcut}
              showIndex={showIndex}
              animationDelay={animationDelay}
              leftClickAction={settings.leftClickAction}
              onPrepareExternalDrag={prepareExternalDrag}
            />
          </div>;
        }} isScrolling={handleVirtuosoScrollState} style={{
          height: '100%'
        }} />
      </SortableContext>
    </div>

    <DragOverlay dropAnimation={null} modifiers={[]} zIndex={1200}>
      {activeItem && activeIndex !== -1 && (() => {
        const overlayClass = settings.rowHeight === 'auto' ? 'h-auto' : heightClass;
        return (
          <div className={`${overlayClass} relative z-[1200] rounded-md border border-qc-border shadow-lg bg-qc-panel/70 backdrop-blur-md`}>
            <ClipboardItem
              item={activeItem.item}
              index={activeIndex}
              sortId={activeItem._sortId}
              isDragActive={!isMultiSelectMode}
              isDraggable={false}
              showShortcut={showShortcut}
              showIndex={showIndex}
              leftClickAction={settings.leftClickAction}
            />
          </div>
        );
      })()}
    </DragOverlay>
  </DndContext>;
});
ClipboardList.displayName = 'ClipboardList';
export default ClipboardList;
