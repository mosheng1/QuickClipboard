import { invoke } from '@tauri-apps/api/core';

async function uint8ArrayToNumberArrayChunked(data, chunkSize = 256 * 1024) {
  if (!data) return [];
  if (Array.isArray(data)) return data;
  const u8 = data instanceof Uint8Array ? data : new Uint8Array(data);
  const result = new Array(u8.length);
  for (let offset = 0; offset < u8.length; offset += chunkSize) {
    const end = Math.min(u8.length, offset + chunkSize);
    for (let index = offset; index < end; index += 1) {
      result[index] = u8[index];
    }
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
  return result;
}

export async function ensureDropProxy() {
  return await invoke('drop_proxy_ensure');
}

export async function showDropProxy(bounds) {
  return await invoke('drop_proxy_show', { bounds });
}

export async function hideDropProxy() {
  return await invoke('drop_proxy_hide');
}

export async function routeDropProxyPathsAtCursor(paths, cursorPos) {
  return await invoke('drop_proxy_route_paths_at_cursor', { paths, cursorPos });
}

export async function saveDropProxyResource(filename, data) {
  const payloadData = await uint8ArrayToNumberArrayChunked(data);
  return await invoke('drop_proxy_save_resource', {
    payload: {
      filename,
      data: payloadData,
    },
  });
}

export async function saveDropProxyUrl(filename, url) {
  return await invoke('drop_proxy_save_url', {
    payload: {
      filename,
      url,
    },
  });
}

export async function cleanupDropProxyOrphanResources(minAgeMs = 5000) {
  return await invoke('drop_proxy_cleanup_orphan_resources', { minAgeMs });
}
