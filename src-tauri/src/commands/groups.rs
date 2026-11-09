use crate::services::database::{
    add_group as db_add_group, delete_group as db_delete_group, get_all_groups,
    reorder_groups as db_reorder_groups, update_group as db_update_group, GroupInfo,
};

// 获取所有分组
#[tauri::command]
pub fn get_groups() -> Result<Vec<GroupInfo>, String> {
    get_all_groups()
}

// 添加分组
#[tauri::command]
pub fn add_group(name: String, icon: String, color: String) -> Result<GroupInfo, String> {
    let result = db_add_group(name, icon, color);
    if result.is_ok() {
        notify_lan_change("groups");
    }
    result
}

// 更新分组
#[tauri::command]
pub fn update_group(
    old_name: String,
    new_name: String,
    new_icon: String,
    new_color: String,
) -> Result<GroupInfo, String> {
    let result = db_update_group(old_name, new_name, new_icon, new_color);
    if result.is_ok() {
        notify_lan_change("groups");
    }
    result
}

// 删除分组
#[tauri::command]
pub fn delete_group(name: String) -> Result<(), String> {
    let result = db_delete_group(name);
    if result.is_ok() {
        notify_lan_change("groups");
    }
    result
}

// 更新分组排序
#[tauri::command]
pub fn reorder_groups(group_orders: Vec<(String, i32)>) -> Result<(), String> {
    let result = db_reorder_groups(group_orders);
    if result.is_ok() {
        notify_lan_change("groups");
    }
    result
}

fn notify_lan_change(reason: &'static str) {
    if let Some(app) = crate::services::clipboard::get_app_handle() {
        crate::services::sync_transfer::lan_notify_local_change(app, reason);
    }
}
