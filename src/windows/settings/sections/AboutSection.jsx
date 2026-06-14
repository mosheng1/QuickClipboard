import '@tabler/icons-webfont/dist/tabler-icons.min.css';
import { useTranslation } from 'react-i18next';
import { useState, useEffect } from 'react';
import { openUrl } from '@tauri-apps/plugin-opener';
import { getAppVersion } from '@shared/services/settingsService';
import { copyTextToClipboard } from '@shared/api';
import { toast } from '@shared/store/toastStore';
import SettingsSection from '../components/SettingsSection';
import SettingItem from '../components/SettingItem';
import Button from '@shared/components/ui/Button';
import Select from '@shared/components/ui/Select';
import Toggle from '@shared/components/ui/Toggle';
import { formatUserMessage } from '@shared/utils/userMessages';
import logoIcon from '@/assets/icon1024.png';
import wxzsm from '@/assets/wxzsm.png';
import appLinks from '@shared/config/appLinks.json';

function isPrereleaseVersion(version) {
  const value = String(version || '').toLowerCase();
  return value.includes('alpha') || value.includes('beta') || value.includes('rc') || value.includes('dev');
}

function AboutSection({
  settings,
  onSettingChange
}) {
  const {
    t
  } = useTranslation();
  const [showQROverlay, setShowQROverlay] = useState(false);
  const [version, setVersion] = useState('1.0.0');
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const updateIntervalOptions = [{
    value: 'daily',
    label: t('settings.about.updateIntervalDaily')
  }, {
    value: 'every3days',
    label: t('settings.about.updateIntervalEvery3Days')
  }, {
    value: 'weekly',
    label: t('settings.about.updateIntervalWeekly')
  }];
  const includeBetaUpdates = settings.includeBetaUpdates ?? isPrereleaseVersion(version);
  
  useEffect(() => {
    const fetchVersion = async () => {
      try {
        const versionInfo = await getAppVersion();
        setVersion(versionInfo.version || '1.0.0');
      } catch (error) {
        console.error('获取版本信息失败:', error);
        setVersion('1.0.0');
      }
    };
    
    fetchVersion();
  }, []);
  
  const handleCheckUpdates = async () => {
    if (checkingUpdate) return;
    setCheckingUpdate(true);
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const opened = await invoke('check_updates_and_open_window');
      if (opened) {
        toast.success(t('updater.updateWindowOpened'));
      } else {
        toast.info(t('updater.noUpdate'));
      }
    } catch (e) {
      toast.error(t('updater.checkFailed', {
        msg: formatUserMessage(e, t, 'errors.operationFailed'),
      }));
    } finally {
      setCheckingUpdate(false);
    }
  };
  const handleCopyVersion = async () => {
    try {
      await copyTextToClipboard(`${t('settings.about.appName')} v${version}`);
      toast.success(t('settings.about.versionCopied'));
    } catch (error) {
      console.error('复制版本号失败:', error);
      toast.error(t('common.operationFailed'));
    }
  };
  const handleOpenGitHub = async () => {
    try {
      await openUrl(appLinks.github);
    } catch (error) {
      console.error('打开GitHub链接失败:', error);
    }
  };
  const handleOpenBilibili = async () => {
    try {
      await openUrl(appLinks.bilibili);
    } catch (error) {
      console.error('打开Bilibili链接失败:', error);
    }
  };
  const handleOpenCommunity = async () => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('open_community_window');
    } catch (error) {
      console.error('打开社区交流窗口失败:', error);
    }
  };

  return (
    <SettingsSection title={t('settings.about.title')} description={t('settings.about.description')}>
      <div className="space-y-6">
        <div className="text-center">
          <div className="inline-flex items-center justify-center w-16 h-16 bg-blue-100 rounded-full mb-3 overflow-hidden">
            <img src={logoIcon} alt="QuickClipboard Logo" className="w-12 h-12 object-contain" />
          </div>
          <h3 className="text-xl font-bold text-qc-fg mb-1">
            {t('settings.about.appName')}
          </h3>
          <div
            className="inline-flex items-center gap-1 cursor-pointer text-sm text-qc-fg-muted mb-3 hover:text-qc-fg transition-colors"
            onClick={handleCopyVersion}
            title={t('settings.about.clickToCopyVersion')}
          >
            <span>{t('settings.about.version')} {version}</span>
            <i className="ti ti-copy text-xs opacity-60"></i>
          </div>
          <p className="text-sm text-qc-fg-muted max-w-md mx-auto">
            {t('settings.about.descriptionText')}
          </p>
        </div>

        <div className="flex flex-wrap gap-2 justify-center">
          <Button
            variant="primary"
            icon={checkingUpdate ? <i className="ti ti-loader-2 animate-spin"></i> : <i className="ti ti-download"></i>}
            onClick={handleCheckUpdates}
            disabled={checkingUpdate}
          >
            {checkingUpdate ? t('updater.checking') : t('settings.about.checkUpdates')}
          </Button>
          <Button variant="secondary" icon={<i className="ti ti-brand-github"></i>} onClick={handleOpenGitHub}>
            GitHub
          </Button>
          <Button variant="secondary" icon={<i className="ti ti-users"></i>} onClick={handleOpenCommunity}>
            社区交流
          </Button>
          <Button variant="secondary" icon={<i className="ti ti-brand-bilibili"></i>} onClick={handleOpenBilibili}>
            Bilibili
          </Button>
        </div>

        <div className="rounded-lg border border-qc-border bg-qc-surface px-4">
          <div className="pt-4 pb-2">
            <h4 className="text-sm font-semibold text-qc-fg">
              {t('settings.about.updateSettingsTitle')}
            </h4>
            <p className="mt-1 text-xs leading-relaxed text-qc-fg-muted">
              {t('settings.about.updateSettingsDesc')}
            </p>
          </div>

          <SettingItem
            label={t('settings.about.updateInterval')}
            description={t('settings.about.updateIntervalDesc')}
          >
            <Select
              value={settings.updateCheckInterval || 'daily'}
              onChange={value => onSettingChange('updateCheckInterval', value)}
              options={updateIntervalOptions}
              className="w-32"
            />
          </SettingItem>

          <SettingItem
            label={t('settings.about.includeBetaUpdates')}
            description={t('settings.about.includeBetaUpdatesDesc')}
          >
            <Toggle
              checked={includeBetaUpdates}
              onChange={checked => onSettingChange('includeBetaUpdates', checked)}
            />
          </SettingItem>

          <SettingItem
            label={t('settings.about.disableUpdatePopup')}
            description={t('settings.about.disableUpdatePopupDesc')}
          >
            <Toggle
              checked={settings.disableUpdatePopup === true}
              onChange={checked => onSettingChange('disableUpdatePopup', checked)}
            />
          </SettingItem>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-6 items-center">
          <div className="space-y-3">
            <div className="flex items-center gap-3 p-3 bg-qc-panel rounded-lg">
              <i className="ti ti-star text-yellow-500 text-lg"></i>
              <span className="text-sm text-qc-fg-muted">{t('settings.about.star')}</span>
            </div>
            <div className="flex items-center gap-3 p-3 bg-qc-panel rounded-lg">
              <i className="ti ti-bug text-red-500 text-lg"></i>
              <span className="text-sm text-qc-fg-muted">{t('settings.about.feedback')}</span>
            </div>
            <div className="flex items-center gap-3 p-3 bg-qc-panel rounded-lg">
              <i className="ti ti-speakerphone text-blue-500 text-lg"></i>
              <span className="text-sm text-qc-fg-muted">{t('settings.about.share')}</span>
            </div>
            <div className="flex items-center gap-3 p-3 bg-qc-panel rounded-lg">
              <i className="ti ti-coffee text-orange-500 text-lg"></i>
              <span className="text-sm text-qc-fg-muted">{t('settings.about.donate')}</span>
            </div>
          </div>

          <div className="flex justify-center">
            <div 
              className="relative cursor-pointer"
              onMouseEnter={() => setShowQROverlay(true)}
              onMouseLeave={() => setShowQROverlay(false)}
            >
              <div className="w-50 h-50 bg-gradient-to-br from-qc-panel to-qc-panel-2 rounded-xl flex items-center justify-center border border-qc-border">
                <img src={wxzsm} alt="微信赞赏码" className="w-full h-full object-contain rounded-lg" />
              </div>
              <div className="absolute -top-2 -right-2 w-7 h-7 bg-green-500 rounded-full flex items-center justify-center">
                <i className="ti ti-brand-wechat text-white"></i>
              </div>
              <p className="text-sm text-qc-fg-muted text-center mt-2">微信赞赏</p>
              
              {showQROverlay && (
                <div className="absolute top-1/2 left-1/2 transform -translate-x-1/2 -translate-y-1/2 z-10 bg-qc-surface rounded-xl shadow-2xl p-4 border border-qc-border">
                  <div className="w-56 h-56 bg-qc-surface rounded-lg">
                    <img src={wxzsm} alt="微信赞赏码" className="w-full h-full object-contain" />
                  </div>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>
    </SettingsSection>
  );
}
export default AboutSection;
