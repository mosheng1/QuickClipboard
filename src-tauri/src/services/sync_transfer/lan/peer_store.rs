use serde::{Deserialize, Serialize};

const PAIRED_PEERS_KEY: &str = "sync_transfer_lan_paired_peers";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedPeer {
    pub device_id: String,
    pub device_name: String,
    pub base_url: String,
    pub peer_token: String,
    pub paired_at_ms: i64,
    pub last_seen_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedPeerInfo {
    pub device_id: String,
    pub device_name: String,
    pub base_url: String,
    pub paired_at_ms: i64,
    pub last_seen_at_ms: Option<i64>,
}

impl PairedPeer {
    pub fn new(
        device_id: String,
        device_name: String,
        base_url: String,
        peer_token: String,
    ) -> Self {
        Self {
            device_id,
            device_name,
            base_url,
            peer_token,
            paired_at_ms: chrono::Utc::now().timestamp_millis(),
            last_seen_at_ms: None,
        }
    }

    pub fn info(&self) -> PairedPeerInfo {
        PairedPeerInfo {
            device_id: self.device_id.clone(),
            device_name: self.device_name.clone(),
            base_url: self.base_url.clone(),
            paired_at_ms: self.paired_at_ms,
            last_seen_at_ms: self.last_seen_at_ms,
        }
    }
}

pub fn list_peers() -> Vec<PairedPeer> {
    let peers =
        crate::services::store::get::<Vec<PairedPeer>>(PAIRED_PEERS_KEY).unwrap_or_default();
    let original_len = peers.len();
    let peers = dedupe_peers(peers);
    if peers.len() != original_len {
        let _ = save_peers(&peers);
    }
    peers
}

pub fn list_peer_infos() -> Vec<PairedPeerInfo> {
    list_peers().into_iter().map(|peer| peer.info()).collect()
}

pub fn save_peers(peers: &[PairedPeer]) -> Result<(), String> {
    crate::services::store::set(PAIRED_PEERS_KEY, &peers.to_vec())
}

pub fn upsert_peer(peer: PairedPeer) -> Result<(), String> {
    let mut peers = list_peers();
    peers.retain(|item| !same_peer_identity(item, &peer));
    peers.push(peer);
    save_peers(&peers)
}

fn dedupe_peers(peers: Vec<PairedPeer>) -> Vec<PairedPeer> {
    let mut out: Vec<PairedPeer> = Vec::new();
    for peer in peers {
        if let Some(index) = out.iter().position(|item| same_peer_identity(item, &peer)) {
            out[index] = peer;
        } else {
            out.push(peer);
        }
    }
    out
}

fn same_peer_identity(left: &PairedPeer, right: &PairedPeer) -> bool {
    if left.device_id == right.device_id {
        return true;
    }
    let left_base_url = normalized_base_url(&left.base_url);
    !left_base_url.is_empty() && left_base_url == normalized_base_url(&right.base_url)
}

fn normalized_base_url(base_url: &str) -> String {
    base_url.trim().trim_end_matches('/').to_ascii_lowercase()
}

pub fn mark_peer_seen(device_id: &str) -> Result<(), String> {
    let mut peers = list_peers();
    let Some(peer) = peers.iter_mut().find(|peer| peer.device_id == device_id) else {
        return Ok(());
    };
    peer.last_seen_at_ms = Some(chrono::Utc::now().timestamp_millis());
    save_peers(&peers)
}

pub fn remove_peer(device_id: &str) -> Result<bool, String> {
    let mut peers = list_peers();
    let before = peers.len();
    peers.retain(|peer| peer.device_id != device_id);
    if peers.len() == before {
        return Ok(false);
    }
    save_peers(&peers)?;
    Ok(true)
}
