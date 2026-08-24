import { getPrimaryType } from '@shared/utils/contentType';
import { mergePasteClipboardItems } from '@shared/api/clipboard';
import { mergePasteFavoriteItems } from '@shared/api/favorites';
import { clipboardStore } from '@shared/store/clipboardStore';
import { favoritesStore } from '@shared/store/favoritesStore';

const TEXT_TYPES = new Set(['text', 'link', 'rich_text']);
const MERGEABLE_TYPES = new Set(['text', 'link', 'rich_text', 'image', 'file']);

export function getSelectionMergeState(selectedEntries = []) {
  if (!selectedEntries.length) {
    return {
      canMerge: false,
      reasonKey: 'selectFirst',
    };
  }

  const primaryTypes = selectedEntries.map(entry => getPrimaryType(entry.contentType));
  const hasUnsupportedType = primaryTypes.some(type => !MERGEABLE_TYPES.has(type));
  if (hasUnsupportedType) {
    return {
      canMerge: false,
      reasonKey: 'unsupportedType',
    };
  }

  const hasFile = primaryTypes.includes('file');
  const hasNonFile = primaryTypes.some(type => type !== 'file');
  if (hasFile && hasNonFile) {
    return {
      canMerge: false,
      reasonKey: 'fileMixedUnsupported',
    };
  }

  return {
    canMerge: true,
    requiresRichText: primaryTypes.some(type => type === 'image' || type === 'rich_text'),
    isFileOnly: primaryTypes.every(type => type === 'file'),
    isTextOnly: primaryTypes.every(type => TEXT_TYPES.has(type)),
    reasonKey: null,
  };
}

// 执行多选合并粘贴，供操作栏和粘贴快捷键共用。
export async function mergePasteSelectedItems(activeTab) {
  const currentStore = activeTab === 'clipboard'
    ? clipboardStore
    : activeTab === 'favorites'
      ? favoritesStore
      : null;

  if (!currentStore) {
    return false;
  }

  const selectedEntries = currentStore.selectedEntries || [];
  if (!getSelectionMergeState(selectedEntries).canMerge) {
    return false;
  }

  const selectedIds = currentStore.getSelectedIds();
  if (!selectedIds.length) {
    return false;
  }

  if (activeTab === 'clipboard') {
    await mergePasteClipboardItems(selectedIds);
  } else {
    await mergePasteFavoriteItems(selectedIds);
  }

  currentStore.exitMultiSelectMode();
  return true;
}
