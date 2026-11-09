use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use super::pairing::{create_pairing_challenge, PairingChallenge};
use super::peer_store::PairedPeer;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingCodeView {
    pub pairing_code: String,
    pub expires_at_ms: i64,
    pub remaining_attempts: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanRuntimeStatus {
    pub device_id: String,
    pub device_name: String,
    pub http_port: u16,
    pub http_running: bool,
    pub discovery_running: bool,
    pub local_endpoints: Vec<super::discovery::LanLocalEndpoint>,
    pub pairing_code: PairingCodeView,
    pub paired_count: usize,
}

#[derive(Debug, Clone)]
struct PairingState {
    challenge: PairingChallenge,
    failed_attempts: u8,
}

#[derive(Debug, Default)]
struct LanRuntime {
    pairing_state: Option<PairingState>,
}

static RUNTIME: Lazy<Mutex<LanRuntime>> = Lazy::new(|| Mutex::new(LanRuntime::default()));

pub fn status() -> LanRuntimeStatus {
    let pairing_code = current_pairing_code();
    let http_port = super::http_server::running_port().unwrap_or(super::DEFAULT_HTTP_PORT);
    LanRuntimeStatus {
        device_id: device_id(),
        device_name: device_name(),
        http_port,
        http_running: super::http_server::is_running(),
        discovery_running: super::discovery::is_running(),
        local_endpoints: super::discovery::local_endpoints(http_port),
        pairing_code,
        paired_count: super::peer_store::list_peers().len(),
    }
}

pub fn device_id() -> String {
    crate::services::sync_transfer::device_id()
}

pub fn device_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "QuickClipboard Desktop".to_string())
}

pub fn current_pairing_code() -> PairingCodeView {
    let mut runtime = RUNTIME.lock();
    let state = ensure_pairing_state(&mut runtime);
    view_pairing_state(state)
}

pub fn refresh_pairing_code() -> PairingCodeView {
    let challenge = create_pairing_challenge();
    let state = PairingState {
        challenge,
        failed_attempts: 0,
    };
    let view = view_pairing_state(&state);
    RUNTIME.lock().pairing_state = Some(state);
    view
}

pub fn verify_pairing_code(pairing_code: &str) -> Result<(), String> {
    let mut runtime = RUNTIME.lock();
    let Some(state) = runtime.pairing_state.as_mut() else {
        runtime.pairing_state = Some(PairingState {
            challenge: create_pairing_challenge(),
            failed_attempts: 0,
        });
        return Err("配对码已刷新，请重新输入".to_string());
    };
    if is_expired(state.challenge.expires_at_ms) {
        runtime.pairing_state = Some(PairingState {
            challenge: create_pairing_challenge(),
            failed_attempts: 0,
        });
        return Err("配对码已过期，请重新输入".to_string());
    }
    if state.failed_attempts >= state.challenge.max_attempts {
        return Err("配对码尝试次数过多，请刷新后重试".to_string());
    }
    if state.challenge.pairing_code != pairing_code.trim() {
        state.failed_attempts = state.failed_attempts.saturating_add(1);
        return Err("配对码不正确".to_string());
    }
    state.failed_attempts = 0;
    Ok(())
}

pub fn confirm_pairing(
    device_id: String,
    device_name: String,
    base_url: String,
    pairing_code: String,
) -> Result<String, String> {
    let device_id = device_id.trim().to_string();
    if device_id.is_empty() {
        return Err("设备 ID 不能为空".to_string());
    }
    if device_id == self::device_id() {
        return Err("不能配对当前设备自身".to_string());
    }

    verify_pairing_code(&pairing_code)?;

    let peer_token = super::pairing::create_peer_token();
    let peer = PairedPeer::new(
        device_id,
        device_name.trim().to_string(),
        base_url.trim().to_string(),
        peer_token.clone(),
    );
    super::peer_store::upsert_peer(peer)?;
    Ok(peer_token)
}

pub fn verify_peer_token(device_id: &str, peer_token: &str) -> bool {
    let device_id = device_id.trim();
    let peer_token = peer_token.trim();
    if device_id.is_empty() || peer_token.is_empty() {
        return false;
    }
    super::peer_store::list_peers()
        .into_iter()
        .any(|peer| peer.device_id == device_id && peer.peer_token == peer_token)
}

fn view_pairing_state(state: &PairingState) -> PairingCodeView {
    PairingCodeView {
        pairing_code: state.challenge.pairing_code.clone(),
        expires_at_ms: state.challenge.expires_at_ms,
        remaining_attempts: state
            .challenge
            .max_attempts
            .saturating_sub(state.failed_attempts),
    }
}

fn ensure_pairing_state(runtime: &mut LanRuntime) -> &PairingState {
    let should_refresh = runtime
        .pairing_state
        .as_ref()
        .map(|state| is_expired(state.challenge.expires_at_ms))
        .unwrap_or(true);
    if should_refresh {
        runtime.pairing_state = Some(PairingState {
            challenge: create_pairing_challenge(),
            failed_attempts: 0,
        });
    }
    runtime
        .pairing_state
        .as_ref()
        .expect("pairing_state must exist")
}

fn is_expired(expires_at_ms: i64) -> bool {
    chrono::Utc::now().timestamp_millis() >= expires_at_ms
}
