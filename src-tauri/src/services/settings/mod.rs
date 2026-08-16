mod model;
mod state;
pub mod storage;

pub use model::AppSettings;
pub use state::{get_data_directory, get_settings, update_settings, update_with};
pub use storage::SettingsStorage;

pub fn load_settings_from_file() -> Result<AppSettings, String> {
    SettingsStorage::load()
}
