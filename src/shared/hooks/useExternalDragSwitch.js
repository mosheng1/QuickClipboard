import { useCallback, useEffect, useRef, useState } from 'react';
import { startDrag } from '@crabnebula/tauri-plugin-drag';
import { invoke } from '@tauri-apps/api/core';
import { getClipboardItemById, getFavoriteItemById } from '@shared/api/clipboard';

const EXTERNAL_DRAG_EDGE_THRESHOLD_PX = 24;

function hasDragPayload(dragInfo) {
  return Boolean(dragInfo?.textSource?.source && dragInfo.textSource.itemId)
    || Boolean(dragInfo?.item && ((Array.isArray(dragInfo.item) && dragInfo.item.length) || (!Array.isArray(dragInfo.item) && dragInfo.item.data)));
}

async function loadFullTextPayload(textSource) {
  const item = textSource.source === 'clipboard'
    ? await getClipboardItemById(Number(textSource.itemId))
    : await getFavoriteItemById(String(textSource.itemId));

  if (!item) {
    throw new Error('获取拖拽文本失败：条目不存在');
  }

  const plain = typeof item.content === 'string' ? item.content : '';
  const html = typeof item.html_content === 'string' && item.html_content ? item.html_content : null;
  const fallbackPlain = html ? html.replace(/<[^>]+>/g, '') : '';
  if (!plain && !fallbackPlain) {
    throw new Error('获取拖拽文本失败：内容为空');
  }

  return { plain: plain || fallbackPlain, html };
}

export function useExternalDragSwitch({ onDragStart, onDragEnd, onDragCancel, closePreview }) {
  const [showSafeZones, setShowSafeZones] = useState(false);
  const switchingRef = useRef(false);
  const cancelDndDropRef = useRef(false);
  const previewRef = useRef(null);
  const activeDragRef = useRef(null);
  const previousClipPathRef = useRef('');

  const clearDragVisuals = useCallback(() => {
    document.body.classList.remove('dragging-cursor');
    document.body.style.clipPath = previousClipPathRef.current;
  }, []);

  const createPreview = useCallback(async (dragInfo) => {
    try {
      if (typeof dragInfo?.iconPath === 'function') {
        return await dragInfo.iconPath({ paths: dragInfo.paths, mode: 'copy' });
      }
      return dragInfo?.iconPath || dragInfo?.paths?.[0];
    } catch (error) {
      console.error('生成系统拖拽预览失败:', error);
      return dragInfo?.paths?.[0];
    }
  }, []);

  const prepareExternalDrag = useCallback((sortId, dragInfo) => {
    if (!sortId || !hasDragPayload(dragInfo) || previewRef.current?.sortId === sortId) return;
    previewRef.current = { sortId, promise: createPreview(dragInfo) };
  }, [createPreview]);

  const switchToExternalDrag = useCallback((sortId, dragInfo, event) => {
    if (switchingRef.current || !hasDragPayload(dragInfo)) return;
    switchingRef.current = true;
    cancelDndDropRef.current = true;
    activeDragRef.current = null;
    setShowSafeZones(false);
    clearDragVisuals();
    onDragCancel();
    closePreview?.();

    document.dispatchEvent(new MouseEvent('mouseup', {
      bubbles: true,
      cancelable: true,
      view: window,
      clientX: event?.clientX || 0,
      clientY: event?.clientY || 0,
      screenX: event?.screenX || 0,
      screenY: event?.screenY || 0,
      buttons: 0,
    }));

    (async () => {
      try {
        const preview = previewRef.current;
        const icon = preview?.sortId === sortId ? await preview.promise : await createPreview(dragInfo);
        if (dragInfo.textSource) {
          const textPayload = await loadFullTextPayload(dragInfo.textSource);
          await invoke('start_text_drag', textPayload);
        } else {
          await startDrag({ item: dragInfo.item, icon: icon || dragInfo.paths?.[0], mode: 'copy' });
        }
      } catch (error) {
        console.error('启动系统拖拽失败:', error);
      } finally {
        switchingRef.current = false;
        previewRef.current = null;
      }
    })();
  }, [clearDragVisuals, closePreview, createPreview, onDragCancel]);

  useEffect(() => {
    const handleMouseMove = (event) => {
      const activeDrag = activeDragRef.current;
      if (!activeDrag || switchingRef.current) return;
      if (event.clientX <= EXTERNAL_DRAG_EDGE_THRESHOLD_PX || event.clientX >= window.innerWidth - EXTERNAL_DRAG_EDGE_THRESHOLD_PX) {
        switchToExternalDrag(activeDrag.sortId, activeDrag.dragInfo, event);
      }
    };
    document.addEventListener('mousemove', handleMouseMove, true);
    return () => document.removeEventListener('mousemove', handleMouseMove, true);
  }, [switchToExternalDrag]);

  const handleDndDragStart = useCallback((event) => {
    const dragInfo = event.active.data.current?.externalDrag;
    prepareExternalDrag(event.active.id, dragInfo);
    setShowSafeZones(hasDragPayload(dragInfo));
    activeDragRef.current = hasDragPayload(dragInfo) ? { sortId: event.active.id, dragInfo } : null;
    previousClipPathRef.current = document.body.style.clipPath;
    document.body.classList.add('dragging-cursor');
    document.body.style.clipPath = 'inset(5px round 8px)';
    onDragStart(event);
  }, [onDragStart, prepareExternalDrag]);

  const handleDndDragEnd = useCallback((event) => {
    activeDragRef.current = null;
    setShowSafeZones(false);
    clearDragVisuals();
    onDragEnd(event);
  }, [clearDragVisuals, onDragEnd]);

  const handleDndDragCancel = useCallback((event) => {
    activeDragRef.current = null;
    cancelDndDropRef.current = false;
    setShowSafeZones(false);
    clearDragVisuals();
    onDragCancel(event);
  }, [clearDragVisuals, onDragCancel]);

  return {
    showSafeZones,
    prepareExternalDrag,
    shouldCancelDndDrop: () => cancelDndDropRef.current,
    handleDndDragStart,
    handleDndDragEnd,
    handleDndDragCancel,
  };
}
