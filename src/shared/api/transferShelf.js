import { invoke } from '@tauri-apps/api/core';

export async function createTransferShelf() {
  return await invoke('transfer_shelf_create');
}

export async function listTransferShelves() {
  return await invoke('transfer_shelf_list');
}

export async function renameTransferShelf(id, name) {
  return await invoke('transfer_shelf_rename', { id, name });
}

export async function closeTransferShelf(id) {
  return await invoke('transfer_shelf_close', { id });
}

export async function describeTransferShelfPaths(paths) {
  return await invoke('transfer_shelf_describe_paths', { paths });
}

export async function addPathsToTransferShelf(id, paths) {
  return await invoke('transfer_shelf_add_paths', { id, paths });
}

export async function sendTransferShelf(id, targets) {
  return await invoke('transfer_shelf_send', { id, targets });
}

export async function uploadTransferShelfCloud(id, targets) {
  return await invoke('transfer_shelf_upload_cloud', { id, targets });
}

export async function loadTransferShelfState(id) {
  return await invoke('transfer_shelf_load_state', { id });
}

export async function saveTransferShelfState(id, files, selectedPeerIds) {
  return await invoke('transfer_shelf_save_state', {
    id,
    files,
    selectedPeerIds,
  });
}

export async function saveTransferShelfGeometry(id) {
  return await invoke('transfer_shelf_save_geometry', { id });
}

