import { invoke } from '@tauri-apps/api/core'

// 获取所有分组
export async function getGroups() {
  return await invoke('get_groups')
}

// 添加分组
export async function addGroup(name, icon = 'ti ti-folder', color = '#dc2626') {
  return await invoke('add_group', { name, icon, color })
}

// 更新分组
export async function updateGroup(oldName, newName, newIcon, newColor) {
  return await invoke('update_group', {
    oldName,
    newName,
    newIcon,
    newColor
  })
}

// 删除分组
export async function deleteGroup(name) {
  return await invoke('delete_group', { name })
}

// 更新分组排序
export async function reorderGroups(groupOrders) {
  return await invoke('reorder_groups', { groupOrders })
}

// 移动收藏项到分组
export async function moveFavoriteToGroup(id, groupName) {
  return await invoke('move_quick_text_to_group', { id, groupName })
}

