import { useState, useEffect, useMemo, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useSnapshot } from 'valtio';
import { settingsStore, initSettings } from '@shared/store/settingsStore';
import { useTheme, applyThemeToBody } from '@shared/hooks/useTheme';
import { useSettingsSync } from '@shared/hooks/useSettingsSync';
import {
  getClipboardItemById,
  getFavoriteItemById,
  updateClipboardItem,
  updateFavorite,
  addFavorite,
} from '@shared/api';
import { groupsStore, loadGroups } from '@shared/store/groupsStore';
import { hasType } from '@shared/utils/contentType';
import { applyBackgroundImage, clearBackgroundImage } from '@shared/utils/backgroundManager';
import TitleBar from './components/TitleBar';
import EditorToolbar from './components/EditorToolbar';
import TextEditor from './components/TextEditor';
import HtmlEditor from './components/HtmlEditor';
import StatusBar from './components/StatusBar';
import ToastContainer from '@shared/components/common/ToastContainer';
import { resolveEditorSavePayload } from './savePayload';

function resolveEditableTypes(contentType, htmlContent) {
  const normalizedType = String(contentType || '');
  const hasHtmlType = hasType(normalizedType, 'rich_text');
  const hasHtmlContent = typeof htmlContent === 'string' && htmlContent.trim().length > 0;
  return hasHtmlType || hasHtmlContent ? ['text', 'html'] : ['text'];
}

function extractPlainTextFromHtml(html) {
  if (typeof html !== 'string' || !html.trim()) {
    return '';
  }
  const parser = new DOMParser();
  const doc = parser.parseFromString(html, 'text/html');
  return (doc.body?.textContent || '').replace(/\r\n/g, '\n').trim();
}

function App() {
  const { t } = useTranslation();
  const { theme } = useSnapshot(settingsStore);
  const { isDark, effectiveTheme, lightThemeStyle, darkThemeStyle, isBackground } = useTheme();
  const groupsSnap = useSnapshot(groupsStore);
  useSettingsSync();

  const [editorData, setEditorData] = useState(null);
  const [title, setTitle] = useState('');
  const [selectedGroup, setSelectedGroup] = useState('全部');
  const [editMode, setEditMode] = useState('text');
  const [wordWrap, setWordWrap] = useState(true);

  const [textContent, setTextContent] = useState('');
  const [htmlContent, setHtmlContent] = useState('');
  const [originalTextContent, setOriginalTextContent] = useState('');
  const [originalHtmlContent, setOriginalHtmlContent] = useState('');
  const [originalTitle, setOriginalTitle] = useState('');
  const lastEditedModeRef = useRef(null);

  const [hasChanges, setHasChanges] = useState(false);
  const [charCount, setCharCount] = useState(0);
  const [lineCount, setLineCount] = useState(1);

  const editableTypes = useMemo(
    () => resolveEditableTypes(editorData?.contentType, htmlContent || originalHtmlContent),
    [editorData?.contentType, htmlContent, originalHtmlContent],
  );

  useEffect(() => {
    if (!editableTypes.includes(editMode)) {
      setEditMode('text');
    }
  }, [editableTypes, editMode]);

  useEffect(() => {
    const init = async () => {
      await initSettings();
      applyThemeToBody(settingsStore.theme, 'text-editor');
    };
    init();
  }, []);

  useEffect(() => {
    applyThemeToBody(theme, 'text-editor');
  }, [theme, lightThemeStyle, darkThemeStyle, effectiveTheme]);

  useEffect(() => {
    if (isBackground && settingsStore.backgroundImagePath) {
      applyBackgroundImage({
        containerSelector: '.text-editor-container',
        backgroundImagePath: settingsStore.backgroundImagePath,
        windowName: 'text-editor',
      });
    } else {
      clearBackgroundImage('.text-editor-container');
    }
  }, [isBackground, settingsStore.backgroundImagePath]);

  useEffect(() => {
    loadGroups();
  }, []);

  useEffect(() => {
    const loadData = async () => {
      try {
        const params = new URLSearchParams(window.location.search);
        const id = params.get('id');
        const type = params.get('type');
        const index = params.get('index');
        const group = params.get('group');

        if (!id || !type) {
          return;
        }

        if (id === '-1' && type === 'favorite') {
          const initialGroup = group || '全部';
          setEditorData({
            id: null,
            type: 'favorite',
            groupId: null,
            contentType: 'text',
          });
          setTitle('');
          setOriginalTitle('');
          setTextContent('');
          setHtmlContent('');
          setOriginalTextContent('');
          setOriginalHtmlContent('');
          setSelectedGroup(initialGroup);
          setEditMode('text');
          return;
        }

        if (type === 'clipboard') {
          const numericId = parseInt(id, 10);
          const item = await getClipboardItemById(numericId);
          const displayTitle = t('textEditor.clipboardItem', { number: parseInt(index, 10) });
          setEditorData({
            id: item.id,
            type: 'clipboard',
            index: parseInt(index, 10),
            contentType: item.content_type || 'text',
          });
          setTitle(displayTitle);
          setOriginalTitle(displayTitle);
          setTextContent(item.content || '');
          setHtmlContent(item.html_content || '');
          setOriginalTextContent(item.content || '');
          setOriginalHtmlContent(item.html_content || '');
          setEditMode('text');
        } else if (type === 'favorite') {
          const item = await getFavoriteItemById(id);
          setEditorData({
            id: item.id,
            type: 'favorite',
            groupId: item.group_name || null,
            contentType: item.content_type || 'text',
          });
          setTitle(item.title || '');
          setOriginalTitle(item.title || '');
          setTextContent(item.content || '');
          setHtmlContent(item.html_content || '');
          setOriginalTextContent(item.content || '');
          setOriginalHtmlContent(item.html_content || '');
          setSelectedGroup(item.group_name || '全部');
          setEditMode('text');
        }
      } catch (error) {
        console.error('加载编辑数据失败:', error);
      }
    };

    loadData();
  }, [t]);

  useEffect(() => {
    const textChanged = textContent !== originalTextContent;
    const htmlChanged = htmlContent !== originalHtmlContent;
    const titleChanged = editorData?.type === 'favorite' && title !== originalTitle;
    setHasChanges(textChanged || htmlChanged || titleChanged);
  }, [
    textContent,
    originalTextContent,
    htmlContent,
    originalHtmlContent,
    title,
    originalTitle,
    editorData,
  ]);

  useEffect(() => {
    const currentContent = editMode === 'html' ? htmlContent : textContent;
    const safeContent = typeof currentContent === 'string' ? currentContent : '';
    setCharCount(safeContent.length);
    setLineCount(safeContent.split('\n').length || 1);
  }, [editMode, textContent, htmlContent]);

  const handleSave = async () => {
    if (!editorData) return;

    try {
      const { contentForSave, htmlPayload } = resolveEditorSavePayload({
        supportsHtml: editableTypes.includes('html'),
        textContent,
        htmlContent,
        originalTextContent,
        originalHtmlContent,
        lastEditedMode: lastEditedModeRef.current,
        extractPlainTextFromHtml,
      });

      if (editorData.type === 'clipboard') {
        await updateClipboardItem(editorData.id, contentForSave, htmlPayload);
      } else if (editorData.type === 'favorite') {
        if (editorData.id) {
          await updateFavorite(editorData.id, title, contentForSave, selectedGroup, htmlPayload);
        } else {
          await addFavorite(title.trim(), contentForSave, selectedGroup);
        }
      }

      setTextContent(contentForSave);
      setOriginalTextContent(contentForSave);
      setOriginalHtmlContent(htmlContent);
      setOriginalTitle(title);
      setHasChanges(false);

      const { Window } = await import('@tauri-apps/api/window');
      const currentWindow = Window.getCurrent();
      await currentWindow.close();
    } catch (error) {
      console.error('保存失败:', error);
    }
  };

  const handleCancel = async () => {
    const { Window } = await import('@tauri-apps/api/window');
    const currentWindow = Window.getCurrent();
    await currentWindow.close();
  };

  const handleReset = () => {
    if (editMode === 'html') {
      setHtmlContent(originalHtmlContent);
      return;
    }
    setTextContent(originalTextContent);
  };

  const outerContainerClasses = `
    h-screen w-screen
    ${isDark ? 'dark' : ''}
  `.trim().replace(/\s+/g, ' ');
  const containerClasses = `
    text-editor-container
    h-full w-full
    flex flex-col
    overflow-hidden
    bg-qc-surface
    ${isBackground ? 'bg-opacity-0' : ''}
  `.trim().replace(/\s+/g, ' ');

  return (
    <div className={outerContainerClasses} style={{
      padding: '5px'
    }}>
      <div className={`${containerClasses} text-editor-shell`}>
        <TitleBar title={title} hasChanges={hasChanges} />

        <EditorToolbar
          onReset={handleReset}
          title={title}
          onTitleChange={setTitle}
          wordWrap={wordWrap}
          onWordWrapChange={() => setWordWrap(!wordWrap)}
          showTitle={editorData?.type === 'favorite'}
          groups={groupsSnap.groups}
          selectedGroup={selectedGroup}
          onGroupChange={setSelectedGroup}
          showGroupSelector={editorData?.type === 'favorite'}
          editableTypes={editableTypes}
          editMode={editMode}
          onEditModeChange={setEditMode}
        />

        {editMode === 'html' && editableTypes.includes('html') ? (
          <HtmlEditor
            content={htmlContent}
            onContentChange={(content) => {
              lastEditedModeRef.current = 'html';
              setHtmlContent(content);
            }}
            onStatsChange={({ chars, lines }) => {
              setCharCount(chars);
              setLineCount(lines);
            }}
            wordWrap={wordWrap}
          />
        ) : (
          <TextEditor
            content={textContent}
            onContentChange={(content) => {
              lastEditedModeRef.current = 'text';
              setTextContent(content);
            }}
            onStatsChange={({ chars, lines }) => {
              setCharCount(chars);
              setLineCount(lines);
            }}
            wordWrap={wordWrap}
          />
        )}

        <StatusBar
          charCount={charCount}
          lineCount={lineCount}
          hasChanges={hasChanges}
          onSave={handleSave}
          onCancel={handleCancel}
        />

        <ToastContainer />
      </div>
    </div>
  );
}

export default App;
