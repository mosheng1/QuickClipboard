use super::types::{SyncCollection, SyncIndex};
use super::webdav_client::WebdavClient;

pub async fn load_index(
    client: &WebdavClient,
    collection: SyncCollection,
) -> Result<SyncIndex, String> {
    let path = format!("{}/index.json", collection.dir());
    let index = client.get_json(&path).await?;
    if index.is_some() {
        client.mark_dir_ensured("");
        client.mark_dir_ensured(collection.dir());
        client.mark_dir_ensured(&format!("{}/chunks", collection.dir()));
    }
    Ok(index.unwrap_or_default())
}

pub async fn save_index(
    client: &WebdavClient,
    collection: SyncCollection,
    index: &SyncIndex,
) -> Result<(), String> {
    let path = format!("{}/index.json", collection.dir());
    client.put_json(&path, index).await
}
