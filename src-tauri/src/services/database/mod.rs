pub mod clipboard;
pub mod connection;
pub mod favorites;
pub mod groups;
mod models;
pub mod tombstones;

pub use clipboard::*;
pub use connection::init_database;
pub use favorites::*;
pub use groups::*;
pub use models::*;
pub use tombstones::*;

pub fn webdav_local_sync_parts_signature() -> Result<WebdavLocalSyncSignature, String> {
    connection::with_connection(|conn| {
        let clipboard: (i64, i64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(MAX(updated_at), 0) FROM clipboard",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let favorites: (i64, i64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(MAX(updated_at), 0) FROM favorites",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let groups: (i64, i64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(MAX(updated_at), 0) FROM groups",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let tombstones: (i64, i64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(MAX(deleted_at), 0) FROM sync_tombstones",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        Ok(WebdavLocalSyncSignature {
            clipboard: format!("{}:{}", clipboard.0, clipboard.1),
            favorites: format!("{}:{}", favorites.0, favorites.1),
            groups: format!("{}:{}", groups.0, groups.1),
            tombstones: format!("{}:{}", tombstones.0, tombstones.1),
        })
    })
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WebdavLocalSyncSignature {
    pub clipboard: String,
    pub favorites: String,
    pub groups: String,
    pub tombstones: String,
}
