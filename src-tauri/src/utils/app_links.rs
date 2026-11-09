use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppLinks {
    pub website: String,
    pub github: String,
    #[serde(rename = "pdChannel", default)]
    pub pd_channel: String,
    #[serde(rename = "qqGroup1", default)]
    pub qq_group_1: String,
    #[serde(rename = "qqGroup2", default)]
    pub qq_group_2: String,
    pub bilibili: String,
    pub changelog: String,
    #[serde(rename = "releasesLatest")]
    pub releases_latest: String,
}

static LINKS: Lazy<Result<AppLinks, String>> = Lazy::new(|| {
    let json = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../src/shared/config/appLinks.json"
    ));
    serde_json::from_str::<AppLinks>(json).map_err(|e| format!("appLinks.json 解析失败: {}", e))
});

pub fn app_links() -> Result<&'static AppLinks, String> {
    match LINKS.as_ref() {
        Ok(v) => Ok(v),
        Err(e) => Err(e.clone()),
    }
}
