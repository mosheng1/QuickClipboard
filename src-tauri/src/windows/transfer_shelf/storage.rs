// 文件盒的持久化层
//
// 复用 services::store 把多个 shelf 的暂存队列、目标设备、窗口几何写入 app-store.json。
// task 状态属于运行期数据，不在持久化范围。

use serde::{Deserialize, Serialize};

const STATE_KEY: &str = "transfer_shelf_state";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ShelfFilePersisted {
    pub path: String,
    #[serde(default)]
    pub added_at_ms: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ShelfGeometryPersisted {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ShelfPersisted {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub files: Vec<ShelfFilePersisted>,
    #[serde(default)]
    pub selected_peer_ids: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ShelfStatePersisted {
    #[serde(default)]
    pub shelves: Vec<ShelfPersisted>,
    #[serde(default)]
    pub geometries: std::collections::HashMap<String, ShelfGeometryPersisted>,
    #[serde(default)]
    pub name_counter: u32,
}

pub fn load() -> ShelfStatePersisted {
    crate::services::store::get::<ShelfStatePersisted>(STATE_KEY).unwrap_or_default()
}

pub fn save(state: &ShelfStatePersisted) -> Result<(), String> {
    crate::services::store::set(STATE_KEY, state)
}

/// 写入或更新单个 shelf 的暂存数据。
pub fn upsert_shelf(shelf: ShelfPersisted) -> Result<(), String> {
    let mut state = load();
    match state.shelves.iter_mut().find(|item| item.id == shelf.id) {
        Some(existing) => *existing = shelf,
        None => state.shelves.push(shelf),
    }
    save(&state)
}

pub fn remove_shelf(id: &str) -> Result<(), String> {
    let mut state = load();
    let before = state.shelves.len();
    state.shelves.retain(|item| item.id != id);
    state.geometries.remove(id);
    if state.shelves.len() == before && !state.geometries.contains_key(id) {
        return Ok(());
    }
    save(&state)
}

pub fn upsert_geometry(id: &str, geometry: ShelfGeometryPersisted) -> Result<(), String> {
    let mut state = load();
    state.geometries.insert(id.to_string(), geometry);
    save(&state)
}
