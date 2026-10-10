use super::model::{
    AppSettings, SETTINGS_MIGRATION_VERSION_V1, SETTINGS_MIGRATION_VERSION_V2,
    SETTINGS_MIGRATION_VERSION_V3,
};
use std::{env, fs, path::PathBuf};

pub struct SettingsStorage;

impl SettingsStorage {
    fn migrate_settings(settings: &mut AppSettings) -> bool {
        let mut migrated = false;
        let migration_version = settings.settings_migration_version.unwrap_or(0);

        if migration_version < SETTINGS_MIGRATION_VERSION_V1 {
            settings.image_preview = true;
            settings.text_preview = true;
            settings.file_preview = true;
            settings.settings_migration_version = Some(SETTINGS_MIGRATION_VERSION_V1);
            migrated = true;
        }

        if migration_version < SETTINGS_MIGRATION_VERSION_V2 {
            settings.settings_migration_version = Some(SETTINGS_MIGRATION_VERSION_V2);
            migrated = true;
        }

        if migration_version < SETTINGS_MIGRATION_VERSION_V3 {
            let _ = settings.normalize_app_filter_blocklist();
            settings.settings_migration_version = Some(SETTINGS_MIGRATION_VERSION_V3);
            migrated = true;
        }

        migrated
    }

    fn is_portable_mode() -> bool {
        if crate::services::is_portable_build() {
            return true;
        }
        env::current_exe()
            .ok()
            .and_then(|exe| {
                exe.parent()
                    .map(|p| p.join("portable.flag").exists() || p.join("portable.txt").exists())
            })
            .unwrap_or(false)
    }

    fn get_data_dir() -> Result<PathBuf, String> {
        if Self::is_portable_mode() {
            let exe_dir = env::current_exe()
                .map_err(|e| e.to_string())?
                .parent()
                .ok_or("无法获取执行目录")?
                .to_path_buf();
            return Ok(exe_dir.join("data"));
        }

        Ok(dirs::data_local_dir()
            .ok_or("无法获取数据目录")?
            .join("quickclipboard"))
    }

    pub fn get_settings_path() -> Result<PathBuf, String> {
        let dir = Self::get_data_dir()?;
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(dir.join("settings.json"))
    }

    pub fn load() -> Result<AppSettings, String> {
        let path = Self::get_settings_path()?;

        if !path.exists() {
            return Ok(AppSettings::default());
        }

        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let has_legacy_lan_sync_settings = content.contains("\"lanSync");
        let mut settings: AppSettings =
            serde_json::from_str(&content).map_err(|e| e.to_string())?;
        let had_legacy_webdav_password = !settings.webdav_password.is_empty();
        if had_legacy_webdav_password {
            if !settings.webdav_url.trim().is_empty() && !settings.webdav_username.trim().is_empty()
            {
                if let Err(e) = crate::services::secure_credentials::set_webdav_password(
                    &settings.webdav_url,
                    &settings.webdav_username,
                    &settings.webdav_password,
                ) {
                    eprintln!("迁移 WebDAV 密码到系统凭据库失败: {}", e);
                }
            }
            settings.webdav_password.clear();
        }
        let normalized = settings.normalize_app_filter_blocklist();
        let migrated = Self::migrate_settings(&mut settings)
            || normalized
            || has_legacy_lan_sync_settings
            || had_legacy_webdav_password;

        if migrated {
            let _ = Self::save(&settings);
        }

        Ok(settings)
    }

    pub fn exists() -> Result<bool, String> {
        let path = Self::get_settings_path()?;
        Ok(path.exists())
    }

    pub fn save(settings: &AppSettings) -> Result<(), String> {
        let path = Self::get_settings_path()?;
        let content = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
        fs::write(&path, content).map_err(|e| e.to_string())
    }

    pub fn get_data_directory(settings: &AppSettings) -> Result<PathBuf, String> {
        if settings.use_custom_storage {
            if let Some(ref path) = settings.custom_storage_path {
                let custom_dir = PathBuf::from(path);
                fs::create_dir_all(&custom_dir).map_err(|e| e.to_string())?;
                return Ok(custom_dir);
            }
        }

        let dir = Self::get_data_dir()?;
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(dir)
    }
}
