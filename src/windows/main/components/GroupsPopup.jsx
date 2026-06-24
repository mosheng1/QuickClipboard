import '@tabler/icons-webfont/dist/tabler-icons.min.css';
import { useSnapshot } from 'valtio';
import { useState, useRef, useEffect, forwardRef, useImperativeHandle } from 'react';
import { useTranslation } from 'react-i18next';
import { DndContext, closestCenter, PointerSensor, useSensor, useSensors } from '@dnd-kit/core';
import { SortableContext, verticalListSortingStrategy, useSortable, arrayMove } from '@dnd-kit/sortable';
import { restrictToVerticalAxis, restrictToParentElement } from '@dnd-kit/modifiers';
import { CSS } from '@dnd-kit/utilities';
import { groupsStore, reorderGroups } from '@shared/store/groupsStore';
import { settingsStore } from '@shared/store/settingsStore';
import { showConfirm, showError } from '@shared/utils/dialog';
import GroupModal from './GroupModal';
import Tooltip from '@shared/components/common/Tooltip.jsx';

const ACTIVE_ICON_BUTTON_CLASS = 'bg-[var(--qc-accent)] text-[var(--qc-accent-fg)] hover:bg-[var(--qc-accent)]';
const SIDEBAR_GROUP_PANEL_WIDTH = 184;
const SIDEBAR_GROUP_PANEL_COLLAPSED_WIDTH = 56;

const SortableGroupItem = ({ group, isActive, onSelect, onEdit, onDelete, t, compact = false }) => {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ 
    id: group.name,
    disabled: group.name === '全部'
  });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    zIndex: isDragging ? 50 : undefined,
    opacity: isDragging ? 0.8 : 1,
  };

  const row = (
    <div
      ref={setNodeRef}
      style={style}
      data-group-name={group.name}
      onClick={() => onSelect(group.name)}
      className="group relative w-full"
    >
      <div className={`flex items-center cursor-pointer transition-all w-full ${
        compact ? 'justify-center gap-0 px-1.5 py-1' : 'gap-1.5 px-2 py-1'
      } ${
        isActive
          ? 'bg-[var(--qc-accent)] text-[var(--qc-accent-fg)]'
          : 'text-qc-fg hover:bg-qc-hover'
      } ${isDragging ? 'shadow-lg rounded bg-qc-panel' : ''}`}>
        {!compact && (
          <div
            {...attributes}
            {...listeners}
            className={`flex-shrink-0 cursor-grab active:cursor-grabbing opacity-30 hover:opacity-70 ${
              isActive ? 'opacity-50 hover:opacity-80' : ''
            }`}
            onClick={(e) => e.stopPropagation()}
          >
            <i className="ti ti-grip-vertical" style={{ fontSize: 10 }}></i>
          </div>
        )}

        <div className="flex-shrink-0">
          <i
            className={group.icon}
            style={{ fontSize: compact ? 14 : 12, color: isActive ? undefined : (group.color || '#dc2626') }}
          ></i>
        </div>

        {!compact && (
          <div className="flex-1 text-[11px] font-medium truncate">
            {group.name}
          </div>
        )}
      </div>

      {!compact && group.name !== '全部' && !isDragging && (
        <div className={`absolute right-1 top-1/2 -translate-y-1/2 flex gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity ${
          isActive ? 'opacity-100' : ''
        }`}>
          <Tooltip content={t('groups.edit')} placement="left" asChild>
            <button
              onClick={(e) => onEdit(e, group)}
              className={`w-5 h-5 flex items-center justify-center rounded-md transition-all ${
                isActive
                  ? 'bg-qc-border hover:bg-qc-border-strong text-white'
                  : 'bg-qc-panel/90 hover:bg-qc-hover text-qc-fg hover:text-[var(--qc-accent)]'
              }`}
            >
              <i className="ti ti-edit" style={{ fontSize: 10 }}></i>
            </button>
          </Tooltip>
          <Tooltip content={t('groups.delete')} placement="left" asChild>
            <button
              onClick={(e) => onDelete(e, group.name)}
              className={`w-5 h-5 flex items-center justify-center rounded-md transition-all ${
                isActive
                  ? 'bg-qc-border hover:bg-red-400/50 text-white'
                  : 'bg-qc-panel/80 hover:bg-red-100 text-qc-fg hover:text-red-600'
              }`}
            >
              <i className="ti ti-trash" style={{ fontSize: 10 }}></i>
            </button>
          </Tooltip>
        </div>
      )}
    </div>
  );

  if (compact) {
    return (
      <Tooltip content={group.name} placement="right" asChild>
        {row}
      </Tooltip>
    );
  }

  return row;
};

const GroupsPopup = forwardRef(({
  activeTab,
  onTabChange,
  onGroupChange,
  onOpenChange,
  mode = 'footer',
  compact = false
}, ref) => {
  const { t } = useTranslation();
  const groups = useSnapshot(groupsStore);
  const settings = useSnapshot(settingsStore);
  const uiAnimationEnabled = settings.uiAnimationEnabled !== false;
  const [isOpen, setIsOpen] = useState(false);
  const [showModal, setShowModal] = useState(false);
  const [editingGroup, setEditingGroup] = useState(null);
  const [isClosing, setIsClosing] = useState(false);
  const [isPinned, setIsPinned] = useState(false);
  const [isSidebarCollapsed, setIsSidebarCollapsed] = useState(false);
  const [tabPanelWidth, setTabPanelWidth] = useState(360);
  const [tabPanelRightOffset, setTabPanelRightOffset] = useState(0);
  const [tabPanelTopOffset, setTabPanelTopOffset] = useState(35);
  const rootRef = useRef(null);
  const closeTimerRef = useRef(null);
  const animationTimerRef = useRef(null);
  const sidebarCollapseTimerRef = useRef(null);
  const previousSidebarCollapsedRef = useRef(false);
  const listContainerRef = useRef(null);
  const pendingCloseRef = useRef(false);
  const isSidebarMode = mode === 'sidebar' || mode === 'tab-sidebar';
  const isTabMode = mode === 'tab';

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 5,
      },
    })
  );

  // 清理定时器
  useEffect(() => {
    return () => {
      if (closeTimerRef.current) {
        clearTimeout(closeTimerRef.current);
      }
      if (animationTimerRef.current) {
        clearTimeout(animationTimerRef.current);
      }
      if (sidebarCollapseTimerRef.current) {
        clearTimeout(sidebarCollapseTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    if (!isTabMode) {
      return undefined;
    }

    const rootElement = rootRef.current;
    if (!rootElement) {
      return undefined;
    }
    const tabNavigationElement = rootElement.closest('.tab-navigation');
    const tabRightAreaElement = tabNavigationElement?.querySelector('.tab-navigation-right');
    const mainContainerElement = rootElement.closest('.main-container');

    const updateTabPanelWidth = () => {
      const baseWidth = tabRightAreaElement?.clientWidth
        || (tabNavigationElement?.clientWidth ? tabNavigationElement.clientWidth * 0.5 : 0)
        || mainContainerElement?.clientWidth
        || window.innerWidth;
      const nextWidth = Math.floor(baseWidth);
      setTabPanelWidth(nextWidth);

      const alignElement = tabRightAreaElement || tabNavigationElement;
      if (alignElement) {
        const alignRect = alignElement.getBoundingClientRect();
        const rootRect = rootElement.getBoundingClientRect();
        const nextRightOffset = alignRect.right - rootRect.right;
        setTabPanelRightOffset(nextRightOffset);
      } else {
        setTabPanelRightOffset(0);
      }

      if (tabNavigationElement) {
        const tabNavigationRect = tabNavigationElement.getBoundingClientRect();
        const rootRect = rootElement.getBoundingClientRect();
        const nextTopOffset = Math.round(tabNavigationRect.bottom - rootRect.top - 1);
        setTabPanelTopOffset(nextTopOffset);
      } else {
        setTabPanelTopOffset(rootElement.clientHeight - 1);
      }
    };

    updateTabPanelWidth();
    if (typeof ResizeObserver !== 'undefined' && (mainContainerElement || tabNavigationElement || tabRightAreaElement)) {
      const observer = new ResizeObserver(() => {
        updateTabPanelWidth();
      });
      if (mainContainerElement) {
        observer.observe(mainContainerElement);
      }
      if (tabNavigationElement && tabNavigationElement !== mainContainerElement) {
        observer.observe(tabNavigationElement);
      }
      if (tabRightAreaElement && tabRightAreaElement !== tabNavigationElement && tabRightAreaElement !== mainContainerElement) {
        observer.observe(tabRightAreaElement);
      }
      return () => {
        observer.disconnect();
      };
    }

    window.addEventListener('resize', updateTabPanelWidth);
    return () => {
      window.removeEventListener('resize', updateTabPanelWidth);
    };
  }, [isTabMode, isOpen]);

  useEffect(() => {
    if (!isSidebarMode) {
      setIsSidebarCollapsed(false);
      if (sidebarCollapseTimerRef.current) {
        clearTimeout(sidebarCollapseTimerRef.current);
        sidebarCollapseTimerRef.current = null;
      }
    }
  }, [isSidebarMode]);

  useEffect(() => {
    if (onOpenChange) {
      onOpenChange(isOpen);
    }
  }, [isOpen, onOpenChange]);

  // 关闭面板（带动画）
  const handleClose = () => {
    if (isSidebarMode) {
      setIsOpen(false);
      return;
    }
    if (isPinned) return;
    setIsClosing(true);
    animationTimerRef.current = setTimeout(() => {
      setIsOpen(false);
      setIsClosing(false);
    }, 200);
  };

  // 切换弹出状态
  const togglePopup = () => {
    if (isSidebarMode) {
      if (sidebarCollapseTimerRef.current) {
        clearTimeout(sidebarCollapseTimerRef.current);
        sidebarCollapseTimerRef.current = null;
      }
      setIsOpen(prev => !prev);
      return;
    }
    if (closeTimerRef.current) {
      clearTimeout(closeTimerRef.current);
      closeTimerRef.current = null;
    }
    setIsOpen(!isOpen);
  };

  // 切换固定状态
  const togglePin = e => {
    if (e) {
      e.stopPropagation();
    }
    setIsPinned(!isPinned);
  };

  // sidebar 模式下自动收起面板并恢复用户原始折叠状态
  const scheduleSidebarAutoClose = () => {
    if (sidebarCollapseTimerRef.current) {
      clearTimeout(sidebarCollapseTimerRef.current);
    }
    sidebarCollapseTimerRef.current = setTimeout(() => {
      setIsOpen(false);
      setIsSidebarCollapsed(previousSidebarCollapsedRef.current);
      sidebarCollapseTimerRef.current = null;
    }, 1200);
  };

  // 临时显示分组面板
  const showTemporarily = (groupName) => {
    const targetGroup = groupName || groups.currentGroup;
    const doScroll = () => {
      if (targetGroup) {
        scrollToActiveGroup(targetGroup);
      }
    };

    if (isSidebarMode) {
      if (isOpen && !isSidebarCollapsed) {
        // 已展开，立即滚动并重置关闭定时器
        doScroll();
        scheduleSidebarAutoClose();
        return;
      }

      previousSidebarCollapsedRef.current = isSidebarCollapsed;
      setIsOpen(true);
      setIsSidebarCollapsed(false);
      // 面板已打开但收起时，等展开过渡动画完成（对应 sidebar duration-200）；否则双重 RAF 确保挂载
      if (isOpen && isSidebarCollapsed) {
        setTimeout(doScroll, 200);
      } else {
        requestAnimationFrame(() => {
          requestAnimationFrame(() => {
            doScroll();
          });
        });
      }
      scheduleSidebarAutoClose();
      return;
    }

    const scheduleCloseAfterScroll = () => {
      if (!isPinned && pendingCloseRef.current) {
        if (closeTimerRef.current) {
          clearTimeout(closeTimerRef.current);
        }
        closeTimerRef.current = setTimeout(() => {
          closeTimerRef.current = null;
          handleClose();
        }, 500);
      }
    };
    const doScrollAndScheduleClose = () => {
      if (targetGroup) {
        scrollToActiveGroup(targetGroup, () => {
          scheduleCloseAfterScroll();
        });
      } else {
        scheduleCloseAfterScroll();
      }
    };
    pendingCloseRef.current = true;
    if (!isOpen) {
      setIsOpen(true);
      if (uiAnimationEnabled) {
        setTimeout(doScrollAndScheduleClose, 250);
      } else {
        requestAnimationFrame(() => {
          requestAnimationFrame(() => {
            doScrollAndScheduleClose();
          });
        });
      }
    } else {
      doScrollAndScheduleClose();
    }
  };

  const scrollToActiveGroup = (groupName, onScrollEnd) => {
    const container = listContainerRef.current;
    if (!container || !groupName) {
      onScrollEnd?.();
      return;
    }
    const items = container.querySelectorAll('[data-group-name]');
    let activeEl = null;
    for (let i = 0; i < items.length; i++) {
      if (items[i].dataset.groupName === groupName) {
        activeEl = items[i];
        break;
      }
    }
    if (!activeEl) {
      onScrollEnd?.();
      return;
    }
    const containerRect = container.getBoundingClientRect();
    const elementRect = activeEl.getBoundingClientRect();
    let targetScrollTop = container.scrollTop;
    if (elementRect.top < containerRect.top) {
      targetScrollTop -= containerRect.top - elementRect.top;
    } else if (elementRect.bottom > containerRect.bottom) {
      targetScrollTop += elementRect.bottom - containerRect.bottom;
    }
    if (targetScrollTop !== container.scrollTop) {
      let settled = false;
      const onSettle = () => {
        if (settled) return;
        settled = true;
        onScrollEnd?.();
      };
      container.addEventListener('scrollend', onSettle, { once: true });
      container.scrollTo({ top: targetScrollTop, behavior: 'smooth' });
      setTimeout(() => {
        if (settled) return;
        settled = true;
        container.removeEventListener('scrollend', onSettle);
        onScrollEnd?.();
      }, 500);
    } else {
      onScrollEnd?.();
    }
  };

  // 暴露方法给父组件
  useImperativeHandle(ref, () => ({
    togglePopup,
    togglePin: () => togglePin(null),
    showTemporarily
  }));

  // 鼠标进入触发区或面板
  const handleMouseEnter = () => {
    if (isClosing) {
      return;
    }
    pendingCloseRef.current = false;
    if (closeTimerRef.current) {
      clearTimeout(closeTimerRef.current);
      closeTimerRef.current = null;
    }
    if (isSidebarMode && sidebarCollapseTimerRef.current) {
      clearTimeout(sidebarCollapseTimerRef.current);
      sidebarCollapseTimerRef.current = null;
    }
  };

  // 鼠标离开触发区或面板
  const handleMouseLeave = () => {
    if (isSidebarMode) {
      return;
    }
    if (!isPinned && isOpen && !isClosing) {
      closeTimerRef.current = setTimeout(() => {
        handleClose();
      }, 150);
    }
  };

  const handleToggleSidebarCollapse = e => {
    if (e) {
      e.stopPropagation();
    }

    if (sidebarCollapseTimerRef.current) {
      clearTimeout(sidebarCollapseTimerRef.current);
      sidebarCollapseTimerRef.current = null;
    }

    setIsSidebarCollapsed(prev => !prev);
  };

  // 选择分组
  const handleSelectGroup = groupName => {
    groupsStore.setCurrentGroup(groupName);
    if (activeTab !== 'favorites') {
      if (onTabChange) {
        onTabChange('favorites');
      }
    }
    if (onGroupChange) {
      onGroupChange(groupName);
    }
    if (!isSidebarMode) {
      handleClose();
    }
  };

  // 显示新增分组模态框
  const handleAddGroup = e => {
    e.stopPropagation();
    if (closeTimerRef.current) {
      clearTimeout(closeTimerRef.current);
      closeTimerRef.current = null;
    }
    if (animationTimerRef.current) {
      clearTimeout(animationTimerRef.current);
      animationTimerRef.current = null;
    }
    setIsOpen(false);
    setIsClosing(false);
    setEditingGroup(null);
    setShowModal(true);
  };

  // 显示编辑分组模态框
  const handleEditGroup = (e, group) => {
    e.stopPropagation();
    setEditingGroup(group);
    setShowModal(true);
  };

  // 删除分组
  const handleDeleteGroup = async (e, groupName) => {
    e.stopPropagation();
    const confirmed = await showConfirm(t('groups.confirmDelete', { name: groupName }), t('common.confirm'));
    if (!confirmed) {
      return;
    }
    try {
      const { deleteGroup } = await import('@shared/store/groupsStore');
      await deleteGroup(groupName);
      const { refreshFavorites } = await import('@shared/store/favoritesStore');
      await refreshFavorites();
    } catch (error) {
      console.error('删除分组失败:', error);
      await showError(t('groups.deleteFailed'), t('common.confirm'));
    }
  };

  // 处理拖拽结束
  const handleDragEnd = async (event) => {
    const { active, over } = event;
    
    if (!over || active.id === over.id) {
      return;
    }

    const sortableGroups = groups.groups.filter(g => g.name !== '全部');
    const oldIndex = sortableGroups.findIndex(g => g.name === active.id);
    const newIndex = sortableGroups.findIndex(g => g.name === over.id);

    if (oldIndex !== -1 && newIndex !== -1) {
      const newSortableGroups = arrayMove([...sortableGroups], oldIndex, newIndex);
      const allGroup = groups.groups.find(g => g.name === '全部');
      const newGroups = allGroup ? [allGroup, ...newSortableGroups] : newSortableGroups;
      
      try {
        await reorderGroups(newGroups);

        if (groups.currentGroup === '全部') {
          const { refreshFavorites } = await import('@shared/store/favoritesStore');
          await refreshFavorites();
        }
      } catch (error) {
        console.error('排序失败:', error);
      }
    }
  };

  const renderAllGroupRows = (compactMode = false) => groups.groups.filter(g => g.name === '全部').map(group => {
    const row = (
      <div
        key={group.name}
        data-group-name={group.name}
        onClick={() => handleSelectGroup(group.name)}
        className="group relative w-full"
      >
        <div className={`flex items-center cursor-pointer transition-all w-full ${
          compactMode ? 'justify-center gap-0 px-1.5 py-1' : 'gap-1.5 px-2 py-1'
        } ${
          groups.currentGroup === group.name
            ? 'bg-[var(--qc-accent)] text-[var(--qc-accent-fg)]'
            : 'text-qc-fg hover:bg-qc-hover'
        }`}>
          {!compactMode && (
            <div className="flex-shrink-0 opacity-0">
              <i className="ti ti-grip-vertical" style={{ fontSize: 10 }}></i>
            </div>
          )}
          <div className="flex-shrink-0">
            <i className={group.icon} style={{ fontSize: compactMode ? 14 : 12 }}></i>
          </div>
          {!compactMode && (
            <div className="flex-1 text-[11px] font-medium truncate">
              {group.name}
            </div>
          )}
        </div>
      </div>
    );

    if (compactMode) {
      return (
        <Tooltip key={group.name} content={group.name} placement="right" asChild>
          {row}
        </Tooltip>
      );
    }

    return row;
  });

  const renderSortableGroupList = (compactMode = false) => (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCenter}
      onDragEnd={handleDragEnd}
      modifiers={[restrictToVerticalAxis, restrictToParentElement]}
    >
      <SortableContext
        items={groups.groups.filter(g => g.name !== '全部').map(g => g.name)}
        strategy={verticalListSortingStrategy}
      >
        {groups.groups.filter(g => g.name !== '全部').map(group => (
          <SortableGroupItem
            key={group.name}
            group={group}
            isActive={groups.currentGroup === group.name}
            onSelect={handleSelectGroup}
            onEdit={handleEditGroup}
            onDelete={handleDeleteGroup}
            t={t}
            compact={compactMode}
          />
        ))}
      </SortableContext>
    </DndContext>
  );

  const panelClassName = isTabMode
    ? 'groups-panel absolute top-0 right-0 max-h-[350px] bg-qc-panel border border-qc-border border-t-0 rounded-b-xl rounded-t-none shadow-lg z-[70] overflow-hidden flex flex-col'
    : 'groups-panel absolute bottom-full left-0 right-0 max-h-[350px] backdrop-blur-xl bg-qc-panel border border-b-0 border-qc-border rounded-t-xl shadow-2xl z-40 overflow-hidden flex flex-col';

  const panelStyle = isTabMode
    ? { width: `${tabPanelWidth}px`, right: `${-tabPanelRightOffset}px`, top: `${tabPanelTopOffset}px` }
    : {};

  const panelAnimationClass = uiAnimationEnabled
    ? (isClosing
      ? (isTabMode ? 'animate-dropdown-up' : 'animate-slide-down')
      : (isTabMode ? 'animate-dropdown-down' : 'animate-slide-up'))
    : '';
  const triggerTooltipContent = isTabMode
    ? (groups.currentGroup || t('groups.title'))
    : t('groups.title');

  if (isSidebarMode) {
    if (!isOpen && !showModal) {
      return null;
    }

    return (
      <>
        {isOpen && (
          <div
            ref={rootRef}
            className="groups-panel relative flex h-full min-h-0 flex-col bg-qc-panel border-r border-qc-border shadow-sm transition-all duration-200 overflow-hidden"
            style={{
              width: `${isSidebarCollapsed ? SIDEBAR_GROUP_PANEL_COLLAPSED_WIDTH : SIDEBAR_GROUP_PANEL_WIDTH}px`,
              minWidth: `${isSidebarCollapsed ? SIDEBAR_GROUP_PANEL_COLLAPSED_WIDTH : SIDEBAR_GROUP_PANEL_WIDTH}px`
            }}
            onMouseEnter={handleMouseEnter}
            onMouseLeave={handleMouseLeave}
          >
            <div className={`flex items-center justify-between px-2.5 py-2 border-b border-qc-border w-full ${isSidebarCollapsed ? 'gap-2' : 'gap-3'}`}>
              <div className={`flex items-center min-w-0 ${isSidebarCollapsed ? 'gap-0 justify-center w-full' : 'gap-2'}`}>
                <i className="ti ti-folders" style={{ fontSize: 16 }}></i>
                {!isSidebarCollapsed && (
                  <h3 className="text-xs font-semibold text-qc-fg truncate">
                    {t('groups.title')}
                  </h3>
                )}
              </div>
              {!isSidebarCollapsed && (
                <div className="flex items-center gap-0.5">
                  <Tooltip content={t('groups.add')} placement="bottom" asChild>
                    <button
                      onClick={handleAddGroup}
                      className="w-6 h-6 flex items-center justify-center rounded-md hover:bg-qc-hover transition-all text-qc-fg-muted"
                    >
                      <i className="ti ti-plus" style={{ fontSize: 12 }}></i>
                    </button>
                  </Tooltip>
                </div>
              )}
            </div>

          <div ref={listContainerRef} className={`flex-1 min-h-0 overflow-y-auto py-1 ${isSidebarCollapsed ? 'px-1' : 'px-2'}`}>
            <div className="flex flex-col gap-1 items-stretch w-full min-w-0">
              {renderAllGroupRows(isSidebarCollapsed)}
              {renderSortableGroupList(isSidebarCollapsed)}
            </div>
          </div>

            <div className="mt-auto p-2 pt-1">
              <Tooltip content={isSidebarCollapsed ? '展开分组栏' : '收起分组栏'} placement="right" asChild>
                <button
                  type="button"
                  onClick={handleToggleSidebarCollapse}
                  className={`relative z-10 flex items-center h-9 rounded-lg focus:outline-none transition-all duration-200 ${
                    isSidebarCollapsed
                      ? 'justify-center w-full text-qc-fg-muted hover:bg-qc-hover'
                      : 'justify-start gap-2 px-3 w-full text-qc-fg-muted hover:bg-qc-hover'
                  }`}
                >
                  <i
                    className={isSidebarCollapsed ? 'ti ti-layout-sidebar-right-expand' : 'ti ti-layout-sidebar-right-collapse'}
                    style={{ fontSize: 16 }}
                  />
                  {!isSidebarCollapsed && (
                    <span className="text-[12px] font-medium leading-none whitespace-nowrap">
                      收起
                    </span>
                  )}
                </button>
              </Tooltip>
            </div>
          </div>
        )}

        {showModal && (
          <GroupModal
            group={editingGroup}
            onClose={() => {
              setShowModal(false);
              setEditingGroup(null);
            }}
            onSave={async () => {
              setShowModal(false);
              setEditingGroup(null);
              const { refreshFavorites } = await import('@shared/store/favoritesStore');
              await refreshFavorites();
            }}
          />
        )}
      </>
    );
  }

  return (
    <>
      <div
        ref={rootRef}
        className={isTabMode
          ? 'relative h-full w-[60px] flex items-center justify-center'
          : 'relative flex flex-col h-full w-full'}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
      >
        {/* 弹出面板 */}
        {isOpen && (
          <div className={`${panelClassName} ${panelAnimationClass}`} style={panelStyle}>
            {/* 头部 */}
            <div className="flex items-center justify-between px-2.5 py-2 border-b border-qc-border">
              <h3 className="text-xs font-semibold text-qc-fg">
                {t('groups.title')}
              </h3>
              <div className="flex items-center gap-0.5">
                <Tooltip content={t('groups.add')} placement="bottom" asChild>
                  <button
                    onClick={handleAddGroup}
                    className="w-6 h-6 flex items-center justify-center rounded-md hover:bg-qc-hover transition-all text-qc-fg-muted"
                  >
                    <i className="ti ti-plus" style={{ fontSize: 12 }}></i>
                  </button>
                </Tooltip>
                <Tooltip content={isPinned ? t('groups.unpin') : t('groups.pin')} placement="bottom" asChild>
                  <button
                    onClick={togglePin}
                    className={`w-6 h-6 flex items-center justify-center rounded-md transition-all ${
                      isPinned
                        ? ACTIVE_ICON_BUTTON_CLASS
                        : 'hover:bg-qc-hover text-qc-fg-muted'
                    }`}
                  >
                    {isPinned ? (
                      <i className="ti ti-pinned" style={{ fontSize: 12 }}></i>
                    ) : (
                      <i className="ti ti-pin" style={{ fontSize: 12 }}></i>
                    )}
                  </button>
                </Tooltip>
              </div>
            </div>

            {/* 分组列表 */}
            <div ref={listContainerRef} className="flex-1 overflow-y-auto py-1">
              {groups.groups.filter(g => g.name === '全部').map(group => (
                <div
                  key={group.name}
                  data-group-name={group.name}
                  onClick={() => handleSelectGroup(group.name)}
                  className="group relative"
                >
                  <div className={`flex items-center gap-1.5 px-2 py-1 cursor-pointer transition-all ${
                    groups.currentGroup === group.name
                      ? 'bg-[var(--qc-accent)] text-[var(--qc-accent-fg)]'
                      : 'text-qc-fg hover:bg-qc-hover'
                  }`}>

                    <div className="flex-shrink-0 opacity-0">
                      <i className="ti ti-grip-vertical" style={{ fontSize: 10 }}></i>
                    </div>
                    <div className="flex-shrink-0">
                      <i className={group.icon} style={{ fontSize: 12 }}></i>
                    </div>
                    <div className="flex-1 text-[11px] font-medium truncate">
                      {group.name}
                    </div>
                  </div>
                </div>
              ))}

              <DndContext
                sensors={sensors}
                collisionDetection={closestCenter}
                onDragEnd={handleDragEnd}
                modifiers={[restrictToVerticalAxis, restrictToParentElement]}
              >
                <SortableContext
                  items={groups.groups.filter(g => g.name !== '全部').map(g => g.name)}
                  strategy={verticalListSortingStrategy}
                >
                  {groups.groups.filter(g => g.name !== '全部').map(group => (
                    <SortableGroupItem
                      key={group.name}
                      group={group}
                      isActive={groups.currentGroup === group.name}
                      onSelect={handleSelectGroup}
                      onEdit={handleEditGroup}
                      onDelete={handleDeleteGroup}
                      t={t}
                    />
                  ))}
                </SortableContext>
              </DndContext>
            </div>
          </div>
        )}

        {/* 触发按钮 */}
        <Tooltip content={triggerTooltipContent} placement={isTabMode ? 'bottom' : 'top'} asChild>
          <button
                onClick={togglePopup}
                className={isTabMode
                ? `relative z-10 flex items-center justify-center gap-1 w-[60px] h-7 rounded-lg focus:outline-none transition-all duration-200 ${
                  isOpen
                    ? ACTIVE_ICON_BUTTON_CLASS
                    : 'text-qc-fg-muted hover:bg-qc-hover'
                }`
              : `flex items-center justify-center gap-1.5 w-full h-full px-3 transition-all duration-300 ${
                  isOpen
                    ? 'bg-qc-panel/95 text-qc-fg shadow-lg border border-t-0 border-qc-border'
                    : 'bg-transparent text-qc-fg-muted hover:bg-qc-hover'
                }`}
            type="button"
          >
            <i className="ti ti-folders" style={{ fontSize: 16 }}></i>
            {isTabMode && (
              <span className="text-[11px] font-medium leading-none whitespace-nowrap">
                {t('groups.title') || '分组'}
              </span>
            )}
            {!isTabMode && (
              <span className="text-[10px] font-medium truncate max-w-[60px]">{groups.currentGroup}</span>
            )}
          </button>
        </Tooltip>
      </div>

      {/* 分组模态框 */}
      {showModal && (
        <GroupModal
          group={editingGroup}
          onClose={() => {
            setShowModal(false);
            setEditingGroup(null);
          }}
          onSave={async () => {
            setShowModal(false);
            setEditingGroup(null);
            const { refreshFavorites } = await import('@shared/store/favoritesStore');
            await refreshFavorites();
          }}
        />
      )}
    </>
  );
});

GroupsPopup.displayName = 'GroupsPopup';
export default GroupsPopup;
