use serde::{Deserialize, Serialize};

// 剪贴板项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favorite_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_device_id: Option<String>,
    pub is_remote: bool,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html_content: Option<String>,
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_id: Option<String>,
    pub item_order: i64,
    pub is_pinned: bool,
    pub paste_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_app: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_icon_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub char_count: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

// 剪贴板原始格式数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardDataItem {
    pub id: i64,
    pub target_kind: String,
    pub target_id: String,
    pub format_name: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub raw_data: Vec<u8>,
    pub is_primary: bool,
    pub format_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

// 剪贴板原始格式写入项
#[derive(Debug, Clone)]
pub struct ClipboardDataSeed {
    pub format_name: String,
    pub raw_data: Vec<u8>,
    pub is_primary: bool,
    pub format_order: i64,
}

// 可粘贴选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasteOption {
    pub id: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_format_name: Option<String>,
    pub is_primary: bool,
}

// 收藏项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavoriteItem {
    pub id: String,
    pub title: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html_content: Option<String>,
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_id: Option<String>,
    pub group_name: String,
    pub item_order: i64,
    pub paste_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub char_count: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

// 分组信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupInfo {
    pub name: String,
    pub icon: String,
    pub color: String,
    pub order: i32,
    pub item_count: i32,
}

// 分页查询结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResult<T> {
    // 总记录数
    pub total_count: i64,
    // 当前页数据
    pub items: Vec<T>,
    // 偏移量
    pub offset: i64,
    // 每页数量
    pub limit: i64,
    // 是否还有更多数据
    pub has_more: bool,
}

impl<T> PaginatedResult<T> {
    pub fn new(total_count: i64, items: Vec<T>, offset: i64, limit: i64) -> Self {
        let items_len = items.len() as i64;
        let has_more = offset + items_len < total_count;
        Self {
            total_count,
            items,
            offset,
            limit,
            has_more,
        }
    }
}

// 查询参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryParams {
    // 偏移量
    pub offset: i64,
    // 每页数量
    pub limit: i64,
    // 搜索关键词（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
    // 内容类型过滤（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
}

impl Default for QueryParams {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: 50,
            search: None,
            content_type: None,
        }
    }
}

// 收藏查询参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavoritesQueryParams {
    // 偏移量
    pub offset: i64,
    // 每页数量
    pub limit: i64,
    // 分组名称（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_name: Option<String>,
    // 搜索关键词（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
    // 内容类型过滤（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
}

impl Default for FavoritesQueryParams {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: 50,
            group_name: None,
            search: None,
            content_type: None,
        }
    }
}
