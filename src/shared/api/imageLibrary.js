import { invoke } from '@tauri-apps/api/core'
import { convertFileSrc } from '@tauri-apps/api/core'

async function uint8ArrayToNumberArrayChunked(data, chunkSize = 256 * 1024) {
  if (!data) return []
  if (Array.isArray(data)) return data
  const u8 = data instanceof Uint8Array ? data : new Uint8Array(data)
  const result = new Array(u8.length)
  for (let offset = 0; offset < u8.length; offset += chunkSize) {
    const end = Math.min(u8.length, offset + chunkSize)
    for (let i = offset; i < end; i++) {
      result[i] = u8[i]
    }
    await new Promise(resolve => setTimeout(resolve, 0))
  }
  return result
}

// 保存图片
export async function saveImage(group, filename, data) {
  const payloadData = await uint8ArrayToNumberArrayChunked(data)
  return await invoke('il_save_image', {
    payload: { group, filename, data: payloadData }
  })
}

// 获取图片列表
export async function getImageList(group, offset = 0, limit = 20) {
  return await invoke('il_get_image_list', {
    payload: { group, offset, limit }
  })
}

// 获取图片总数
export async function getImageCount(group) {
  return await invoke('il_get_image_count', {
    payload: { group }
  })
}

// 删除图片
export async function deleteImage(group, filename) {
  return await invoke('il_delete_image', {
    payload: { group, filename }
  })
}

// 重命名图片
export async function renameImage(group, oldFilename, newFilename) {
  return await invoke('il_rename_image', {
    payload: { group, old_filename: oldFilename, new_filename: newFilename }
  })
}

// 获取图库分组
export async function getImageGroups() {
  return await invoke('il_get_groups')
}

// 新增图库分组
export async function addImageGroup(name, icon = 'ti ti-photo', color = '#2563eb') {
  return await invoke('il_add_group', {
    payload: { name, icon, color }
  })
}

// 更新图库分组
export async function updateImageGroup(oldName, newName, icon = 'ti ti-photo', color = '#2563eb') {
  return await invoke('il_update_group', {
    payload: { old_name: oldName, new_name: newName, icon, color }
  })
}

// 移动图片到分组
export async function moveImageToGroup(sourceGroup, filename, targetGroup) {
  return await invoke('il_move_image_to_group', {
    payload: { source_group: sourceGroup, filename, target_group: targetGroup }
  })
}

// 删除图库分组
export async function deleteImageGroup(name, moveImagesToDefault = false) {
  return await invoke('il_delete_group', {
    payload: { name, move_images_to_default: moveImagesToDefault }
  })
}

// 将本地路径转换为URL
export function getImageUrl(path) {
  return convertFileSrc(path)
}
