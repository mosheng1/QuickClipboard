import { useTranslation } from 'react-i18next';
import SettingsSection from '../components/SettingsSection';
import SettingItem from '../components/SettingItem';
import Toggle from '@shared/components/ui/Toggle';
import Select from '@shared/components/ui/Select';
import Input from '@shared/components/ui/Input';
import Textarea from '@shared/components/ui/Textarea';
import MultiSegmentedControl from '@shared/components/ui/MultiSegmentedControl';
function ClipboardSection({
  settings,
  onSettingChange
}) {
  const {
    t
  } = useTranslation();
  const positionOptions = [{
    value: 'smart',
    label: t('settings.clipboard.positionSmart')
  }, {
    value: 'remember',
    label: t('settings.clipboard.positionRemember')
  }];
  const pasteShortcutModeOptions = [{
    value: 'ctrl_v',
    label: t('settings.clipboard.pasteShortcutCtrlV')
  }, {
    value: 'shift_insert',
    label: t('settings.clipboard.pasteShortcutShiftInsert')
  }];
  const leftClickActionOptions = [{
    value: 'none',
    label: t('settings.clipboard.leftClickActionNone')
  }, {
    value: 'single_paste',
    label: t('settings.clipboard.leftClickActionSinglePaste')
  }, {
    value: 'single_copy',
    label: t('settings.clipboard.leftClickActionSingleCopy')
  }, {
    value: 'double_paste',
    label: t('settings.clipboard.leftClickActionDoublePaste')
  }, {
    value: 'double_copy',
    label: t('settings.clipboard.leftClickActionDoubleCopy')
  }];
  const displayPriorityOptions = [{
    value: 'text,html,image',
    label: t('settings.clipboard.displayPriorityTextHtmlImage')
  }, {
    value: 'text,image,html',
    label: t('settings.clipboard.displayPriorityTextImageHtml')
  }, {
    value: 'html,text,image',
    label: t('settings.clipboard.displayPriorityHtmlTextImage')
  }, {
    value: 'html,image,text',
    label: t('settings.clipboard.displayPriorityHtmlImageText')
  }, {
    value: 'image,text,html',
    label: t('settings.clipboard.displayPriorityImageTextHtml')
  }, {
    value: 'image,html,text',
    label: t('settings.clipboard.displayPriorityImageHtmlText')
  }];
  const previewOptionValues = [
    settings.imagePreview !== false ? 'imagePreview' : null,
    settings.textPreview !== false ? 'textPreview' : null,
    settings.filePreview !== false ? 'filePreview' : null
  ].filter(Boolean);
  const previewOptions = [{
    value: 'imagePreview',
    label: t('settings.clipboard.imagePreview')
  }, {
    value: 'textPreview',
    label: t('settings.clipboard.textPreview')
  }, {
    value: 'filePreview',
    label: t('settings.clipboard.filePreview')
  }];
  const handlePreviewOptionsChange = nextValues => {
    const nextPreviewValues = Array.isArray(nextValues) ? nextValues : [];
    onSettingChange({
      imagePreview: nextPreviewValues.includes('imagePreview'),
      textPreview: nextPreviewValues.includes('textPreview'),
      filePreview: nextPreviewValues.includes('filePreview')
    });
  };
  return <>
      <SettingsSection title={t('settings.clipboard.title')} description={t('settings.clipboard.description')}>
        <SettingItem label={t('settings.clipboard.monitor')} description={t('settings.clipboard.monitorDesc')}>
          <Toggle checked={settings.clipboardMonitor} onChange={checked => onSettingChange('clipboardMonitor', checked)} />
        </SettingItem>

        <SettingItem label={t('settings.clipboard.leftClickAction')} description={t('settings.clipboard.leftClickActionDesc')}>
          <Select value={settings.leftClickAction || 'single_paste'} onChange={value => onSettingChange('leftClickAction', value)} options={leftClickActionOptions} className="w-48" />
        </SettingItem>

        <SettingItem label={t('settings.clipboard.saveImages')} description={t('settings.clipboard.saveImagesDesc')}>
          <Toggle checked={settings.saveImages} onChange={checked => onSettingChange('saveImages', checked)} />
        </SettingItem>

        <SettingItem label={t('settings.clipboard.previewOptions')} description={t('settings.clipboard.previewOptionsDesc')}>
          <MultiSegmentedControl
            values={previewOptionValues}
            onChange={handlePreviewOptionsChange}
            options={previewOptions}
            wrap
            columns={3}
            className="w-full max-w-md"
          />
        </SettingItem>

        <SettingItem label={t('settings.clipboard.displayPriority')} description={t('settings.clipboard.displayPriorityDesc')}>
          <Select value={settings.displayPriorityOrder || 'text,html,image'} onChange={value => onSettingChange('displayPriorityOrder', value)} options={displayPriorityOptions} className="w-56" />
        </SettingItem>

        <SettingItem label={t('settings.clipboard.imageMaxSize')} description={t('settings.clipboard.imageMaxSizeDesc')}>
          <Input type="number" value={settings.imageMaxSizeMb ?? 15} onChange={e => onSettingChange('imageMaxSizeMb', parseInt(e.target.value) || 15)} min={1} max={100} className="w-24" suffix="MB" />
        </SettingItem>

        <SettingItem label={t('settings.clipboard.imageMaxDimension')} description={t('settings.clipboard.imageMaxDimensionDesc')}>
          <div className="flex items-center gap-2">
            <Input type="number" value={settings.imageMaxWidth ?? 4096} onChange={e => onSettingChange('imageMaxWidth', parseInt(e.target.value) || 4096)} min={256} max={16384} className="w-24" />
            <span className="text-qc-fg-muted">×</span>
            <Input type="number" value={settings.imageMaxHeight ?? 4096} onChange={e => onSettingChange('imageMaxHeight', parseInt(e.target.value) || 4096)} min={256} max={16384} className="w-24" suffix="px" />
          </div>
        </SettingItem>

        <SettingItem label={t('settings.clipboard.windowPosition')} description={t('settings.clipboard.windowPositionDesc')}>
          <Select value={settings.windowPositionMode} onChange={value => onSettingChange('windowPositionMode', value)} options={positionOptions} className="w-48" />
        </SettingItem>

        <SettingItem label={t('settings.clipboard.rememberWindowSize')} description={t('settings.clipboard.rememberWindowSizeDesc')}>
          <Toggle checked={settings.rememberWindowSize} onChange={checked => onSettingChange('rememberWindowSize', checked)} />
        </SettingItem>

        <SettingItem label={t('settings.clipboard.edgeHideEnabled')} description={t('settings.clipboard.edgeHideEnabledDesc')}>
          <Toggle checked={settings.edgeHideEnabled} onChange={checked => onSettingChange('edgeHideEnabled', checked)} />
        </SettingItem>

        <SettingItem label={t('settings.clipboard.edgeHideOffset')} description={t('settings.clipboard.edgeHideOffsetDesc')}>
          <Input type="number" value={settings.edgeHideOffset ?? 3} onChange={e => onSettingChange('edgeHideOffset', parseInt(e.target.value))} min={0} max={50} className="w-24" suffix={t('settings.common.pixels')} />
        </SettingItem>

        <SettingItem label={t('settings.clipboard.autoScrollToTop')} description={t('settings.clipboard.autoScrollToTopDesc')}>
          <Toggle checked={settings.autoScrollToTopOnShow} onChange={checked => onSettingChange('autoScrollToTopOnShow', checked)} />
        </SettingItem>

        <SettingItem label={t('settings.clipboard.autoClearSearch')} description={t('settings.clipboard.autoClearSearchDesc')}>
          <Toggle checked={settings.autoClearSearch} onChange={checked => onSettingChange('autoClearSearch', checked)} />
        </SettingItem>

        <SettingItem label={t('settings.clipboard.autoFocusSearch')} description={t('settings.clipboard.autoFocusSearchDesc')}>
          <Toggle checked={settings.autoFocusSearch} onChange={checked => onSettingChange('autoFocusSearch', checked)} />
        </SettingItem>

        <SettingItem label={t('settings.clipboard.pasteToTop')} description={t('settings.clipboard.pasteToTopDesc')}>
          <Toggle checked={settings.pasteToTop} onChange={checked => onSettingChange('pasteToTop', checked)} />
        </SettingItem>

        <SettingItem label={t('settings.clipboard.showBadges')} description={t('settings.clipboard.showBadgesDesc')}>
          <Toggle checked={settings.showBadges !== false} onChange={checked => onSettingChange('showBadges', checked)} />
        </SettingItem>

        <SettingItem label={t('settings.clipboard.showSourceIcon')} description={t('settings.clipboard.showSourceIconDesc')}>
          <Toggle checked={settings.showSourceIcon !== false} onChange={checked => onSettingChange('showSourceIcon', checked)} />
        </SettingItem>

        <SettingItem label={t('listSettings.showShortcuts.label')} description={t('listSettings.showShortcuts.description')}>
          <Toggle checked={settings.showListShortcuts !== false} onChange={checked => onSettingChange('showListShortcuts', checked)} />
        </SettingItem>

        <SettingItem label={t('listSettings.showIndex.label')} description={t('listSettings.showIndex.description')}>
          <Toggle checked={settings.showListIndex !== false} onChange={checked => onSettingChange('showListIndex', checked)} />
        </SettingItem>

        <SettingItem label={t('settings.clipboard.pasteShortcutMode')} description={t('settings.clipboard.pasteShortcutModeDesc')}>
          <Select value={settings.pasteShortcutMode || 'ctrl_v'} onChange={value => onSettingChange('pasteShortcutMode', value)} options={pasteShortcutModeOptions} className="w-48" />
        </SettingItem>

        <SettingItem label={t('settings.clipboard.modifierClickMultiSelect')} description={t('settings.clipboard.modifierClickMultiSelectDesc')}>
          <Toggle checked={settings.modifierClickMultiSelect !== false} onChange={checked => onSettingChange('modifierClickMultiSelect', checked)} />
        </SettingItem>
      </SettingsSection>
    </>;
}
export default ClipboardSection;
