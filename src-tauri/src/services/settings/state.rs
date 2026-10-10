use super::{storage::SettingsStorage, AppSettings};
use once_cell::sync::Lazy;
use parking_lot::RwLock;

static SETTINGS: Lazy<RwLock<AppSettings>> =
    Lazy::new(|| RwLock::new(SettingsStorage::load().unwrap_or_default()));

pub fn get_settings() -> AppSettings {
    SETTINGS.read().clone()
}

pub fn update_settings(settings: AppSettings) -> Result<(), String> {
    *SETTINGS.write() = settings.clone();
    SettingsStorage::save(&settings)
}

pub fn update_with<F>(updater: F) -> Result<(), String>
where
    F: FnOnce(&mut AppSettings),
{
    let mut settings = SETTINGS.write();
    updater(&mut settings);
    SettingsStorage::save(&settings)
}

pub fn get_data_directory() -> Result<std::path::PathBuf, String> {
    let settings = SETTINGS.read();
    SettingsStorage::get_data_directory(&settings)
}
