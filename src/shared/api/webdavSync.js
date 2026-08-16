import { invoke } from '@tauri-apps/api/core';

export async function testWebdavConnection() {
  return await invoke('webdav_test_connection');
}

export async function uploadWebdav() {
  return await invoke('webdav_upload');
}

export async function downloadWebdav() {
  return await invoke('webdav_download');
}

export async function downloadAllWebdav() {
  return await invoke('webdav_download_all');
}

export async function getWebdavLastReport() {
  return await invoke('webdav_get_last_report');
}

export async function startWebdavScheduler() {
  return await invoke('webdav_start_scheduler');
}

export async function stopWebdavScheduler() {
  return await invoke('webdav_stop_scheduler');
}

export async function hasSavedWebdavPassword(url, username) {
  return await invoke('webdav_has_saved_password', { url, username });
}

export async function setWebdavPassword(url, username, password) {
  return await invoke('webdav_set_password', { url, username, password });
}

export async function hasSavedWebdavEncryptionPassword(url, username, rootPath) {
  return await invoke('webdav_has_saved_encryption_password', { url, username, rootPath });
}

export async function setWebdavEncryptionPassword(url, username, rootPath, password) {
  return await invoke('webdav_set_encryption_password', { url, username, rootPath, password });
}
