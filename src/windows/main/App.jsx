import { useState, useEffect, useRef, lazy, Suspense } from 'react';
import { useTranslation } from 'react-i18next';
import { useSnapshot } from 'valtio';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { settingsStore } from '@shared/store/settingsStore';
import { groupsStore } from '@shared/store/groupsStore';
import { navigationStore } from '@shared/store/navigationStore';
import { clipboardStore } from '@shared/store/clipboardStore';
import { favoritesStore } from '@shared/store/favoritesStore';
import { useWindowDrag } from '@shared/hooks/useWindowDrag';
import { useTheme, applyThemeToBody } from '@shared/hooks/useTheme';
import { useSettingsSync } from '@shared/hooks/useSettingsSync';
import { useNavigationKeyboard } from '@shared/hooks/useNavigationKeyboard';
import { useWindowAnimation } from '@shared/hooks/useWindowAnimation';
import { applyBackgroundImage, clearBackgroundImage } from '@shared/utils/backgroundManager';
import { getUpdateBannerState } from '@shared/api/settings';
import { promptDisableWinVHotkeyIfNeeded } from '@shared/api/system';
import { toggleWindowPin } from '@shared/services/titleBarActions';
import { getVisibleMainTabs, isMainTabVisible } from '@shared/constants/tabVisibility';
import { toast, TOAST_POSITIONS, TOAST_SIZES } from '@shared/store/toastStore';
import { formatUserMessage } from '@shared/utils/userMessages';
import TitleBar from './components/TitleBar';
import TabNavigation from './components/TabNavigation';
import ClipboardTab from './components/ClipboardTab';
import FavoritesTab from './components/FavoritesTab';
const EmojiTab = lazy(() => import('./components/EmojiTab'));
import MultiSelectActionBar from './components/MultiSelectActionBar';
import WindowResizeHandles from './components/WindowResizeHandles';
import ToastContainer from '@shared/components/common/ToastContainer';

const TAB_NAVIGATION_MODE = {
  HORIZONTAL: 'horizontal',
  SIDEBAR: 'sidebar'
};
const WEBDAV_TOAST_CONFIG = {
  size: TOAST_SIZES.EXTRA_SMALL,
  position: TOAST_POSITIONS.BOTTOM_RIGHT
};

function App() {
  const {
    t
  } = useTranslation();
  const settings = useSnapshot(settingsStore);
  const {
    theme,
    lightThemeStyle,
    darkThemeStyle,
    backgroundImagePath
  } = settings;
  const visibleTabs = getVisibleMainTabs(settings.visibleOptionalTabs);
  const {
    effectiveTheme,
    isDark,
    isBackground
  } = useTheme();
  const [activeTab, setActiveTab] = useState('clipboard');
  const [contentFilter, setContentFilter] = useState('all');
  const [searchQuery, setSearchQuery] = useState('');
  const [emojiMode, setEmojiMode] = useState('emoji'); // 'emoji' | 'symbols' | 'images'
  const [updateBannerState, setUpdateBannerState] = useState(null);
  const [windowWidth, setWindowWidth] = useState(() => (typeof window !== 'undefined' ? window.innerWidth : 0));
  const clipboardTabRef = useRef(null);
  const favoritesTabRef = useRef(null);
  const groupsPopupRef = useRef(null);
  const searchRef = useRef(null);
  const sidebarTabsBreakpoint = 550;
  const tabNavigationMode = windowWidth >= sidebarTabsBreakpoint
    ? TAB_NAVIGATION_MODE.SIDEBAR
    : TAB_NAVIGATION_MODE.HORIZONTAL;
  const isSidebarTabsLayout = tabNavigationMode === TAB_NAVIGATION_MODE.SIDEBAR;

  // 监听设置变更事件
  useSettingsSync();

  useEffect(() => {
    if (!isMainTabVisible(activeTab, settings.visibleOptionalTabs)) {
      setActiveTab('clipboard');
    }
  }, [activeTab, settings.visibleOptionalTabs]);

  useEffect(() => {
    let mounted = true;

    const loadUpdateBannerState = async () => {
      try {
        const state = await getUpdateBannerState();
        if (mounted) {
          setUpdateBannerState(state || null);
        }
      } catch (error) {
        console.error('获取更新提示状态失败:', error);
      }
    };

    const unlistenPromise = listen('update-banner-state-changed', (event) => {
      setUpdateBannerState(event.payload || null);
    });

    loadUpdateBannerState();

    return () => {
      mounted = false;
      unlistenPromise.then((unlisten) => unlisten()).catch(() => {});
    };
  }, []);

  // 启动时检查 Win+V
  useEffect(() => {
    const checkWinV = async () => {
      try {
        if (settings.toggleShortcut === 'Win+V') {
          await promptDisableWinVHotkeyIfNeeded();
        }
      } catch (error) {
        console.error('调用 Win+V 禁用提示命令失败:', error);
      }
    };

    checkWinV();
  }, []);

  // 窗口动画
  useWindowAnimation();

  // 监听窗口宽度，切换 Tab 栏布局模式
  useEffect(() => {
    const updateWindowWidth = () => {
      setWindowWidth(typeof window !== 'undefined' ? window.innerWidth : 0);
    };

    updateWindowWidth();
    window.addEventListener('resize', updateWindowWidth);
    return () => {
      window.removeEventListener('resize', updateWindowWidth);
    };
  }, []);

  // 同步当前标签页到导航store
  useEffect(() => {
    navigationStore.setActiveTab(activeTab);
    clipboardStore.exitMultiSelectMode();
    favoritesStore.exitMultiSelectMode();
  }, [activeTab]);
  useEffect(() => {
    const setupListeners = async () => {
      const handleWindowShow = async () => {
        try {
          const { saveCurrentFocus } = await import('@shared/api/window');
          await saveCurrentFocus();
        } catch (err) {
          console.warn('保存焦点失败:', err);
        }
        
        if (settingsStore.autoClearSearch) {
          setSearchQuery('');
        }
        if (settingsStore.autoFocusSearch) {
            setTimeout(() => {
                searchRef.current?.focus?.();
            }, 200);
        }
      };
      const handleWindowHide = () => {
        searchRef.current?.blur?.();
      };
      const unlisten1 = await listen('window-show-animation', handleWindowShow);
      const unlisten2 = await listen('edge-snap-show', handleWindowShow);
      const unlisten3 = await listen('paste-plain-text-selected', () => {
        if (activeTab === 'clipboard' && clipboardTabRef.current?.executePlainTextPaste) {
          clipboardTabRef.current.executePlainTextPaste();
        } else if (activeTab === 'favorites' && favoritesTabRef.current?.executePlainTextPaste) {
          favoritesTabRef.current.executePlainTextPaste();
        }
      });
      const unlisten4 = await listen('webdav-window-show-pull-report', event => {
        const result = event.payload || {};
        const pulled = result?.pulled || 0;
        if (pulled <= 0) {
          return;
        }

        toast.success(t('settings.webdav.successWithDetail', {
          title: t('settings.webdav.autoPullOnWindowShowComplete'),
          detail: t('settings.webdav.syncResultDetail', {
            total: pulled,
            clipboard: result?.pulled_clipboard || 0,
            favorites: result?.pulled_favorites || 0,
            groups: result?.pulled_groups || 0,
          }),
        }), { ...WEBDAV_TOAST_CONFIG, duration: 5000 });
      });
      const unlisten5 = await listen('webdav-window-show-pull-error', event => {
        console.error('主窗口显示时 WebDAV 自动拉取失败:', event.payload);
        toast.error(formatUserMessage(event.payload, t, 'errors.webdav.operationFailed'), { ...WEBDAV_TOAST_CONFIG, duration: 6000 });
      });
      const unlisten6 = await listen('window-hide-animation', handleWindowHide);
      const unlisten7 = await listen('edge-snap-hide', handleWindowHide);
      
      return () => {
        unlisten1();
        unlisten2();
        unlisten3();
        unlisten4();
        unlisten5();
        unlisten6();
        unlisten7();
      };
    };
    let cleanup = setupListeners();
    return () => cleanup.then(fn => fn());
  }, [activeTab, t]);

  useEffect(() => {
    let refreshingTopmost = false;
    let lastRefreshAt = 0;

    const refreshMainWindowTopmost = async () => {
      const now = Date.now();
      if (refreshingTopmost || now - lastRefreshAt < 200) {
        return;
      }

      refreshingTopmost = true;
      lastRefreshAt = now;

      try {
        await invoke('raise_main_window_topmost');
      } catch (err) {
        console.warn('刷新主窗口置顶失败:', err);
      } finally {
        refreshingTopmost = false;
      }
    };

    const handleMouseEnter = async () => {
      await refreshMainWindowTopmost();

      try {
        const { saveCurrentFocus } = await import('@shared/api/window');
        await saveCurrentFocus();
      } catch (err) {
        console.warn('鼠标进入时保存焦点失败:', err);
      }
    };

    const handlePointerDown = () => {
      refreshMainWindowTopmost();
    };

    const root = document.documentElement;
    root.addEventListener('pointerenter', handleMouseEnter);
    document.addEventListener('pointerdown', handlePointerDown, true);

    return () => {
      root.removeEventListener('pointerenter', handleMouseEnter);
      document.removeEventListener('pointerdown', handlePointerDown, true);
    };
  }, []);
  useEffect(() => {
    let resizeTimer = null;
    let moveTimer = null;
    const handleResize = async () => {
      if (!settingsStore.rememberWindowSize) {
        return;
      }
      if (resizeTimer) clearTimeout(resizeTimer);
      resizeTimer = setTimeout(async () => {
        try {
          const appWindow = getCurrentWindow();
          const size = await appWindow.innerSize();
          const scaleFactor = await appWindow.scaleFactor();
          const logicalWidth = Math.max(1, Math.round(size.width / Math.max(scaleFactor, 1)));
          const logicalHeight = Math.max(1, Math.round(size.height / Math.max(scaleFactor, 1)));
          const {
            saveWindowSize
          } = await import('@shared/api/settings');
          await saveWindowSize(logicalWidth, logicalHeight);
        } catch (error) {
          console.error('保存窗口大小失败:', error);
        }
      }, 500);
    };
    const handleMove = async () => {
      if (settingsStore.windowPositionMode !== 'remember') {
        return;
      }
      if (moveTimer) clearTimeout(moveTimer);
      moveTimer = setTimeout(async () => {
        try {
          const appWindow = getCurrentWindow();
          const position = await appWindow.outerPosition();
          const {
            saveWindowPosition
          } = await import('@shared/api/settings');
          await saveWindowPosition(position.x, position.y);
        } catch (error) {
          console.error('保存窗口位置失败:', error);
        }
      }, 500);
    };
    const setupListeners = async () => {
      const appWindow = getCurrentWindow();
      const unlistenResize = await appWindow.onResized(handleResize);
      const unlistenMove = await appWindow.onMoved(handleMove);
      return () => {
        unlistenResize();
        unlistenMove();
      };
    };
    let cleanup = setupListeners();
    return () => {
      if (resizeTimer) clearTimeout(resizeTimer);
      if (moveTimer) clearTimeout(moveTimer);
      cleanup.then(fn => fn());
    };
  }, []);

  // 主内容区域拖拽，排除所有交互元素和列表项
  const contentDragRef = useWindowDrag({
    excludeSelectors: ['[data-no-drag]', 'button', '[role="button"]', 'a', 'input', 'textarea'],
    allowChildren: true
  });

  // 应用主题到body
  useEffect(() => {
    applyThemeToBody(theme, 'main');
  }, [theme, lightThemeStyle, darkThemeStyle, effectiveTheme]);

  // 应用背景图片（仅在背景主题时）
  useEffect(() => {
    if (isBackground && backgroundImagePath) {
      applyBackgroundImage({
        containerSelector: '.main-container',
        backgroundImagePath,
        windowName: 'main'
      });
    } else {
      clearBackgroundImage('.main-container');
    }
  }, [isBackground, backgroundImagePath]);

  // 处理分组切换
  const handleGroupChange = async groupName => {
    groupsStore.setCurrentGroup(groupName);
    // 重置导航索引
    navigationStore.resetNavigation();
    // 重新加载收藏列表
    const {
      initFavorites
    } = await import('@shared/store/favoritesStore');
    await initFavorites(groupName);
  };

  // 导航键盘事件处理
  const blurSearchInput = () => {
    searchRef.current?.blur?.();
  };
  const handleNavigateUp = () => {
    blurSearchInput();
    if (activeTab === 'clipboard' && clipboardTabRef.current?.navigateUp) {
      clipboardTabRef.current.navigateUp();
    } else if (activeTab === 'favorites' && favoritesTabRef.current?.navigateUp) {
      favoritesTabRef.current.navigateUp();
    }
  };
  const handleNavigateDown = () => {
    blurSearchInput();
    if (activeTab === 'clipboard' && clipboardTabRef.current?.navigateDown) {
      clipboardTabRef.current.navigateDown();
    } else if (activeTab === 'favorites' && favoritesTabRef.current?.navigateDown) {
      favoritesTabRef.current.navigateDown();
    }
  };
  const handleExecuteItem = () => {
    blurSearchInput();
    if (activeTab === 'clipboard' && clipboardTabRef.current?.executeCurrentItem) {
      clipboardTabRef.current.executeCurrentItem();
    } else if (activeTab === 'favorites' && favoritesTabRef.current?.executeCurrentItem) {
      favoritesTabRef.current.executeCurrentItem();
    }
  };
  const handleTabLeft = () => {
    setActiveTab(currentTab => {
      const tabs = visibleTabs;
      const currentIndex = tabs.indexOf(currentTab);
      if (currentIndex === -1) return tabs[tabs.length - 1];
      return tabs[currentIndex === 0 ? tabs.length - 1 : currentIndex - 1];
    });
  };
  const handleTabRight = () => {
    setActiveTab(currentTab => {
      const tabs = visibleTabs;
      const currentIndex = tabs.indexOf(currentTab);
      if (currentIndex === -1) return tabs[0];
      return tabs[currentIndex === tabs.length - 1 ? 0 : currentIndex + 1];
    });
  };
  const handleToggleSearch = () => {
    if (searchRef.current?.toggleFocus) {
      searchRef.current.toggleFocus();
    }
  };

  // 固定/取消固定窗口
  const handleTogglePin = async () => {
    try {
      await toggleWindowPin();
    } catch (error) {
      console.error('切换窗口固定状态失败:', error);
    }
  };

  // 切换到上一个分组
  const handlePreviousGroup = () => {
    if (!isMainTabVisible('favorites', settings.visibleOptionalTabs)) {
      return;
    }
    if (activeTab !== 'favorites') {
      setActiveTab('favorites');
    }
    const groups = groupsStore.groups;
    if (groups.length === 0) return;
    const currentIndex = groups.findIndex(g => g.name === groupsStore.currentGroup);
    const prevIndex = currentIndex <= 0 ? groups.length - 1 : currentIndex - 1;
    const prevGroup = groups[prevIndex];
    groupsStore.setCurrentGroup(prevGroup.name);
    handleGroupChange(prevGroup.name);
    if (groupsPopupRef.current?.showTemporarily) {
      groupsPopupRef.current.showTemporarily(prevGroup.name);
    }
  };

  // 切换到下一个分组
  const handleNextGroup = () => {
    if (!isMainTabVisible('favorites', settings.visibleOptionalTabs)) {
      return;
    }
    if (activeTab !== 'favorites') {
      setActiveTab('favorites');
    }
    const groups = groupsStore.groups;
    if (groups.length === 0) return;
    const currentIndex = groups.findIndex(g => g.name === groupsStore.currentGroup);
    const nextIndex = currentIndex >= groups.length - 1 ? 0 : currentIndex + 1;
    const nextGroup = groups[nextIndex];
    groupsStore.setCurrentGroup(nextGroup.name);
    handleGroupChange(nextGroup.name);
    if (groupsPopupRef.current?.showTemporarily) {
      groupsPopupRef.current.showTemporarily(nextGroup.name);
    }
  };

  // 设置全局键盘导航
  useNavigationKeyboard({
    onNavigateUp: handleNavigateUp,
    onNavigateDown: handleNavigateDown,
    onExecuteItem: handleExecuteItem,
    onTabLeft: handleTabLeft,
    onTabRight: handleTabRight,
    onToggleSearch: handleToggleSearch,
    onTogglePin: handleTogglePin,
    onPreviousGroup: handlePreviousGroup,
    onNextGroup: handleNextGroup,
    enabled: true
  });
  const outerContainerClasses = `
    h-screen w-screen 
    relative
    ${isDark ? 'dark' : ''}
  `.trim().replace(/\s+/g, ' ');
  const containerClasses = `
    main-container 
    h-full w-full
    flex ${settings.titleBarPosition === 'left' || settings.titleBarPosition === 'right' ? 'flex-row' : 'flex-col'}
    overflow-hidden
    transition-colors duration-500 ease-in-out
    bg-qc-surface
  `.trim().replace(/\s+/g, ' ');
  const TitleBarComponent = <TitleBar ref={searchRef} searchQuery={searchQuery} onSearchChange={setSearchQuery} searchPlaceholder={t('search.placeholder')} position={settings.titleBarPosition} activeTab={activeTab} updateBannerState={updateBannerState} />;
  const TabNavigationComponent = <TabNavigation activeTab={activeTab} onTabChange={setActiveTab} contentFilter={contentFilter} onFilterChange={setContentFilter} emojiMode={emojiMode} onEmojiModeChange={setEmojiMode} onGroupChange={handleGroupChange} groupsPopupRef={groupsPopupRef} navigationMode={tabNavigationMode} />;
  const ContentComponent = <div ref={contentDragRef} className="main-content-area flex-1 min-h-0 overflow-hidden relative pb-[8px] bg-qc-surface transition-colors duration-500">
      {activeTab === 'clipboard' && <ClipboardTab ref={clipboardTabRef} contentFilter={contentFilter} searchQuery={searchQuery} />}
      {activeTab === 'favorites' && <FavoritesTab ref={favoritesTabRef} contentFilter={contentFilter} searchQuery={searchQuery} />}
      {activeTab === 'emoji' && <Suspense fallback={null}><EmojiTab emojiMode={emojiMode} onEmojiModeChange={setEmojiMode} /></Suspense>}
    </div>;
  const ActionBarComponent = <MultiSelectActionBar activeTab={activeTab} />;
  const renderWorkspace = () => {
    if (isSidebarTabsLayout) {
      return <div className="flex flex-1 min-h-0 min-w-0 flex-row overflow-hidden">
          {TabNavigationComponent}
          <div className="flex-1 min-h-0 min-w-0 flex flex-col overflow-hidden">
            {ContentComponent}
            {ActionBarComponent}
          </div>
        </div>;
    }

    return <div className="flex flex-1 min-h-0 min-w-0 flex-col overflow-hidden">
        {TabNavigationComponent}
        {ContentComponent}
        {ActionBarComponent}
      </div>;
  };
  const renderLayout = () => {
    switch (settings.titleBarPosition) {
      case 'top':
        return <>
            {TitleBarComponent}
            {renderWorkspace()}
          </>;
      case 'bottom':
        return <>
            {renderWorkspace()}
            {TitleBarComponent}
          </>;
      case 'left':
        return <>
            <div className="flex flex-1 min-h-0 min-w-0 flex-row overflow-hidden">
              <div className="flex h-full flex-shrink-0 flex-col">
                {TitleBarComponent}
              </div>
              <div className="flex flex-1 min-h-0 min-w-0 overflow-hidden">
                {renderWorkspace()}
              </div>
            </div>
          </>;
      case 'right':
        return <>
            <div className="flex flex-1 min-h-0 min-w-0 overflow-hidden">
              {renderWorkspace()}
            </div>
            <div className="flex h-full flex-shrink-0 flex-col">
              {TitleBarComponent}
            </div>
          </>;
      default:
        return <>
            {TitleBarComponent}
            {TabNavigationComponent}
            {ContentComponent}
            {ActionBarComponent}
          </>;
    }
  };
  return <div className={outerContainerClasses} style={{
    padding: '5px'
  }}>
      <div className={containerClasses} style={{
      borderRadius: '8px',
      boxShadow: '0 0 5px 1px rgba(0, 0, 0, 0.3), 0 0 3px 0 rgba(0, 0, 0, 0.2)'
    }}>
        {renderLayout()}
        <ToastContainer />
      </div>
      <WindowResizeHandles />
    </div>;
}
export default App;
