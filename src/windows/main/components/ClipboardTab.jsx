import { useRef, forwardRef, useImperativeHandle, useEffect, useState, useCallback } from 'react';
import { useSnapshot } from 'valtio';
import { listen } from '@tauri-apps/api/event';
import { clipboardStore, refreshClipboardHistory } from '@shared/store/clipboardStore';
import { navigationStore } from '@shared/store/navigationStore';
import { settingsStore } from '@shared/store/settingsStore';
import ClipboardList from './ClipboardList';
import FloatingToolbar from './FloatingToolbar';

const SEARCH_DEBOUNCE_DELAY = 200;
const ClipboardTab = forwardRef(({
  contentFilter,
  searchQuery
}, ref) => {
  const snap = useSnapshot(clipboardStore);
  const settings = useSnapshot(settingsStore);
  const listRef = useRef(null);
  const [isAtTop, setIsAtTop] = useState(true);
  const prevTotalCountRef = useRef(snap.totalCount);
  const searchDebounceRef = useRef(null);

  const debouncedSearch = useCallback((query, filter) => {
    if (searchDebounceRef.current) {
      clearTimeout(searchDebounceRef.current);
    }

    searchDebounceRef.current = setTimeout(() => {
      clipboardStore.setFilter(query);
      refreshClipboardHistory();
      if (query) {
        navigationStore.setSelectedIndex(0);
      } else {
        navigationStore.resetNavigation();
      }
    }, SEARCH_DEBOUNCE_DELAY);
  }, []);

  useEffect(() => {
    clipboardStore.setContentType(contentFilter);
    debouncedSearch(searchQuery, contentFilter);

    return () => {
      if (searchDebounceRef.current) {
        clearTimeout(searchDebounceRef.current);
      }
    };
  }, [searchQuery, contentFilter, debouncedSearch]);


  useEffect(() => {
    if (snap.totalCount > prevTotalCountRef.current) {
      handleScrollToTop({ checkSetting: true, delay: 100 });
    }
    prevTotalCountRef.current = snap.totalCount;
  }, [snap.totalCount]);
  useEffect(() => {
    const setupListeners = async () => {
      const unlisten1 = await listen('window-show-animation', () => handleScrollToTop({ checkSetting: true, delay: 50 }));
      const unlisten2 = await listen('edge-snap-show', () => handleScrollToTop({ checkSetting: true, delay: 50 }));
      return () => {
        unlisten1();
        unlisten2();
      };
    };
    let cleanup = setupListeners();
    return () => cleanup.then(fn => fn());
  }, [settings.autoScrollToTopOnShow]);

  // 暴露导航方法给父组件
  useImperativeHandle(ref, () => ({
    navigateUp: () => listRef.current?.navigateUp?.(),
    navigateDown: () => listRef.current?.navigateDown?.(),
    executeCurrentItem: () => listRef.current?.executeCurrentItem?.(),
    executePlainTextPaste: () => listRef.current?.executePlainTextPaste?.()
  }));

  // 处理滚动状态变化
  const handleScrollStateChange = ({
    atTop
  }) => {
    setIsAtTop(atTop);
  };

  const handleScrollToTop = (options = {}) => {
    const {
      checkSetting = true,
      delay = 0
    } = options;

    if (checkSetting && !settings.autoScrollToTopOnShow) {
      return;
    }

    setTimeout(() => {
      listRef.current?.scrollToTop?.();
      navigationStore.resetNavigation();
    }, delay);
  };
  
  return <div className="h-full flex flex-col relative">
    {/* 列表 */}
    <ClipboardList ref={listRef} onScrollStateChange={handleScrollStateChange} />

    {/* 悬浮工具栏 */}
    <FloatingToolbar showScrollTop={!isAtTop && snap.totalCount > 0} showAddFavorite={false} onScrollTop={() => handleScrollToTop({
      checkSetting: false
    })} />
  </div>;
});
ClipboardTab.displayName = 'ClipboardTab';
export default ClipboardTab;
