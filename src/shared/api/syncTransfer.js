import { invoke } from '@tauri-apps/api/core';

export async function getSyncTransferModeInfos() {
  return await invoke('sync_transfer_get_mode_infos');
}

export async function getSyncTransferLanStatus() {
  return await invoke('sync_transfer_lan_get_status');
}

export async function refreshSyncTransferLanPairingCode() {
  return await invoke('sync_transfer_lan_refresh_pairing_code');
}

export async function listSyncTransferLanPairedPeers() {
  return await invoke('sync_transfer_lan_list_paired_peers');
}

export async function removeSyncTransferLanPairedPeer(deviceId) {
  return await invoke('sync_transfer_lan_remove_paired_peer', {
    deviceId,
  });
}

export async function pairSyncTransferLanPeer(baseUrl, pairingCode) {
  return await invoke('sync_transfer_lan_pair_with_peer', {
    baseUrl,
    pairingCode,
  });
}

export async function fetchSyncTransferLanPeerSnapshot(deviceId) {
  return await invoke('sync_transfer_lan_fetch_peer_snapshot', {
    deviceId,
  });
}

export async function getSyncTransferLanLocalSnapshot() {
  return await invoke('sync_transfer_lan_get_local_snapshot');
}

export async function discoverSyncTransferLanPeers(timeoutMs = 1200) {
  return await invoke('sync_transfer_lan_discover_peers', {
    timeoutMs,
  });
}

export async function getSyncTransferLanAutoSyncStatus() {
  return await invoke('sync_transfer_lan_get_auto_sync_status');
}

export async function updateSyncTransferLanAutoSyncSettings(settings) {
  return await invoke('sync_transfer_lan_update_auto_sync_settings', {
    settings,
  });
}

export async function pushSyncTransferLanPeer(deviceId) {
  return await invoke('sync_transfer_lan_push_to_peer', {
    deviceId,
  });
}

