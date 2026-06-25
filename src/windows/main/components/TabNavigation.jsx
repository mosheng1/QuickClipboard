import '@tabler/icons-webfont/dist/tabler-icons.min.css';
import { useTranslation } from 'react-i18next';
import { useRef, useEffect, useState, useCallback } from 'react';
import { useSnapshot } from 'valtio';
import { settingsStore } from '@shared/store/settingsStore';
import { normalizeVisibleOptionalTabs } from '@shared/constants/tabVisibility';
import TabButton from './TabButton';
import FilterButton from './FilterButton';
import GroupsPopup from './GroupsPopup';
import Tooltip from '@shared/components/common/Tooltip.jsx';

const FILTER_BUTTON_SIZE = 28;
const FILTER_BUTTON_GAP = 4;
const GROUP_BUTTON_WIDTH = 60;
const FILTER_IDS = ['all', 'text', 'image', 'file', 'link'];

function getCollapsedFilterWidth(filterCount, groupButtonWidth) {
  if (filterCount <= 0) {
    return groupButtonWidth;
  }

  return filterCount * FILTER_BUTTON_SIZE
    + (filterCount - 1) * FILTER_BUTTON_GAP
    + FILTER_BUTTON_GAP
    + groupButtonWidth;
}

function getVisibleFilterCountByWidth(width, groupButtonWidth) {
  for (let count = FILTER_IDS.length; count >= 1; count -= 1) {
    if (width >= getCollapsedFilterWidth(count, groupButtonWidth)) {
      return count;
    }
  }

  return 1;
}

function TabNavigation({
  activeTab,
  onTabChange,
  contentFilter,
  onFilterChange,
  emojiMode,
  onEmojiModeChange,
  onGroupChange,
  groupsPopupRef,
  navigationMode = 'horizontal'
}) {
  const {
    t
  } = useTranslation();
  const settings = useSnapshot(settingsStore);
  const uiAnimationEnabled = settings.uiAnimationEnabled !== false;
  const visibleOptionalTabs = normalizeVisibleOptionalTabs(settings.visibleOptionalTabs);
  const isSidebarLayout = navigationMode === 'sidebar';
  const [isSidebarCollapsed, setIsSidebarCollapsed] = useState(false);
  const [isGroupsPanelOpen, setIsGroupsPanelOpen] = useState(false);
  const tabsRef = useRef({});
  const filtersRef = useRef({});
  const emojiModesRef = useRef({});
  const rightAreaRef = useRef(null);
  const [tabIndicator, setTabIndicator] = useState({
    width: 0,
    left: 0
  });
  const [filterIndicator, setFilterIndicator] = useState({
    width: 0,
    left: 0
  });
  const [emojiModeIndicator, setEmojiModeIndicator] = useState({
    width: 0,
    left: 0
  });
  const [tabAnimationKey, setTabAnimationKey] = useState(0);
  const [filterAnimationKey, setFilterAnimationKey] = useState(0);
  const [emojiModeAnimationKey, setEmojiModeAnimationKey] = useState(0);
  const [isFilterExpanded, setIsFilterExpanded] = useState(false);
  const [collapsedVisibleFilterCount, setCollapsedVisibleFilterCount] = useState(3);
  const [sidebarFixedWidth, setSidebarFixedWidth] = useState(null);
  const sidebarTabsMainRef = useRef(null);
  const filterCollapseTimerRef = useRef(null);

  // 指示器实时跟踪：CSS flex 过渡期间按钮位置持续变化，
  // ResizeObserver 观察尺寸变化 + transitionend 收尾，
  // 直设 DOM style 绕过 React state 的批量更新延迟
  const expandableContainerRef = useRef(null);
  const groupBtnContainerRef = useRef(null);
  const indicatorElRef = useRef(null);
  const isTrackingRef = useRef(false);
  const observerRef = useRef(null);
  const observedTargetRef = useRef(null);
  const contentFilterRef = useRef(contentFilter);
  const prevContentFilterRef = useRef(contentFilter);

  const allTabs = [{
    id: 'clipboard',
    label: t('clipboard.title') || '剪贴板',
    icon: 'ti ti-clipboard-text'
  }, {
    id: 'favorites',
    label: t('favorites.title') || '收藏',
    icon: 'ti ti-star'
  }, {
    id: 'emoji',
    label: t('emoji.title') || '符号',
    icon: 'ti ti-mood-smile'
  }];
  const tabs = allTabs.filter(tab => tab.id === 'clipboard' || visibleOptionalTabs.includes(tab.id));
  const horizontalTabAreaMinPercent = 28;
  const horizontalTabAreaMaxPercent = 50;
  const horizontalTabAreaPercent = allTabs.length <= 1
    ? horizontalTabAreaMaxPercent
    : horizontalTabAreaMinPercent + (tabs.length - 1) * ((horizontalTabAreaMaxPercent - horizontalTabAreaMinPercent) / (allTabs.length - 1));
  const horizontalRightAreaPercent = 100 - horizontalTabAreaPercent;

  const emojiModes = [{
    id: 'emoji',
    label: t('emoji.emoji') || 'Emoji',
    icon: 'ti ti-mood-smile',
    emoji: '😀'
  }, {
    id: 'symbols',
    label: t('emoji.symbols') || '符号',
    icon: 'ti ti-math-symbols'
  }, {
    id: 'images',
    label: t('emoji.images') || '图片',
    icon: 'ti ti-photo-star'
  }];

  const filters = [{
    id: 'all',
    label: t('filter.all') || '全部',
    icon: "ti ti-category"
  }, {
    id: 'text',
    label: t('filter.text') || '文本',
    icon: "ti ti-file-text"
  }, {
    id: 'image',
    label: t('filter.image') || '图片',
    icon: "ti ti-photo"
  }, {
    id: 'file',
    label: t('filter.file') || '文件',
    icon: "ti ti-folder"
  }, {
    id: 'link',
    label: t('filter.link') || '链接',
    icon: "ti ti-link"
  }];

  const isFilterAutoExpanded = collapsedVisibleFilterCount >= 5;
  const expandableFilters = filters.slice(collapsedVisibleFilterCount);
  const useFloatingExpandedFilters = !isFilterAutoExpanded && collapsedVisibleFilterCount <= 2 && expandableFilters.length > 0;
  const shouldStretchHorizontalFilters = !isSidebarLayout && !useFloatingExpandedFilters;
  const shouldExpandFilters = isFilterAutoExpanded || isFilterExpanded;
  const shouldHideGroupButton = !useFloatingExpandedFilters && !isFilterAutoExpanded && shouldExpandFilters;
  const expandedExtraWidth = expandableFilters.length > 0
    ? expandableFilters.length * FILTER_BUTTON_SIZE + (expandableFilters.length - 1) * FILTER_BUTTON_GAP
    : 0;
  const groupButtonWidth = isSidebarLayout ? 92 : GROUP_BUTTON_WIDTH;
  const sidebarShowLabel = isSidebarLayout ? !isSidebarCollapsed : true;

  const updateTabIndicator = useCallback(() => {
    const activeElement = tabsRef.current[activeTab];
    if (activeElement) {
      setTabIndicator({
        width: activeElement.offsetWidth,
        left: activeElement.offsetLeft
      });
    }
  }, [activeTab]);

  const updateFilterIndicator = useCallback(() => {
    const activeElement = filtersRef.current[contentFilter];
    const activeFilterIndex = FILTER_IDS.indexOf(contentFilter);
    const isHiddenInCollapsedState = !shouldExpandFilters && activeFilterIndex >= collapsedVisibleFilterCount;

    if (isHiddenInCollapsedState) {
      setFilterIndicator({ width: 0, left: 0 });
      return;
    }

    if (!activeElement) {
      return;
    }
    setFilterIndicator({
      width: activeElement.offsetWidth,
      left: activeElement.offsetLeft
    });
  }, [contentFilter, shouldExpandFilters, collapsedVisibleFilterCount]);

  const updateFilterIndicatorRef = useRef(updateFilterIndicator);

  // stopTracking: 断开 ResizeObserver，标志位清零。不恢复 CSS transition
  //   （由调用方的 finishTracking 或 Bug 2 路径自行处理）
  const stopTracking = useCallback(() => {
    if (!isTrackingRef.current) return;
    isTrackingRef.current = false;
    observerRef.current?.disconnect();
    observerRef.current = null;
    observedTargetRef.current = null;
  }, []);

  // finishTracking: 同步 React state 到最终 DOM 位置，然后恢复 CSS transition。
  //   transition 恢复推迟到 rAF 避免触发 CSS 属性变化的过渡动画
  const finishTracking = useCallback(() => {
    updateFilterIndicatorRef.current();
    requestAnimationFrame(() => {
      const el = indicatorElRef.current;
      if (el) el.style.transition = '';
    });
  }, []);

  // startTracking: 启动 ResizeObserver，连接指示器到按钮/容器的尺寸变化。
  //   观察策略：按钮在 expandable 容器内 → 观容器（捕获固定宽度按钮位移），
  //   否则 → 观按钮自身（捕获 flex 拉伸尺寸变化）。
  //   sync() 通过 contentFilterRef 间接读取当前按钮，支持跟踪中切换过滤器
  const startTracking = useCallback(() => {
    if (isTrackingRef.current) return;
    const el = indicatorElRef.current;
    const btn = filtersRef.current[contentFilterRef.current];
    if (!el || !btn) return;

    isTrackingRef.current = true;
    el.style.transition = 'none';

    const sync = () => {
      if (!isTrackingRef.current) return;
      const btn = filtersRef.current[contentFilterRef.current];
      if (!btn) return;
      el.style.left = btn.offsetLeft + 'px';
      el.style.width = btn.offsetWidth + 'px';
    };
    sync();

    const obs = new ResizeObserver(sync);
    const container = expandableContainerRef.current;
    if (container && container.contains(btn)) {
      obs.observe(container);
      observedTargetRef.current = container;
    } else {
      obs.observe(btn);
      observedTargetRef.current = btn;
    }
    observerRef.current = obs;
  }, []);

  const updateEmojiModeIndicator = useCallback(() => {
    const activeElement = emojiModesRef.current[emojiMode];
    if (activeElement) {
      setEmojiModeIndicator({
        width: activeElement.offsetWidth,
        left: activeElement.offsetLeft
      });
    }
  }, [emojiMode]);

  useEffect(() => {
    return () => {
      if (filterCollapseTimerRef.current) {
        clearTimeout(filterCollapseTimerRef.current);
        filterCollapseTimerRef.current = null;
      }
    };
  }, []);

  useEffect(() => {
    updateFilterIndicatorRef.current = updateFilterIndicator;
    contentFilterRef.current = contentFilter;
    if (!isTrackingRef.current || !observerRef.current) {
      prevContentFilterRef.current = contentFilter;
      return;
    }
    const btn = filtersRef.current[contentFilter];
    if (!btn) return;
    const container = expandableContainerRef.current;
    const target = container && container.contains(btn) ? container : btn;
    if (target !== observedTargetRef.current) {
      observerRef.current.disconnect();
      observerRef.current.observe(target);
      observedTargetRef.current = target;
    } else if (contentFilter !== prevContentFilterRef.current) {
      const el = indicatorElRef.current;
      if (el) {
        el.style.left = btn.offsetLeft + 'px';
        el.style.width = btn.offsetWidth + 'px';
      }
    }
    prevContentFilterRef.current = contentFilter;
  });

  useEffect(() => {
    if (isSidebarLayout) {
      return undefined;
    }

    const expandable = expandableContainerRef.current;
    const groupBtn = groupBtnContainerRef.current;

    let pending = false;

    const handleEnd = (e) => {
      if (e.propertyName !== 'width') return;
      if (pending) return;
      pending = true;
      requestAnimationFrame(() => {
        stopTracking();
        finishTracking();
        pending = false;
      });
    };

    expandable?.addEventListener('transitionend', handleEnd);
    groupBtn?.addEventListener('transitionend', handleEnd);
    return () => {
      expandable?.removeEventListener('transitionend', handleEnd);
      groupBtn?.removeEventListener('transitionend', handleEnd);
    };
  }, [isSidebarLayout, collapsedVisibleFilterCount, stopTracking]);

  useEffect(() => {
    if (isSidebarLayout) {
      return undefined;
    }

    updateTabIndicator();
    const timer = setTimeout(() => {
      setTabAnimationKey(prev => prev + 1);
    }, 300);
    return () => {
      clearTimeout(timer);
    };
  }, [updateTabIndicator, isSidebarLayout, tabs.length, horizontalTabAreaPercent]);

  useEffect(() => {
    if (!isSidebarLayout) {
      setIsSidebarCollapsed(false);
    }
  }, [isSidebarLayout]);

  useEffect(() => {
    if (isTrackingRef.current) return;
    updateFilterIndicatorRef.current();
  }, [contentFilter, activeTab, horizontalRightAreaPercent, collapsedVisibleFilterCount]);

  // 折叠/展开过渡处理：
  //   Bug 1（可见过滤器）→ 启动 ResizeObserver 实时跟踪
  //   Bug 2（选中过滤器被折叠）→ 立即停跟踪 + 宽度归零
  useEffect(() => {
    if (isSidebarLayout || activeTab === 'emoji') return;

    const activeFilterIndex = FILTER_IDS.indexOf(contentFilter);
    const isHiddenInCollapsed = !shouldExpandFilters && activeFilterIndex >= collapsedVisibleFilterCount;

    if (isHiddenInCollapsed) {
      updateFilterIndicatorRef.current();
      return;
    }

    startTracking();
  }, [shouldExpandFilters, startTracking]);

  useEffect(() => {
    if (activeTab === 'emoji') {
      return undefined;
    }
    const timer = setTimeout(() => {
      setFilterAnimationKey(prev => prev + 1);
    }, 300);
    return () => {
      clearTimeout(timer);
    };
  }, [contentFilter, activeTab]);

  useEffect(() => {
    updateEmojiModeIndicator();
    const timer = setTimeout(() => {
      setEmojiModeAnimationKey(prev => prev + 1);
    }, 300);
    return () => {
      clearTimeout(timer);
    };
  }, [updateEmojiModeIndicator, tabs.length, horizontalRightAreaPercent]);

  useEffect(() => {
    setIsFilterExpanded(false);
  }, [activeTab]);

  useEffect(() => {
    if (isSidebarLayout) {
      setCollapsedVisibleFilterCount(FILTER_IDS.length);
      return undefined;
    }

    if (activeTab === 'emoji') {
      setCollapsedVisibleFilterCount(3);
      return undefined;
    }

    const target = rightAreaRef.current;
    if (!target) {
      return undefined;
    }

    const updateAutoExpanded = () => {
      const width = target.clientWidth;
      const nextCollapsedVisibleCount = getVisibleFilterCountByWidth(width, groupButtonWidth);
      setCollapsedVisibleFilterCount(prev => (prev === nextCollapsedVisibleCount ? prev : nextCollapsedVisibleCount));
    };

    updateAutoExpanded();

    if (typeof ResizeObserver === 'undefined') {
      window.addEventListener('resize', updateAutoExpanded);
      return () => {
        window.removeEventListener('resize', updateAutoExpanded);
      };
    }

    const observer = new ResizeObserver(() => {
      updateAutoExpanded();
    });
    observer.observe(target);
    return () => {
      observer.disconnect();
    };
  }, [activeTab, isSidebarLayout, groupButtonWidth]);

  useEffect(() => {
    if (isFilterAutoExpanded) {
      setIsFilterExpanded(false);
    }
  }, [isFilterAutoExpanded]);

  useEffect(() => {
    if (!isSidebarLayout) {
      setSidebarFixedWidth(null);
      return;
    }

    const updateSidebarWidth = () => {
      const el = sidebarTabsMainRef.current;
      if (!el) return;
      const width = Math.ceil(el.getBoundingClientRect().width);
      if (Number.isFinite(width) && width > 0) {
        setSidebarFixedWidth(width);
      }
    };

    updateSidebarWidth();
    const id = requestAnimationFrame(updateSidebarWidth);
    window.addEventListener('resize', updateSidebarWidth);
    return () => {
      cancelAnimationFrame(id);
      window.removeEventListener('resize', updateSidebarWidth);
    };
  }, [isSidebarLayout, sidebarShowLabel, tabs.length]);

  // 监听窗口大小变化
  useEffect(() => {
    const handleResize = () => {
      if (!isSidebarLayout) {
        updateTabIndicator();
      }
      updateFilterIndicator();
      updateEmojiModeIndicator();
    };
    window.addEventListener('resize', handleResize);
    handleResize();
    return () => {
      window.removeEventListener('resize', handleResize);
    };
  }, [updateTabIndicator, updateFilterIndicator, updateEmojiModeIndicator, isSidebarLayout]);

  const handleEmojiModeChange = (id) => {
    onEmojiModeChange(id);
  };

  const handleFilterAreaMouseEnter = () => {
    if (isFilterAutoExpanded) {
      return;
    }
    if (filterCollapseTimerRef.current) {
      clearTimeout(filterCollapseTimerRef.current);
      filterCollapseTimerRef.current = null;
    }
    setIsFilterExpanded(true);
  };

  const handleFilterAreaMouseLeave = (event) => {
    if (isFilterAutoExpanded) {
      return;
    }
    const nextTarget = event?.relatedTarget;
    if (nextTarget instanceof Node && event.currentTarget.contains(nextTarget)) {
      return;
    }
    if (filterCollapseTimerRef.current) {
      clearTimeout(filterCollapseTimerRef.current);
    }
    filterCollapseTimerRef.current = setTimeout(() => {
      setIsFilterExpanded(false);
      filterCollapseTimerRef.current = null;
    }, 180);
  };

  const renderSidebarButton = ({
    id,
    label,
    icon,
    emoji,
    isActive,
    onClick,
    buttonRef,
    showLabel = true
  }) => {
    const handleClick = () => {
      onClick(id);
    };

    return (
      <div ref={buttonRef} className={showLabel ? 'relative inline-flex h-9 w-full' : 'relative inline-flex h-9 w-10'}>
        <Tooltip content={label} placement="right" asChild>
          <button
            onClick={handleClick}
            className={`
              relative z-10 flex items-center h-9 rounded-lg
              ${showLabel ? 'justify-start gap-2 px-3 w-full min-w-0 whitespace-nowrap' : 'justify-center gap-0 px-0 w-10'}
              focus:outline-none
              ${uiAnimationEnabled ? 'hover:scale-[1.01]' : ''}
              ${isActive
                ? 'bg-blue-500 text-white shadow-md hover:bg-blue-500'
                : 'text-qc-fg-muted hover:bg-qc-hover'}
            `}
            style={uiAnimationEnabled ? {
              transitionProperty: 'transform, box-shadow, background-color, color',
              transitionDuration: '200ms, 200ms, 500ms, 500ms'
            } : {}}
          >
            {emoji ? <span style={{ fontSize: 16 }}>{emoji}</span> : <i className={icon} style={{ fontSize: 16 }} />}
            {showLabel && (
              <span className="text-[12px] font-medium leading-none truncate flex-1 min-w-0 text-left">
                {label}
              </span>
            )}
          </button>
        </Tooltip>
      </div>
    );
  };

  if (isSidebarLayout) {
    return <div className="tab-navigation flex-shrink-0 h-full w-fit min-w-fit bg-qc-panel shadow-sm transition-colors duration-500 tab-bar">
      <div className="flex h-full min-h-0 w-fit">
        <div
          className="flex h-full min-h-0 flex-col border-r border-qc-border"
          style={sidebarFixedWidth ? { width: `${sidebarFixedWidth}px`, minWidth: `${sidebarFixedWidth}px` } : undefined}
        >
          <div ref={sidebarTabsMainRef} className="grid grid-cols-[max-content] gap-1 p-2 pb-2 w-max justify-items-stretch">
            {tabs.map((tab, index) => (
              <TabButton
                key={tab.id}
                id={tab.id}
                label={tab.label}
                icon={tab.icon}
                badgeCount={0}
                isActive={activeTab === tab.id}
                onClick={onTabChange}
                index={index}
                buttonRef={el => { tabsRef.current[tab.id] = el; }}
                navigationMode="sidebar"
                showLabel={sidebarShowLabel}
              />
            ))}
          </div>

          <div
            className="mx-2 h-px shrink-0"
            style={{ backgroundColor: 'var(--bg-titlebar-border, var(--qc-border-strong))', opacity: 0.95 }}
          />

          <div className="flex-1 min-h-0 overflow-y-auto px-2 py-2 w-full min-w-0">
            <div className="grid grid-cols-1 gap-1 w-full min-w-0 justify-items-stretch">
              {activeTab === 'emoji'
                ? emojiModes.map(mode => renderSidebarButton({
                    id: mode.id,
                    label: mode.label,
                    icon: mode.icon,
                    emoji: mode.emoji,
                    isActive: emojiMode === mode.id,
                    onClick: handleEmojiModeChange,
                    showLabel: sidebarShowLabel,
                    buttonRef: el => {
                      emojiModesRef.current[mode.id] = el;
                    }
                  }))
                : filters.map(filter => renderSidebarButton({
                    id: filter.id,
                    label: filter.label,
                    icon: filter.icon,
                    isActive: contentFilter === filter.id,
                    onClick: onFilterChange,
                    showLabel: sidebarShowLabel,
                    buttonRef: el => {
                      filtersRef.current[filter.id] = el;
                    }
                  }))
              }
            </div>
          </div>


            <>
              <div
                className="mx-2 h-px shrink-0"
                style={{ backgroundColor: 'var(--bg-titlebar-border, var(--qc-border-strong))', opacity: 0.95 }}
              />
              <div className="px-2 py-2">
                <Tooltip content="分组" placement="right" asChild>
                  <button
                    type="button"
                    onClick={() => groupsPopupRef.current?.togglePopup?.()}
                    className={`relative z-10 flex items-center h-9 rounded-lg focus:outline-none transition-all duration-200 ${
                      sidebarShowLabel
                        ? `justify-start gap-2 px-3 w-full ${
                            isGroupsPanelOpen
                              ? 'qc-active-icon-button bg-[var(--qc-accent)] text-[var(--qc-accent-fg)] shadow-md hover:bg-[var(--qc-accent)]'
                              : 'text-qc-fg-muted hover:bg-qc-hover'
                          }`
                        : `justify-start gap-2 px-3 w-10 overflow-hidden ${
                            isGroupsPanelOpen
                              ? 'qc-active-icon-button bg-[var(--qc-accent)] text-[var(--qc-accent-fg)] shadow-md hover:bg-[var(--qc-accent)]'
                              : 'text-qc-fg-muted hover:bg-qc-hover'
                          }`
                    }`}
                  >
                    <i className="ti ti-folders" style={{ fontSize: 16 }} />
                    {sidebarShowLabel && (
                      <span className="text-[12px] font-medium leading-none whitespace-nowrap">
                        分组
                      </span>
                    )}
                  </button>
                </Tooltip>
              </div>
            </>

          <div className="mt-auto p-2 pt-1">
            <Tooltip content={sidebarShowLabel ? '收起侧边栏' : '展开侧边栏'} placement="right" asChild>
              <button
                type="button"
                onClick={() => setIsSidebarCollapsed(prev => !prev)}
                className={`relative z-10 flex items-center h-9 rounded-lg focus:outline-none transition-all duration-200 ${
                  sidebarShowLabel
                    ? 'justify-start gap-2 px-3 w-full text-qc-fg-muted hover:bg-qc-hover'
                    : 'justify-start gap-2 px-3 w-10 text-qc-fg-muted hover:bg-qc-hover overflow-hidden'
                }`}
              >
                <i
                  className={isSidebarCollapsed ? 'ti ti-layout-sidebar-right-expand' : 'ti ti-layout-sidebar-left-collapse'}
                  style={{ fontSize: 16 }}
                />
                {sidebarShowLabel && (
                  <span className="text-[12px] font-medium leading-none whitespace-nowrap">
                    收起
                  </span>
                )}
              </button>
            </Tooltip>
          </div>
        </div>

        <div className="flex h-full min-h-0 shrink-0">
          <GroupsPopup
            ref={groupsPopupRef}
            activeTab={activeTab}
            onTabChange={onTabChange}
            onGroupChange={onGroupChange}
            onOpenChange={setIsGroupsPanelOpen}
            mode="sidebar"
          />
        </div>
      </div>
    </div>;
  }

  return <div className={`tab-navigation flex-shrink-0 bg-qc-panel shadow-sm transition-colors duration-500 tab-bar ${
    isSidebarLayout
      ? 'w-[190px] min-w-[190px] h-full border-r border-qc-border'
      : 'border-b border-qc-border'
  }`}>
    <div className={isSidebarLayout ? 'flex h-full min-h-0 flex-col' : 'flex items-stretch h-9 whitespace-nowrap'}>
      <div
        className={isSidebarLayout ? 'flex flex-col gap-1 p-2 pb-1' : 'flex items-center px-2 relative min-w-0'}
        style={!isSidebarLayout ? {
          flex: `0 0 calc(${horizontalTabAreaPercent}% - 1px)`
        } : undefined}
      >
        <div className={isSidebarLayout ? 'flex flex-col gap-1 w-full' : 'flex items-center justify-center gap-1 w-full relative'}>
          {!isSidebarLayout && (
            <div className={`absolute rounded-lg pointer-events-none ${uiAnimationEnabled ? 'transition-all duration-300 ease-out' : ''}`} style={{
              width: `${tabIndicator.width}px`,
              height: '28px',
              left: `${tabIndicator.left}px`,
              top: '50%',
              transform: 'translateY(-50%)'
            }}>
              <div key={`tab-bounce-${tabAnimationKey}`} className={`w-full h-full rounded-lg bg-[var(--qc-accent)] ${uiAnimationEnabled ? 'animate-button-bounce' : ''}`} />
            </div>
          )}
          {tabs.map((tab, index) => (
            <TabButton
              key={tab.id}
              id={tab.id}
              label={tab.label}
              icon={tab.icon}
              badgeCount={0}
              isActive={activeTab === tab.id}
              onClick={onTabChange}
              index={index}
              buttonRef={el => tabsRef.current[tab.id] = el}
              navigationMode={isSidebarLayout ? 'sidebar' : 'horizontal'}
            />
          ))}
        </div>
      </div>

      {!isSidebarLayout && (
        <div
          className="w-[1.5px] my-1.5 shrink-0"
          style={{ backgroundColor: 'var(--bg-titlebar-border, var(--qc-border-strong))', opacity: 0.95 }}
        />
      )}

      <div
        ref={rightAreaRef}
        className={isSidebarLayout ? 'tab-navigation-right flex-1 flex items-end px-2 pb-2 relative min-w-0' : 'tab-navigation-right flex items-center pl-1 pr-1 relative min-w-0'}
        style={!isSidebarLayout ? {
          flex: `0 0 calc(${horizontalRightAreaPercent}% - 1px)`
        } : undefined}
      >
        <div
          className={`flex min-w-0 max-w-full items-center gap-1 relative overflow-visible ${
            activeTab === 'emoji' || isFilterAutoExpanded
              ? 'w-full justify-center'
              : 'w-full'
          }`}
          onMouseLeave={activeTab === 'emoji' ? undefined : handleFilterAreaMouseLeave}
        >
          {!isSidebarLayout && (
            <div ref={indicatorElRef} className={`absolute rounded-lg pointer-events-none ${uiAnimationEnabled ? 'transition-all duration-300 ease-out' : ''}`} style={{
              width: `${activeTab === 'emoji' ? emojiModeIndicator.width : filterIndicator.width}px`,
              height: '28px',
              left: `${activeTab === 'emoji' ? emojiModeIndicator.left : filterIndicator.left}px`,
              top: '50%',
              transform: 'translateY(-50%)'
            }}>
              <div key={activeTab === 'emoji' ? `emoji-mode-bounce-${emojiModeAnimationKey}` : `filter-bounce-${filterAnimationKey}`} className={`w-full h-full rounded-lg bg-[var(--qc-accent)] ${uiAnimationEnabled ? 'animate-button-bounce' : ''}`} />
            </div>
          )}
          {activeTab === 'emoji'
            ? emojiModes.map(mode => (
                <div key={mode.id} ref={el => emojiModesRef.current[mode.id] = el} className="relative flex-1 h-7">
                  <Tooltip content={mode.label} placement={isSidebarLayout ? 'right' : 'bottom'} asChild>
                    <button
                      onClick={() => handleEmojiModeChange(mode.id)}
                      className={`relative z-10 flex items-center justify-center w-full h-full rounded-lg focus:outline-none ${uiAnimationEnabled ? 'hover:scale-105' : ''} ${
                        emojiMode === mode.id
                          ? 'qc-active-icon-button bg-[var(--qc-accent)] text-[var(--qc-accent-fg)] shadow-md hover:bg-[var(--qc-accent)]'
                          : 'text-qc-fg-muted hover:bg-qc-hover'
                      }`}
                      style={uiAnimationEnabled ? {
                        transitionProperty: 'transform, box-shadow, background-color, color',
                        transitionDuration: '200ms, 200ms, 500ms, 500ms'
                      } : {}}
                    >
                      {mode.emoji ? <span style={{ fontSize: 16 }}>{mode.emoji}</span> : <i className={mode.icon} style={{ fontSize: 16 }} />}
                    </button>
                  </Tooltip>
                </div>
              ))
            : (
                <>
                  {isFilterAutoExpanded ? (
                    <div className="flex items-center gap-1 flex-1 min-w-0" onMouseEnter={handleFilterAreaMouseEnter}>
                      {filters.map(filter => (
                        <FilterButton
                          key={filter.id}
                          id={filter.id}
                          label={filter.label}
                          icon={filter.icon}
                          isActive={contentFilter === filter.id}
                          onClick={onFilterChange}
                          stretch={shouldStretchHorizontalFilters}
                          buttonRef={el => {
                            filtersRef.current[filter.id] = el;
                          }}
                        />
                      ))}
                    </div>
                  ) : (
                    <div className="relative flex items-center gap-1 flex-1 min-w-0" onMouseEnter={handleFilterAreaMouseEnter}>
                      {filters.slice(0, collapsedVisibleFilterCount).map(filter => (
                        <FilterButton
                          key={filter.id}
                          id={filter.id}
                          label={filter.label}
                          icon={filter.icon}
                          isActive={contentFilter === filter.id}
                          onClick={onFilterChange}
                          stretch={shouldStretchHorizontalFilters}
                          buttonRef={el => {
                            filtersRef.current[filter.id] = el;
                          }}
                        />
                      ))}

                      {useFloatingExpandedFilters ? (
                        <div
                          className={`absolute right-0 top-[calc(100%+6px)] z-[75] box-content flex w-7 flex-col items-center gap-1 rounded-lg border border-qc-border bg-qc-panel py-1 shadow-lg ${
                            uiAnimationEnabled ? 'transition-all duration-200 ease-out' : ''
                          }`}
                          style={{
                            opacity: shouldExpandFilters ? 1 : 0,
                            transform: shouldExpandFilters ? 'translateY(0)' : 'translateY(-4px)',
                            pointerEvents: shouldExpandFilters ? 'auto' : 'none'
                          }}
                        >
                          {expandableFilters.map(filter => (
                            <FilterButton
                              key={filter.id}
                              id={filter.id}
                              label={filter.label}
                              icon={filter.icon}
                              isActive={contentFilter === filter.id}
                              onClick={onFilterChange}
                              tooltipPlacement="left"
                              buttonRef={el => {
                                filtersRef.current[filter.id] = el;
                              }}
                            />
                          ))}
                        </div>
                      ) : (
                        <div
                          ref={expandableContainerRef}
                          className={`flex items-center gap-1 overflow-hidden shrink-0 min-w-0 ${uiAnimationEnabled ? 'transition-all duration-300 ease-out' : ''}`}
                          style={{
                            width: shouldExpandFilters ? `${expandedExtraWidth}px` : '0px',
                            opacity: shouldExpandFilters ? 1 : 0,
                            pointerEvents: shouldExpandFilters ? 'auto' : 'none'
                          }}
                        >
                          {expandableFilters.map(filter => (
                            <FilterButton
                              key={filter.id}
                              id={filter.id}
                              label={filter.label}
                              icon={filter.icon}
                              isActive={contentFilter === filter.id}
                              onClick={onFilterChange}
                              stretch={false}
                              buttonRef={el => {
                                filtersRef.current[filter.id] = el;
                              }}
                            />
                          ))}
                        </div>
                      )}
                    </div>
                  )}

                  <div
                    ref={groupBtnContainerRef}
                    className={`overflow-visible shrink-0 ${uiAnimationEnabled ? 'transition-all duration-300 ease-out' : ''}`}
                    style={{
                      width: shouldHideGroupButton ? '0px' : `${groupButtonWidth}px`,
                      opacity: shouldHideGroupButton ? 0 : 1,
                      pointerEvents: shouldHideGroupButton ? 'none' : 'auto'
                    }}
                  >
                    <GroupsPopup
                      ref={groupsPopupRef}
                      activeTab={activeTab}
                      onTabChange={onTabChange}
                      onGroupChange={onGroupChange}
                      onOpenChange={setIsGroupsPanelOpen}
                      mode={isSidebarLayout ? 'tab-sidebar' : 'tab'}
                    />
                  </div>
                </>
              )
          }
        </div>
      </div>
    </div>
  </div>;
}
export default TabNavigation;
