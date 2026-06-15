import { Virtuoso } from 'react-virtuoso';
import { useCallback, useState, useMemo, useRef, forwardRef, useImperativeHandle, useEffect } from 'react';
import { useSnapshot } from 'valtio';
import { useCustomScrollbar } from '@shared/hooks/useCustomScrollbar';
import { useSortableList } from '@shared/hooks/useSortable';
import { useNavigation } from '@shared/hooks/useNavigation';
import { ROW_HEIGHT_CONFIG } from '@shared/hooks/useItemCommon';
import { clipboardStore, loadClipboardRange, pasteClipboardItem } from '@shared/store/clipboardStore';
import { navigationStore } from '@shared/store/navigationStore';
import { settingsStore } from '@shared/store/settingsStore';
import { moveClipboardItemToTop, moveClipboardItemById, closePreviewWindow } from '@shared/api';
import { getOneTimePasteEnabled } from '@shared/services/oneTimePaste';
import ClipboardItem from './ClipboardItem';

const SCROLL_DEBOUNCE_DELAY = 50;
const LIST_PRELOAD_PADDING = 20;
const LIST_VIEWPORT_PADDING = 120;

const ClipboardList = forwardRef(({
  onScrollStateChange
}, ref) => {
  const [scrollerElement, setScrollerElement] = useState(null);
  const virtuosoRef = useRef(null);
  const currentRangeRef = useRef({
    startIndex: 0,
    endIndex: 0
  });
  const loadTimeoutRef = useRef(null);
  const onScrollStateChangeRef = useRef(onScrollStateChange);
  const snap = useSnapshot(navigationStore);
  const clipSnap = useSnapshot(clipboardStore);
  const isMultiSelectMode = clipSnap.isMultiSelectMode;
  const settings = useSnapshot(settingsStore);
  const showShortcut = settings.showListShortcuts !== false && !clipSnap.filter && clipSnap.contentType === 'all';
  const showIndex = settings.showListIndex !== false;
  const selectedIdSet = useMemo(() => new Set(clipSnap.selectedEntries.map(entry => entry.id)), [clipSnap.selectedEntries]);
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
    onDragEnd: handleDragEnd
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
  }, [clipSnap.filter, clipSnap.contentType, scrollerElement]);

  const ensureRangeLoaded = useCallback(async (startIndex, endIndex) => {
    const missingIndexes = [];
    for (let index = startIndex; index <= endIndex; index += 1) {
      if (!clipboardStore.hasItem(index)) {
        missingIndexes.push(index);
      }
    }

    if (!missingIndexes.length) {
      return;
    }

    const loadStart = Math.max(0, startIndex - LIST_PRELOAD_PADDING);
    const loadEnd = Math.min(clipSnap.totalCount - 1, endIndex + LIST_PRELOAD_PADDING);
    await loadClipboardRange(loadStart, loadEnd);
  }, [clipSnap.totalCount]);

  const buildSelectionEntries = useCallback((startIndex, endIndex) => {
    const entries = [];
    for (let index = startIndex; index <= endIndex; index += 1) {
      const item = clipboardStore.getItem(index);
      if (!item?.id) continue;
      entries.push({
        id: item.id,
        index,
        contentType: item.content_type,
      });
    }
    return entries;
  }, []);

  const handleItemClick = useCallback(async (item, index, event) => {
    if (settings.modifierClickMultiSelect === false) {
      return false;
    }

    const isCtrlLikePressed = Boolean(event?.ctrlKey || event?.metaKey);
    const isShiftPressed = Boolean(event?.shiftKey);

    if (!isMultiSelectMode) {
      if (!isCtrlLikePressed && !isShiftPressed) {
        return false;
      }

      clipboardStore.enterMultiSelectMode();

      if (isShiftPressed) {
        const anchorIndex = typeof currentSelectedIndex === 'number' && currentSelectedIndex >= 0
          ? currentSelectedIndex
          : index;
        const startIndex = Math.min(anchorIndex, index);
        const endIndex = Math.max(anchorIndex, index);
        await ensureRangeLoaded(startIndex, endIndex);
        clipboardStore.selectRange(buildSelectionEntries(startIndex, endIndex));
        clipboardStore.setSelectionAnchorIndex(anchorIndex);
        return true;
      }

      clipboardStore.toggleSelectedEntry({
        id: item.id,
        index,
        contentType: item.content_type,
      });
      clipboardStore.setSelectionAnchorIndex(index);
      return true;
    }

    if (isShiftPressed && typeof clipSnap.selectionAnchorIndex === 'number') {
      const startIndex = Math.min(clipSnap.selectionAnchorIndex, index);
      const endIndex = Math.max(clipSnap.selectionAnchorIndex, index);
      await ensureRangeLoaded(startIndex, endIndex);
      clipboardStore.selectRange(buildSelectionEntries(startIndex, endIndex));
      return true;
    }

    clipboardStore.toggleSelectedEntry({
      id: item.id,
      index,
      contentType: item.content_type,
    });
    clipboardStore.setSelectionAnchorIndex(index);
    return true;
  }, [buildSelectionEntries, clipSnap.selectionAnchorIndex, currentSelectedIndex, ensureRangeLoaded, isMultiSelectMode, settings.modifierClickMultiSelect]);

  const handleRangeChanged = useCallback(({
    startIndex,
    endIndex
  }) => {
    currentRangeRef.current = {
      startIndex,
      endIndex
    };

    if (loadTimeoutRef.current) {
      clearTimeout(loadTimeoutRef.current);
    }

    loadTimeoutRef.current = setTimeout(() => {
      clipboardStore.updateViewRange(startIndex, endIndex);

      let rangeStart = -1,
        rangeEnd = -1;
      for (let i = startIndex; i <= endIndex && i < clipSnap.totalCount; i++) {
        if (!clipboardStore.hasItem(i)) {
          if (rangeStart === -1) rangeStart = i;
          rangeEnd = i;
        }
      }
      if (rangeStart !== -1) {
        const loadStart = Math.max(0, rangeStart - LIST_PRELOAD_PADDING);
        const loadEnd = Math.min(clipSnap.totalCount - 1, rangeEnd + LIST_PRELOAD_PADDING);
        loadClipboardRange(loadStart, loadEnd);
      }
    }, SCROLL_DEBOUNCE_DELAY);
  }, [clipSnap.totalCount, clipSnap.items]);

  const itemsCount = Object.keys(clipSnap.items).length;

  useEffect(() => {
    if (clipSnap.totalCount > 0 && itemsCount === 0) {
      const {
        startIndex,
        endIndex
      } = currentRangeRef.current;
      if (startIndex >= 0 && endIndex >= startIndex && endIndex < clipSnap.totalCount) {
        loadClipboardRange(
          Math.max(0, startIndex - LIST_PRELOAD_PADDING),
          Math.min(clipSnap.totalCount - 1, endIndex + LIST_PRELOAD_PADDING),
        );
      } else {
        loadClipboardRange(0, Math.min(49, clipSnap.totalCount - 1));
      }
    }
  }, [clipSnap.totalCount, itemsCount]);
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
  const isAdaptiveRowHeight = settings.rowHeight === 'auto';
  const rawAdaptiveMaxHeight = Number(settings.adaptiveMaxHeight);
  const adaptiveMaxHeightPx = Number.isFinite(rawAdaptiveMaxHeight) ? Math.max(80, Math.round(rawAdaptiveMaxHeight)) : 350;
  const adaptiveRowMaxHeightPx = Math.min(adaptiveMaxHeightPx, 350);
  const adaptiveRowStyle = isAdaptiveRowHeight && adaptiveRowMaxHeightPx !== 350 ? {
    maxHeight: `${adaptiveRowMaxHeightPx}px`,
    overflowY: 'auto'
  } : undefined;
  const getCardOuterStyle = (index) => isCardStyle ? {
    paddingLeft: '0.625rem',
    paddingRight: '0.625rem',
    marginTop: index === 0 ? `${cardSpacingPx}px` : undefined,
    marginBottom: `${cardSpacingPx}px`
  } : undefined;

  return <DndContext sensors={sensors} collisionDetection={collisionDetection} onDragStart={handleDragStart} onDragEnd={onDragEnd} onDragCancel={handleDragCancel} modifiers={modifiers}>
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
              <div className={heightClass} style={adaptiveRowStyle}>
                <div className="h-full rounded-lg border border-qc-border bg-qc-panel p-3 animate-pulse">
                  <div className="h-4 bg-qc-panel-2 rounded w-3/4 mb-2"></div>
                  <div className="h-3 bg-qc-panel-2 rounded w-1/2"></div>
                </div>
              </div>
            </div> : <div className={heightClass} style={adaptiveRowStyle}>
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
            <div className={heightClass} style={adaptiveRowStyle}>
              <ClipboardItem
                item={item}
                index={index}
                sortId={entry._sortId}
                isSelected={!isMultiSelectMode && currentSelectedIndex === index}
                isMultiSelected={selectedIdSet.has(item.id)}
                isMultiSelectMode={isMultiSelectMode}
                onHover={() => handleItemHover(index)}
                onClick={handleItemClick}
                isDragActive={!isMultiSelectMode && dragActive}
                isDraggable={!isMultiSelectMode}
                showShortcut={showShortcut}
                showIndex={showIndex}
                animationDelay={animationDelay}
              />
            </div>
          </div> : <div className={heightClass} style={adaptiveRowStyle}>
            <ClipboardItem
              item={item}
              index={index}
              sortId={entry._sortId}
              isSelected={!isMultiSelectMode && currentSelectedIndex === index}
              isMultiSelected={selectedIdSet.has(item.id)}
              isMultiSelectMode={isMultiSelectMode}
              onHover={() => handleItemHover(index)}
              onClick={handleItemClick}
              isDragActive={!isMultiSelectMode && dragActive}
              isDraggable={!isMultiSelectMode}
              showShortcut={showShortcut}
              showIndex={showIndex}
              animationDelay={animationDelay}
            />
          </div>;
        }} isScrolling={handleVirtuosoScrollState} style={{
          height: '100%'
        }} />
      </SortableContext>
    </div>

    <DragOverlay dropAnimation={null}>
      {activeItem && activeIndex !== -1 && (() => {
        const overlayClass = isAdaptiveRowHeight ? 'h-auto' : heightClass;
        const overlayStyle = isAdaptiveRowHeight ? {
          maxHeight: `${adaptiveMaxHeightPx}px`,
          overflowY: 'auto'
        } : undefined;
        return (
          <div className={`${overlayClass} rounded-md border border-qc-border shadow-lg bg-qc-panel/70 backdrop-blur-md`} style={overlayStyle}>
            <ClipboardItem
              item={activeItem.item}
              index={activeIndex}
              sortId={activeItem._sortId}
              isDragActive={!isMultiSelectMode}
              isDraggable={!isMultiSelectMode}
              showShortcut={showShortcut}
              showIndex={showIndex}
            />
          </div>
        );
      })()}
    </DragOverlay>
  </DndContext>;
});
ClipboardList.displayName = 'ClipboardList';
export default ClipboardList;
