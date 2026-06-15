import { convertFileSrc } from '@tauri-apps/api/core';
import { useSnapshot } from 'valtio';
import { settingsStore } from '@shared/store/settingsStore';
import { useTranslation } from 'react-i18next';
import { highlightText } from '@shared/utils/highlightText';
import { formatFileSize } from '@shared/utils/format';

const IMAGE_FILE_EXTENSIONS = ['PNG', 'JPG', 'JPEG', 'GIF', 'BMP', 'WEBP', 'ICO', 'SVG'];
const LARGE_MODE_FILE_ICON_SIZE = {
  small: 40,
  medium: 60,
  large: 80
};
const LARGE_MODE_FILE_ICON_GAP = {
  small: '0.25rem',
  medium: '0.375rem',
  large: '0.5rem'
};

function FileIcon({
  file,
  size = 20
}) {
  const isImageFile = IMAGE_FILE_EXTENSIONS.includes(file.file_type?.toUpperCase());
  const previewPath = file.actual_path || file.path;

  const placeholderSrc = 'data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMjQiIGhlaWdodD0iMjQiIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0ibm9uZSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KPHJlY3QgeD0iMyIgeT0iMyIgd2lkdGg9IjE4IiBoZWlnaHQ9IjE4IiBmaWxsPSIjQ0NDQ0NDIi8+Cjwvc3ZnPgo=';

  if (isImageFile && previewPath) {
    const iconSrc = convertFileSrc(previewPath, 'asset');
    return <img src={iconSrc} alt={file.file_type || '文件'} className="flex-shrink-0 rounded-sm object-cover" style={{
      width: `${size}px`,
      height: `${size}px`
    }} loading="lazy" decoding="async" onError={e => {
      e.target.src = placeholderSrc;
    }} />;
  }

  if (file.icon_data) {
    return <img src={file.icon_data} alt={file.file_type || '文件'} className="flex-shrink-0" style={{
      width: `${size}px`,
      height: `${size}px`,
      objectFit: 'contain'
    }} />;
  }

  return <img src={placeholderSrc} alt={file.file_type || '文件'} className="flex-shrink-0" style={{
    width: `${size}px`,
    height: `${size}px`,
    objectFit: 'contain'
  }} />;
}

function FileContent({
  item,
  compact = false,
  searchKeyword
}) {
  const { t } = useTranslation();
  const settings = useSnapshot(settingsStore);
  const isXSmallHeight = settings.rowHeight === 'xsmall';
  
  const renderFileName = (name) => {
    return searchKeyword ? highlightText(name, searchKeyword) : name;
  };
  
  const renderFilePath = (path) => {
    return searchKeyword ? highlightText(path, searchKeyword) : path;
  };

  let filesData = null;
  try {
    if (item.content?.startsWith('files:')) {
      const filesJson = item.content.substring(6);
      filesData = JSON.parse(filesJson);
    }
  } catch (error) {
    console.error('解析文件数据失败:', error);
    return <div className="text-sm text-red-500">
      文件数据解析错误
    </div>;
  }
  
  if (!filesData || !filesData.files || filesData.files.length === 0) {
    return <div className="text-sm text-qc-fg-muted">
      无文件信息
    </div>;
  }

  const buildTitle = (file) => {
    return `${file.name}\n${file.path || ''}\n${formatFileSize(file.size || 0)}`;
  };

  // 仅图标模式：网格布局
  if (settings.fileDisplayMode === 'iconOnly') {
    const isLargeOrAutoHeight = settings.rowHeight === 'large' || settings.rowHeight === 'auto';
    const largeModeIconSize = LARGE_MODE_FILE_ICON_SIZE[settings.fileIconSize] || LARGE_MODE_FILE_ICON_SIZE.large;
    const largeModeGap = LARGE_MODE_FILE_ICON_GAP[settings.fileIconSize] || LARGE_MODE_FILE_ICON_GAP.large;
    const iconSize = isXSmallHeight ? 18 : compact ? 29 : isLargeOrAutoHeight ? largeModeIconSize : 50;
    const itemSize = isXSmallHeight ? 22 : compact ? 33 : isLargeOrAutoHeight ? largeModeIconSize + 4 : 54;
    const gap = isXSmallHeight ? '0.125rem' : compact ? '0.25rem' : isLargeOrAutoHeight ? largeModeGap : '0.375rem';
    return <div className="w-full h-full overflow-y-auto">
      <div className="w-full flex flex-wrap" style={{
        gap
      }}>
        {filesData.files.map((file, index) => {
          const exists = file.exists !== false;
          const title = exists ? buildTitle(file) : `${file.name}\n${t('clipboard.fileNotFound', '文件不存在')}`;
          return (
            <div
              key={index}
              className={`flex items-center justify-center rounded border transition-colors flex-shrink-0 ${
                exists 
                  ? `bg-qc-panel border-qc-border hover:border-qc-border-strong`
                  : 'bg-red-50 border-red-300/60 opacity-60'
              }`}
              style={{
                width: `${itemSize}px`,
                height: `${itemSize}px`,
                padding: '2px'
              }}
              title={title}
            >
              <FileIcon file={file} size={iconSize} />
            </div>
          );
        })}
      </div>
    </div>;
  }

  // 小行高模式
  if (compact) {
    if (isXSmallHeight) {
      const firstFile = filesData.files[0];
      const exists = firstFile.exists !== false;
      const totalCount = filesData.files.length;
      const title = exists ? buildTitle(firstFile) : `${firstFile.name}\n${t('clipboard.fileNotFound', '文件不存在')}`;
      const metaText = exists ? formatFileSize(firstFile.size || 0) : t('clipboard.fileNotFound', '文件不存在');
      const countText = totalCount > 1 ? t('clipboard.fileCount', { count: totalCount, defaultValue: `共 ${totalCount} 个文件` }) : null;

      return <div
        className={`w-full h-full flex items-center gap-1.5 px-0.5 overflow-hidden ${exists ? '' : 'opacity-70'}`}
        title={title}
      >
        <FileIcon file={firstFile} size={18} />
        <div className="flex-1 min-w-0 flex items-center gap-1.5 overflow-hidden text-xs">
          <span className={`truncate font-medium ${exists ? 'text-qc-fg' : 'text-red-600 line-through'}`}>
            {renderFileName(firstFile.name)}
          </span>
          <span className="text-qc-fg-subtle flex-shrink-0">
            {metaText}
          </span>
          {countText ? (
            <span className="text-qc-fg-subtle truncate">
              {countText}
            </span>
          ) : null}
        </div>
      </div>;
    }

    return <div className="w-full h-full overflow-hidden">
      {filesData.files.map((file, index) => {
        const exists = file.exists !== false;
        const title = exists ? buildTitle(file) : `${file.name}\n${t('clipboard.fileNotFound', '文件不存在')}`;
        return (
          <div
            key={index}
            className={`flex items-center gap-1 px-1 rounded border transition-colors h-full ${
              exists
                ? `bg-qc-panel border-qc-border hover:border-qc-border-strong`
                : 'bg-red-50 border-red-300/60 opacity-60'
            }`}
            title={title}
          >
            <FileIcon file={file} size={24} />
            <div className="flex-1 min-w-0">
              <div className="flex items-baseline gap-1">
                <span className={`text-xs truncate font-medium ${exists ? 'text-qc-fg' : 'text-red-600 line-through'}`}>
                  {renderFileName(file.name)}
                </span>
                <span className="text-xs text-qc-fg-subtle flex-shrink-0">
                  {exists ? formatFileSize(file.size || 0) : t('clipboard.fileNotFound', '文件不存在')}
                </span>
              </div>
              <div className="text-xs text-qc-fg-muted truncate leading-tight">
                {renderFilePath(file.path)}
              </div>
            </div>
          </div>
        );
      })}
    </div>;
  }

  // 正常模式
  const normalIconSize = settings.rowHeight === 'large' || settings.rowHeight === 'auto' ? 48 : 36;
  return <div className="w-full h-full overflow-y-auto space-y-1 pr-1">
    {/* 文件列表 */}
    {filesData.files.map((file, index) => {
      const exists = file.exists !== false;
      const title = exists ? buildTitle(file) : `${file.name}\n${t('clipboard.fileNotFound', '文件不存在')}`;
      return (
        <div
          key={index}
          className={`flex items-center gap-2 px-2 py-1.5 rounded border transition-colors h-full ${
            exists
              ? `bg-qc-panel border-qc-border hover:border-qc-border-strong`
              : 'bg-red-50 border-red-300/60 opacity-60'
          }`}
          title={title}
        >
          {/* 文件图标 */}
          <FileIcon file={file} size={normalIconSize} />

          {/* 文件信息 */}
          <div className="flex-1 min-w-0">
            <div className="flex items-baseline gap-2">
              <span className={`text-sm truncate font-medium ${exists ? 'text-qc-fg' : 'text-red-600 line-through'}`}>
                {renderFileName(file.name)}
              </span>
              <span className="text-xs text-qc-fg-subtle flex-shrink-0">
                {exists ? formatFileSize(file.size || 0) : t('clipboard.fileNotFound', '文件不存在')}
              </span>
            </div>
            <div className="text-xs text-qc-fg-muted truncate mt-0.5">
              {renderFilePath(file.path)}
            </div>
          </div>
        </div>
      );
    })}
  </div>;
}
export default FileContent;
