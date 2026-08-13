use crate::services::get_data_directory;
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

const IMAGE_LIBRARY_DIR: &str = "image_library";
const GROUPS_META_FILE: &str = "groups.json";
const DEFAULT_GROUP_NAME: &str = "默认";
const DEFAULT_GROUP_ICON: &str = "ti ti-photo";
const DEFAULT_GROUP_COLOR: &str = "#2563eb";
const LEGACY_IMAGES_SUBDIR: &str = "images";
const LEGACY_GIFS_SUBDIR: &str = "gifs";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInfo {
    pub id: String,
    pub filename: String,
    pub path: String,
    pub size: u64,
    pub created_at: u64,
    pub category: String,
    pub group: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageListResult {
    pub total: usize,
    pub items: Vec<ImageInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGroupInfo {
    pub name: String,
    pub icon: String,
    pub color: String,
    pub order: i32,
    pub item_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GroupMeta {
    name: String,
    icon: String,
    color: String,
    order: i32,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct GroupsMetadata {
    #[serde(default)]
    groups: Vec<GroupMeta>,
}

// 获取图片库目录路径
pub fn get_image_library_dir() -> Result<PathBuf, String> {
    let data_dir = get_data_directory()?;
    Ok(data_dir.join(IMAGE_LIBRARY_DIR))
}

fn get_groups_meta_path() -> Result<PathBuf, String> {
    Ok(get_image_library_dir()?.join(GROUPS_META_FILE))
}

fn ensure_default_group_dir(root: &Path) -> Result<PathBuf, String> {
    let default_dir = root.join(DEFAULT_GROUP_NAME);
    if !default_dir.is_dir() {
        fs::create_dir_all(&default_dir).map_err(|e| format!("创建默认图库分组失败: {}", e))?;
    }
    Ok(default_dir)
}

// 兼容旧调用：现在返回默认分组目录，不再代表“图片”分类。
pub fn get_images_dir() -> Result<PathBuf, String> {
    ensure_initialized()?;
    Ok(get_image_library_dir()?.join(DEFAULT_GROUP_NAME))
}

// 兼容旧调用：GIF 不再单独分流，返回图库根目录。
pub fn get_gifs_dir() -> Result<PathBuf, String> {
    ensure_initialized()?;
    get_image_library_dir()
}

// 初始化图片库目录结构
pub fn init_image_library() -> Result<(), String> {
    ensure_initialized()
}

fn ensure_initialized() -> Result<(), String> {
    let root = get_image_library_dir()?;
    if !root.exists() {
        fs::create_dir_all(&root).map_err(|e| format!("创建图库目录失败: {}", e))?;
    }

    let meta_path = get_groups_meta_path()?;
    if !meta_path.exists() {
        migrate_legacy_category_dirs(&root)?;
    }

    ensure_default_group_dir(&root)?;

    sync_group_metadata_internal().map(|_| ())
}

fn migrate_legacy_category_dirs(root: &Path) -> Result<(), String> {
    let default_dir = root.join(DEFAULT_GROUP_NAME);
    let mut migrated = false;

    for legacy_name in [LEGACY_IMAGES_SUBDIR, LEGACY_GIFS_SUBDIR] {
        let legacy_dir = root.join(legacy_name);
        if !legacy_dir.is_dir() {
            continue;
        }

        fs::create_dir_all(&default_dir).map_err(|e| format!("创建默认图库分组失败: {}", e))?;

        let entries =
            fs::read_dir(&legacy_dir).map_err(|e| format!("读取旧图库目录失败: {}", e))?;
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let filename = entry.file_name().to_string_lossy().to_string();
            let target = unique_file_path(&default_dir, &filename);
            fs::rename(&path, &target).map_err(|e| format!("迁移旧图库文件失败: {}", e))?;
            migrated = true;
        }

        let _ = fs::remove_dir(&legacy_dir);
    }

    if migrated {
        save_group_metadata(vec![GroupMeta {
            name: DEFAULT_GROUP_NAME.to_string(),
            icon: DEFAULT_GROUP_ICON.to_string(),
            color: DEFAULT_GROUP_COLOR.to_string(),
            order: 0,
        }])?;
    }

    Ok(())
}

fn scan_group_names(root: &Path) -> Result<Vec<String>, String> {
    if !root.exists() {
        return Ok(vec![]);
    }

    let mut names = Vec::new();
    let entries = fs::read_dir(root).map_err(|e| format!("读取图库分组失败: {}", e))?;

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().trim().to_string();
        if !name.is_empty() {
            names.push(name);
        }
    }

    Ok(names)
}

fn load_group_metadata() -> Result<GroupsMetadata, String> {
    let meta_path = get_groups_meta_path()?;
    if !meta_path.exists() {
        return Ok(GroupsMetadata::default());
    }

    let content =
        fs::read_to_string(&meta_path).map_err(|e| format!("读取图库分组配置失败: {}", e))?;
    Ok(serde_json::from_str(&content).unwrap_or_default())
}

fn save_group_metadata(groups: Vec<GroupMeta>) -> Result<(), String> {
    let meta_path = get_groups_meta_path()?;
    let metadata = GroupsMetadata { groups };
    let content = serde_json::to_string_pretty(&metadata)
        .map_err(|e| format!("序列化图库分组配置失败: {}", e))?;
    fs::write(&meta_path, content).map_err(|e| format!("保存图库分组配置失败: {}", e))
}

fn sync_group_metadata_internal() -> Result<Vec<ImageGroupInfo>, String> {
    let root = get_image_library_dir()?;
    let group_names = scan_group_names(&root)?;
    let metadata = load_group_metadata()?;
    let mut meta_by_name: HashMap<String, GroupMeta> = metadata
        .groups
        .into_iter()
        .map(|meta| (meta.name.clone(), meta))
        .collect();
    let mut max_order = meta_by_name
        .values()
        .map(|meta| meta.order)
        .max()
        .unwrap_or(-1);

    let mut groups: Vec<ImageGroupInfo> = group_names
        .into_iter()
        .map(|name| {
            let meta = meta_by_name.remove(&name);
            let order = meta.as_ref().map(|m| m.order).unwrap_or_else(|| {
                max_order += 1;
                max_order
            });
            let icon = meta
                .as_ref()
                .map(|m| m.icon.clone())
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_GROUP_ICON.to_string());
            let color = meta
                .as_ref()
                .map(|m| m.color.clone())
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_GROUP_COLOR.to_string());
            let item_count = count_images_in_dir(&root.join(&name));

            ImageGroupInfo {
                name,
                icon,
                color,
                order,
                item_count,
            }
        })
        .collect();

    groups.sort_by(
        |a, b| match (a.name == DEFAULT_GROUP_NAME, b.name == DEFAULT_GROUP_NAME) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => a.order.cmp(&b.order).then_with(|| a.name.cmp(&b.name)),
        },
    );

    let normalized_groups: Vec<ImageGroupInfo> = groups
        .into_iter()
        .enumerate()
        .map(|(index, mut group)| {
            group.order = index as i32;
            group
        })
        .collect();

    save_group_metadata(
        normalized_groups
            .iter()
            .map(|group| GroupMeta {
                name: group.name.clone(),
                icon: group.icon.clone(),
                color: group.color.clone(),
                order: group.order,
            })
            .collect(),
    )?;

    Ok(normalized_groups)
}

pub fn list_groups() -> Result<Vec<ImageGroupInfo>, String> {
    ensure_initialized()?;
    sync_group_metadata_internal()
}

pub fn add_group(name: &str, icon: &str, color: &str) -> Result<ImageGroupInfo, String> {
    ensure_initialized()?;

    let name = validate_group_name(name)?;
    let root = get_image_library_dir()?;
    let dir = root.join(&name);
    if dir.exists() {
        return Err("图库分组已存在".to_string());
    }

    fs::create_dir_all(&dir).map_err(|e| format!("创建图库分组失败: {}", e))?;

    let mut groups = list_groups()?;
    let order = groups
        .iter()
        .filter(|group| group.name != name)
        .map(|group| group.order)
        .max()
        .unwrap_or(-1)
        + 1;
    for group in &mut groups {
        if group.name == name {
            group.icon = clean_icon(icon);
            group.color = clean_color(color);
            group.order = order;
            group.item_count = 0;
        }
    }

    save_group_metadata(
        groups
            .iter()
            .map(|group| GroupMeta {
                name: group.name.clone(),
                icon: group.icon.clone(),
                color: group.color.clone(),
                order: group.order,
            })
            .collect(),
    )?;

    list_groups()?
        .into_iter()
        .find(|group| group.name == name)
        .ok_or_else(|| "创建图库分组后读取失败".to_string())
}

pub fn update_group(
    old_name: &str,
    new_name: &str,
    new_icon: &str,
    new_color: &str,
) -> Result<ImageGroupInfo, String> {
    ensure_initialized()?;

    let old_name = validate_group_name(old_name)?;
    let new_name = validate_group_name(new_name)?;
    let root = get_image_library_dir()?;
    let old_dir = root.join(&old_name);
    let new_dir = root.join(&new_name);

    if !old_dir.is_dir() {
        return Err("图库分组不存在".to_string());
    }
    if old_name != new_name && new_dir.exists() {
        return Err("目标图库分组已存在".to_string());
    }
    if old_name == DEFAULT_GROUP_NAME && new_name != DEFAULT_GROUP_NAME {
        return Err("默认图库分组不能重命名".to_string());
    }

    let groups_before = list_groups()?;
    if old_name != new_name {
        fs::rename(&old_dir, &new_dir).map_err(|e| format!("重命名图库分组失败: {}", e))?;
    }

    save_group_metadata(
        groups_before
            .into_iter()
            .map(|group| {
                if group.name == old_name {
                    GroupMeta {
                        name: new_name.clone(),
                        icon: clean_icon(new_icon),
                        color: clean_color(new_color),
                        order: group.order,
                    }
                } else {
                    GroupMeta {
                        name: group.name,
                        icon: group.icon,
                        color: group.color,
                        order: group.order,
                    }
                }
            })
            .collect(),
    )?;

    list_groups()?
        .into_iter()
        .find(|group| group.name == new_name)
        .ok_or_else(|| "更新图库分组后读取失败".to_string())
}

fn clean_icon(icon: &str) -> String {
    let icon = icon.trim();
    if icon.is_empty() {
        DEFAULT_GROUP_ICON.to_string()
    } else {
        icon.to_string()
    }
}

fn clean_color(color: &str) -> String {
    let color = color.trim();
    if color.is_empty() {
        DEFAULT_GROUP_COLOR.to_string()
    } else {
        color.to_string()
    }
}

fn validate_group_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("分组名称不能为空".to_string());
    }
    if name == "." || name == ".." || name.eq_ignore_ascii_case(GROUPS_META_FILE) {
        return Err("分组名称无效".to_string());
    }
    if name.ends_with('.') || name.ends_with(' ') {
        return Err("分组名称不能以空格或句点结尾".to_string());
    }
    if name.chars().any(is_invalid_path_char) {
        return Err("分组名称不能包含 \\ / : * ? \" < > |".to_string());
    }

    let reserved = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    if matches!(
        reserved.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    ) {
        return Err("分组名称是系统保留名称".to_string());
    }

    Ok(name.to_string())
}

fn validate_filename(filename: &str) -> Result<String, String> {
    let filename = filename.trim();
    if filename.is_empty() || filename == "." || filename == ".." {
        return Err("文件名无效".to_string());
    }
    if filename.ends_with('.') || filename.ends_with(' ') {
        return Err("文件名不能以空格或句点结尾".to_string());
    }
    if filename.chars().any(is_invalid_path_char) {
        return Err("文件名不能包含 \\ / : * ? \" < > |".to_string());
    }

    Ok(filename.to_string())
}

fn is_invalid_path_char(c: char) -> bool {
    matches!(
        c,
        '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0'
    )
}

fn get_existing_group_dir(group: &str) -> Result<(String, PathBuf), String> {
    ensure_initialized()?;
    let group = validate_group_name(group)?;
    let dir = get_image_library_dir()?.join(&group);
    if !dir.is_dir() {
        return Err("图库分组不存在".to_string());
    }
    Ok((group, dir))
}

fn is_supported_image_file(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    matches!(
        ext.as_str(),
        "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "bmp"
            | "avif"
            | "svg"
            | "ico"
            | "tiff"
            | "tif"
            | "heic"
            | "heif"
            | "jfif"
    )
}

fn count_images_in_dir(dir: &Path) -> usize {
    fs::read_dir(dir)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(|e| e.ok()))
        .filter(|entry| entry.path().is_file() && is_supported_image_file(&entry.path()))
        .count()
}

// 通过文件头魔数判断是否为 GIF
fn is_gif_by_magic(data: &[u8]) -> bool {
    if data.len() < 6 {
        return false;
    }
    &data[0..6] == b"GIF87a" || &data[0..6] == b"GIF89a"
}

// 通过文件头判断是否为 WebP
fn is_webp_by_magic(data: &[u8]) -> bool {
    if data.len() < 12 {
        return false;
    }
    &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP"
}

// 检测 WebP 是否为动态图
fn is_animated_webp(data: &[u8]) -> bool {
    if !is_webp_by_magic(data) || data.len() < 30 {
        return false;
    }

    if data.len() >= 16 && &data[12..16] == b"VP8X" {
        if data.len() >= 21 {
            let flags = data[20];
            return (flags & 0x02) != 0;
        }
    }
    false
}

// 将静态 WebP 转换为 JPG
fn convert_webp_to_jpg(data: &[u8]) -> Result<Vec<u8>, String> {
    use image::ImageReader;
    use std::io::Cursor;

    let reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| format!("读取 WebP 失败: {}", e))?;

    let img = reader
        .decode()
        .map_err(|e| format!("解码 WebP 失败: {}", e))?;

    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    img.write_to(&mut cursor, image::ImageFormat::Jpeg)
        .map_err(|e| format!("编码 JPG 失败: {}", e))?;

    Ok(buffer)
}

// 提取 GIF 第一帧为 PNG 数据
fn extract_gif_first_frame(data: &[u8]) -> Option<Vec<u8>> {
    use image::codecs::gif::GifDecoder;
    use image::AnimationDecoder;
    use std::io::Cursor;

    let decoder = GifDecoder::new(Cursor::new(data)).ok()?;
    let first_frame = decoder.into_frames().next()?.ok()?;

    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    first_frame
        .buffer()
        .write_to(&mut cursor, image::ImageFormat::Png)
        .ok()?;

    Some(buffer)
}

// 使用 OCR 识别图片文字
fn ocr_image_text(data: &[u8]) -> Option<String> {
    use qcocr::recognize_from_bytes;
    use std::sync::mpsc;
    use std::thread;

    let (tx, rx) = mpsc::channel();
    let data = data.to_vec();
    let _ = thread::Builder::new()
        .name("il_ocr".to_string())
        .spawn(move || {
            let res = recognize_from_bytes(&data, None).ok().map(|r| r.text);
            let _ = tx.send(res);
        });

    let text = match rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Some(text)) => text,
        _ => return None,
    };

    let text = text.trim();

    if text.is_empty() {
        return None;
    }

    let cleaned: String = text
        .chars()
        .filter(|c| c.is_alphanumeric())
        .take(50)
        .collect();

    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn extension_from_filename(filename: &str, fallback: &str) -> String {
    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or(fallback)
        .trim()
        .to_lowercase();

    if !ext.is_empty() && ext.len() <= 8 && ext.chars().all(|c| c.is_ascii_alphanumeric()) {
        ext
    } else {
        fallback.to_string()
    }
}

fn unique_file_path(dir: &Path, filename: &str) -> PathBuf {
    let filename = validate_filename(filename).unwrap_or_else(|_| "image.png".to_string());
    let original = dir.join(&filename);
    if !original.exists() {
        return original;
    }

    let path = Path::new(&filename);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("image");
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    for index in 1.. {
        let candidate = if ext.is_empty() {
            format!("{} ({})", stem, index)
        } else {
            format!("{} ({}).{}", stem, index, ext)
        };
        let candidate_path = dir.join(candidate);
        if !candidate_path.exists() {
            return candidate_path;
        }
    }

    original
}

fn move_to_recycle_bin(path: &Path, action: &str) -> Result<(), String> {
    trash::delete(path).map_err(|e| format!("{}失败: {}", action, e))
}

// 保存图片到指定图库分组
pub fn save_image(group: &str, filename: &str, data: &[u8]) -> Result<ImageInfo, String> {
    let (group, target_dir) = get_existing_group_dir(group)?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis();

    let (final_data, extension): (Vec<u8>, String) =
        if is_webp_by_magic(data) && !is_animated_webp(data) {
            let jpg_data = convert_webp_to_jpg(data)?;
            (jpg_data, "jpg".to_string())
        } else if is_gif_by_magic(data) {
            (data.to_vec(), "gif".to_string())
        } else if is_webp_by_magic(data) {
            (data.to_vec(), "webp".to_string())
        } else {
            (data.to_vec(), extension_from_filename(filename, "png"))
        };

    let ocr_text = if is_gif_by_magic(data) {
        extract_gif_first_frame(data).and_then(|frame| ocr_image_text(&frame))
    } else {
        ocr_image_text(&final_data)
    };

    let new_filename = match ocr_text {
        Some(text) => format!("{}_{}.{}", timestamp, text, extension),
        None => format!("{}.{}", timestamp, extension),
    };
    let file_path = unique_file_path(&target_dir, &new_filename);
    let final_filename = file_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(&new_filename)
        .to_string();

    fs::write(&file_path, &final_data).map_err(|e| format!("保存图片失败: {}", e))?;

    Ok(ImageInfo {
        id: final_filename.clone(),
        filename: final_filename,
        path: file_path.to_string_lossy().to_string(),
        size: final_data.len() as u64,
        created_at: timestamp as u64,
        category: group.clone(),
        group,
    })
}

// 获取图片列表
pub fn get_image_list(group: &str, offset: usize, limit: usize) -> Result<ImageListResult, String> {
    let (group, dir) = get_existing_group_dir(group)?;
    let mut items: Vec<ImageInfo> = Vec::new();

    let entries: Vec<_> = fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file() && is_supported_image_file(&e.path()))
        .collect();

    let total = entries.len();

    let mut sorted_entries = entries;
    sorted_entries.sort_by(|a, b| {
        let time_a = a.metadata().and_then(|m| m.modified()).ok();
        let time_b = b.metadata().and_then(|m| m.modified()).ok();
        time_b.cmp(&time_a)
    });

    for entry in sorted_entries.into_iter().skip(offset).take(limit) {
        let path = entry.path();
        let filename = entry.file_name().to_string_lossy().to_string();
        let metadata = entry.metadata().ok();

        let created_at = metadata
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let size = metadata.map(|m| m.len()).unwrap_or(0);

        items.push(ImageInfo {
            id: filename.clone(),
            filename,
            path: path.to_string_lossy().to_string(),
            size,
            created_at,
            category: group.clone(),
            group: group.clone(),
        });
    }

    Ok(ImageListResult { total, items })
}

// 获取图片总数
pub fn get_image_count(group: &str) -> Result<usize, String> {
    let (_, dir) = get_existing_group_dir(group)?;
    Ok(count_images_in_dir(&dir))
}

// 删除图片
pub fn delete_image(group: &str, filename: &str) -> Result<(), String> {
    let (_, dir) = get_existing_group_dir(group)?;
    let filename = validate_filename(filename)?;
    let file_path = dir.join(filename);

    if file_path.exists() {
        move_to_recycle_bin(&file_path, "删除图片到回收站")?;
    }

    Ok(())
}

// 重命名图片
pub fn rename_image(
    group: &str,
    old_filename: &str,
    new_filename: &str,
) -> Result<ImageInfo, String> {
    let (group, dir) = get_existing_group_dir(group)?;
    let old_filename = validate_filename(old_filename)?;
    let old_path = dir.join(&old_filename);
    if !old_path.exists() {
        return Err(format!("文件不存在: {}", old_filename));
    }

    let old_ext = Path::new(&old_filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png");

    let new_name_with_ext = if new_filename.contains('.') {
        validate_filename(new_filename)?
    } else {
        validate_filename(&format!("{}.{}", new_filename.trim(), old_ext))?
    };

    let new_path = dir.join(&new_name_with_ext);

    if new_path.exists() {
        return Err("目标文件名已存在".to_string());
    }

    fs::rename(&old_path, &new_path).map_err(|e| format!("重命名失败: {}", e))?;

    image_info_from_path(&group, &new_path)
}

pub fn move_image_to_group(
    source_group: &str,
    filename: &str,
    target_group: &str,
) -> Result<ImageInfo, String> {
    let (source_group, source_dir) = get_existing_group_dir(source_group)?;
    let (target_group, target_dir) = get_existing_group_dir(target_group)?;
    let filename = validate_filename(filename)?;

    if source_group == target_group {
        return image_info_from_path(&source_group, &source_dir.join(filename));
    }

    let source_path = source_dir.join(&filename);
    if !source_path.exists() {
        return Err(format!("文件不存在: {}", filename));
    }

    let target_path = unique_file_path(&target_dir, &filename);
    fs::rename(&source_path, &target_path).map_err(|e| format!("移动图片到分组失败: {}", e))?;

    image_info_from_path(&target_group, &target_path)
}

fn move_regular_files(source_dir: &Path, target_dir: &Path) -> Result<(), String> {
    let entries = fs::read_dir(source_dir).map_err(|e| format!("读取图库分组文件失败: {}", e))?;

    for entry in entries.filter_map(|e| e.ok()) {
        let source_path = entry.path();
        if !source_path.is_file() {
            continue;
        }

        let filename = entry.file_name().to_string_lossy().to_string();
        let target_path = unique_file_path(target_dir, &filename);
        fs::rename(&source_path, &target_path)
            .map_err(|e| format!("移动图库分组文件失败: {}", e))?;
    }

    Ok(())
}

pub fn delete_group(
    name: &str,
    move_images_to_default: bool,
) -> Result<Vec<ImageGroupInfo>, String> {
    ensure_initialized()?;

    let name = validate_group_name(name)?;
    if name == DEFAULT_GROUP_NAME {
        return Err("默认图库分组不能删除".to_string());
    }

    let root = get_image_library_dir()?;
    let group_dir = root.join(&name);
    if !group_dir.is_dir() {
        return Err("图库分组不存在".to_string());
    }

    if move_images_to_default {
        let default_dir = ensure_default_group_dir(&root)?;
        move_regular_files(&group_dir, &default_dir)?;
    }

    move_to_recycle_bin(&group_dir, "删除图库分组到回收站")?;

    sync_group_metadata_internal()
}

fn image_info_from_path(group: &str, path: &Path) -> Result<ImageInfo, String> {
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "文件名无效".to_string())?
        .to_string();
    let metadata = fs::metadata(path).map_err(|e| format!("读取图片信息失败: {}", e))?;
    let created_at = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    Ok(ImageInfo {
        id: filename.clone(),
        filename,
        path: path.to_string_lossy().to_string(),
        size: metadata.len(),
        created_at,
        category: group.to_string(),
        group: group.to_string(),
    })
}
