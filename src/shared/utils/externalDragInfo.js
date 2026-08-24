import { createDragPreviewIcon, createImagesDragPreviewIcon } from './dragPreviewIcon';

export function getExternalDragInfo(item, renderType, t, fallbackIcon, source) {
  const isImage = renderType === 'image';
  const isFile = renderType === 'file';
  if (!isImage && !isFile) {
    if (renderType === 'text' || renderType === 'rich_text') {
      if (!source || !item?.id) return { paths: [], item: null, iconPath: null };
      return { paths: [], item: null, textSource: { source, itemId: item.id }, iconPath: fallbackIcon, canDrag: true };
    }
    return { paths: [], item: null, iconPath: null };
  }
  if (!item.content?.startsWith('files:')) return { paths: [], item: null, iconPath: null };

  try {
    const files = JSON.parse(item.content.substring(6)).files || [];
    if (isImage) {
      const first = files[0];
      const path = first?.exists === false ? null : first?.actual_path || first?.path;
      return path ? { paths: [path], item: [path], iconPath: ({ paths }) => createImagesDragPreviewIcon(paths), canDrag: true } : { paths: [], item: null, iconPath: null };
    }

    const draggableFiles = files.filter(file => file.exists !== false && file.path);
    const paths = draggableFiles.map(file => file.path);
    if (!paths.length) return { paths: [], item: null, iconPath: null };
    const previewIcon = draggableFiles.find(file => file.icon_data)?.icon_data || '';
    return {
      paths,
      item: paths,
      iconPath: ({ paths: previewPaths, mode }) => createDragPreviewIcon(previewIcon, previewPaths.length, mode, {
        copy: t('common.copy', '复制'),
        move: t('transferShelf.move', '移动'),
      }) || previewPaths[0],
    };
  } catch {
    return { paths: [], item: null, iconPath: null };
  }
}
