import { invoke } from '@tauri-apps/api/core';

export async function openReceiveBox() {
  return await invoke('receive_box_open');
}

export async function listReceiveBoxLanFiles() {
  return await invoke('receive_box_list_lan_files');
}

export async function listReceiveBoxCloudFiles() {
  return await invoke('receive_box_list_cloud_files');
}

export async function downloadReceiveBoxCloudFile(fileId) {
  return await invoke('receive_box_download_cloud_file', { fileId });
}

export async function openReceiveBoxLocalFile(path) {
  return await invoke('receive_box_open_local_file', { path });
}

export async function revealReceiveBoxLocalFile(path) {
  return await invoke('receive_box_reveal_local_file', { path });
}

export async function deleteReceiveBoxLocalFile(path) {
  return await invoke('receive_box_delete_local_file', { path });
}

export async function deleteReceiveBoxCloudFile(fileId) {
  return await invoke('receive_box_delete_cloud_file', { fileId });
}

export async function addReceiveBoxFileToTransferShelf(path) {
  return await invoke('receive_box_add_to_transfer_shelf', { path });
}
