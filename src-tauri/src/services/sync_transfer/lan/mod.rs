pub mod auto_sync;
pub mod discovery;
pub mod files;
pub mod http_client;
pub mod http_server;
pub mod pairing;
pub mod peer_store;
pub mod pull;
pub mod push;
pub mod runtime;
pub mod snapshot;
pub mod transfer;

pub const DEFAULT_PAIRING_CODE_TTL_SECS: u64 = 300;
pub const DEFAULT_PAIRING_MAX_ATTEMPTS: u8 = 5;
pub const DEFAULT_HTTP_PORT: u16 = 35691;

pub use auto_sync::{LanAutoSyncSettings, LanAutoSyncStatus};
pub use discovery::DiscoveredLanPeer;
pub use peer_store::PairedPeerInfo;
pub use runtime::{LanRuntimeStatus, PairingCodeView};
pub use snapshot::{LanGroupBatch, LanRecordBatch, LanSyncSnapshot, LanTombstoneBatch};
pub use transfer::{FileTransferProgress, FileTransferProgressCallback, FileTransferResult};
