import '@tabler/icons-webfont/dist/tabler-icons.min.css';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useSnapshot } from 'valtio';
import { open } from '@tauri-apps/plugin-dialog';
import { settingsStore } from '@shared/store/settingsStore';
import { toast } from '@shared/store/toastStore';
import SettingsSection from '../components/SettingsSection';
import SettingItem from '../components/SettingItem';
import Toggle from '@shared/components/ui/Toggle';
import Slider from '@shared/components/ui/Slider';
import Select from '@shared/components/ui/Select';
import SegmentedControl from '@shared/components/ui/SegmentedControl';
import MultiSegmentedControl from '@shared/components/ui/MultiSegmentedControl';
import { normalizeVisibleOptionalTabs, OPTIONAL_TAB_OPTIONS } from '@shared/constants/tabVisibility';
import ThemeOption from '../components/ThemeOption';
function AppearanceSection({
  settings,
  onSettingChange
}) {
  const {
    t
  } = useTranslation();
  const {
    theme,
    lightThemeStyle,
    darkThemeStyle,
    backgroundImagePath,
    customFontStatus
  } = useSnapshot(settingsStore);
  const themeOptions = [{
    id: 'auto',
    label: t('settings.appearance.themeAuto'),
    preview: 'linear-gradient(135deg, #667eea 0%, #764ba2 100%)'
  }, {
    id: 'light',
    label: t('settings.appearance.themeLight'),
    preview: 'linear-gradient(135deg, #f5f7fa 0%, #c3cfe2 100%)'
  }, {
    id: 'dark',
    label: t('settings.appearance.themeDark'),
    preview: 'linear-gradient(135deg, #2c3e50 0%, #000000 100%)'
  }, {
    id: 'background',
    label: t('settings.appearance.themeBackground'),
    preview: 'linear-gradient(135deg, #0093E9 0%, #80D0C7 100%)'
  }];
  const blurScale = Number.isFinite(Number(settings.superBackgroundBlurScale))
    ? Number(settings.superBackgroundBlurScale)
    : 1;
  const blurPercent = Math.round(blurScale * 100);
  const [urlInputValue, setUrlInputValue] = useState(settings.customFontUrl || '');
  const visibleOptionalTabs = normalizeVisibleOptionalTabs(settings.visibleOptionalTabs);
  const tabVisibilityOptions = OPTIONAL_TAB_OPTIONS.map(option => ({
    value: option.id,
    label: t(option.labelKey) || option.fallbackLabel
  }));
  const adaptiveMaxHeight = Number.isFinite(Number(settings.adaptiveMaxHeight))
    ? Math.min(350, Math.max(80, Math.round(Number(settings.adaptiveMaxHeight))))
    : 350;
  const fileIconSizeOptions = [{
    value: 'small',
    label: t('settings.appearance.fileIconSizeSmall', '小')
  }, {
    value: 'medium',
    label: t('settings.appearance.fileIconSizeMedium', '中')
  }, {
    value: 'large',
    label: t('settings.appearance.fileIconSizeLarge', '大')
  }];
  const fieldTitleClass = 'block text-sm font-semibold leading-5 text-qc-fg';
  const fieldDescClass = 'text-xs leading-5 text-qc-fg-subtle';
  const handleAdaptiveMaxHeightChange = value => {
    const nextValue = Number(value);
    if (!Number.isFinite(nextValue)) return;
    onSettingChange('adaptiveMaxHeight', Math.min(350, Math.max(80, Math.round(nextValue))));
  };
  const handleSelectBackgroundImage = async () => {
    try {
      const selected = await open({
        title: t('settings.appearance.selectBackgroundImage'),
        multiple: false,
        filters: [{
          name: 'Images',
          extensions: ['png', 'jpg', 'jpeg', 'gif', 'bmp', 'webp']
        }]
      });
      if (selected) {
        await onSettingChange('backgroundImagePath', selected);
        toast.success(t('settings.appearance.backgroundImageSet'));
      }
    } catch (error) {
      console.error('Failed to select background image:', error);
      toast.error(t('settings.appearance.backgroundImageError'));
    }
  };
  const handleClearBackgroundImage = async () => {
    try {
      await onSettingChange('backgroundImagePath', '');
      toast.success(t('settings.appearance.backgroundImageCleared'));
    } catch (error) {
      console.error('Failed to clear background image:', error);
    }
  };
  return <SettingsSection title={t('settings.appearance.title')} description={t('settings.appearance.description')}>
      <div className="space-y-6">
        <div>
          <label className={`${fieldTitleClass} mb-3`}>
            {t('settings.appearance.themeSelect')}
          </label>
          <p className={`${fieldDescClass} mb-4`}>
            {t('settings.appearance.themeSelectDesc')}
          </p>
          
          <div className="grid grid-cols-4 gap-3">
            {themeOptions.map(option => <ThemeOption key={option.id} option={option} isActive={theme === option.id} onClick={() => settingsStore.setTheme(option.id)} />)}
          </div>
        </div>

        {(theme === 'light' || theme === 'auto') && <div className="animate-slide-in-left-fast">
            <label className={`${fieldTitleClass} mb-3`}>
              {t('settings.appearance.lightThemeStyle') || '亮色风格'}
            </label>
            <p className={`${fieldDescClass} mb-4`}>
              {t('settings.appearance.lightThemeStyleDesc') || '选择亮色主题的显示风格'}
            </p>

            <div className="grid grid-cols-2 gap-3">
              <button onClick={() => onSettingChange('lightThemeStyle', 'modern')} className={`
                  theme-style-option flex flex-col items-start gap-2 p-4 rounded-lg border-2
                  transition-all duration-300
                  focus:outline-none active:scale-95
                  ${lightThemeStyle === 'modern' ? 'is-active border-[var(--qc-accent)] bg-[var(--qc-accent)] text-[var(--qc-accent-fg)] scale-102 shadow-md' : 'border-qc-border hover:border-qc-border-strong hover:scale-101 hover:shadow-md'}
                `}>
                <div className="w-full">
                  <div className={`text-sm font-semibold mb-1 ${lightThemeStyle === 'modern' ? 'text-[var(--qc-accent-fg)]' : 'text-qc-fg'}`}>
                    {t('settings.appearance.lightThemeModern') || '现代风格'}
                  </div>
                  <div className={`text-xs ${lightThemeStyle === 'modern' ? 'text-[var(--qc-accent-fg)] opacity-80' : 'text-qc-fg-muted'}`}>
                    {t('settings.appearance.lightThemeModernDesc') || '当前默认的简洁明亮主题'}
                  </div>
                </div>
              </button>

              <button onClick={() => onSettingChange('lightThemeStyle', 'wireframe')} className={`
                  theme-style-option flex flex-col items-start gap-2 p-4 rounded-lg border-2
                  transition-all duration-300
                  focus:outline-none active:scale-95
                  ${lightThemeStyle === 'wireframe' ? 'is-active border-[var(--qc-accent)] bg-[var(--qc-accent)] text-[var(--qc-accent-fg)] scale-102 shadow-md' : 'border-qc-border hover:border-qc-border-strong hover:scale-101 hover:shadow-md'}
                `}>
                <div className="w-full">
                  <div className={`text-sm font-semibold mb-1 ${lightThemeStyle === 'wireframe' ? 'text-[var(--qc-accent-fg)]' : 'text-qc-fg'}`}>
                    {t('settings.appearance.lightThemeWireframe') || '线框风格'}
                  </div>
                  <div className={`text-xs ${lightThemeStyle === 'wireframe' ? 'text-[var(--qc-accent-fg)] opacity-80' : 'text-qc-fg-muted'}`}>
                    {t('settings.appearance.lightThemeWireframeDesc') || '高对比边框与硬阴影的亮色线框主题'}
                  </div>
                </div>
              </button>
            </div>
          </div>}

        {(theme === 'dark' || theme === 'auto') && <div className="animate-slide-in-left-fast">
            <label className={`${fieldTitleClass} mb-3`}>
              {t('settings.appearance.darkThemeStyle') || '暗色风格'}
            </label>
            <p className={`${fieldDescClass} mb-4`}>
              {t('settings.appearance.darkThemeStyleDesc') || '选择暗色主题的显示风格'}
            </p>

            <div className="grid grid-cols-3 gap-3">
              <button onClick={() => onSettingChange('darkThemeStyle', 'modern')} className={`
                  theme-style-option flex flex-col items-start gap-2 p-4 rounded-lg border-2
                  transition-all duration-300
                  focus:outline-none active:scale-95
                  ${darkThemeStyle === 'modern' ? 'is-active border-[var(--qc-accent)] bg-[var(--qc-accent)] text-[var(--qc-accent-fg)] scale-102 shadow-md' : 'border-qc-border hover:border-qc-border-strong hover:scale-101 hover:shadow-md'}
                `}>
                <div className="w-full">
                  <div className={`text-sm font-semibold mb-1 ${darkThemeStyle === 'modern' ? 'text-[var(--qc-accent-fg)]' : 'text-qc-fg'}`}>
                    {t('settings.appearance.darkThemeModern') || '现代风格'}
                  </div>
                  <div className={`text-xs ${darkThemeStyle === 'modern' ? 'text-[var(--qc-accent-fg)] opacity-80' : 'text-qc-fg-muted'}`}>
                    {t('settings.appearance.darkThemeModernDesc') || '色彩丰富的现代暗色主题'}
                  </div>
                </div>
              </button>

              <button onClick={() => onSettingChange('darkThemeStyle', 'classic')} className={`
                  theme-style-option flex flex-col items-start gap-2 p-4 rounded-lg border-2
                  transition-all duration-300
                  focus:outline-none active:scale-95
                  ${darkThemeStyle === 'classic' ? 'is-active border-[var(--qc-accent)] bg-[var(--qc-accent)] text-[var(--qc-accent-fg)] scale-102 shadow-md' : 'border-qc-border hover:border-qc-border-strong hover:scale-101 hover:shadow-md'}
                `}>
                <div className="w-full">
                  <div className={`text-sm font-semibold mb-1 ${darkThemeStyle === 'classic' ? 'text-[var(--qc-accent-fg)]' : 'text-qc-fg'}`}>
                    {t('settings.appearance.darkThemeClassic') || '经典风格'}
                  </div>
                  <div className={`text-xs ${darkThemeStyle === 'classic' ? 'text-[var(--qc-accent-fg)] opacity-80' : 'text-qc-fg-muted'}`}>
                    {t('settings.appearance.darkThemeClassicDesc') || '低调优雅的灰色暗色主题'}
                  </div>
                </div>
              </button>

              <button onClick={() => onSettingChange('darkThemeStyle', 'sketch')} className={`
                  theme-style-option flex flex-col items-start gap-2 p-4 rounded-lg border-2
                  transition-all duration-300
                  focus:outline-none active:scale-95
                  ${darkThemeStyle === 'sketch' ? 'is-active border-[var(--qc-accent)] bg-[var(--qc-accent)] text-[var(--qc-accent-fg)] scale-102 shadow-md' : 'border-qc-border hover:border-qc-border-strong hover:scale-101 hover:shadow-md'}
                `}>
                <div className="w-full">
                  <div className={`text-sm font-semibold mb-1 ${darkThemeStyle === 'sketch' ? 'text-[var(--qc-accent-fg)]' : 'text-qc-fg'}`}>
                    {t('settings.appearance.darkThemeSketch') || '线框风格'}
                  </div>
                  <div className={`text-xs ${darkThemeStyle === 'sketch' ? 'text-[var(--qc-accent-fg)] opacity-80' : 'text-qc-fg-muted'}`}>
                    {t('settings.appearance.darkThemeSketchDesc') || '高对比边框与硬阴影的复古线框暗色主题'}
                  </div>
                </div>
              </button>
            </div>
          </div>}

        <div className="animate-slide-in-left-fast">
          <SettingItem label={t('settings.appearance.customFontEnable') || '启用自定义字体'} description={t('settings.appearance.customFontEnableDesc') || '开启后将使用自定义字体覆盖所有主题的默认字体'}>
            <Toggle checked={settings.customFontEnabled} onChange={checked => onSettingChange('customFontEnabled', checked)} />
          </SettingItem>

          {settings.customFontEnabled && <div className="space-y-4 mt-4">
            <SettingItem label={t('settings.appearance.customFontSource') || '字体来源'}>
              <SegmentedControl
                value={settings.customFontType || 'file'}
                onChange={value => onSettingChange('customFontType', value)}
                options={[
                  { value: 'file', label: t('settings.appearance.customFontSourceFile') || '本地文件' },
                  { value: 'url', label: t('settings.appearance.customFontSourceUrl') || '在线链接' }
                ]}
                className="max-w-xs"
              />
            </SettingItem>

            {(settings.customFontType || 'file') === 'file' ? (
              <div className="space-y-3">
                <div className="flex items-center gap-3">
                  <button
                    onClick={async () => {
                      try {
                        const selected = await open({
                          title: t('settings.appearance.selectFontFile') || '选择字体文件',
                          multiple: false,
                          filters: [{ name: 'Fonts', extensions: ['ttf', 'otf', 'woff', 'woff2'] }]
                        })
                        if (selected) {
                          await onSettingChange('customFontPath', selected)
                        }
                      } catch (error) {
                        console.error('Failed to select font:', error)
                      }
                    }}
                    className="flex items-center gap-2 px-4 py-2 bg-blue-500 hover:bg-blue-600 text-white rounded-lg transition-colors"
                  >
                    <i className="ti ti-file-text" style={{ fontSize: 18 }}></i>
                    {settings.customFontPath ? (t('settings.appearance.changeFontFile') || '更换字体文件') : (t('settings.appearance.selectFontFile') || '选择字体文件')}
                  </button>
                  {settings.customFontPath && (
                    <button
                      onClick={() => onSettingChange('customFontPath', '')}
                      className="flex items-center gap-2 px-4 py-2 bg-qc-panel hover:bg-qc-hover text-qc-fg rounded-lg transition-colors"
                    >
                      <i className="ti ti-x" style={{ fontSize: 18 }}></i>
                      {t('settings.appearance.clearFontFile') || '清除字体文件'}
                    </button>
                  )}
                  {customFontStatus === 'loading' && <i className="ti ti-loader-2 animate-spin text-blue-500" style={{ fontSize: 18 }}></i>}
                  {customFontStatus === 'loaded' && <i className="ti ti-circle-check text-green-500" style={{ fontSize: 18 }}></i>}
                  {customFontStatus === 'error' && <i className="ti ti-alert-circle text-red-500" style={{ fontSize: 18 }}></i>}
                </div>
                {settings.customFontPath && (
                  <div className="text-xs text-qc-fg-muted truncate">
                    {settings.customFontPath}
                  </div>
                )}
                <div
                  className="p-4 rounded-lg border-2 border-qc-border bg-qc-surface text-center text-2xl"
                  style={{ fontFamily: settings.customFontEnabled && settings.customFontPath ? '"CustomFont","Segoe UI","Microsoft YaHei",sans-serif' : 'inherit' }}
                >
                  Aa 字体预览 123
                </div>
              </div>
            ) : (
              <div className="space-y-3">
                <div>
                  <label className="block text-xs font-medium text-qc-fg-muted mb-1">
                    {t('settings.appearance.customFontUrlLabel') || '字体 CSS 链接'}
                  </label>
                  <div className="relative w-full max-w-xl">
                    <input
                      type="text"
                      value={urlInputValue}
                      onChange={e => setUrlInputValue(e.target.value)}
                      onBlur={() => {
                        const trimmed = urlInputValue.trim()
                        if (trimmed !== (settings.customFontUrl || '')) {
                          onSettingChange('customFontUrl', trimmed)
                          try {
                            const u = new URL(trimmed)
                            const familyParam = u.searchParams.get('family')
                            if (familyParam) {
                              const familyName = familyParam.replace(/\+/g, ' ').replace(/:.*/, '').trim()
                              if (familyName) {
                                onSettingChange('customFontFamily', familyName)
                              }
                            }
                          } catch (_) {}
                        }
                      }}
                      onKeyDown={e => {
                        if (e.key === 'Enter') {
                          e.target.blur()
                        }
                      }}
                      placeholder="https://fonts.googleapis.com/css2?family=..."
                      onFocus={() => {
                        if (!urlInputValue) {
                          setUrlInputValue('https://fonts.googleapis.com/css2?family=')
                        }
                      }}
                      className="w-full px-3 py-2 pr-8 rounded-lg border border-qc-border bg-qc-surface text-qc-fg text-sm"
                    />
                    {customFontStatus === 'loading' && <i className="ti ti-loader-2 animate-spin absolute right-2 top-1/2 -translate-y-1/2 text-blue-500" style={{ fontSize: 16 }}></i>}
                    {customFontStatus === 'loaded' && <i className="ti ti-circle-check absolute right-2 top-1/2 -translate-y-1/2 text-green-500" style={{ fontSize: 16 }}></i>}
                    {customFontStatus === 'error' && <i className="ti ti-alert-circle absolute right-2 top-1/2 -translate-y-1/2 text-red-500" style={{ fontSize: 16 }}></i>}
                  </div>
                </div>
                <div>
                  <label className="block text-xs font-medium text-qc-fg-muted mb-1">
                    {t('settings.appearance.customFontFamilyLabel') || '字体名称'}
                  </label>
                  <input
                    type="text"
                    value={settings.customFontFamily || ''}
                    onChange={e => onSettingChange('customFontFamily', e.target.value)}
                    placeholder={t('settings.appearance.customFontFamilyPlaceholder') || '例如: ZCOOL KuaiLe'}
                    className="w-full max-w-xs px-3 py-2 rounded-lg border border-qc-border bg-qc-surface text-qc-fg text-sm"
                  />
                  <p className="text-xs text-qc-fg-muted mt-1">
                    {t('settings.appearance.customFontFamilyHint') || 'Google Fonts 链接会自动解析，其他链接请手动填写 @font-face 定义的字体名称'}
                  </p>
                </div>
                <div
                  className="p-4 rounded-lg border-2 border-qc-border bg-qc-surface text-center text-2xl"
                  style={{ fontFamily: settings.customFontEnabled && settings.customFontFamily ? `"${settings.customFontFamily}","Segoe UI","Microsoft YaHei",sans-serif` : 'inherit' }}
                >
                  Aa 字体预览 123
                </div>
              </div>
            )}

          </div>}
        </div>

        {theme === 'background' && <div className="space-y-3 animate-slide-in-left-fast">
            <SettingItem label={t('settings.appearance.backgroundBlurStrength')} description={t('settings.appearance.backgroundBlurStrengthDesc')}>
              <Slider
                value={blurPercent}
                onChange={value => onSettingChange('superBackgroundBlurScale', value / 100)}
                min={0}
                max={200}
                step={5}
                unit="%"
                className="w-full max-w-xl"
                sliderClassName="flex-1 min-w-0 w-auto"
              />
            </SettingItem>

            <label className={fieldTitleClass}>
              {t('settings.appearance.backgroundImage')}
            </label>
            <p className={fieldDescClass}>
              {t('settings.appearance.backgroundImageDesc')}
            </p>
            
            <div className="flex items-center gap-3">
              <button onClick={handleSelectBackgroundImage} className="flex items-center gap-2 px-4 py-2 bg-blue-500 hover:bg-blue-600 text-white rounded-lg transition-colors">
                <i className="ti ti-photo" style={{
              fontSize: 18
            }}></i>
                {backgroundImagePath ? t('settings.appearance.changeBackgroundImage') : t('settings.appearance.selectBackgroundImage')}
              </button>

              {backgroundImagePath && <button onClick={handleClearBackgroundImage} className="flex items-center gap-2 px-4 py-2 bg-qc-panel hover:bg-qc-hover text-qc-fg rounded-lg transition-colors">
                  <i className="ti ti-x" style={{
              fontSize: 18
            }}></i>
                  {t('settings.appearance.clearBackgroundImage')}
                </button>}
            </div>

            {backgroundImagePath && <div className="text-xs text-qc-fg-muted truncate">
                {t('settings.appearance.currentImage')}: {backgroundImagePath}
              </div>}
          </div>}

        <div>
          <SettingItem label={t('settings.appearance.clipboardAnimation')} description={t('settings.appearance.clipboardAnimationDesc')}>
            <Toggle checked={settings.clipboardAnimationEnabled} onChange={checked => onSettingChange('clipboardAnimationEnabled', checked)} />
          </SettingItem>

          <SettingItem label={t('settings.appearance.uiAnimation')} description={t('settings.appearance.uiAnimationDesc')}>
            <Toggle checked={settings.uiAnimationEnabled} onChange={checked => onSettingChange('uiAnimationEnabled', checked)} />
          </SettingItem>

          <SettingItem label={t('settings.clipboard.titleBarPosition')} description={t('settings.clipboard.titleBarPositionDesc')}>
            <SegmentedControl
              value={settings.titleBarPosition || 'top'}
              onChange={value => onSettingChange('titleBarPosition', value)}
              options={[
                { value: 'top', label: t('settings.clipboard.positionTop') },
                { value: 'bottom', label: t('settings.clipboard.positionBottom') },
                { value: 'left', label: t('settings.clipboard.positionLeft') },
                { value: 'right', label: t('settings.clipboard.positionRight') }
              ]}
              className="max-w-md"
            />
          </SettingItem>

          <SettingItem
            label={t('settings.appearance.visibleTabs') || '显示标签页'}
            description={t('settings.appearance.visibleTabsDesc') || '控制标签栏中显示哪些可选页面'}
          >
            <MultiSegmentedControl
              values={visibleOptionalTabs}
              onChange={value => onSettingChange('visibleOptionalTabs', value)}
              options={tabVisibilityOptions}
              wrap
              columns={2}
              className="w-full max-w-md"
            />
          </SettingItem>

          <SettingItem label={t('listSettings.listStyle.label')} description={t('listSettings.title')}>
            <SegmentedControl value={settings.listStyle || 'card'} onChange={value => onSettingChange('listStyle', value)} options={[{
              value: 'compact',
              label: t('listSettings.listStyle.compact')
            }, {
              value: 'card',
              label: t('listSettings.listStyle.card')
            }]} className="max-w-sm" />
          </SettingItem>

          <SettingItem label={t('listSettings.rowHeight.label')} description={t('listSettings.title')}>
            <SegmentedControl value={settings.rowHeight || 'medium'} onChange={value => onSettingChange('rowHeight', value)} options={[{
              value: 'auto',
              label: t('listSettings.rowHeight.auto')
            }, {
              value: 'large',
              label: t('listSettings.rowHeight.large')
            }, {
              value: 'medium',
              label: t('listSettings.rowHeight.medium')
            }, {
              value: 'small',
              label: t('listSettings.rowHeight.small')
            }, {
              value: 'xsmall',
              label: t('listSettings.rowHeight.xsmall')
            }]} className="max-w-xl" />
          </SettingItem>

          {settings.rowHeight === 'auto' && <SettingItem label={t('settings.appearance.adaptiveMaxHeight', '自适应最大高度')} description={t('settings.appearance.adaptiveMaxHeightDesc', '限制自适应行高时单条记录的最大高度')}>
              <Slider
                value={adaptiveMaxHeight}
                onChange={handleAdaptiveMaxHeightChange}
                min={80}
                max={350}
                step={10}
                unit="px"
                className="w-full max-w-xl"
                sliderClassName="flex-1 min-w-0 w-auto"
              />
            </SettingItem>}

          {settings.listStyle === 'card' && <SettingItem label={t('settings.appearance.cardSpacing')} description={t('settings.appearance.cardSpacingDesc')}>
              <SegmentedControl value={String(settings.cardSpacing ?? 8)} onChange={value => onSettingChange('cardSpacing', parseInt(value, 10))} options={[0, 4, 8, 12, 16, 20].map(v => ({
              value: String(v),
              label: `${v}px`
            }))} wrap columns={3} className="max-w-sm" />
            </SettingItem>}
            
          <SettingItem label={t('listSettings.fileDisplayMode.label')} description={t('listSettings.title')}>
            <SegmentedControl value={settings.fileDisplayMode || 'detailed'} onChange={value => onSettingChange('fileDisplayMode', value)} options={[{
              value: 'detailed',
              label: t('listSettings.fileDisplayMode.detailed')
            }, {
              value: 'iconOnly',
              label: t('listSettings.fileDisplayMode.iconOnly')
            }]} className="max-w-md" />
          </SettingItem>

          {settings.fileDisplayMode === 'iconOnly' && <SettingItem label={t('settings.appearance.fileIconSize', '文件图标大小')} description={t('settings.appearance.fileIconSizeDesc', '调整大行高和自适应模式下的文件图标大小')}>
              <Select value={settings.fileIconSize || 'large'} onChange={value => onSettingChange('fileIconSize', value)} options={fileIconSizeOptions} className="w-36" />
            </SettingItem>}

        </div>
      </div>
    </SettingsSection>;
}
export default AppearanceSection;
