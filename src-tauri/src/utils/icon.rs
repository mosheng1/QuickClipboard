use file_icon_provider::get_file_icon;
use image::{ImageFormat, RgbaImage};
use sha2::{Digest, Sha256};
use std::io::Cursor;

// 获取文件图标并转换为 Base64 Data URL
pub fn get_file_icon_base64(path: &str) -> Option<String> {
    match get_file_icon(path, 32) {
        Ok(icon) => {
            if let Ok(png_data) = icon_to_png(&icon) {
                use base64::{engine::general_purpose, Engine as _};
                let base64_str = general_purpose::STANDARD.encode(&png_data);
                return Some(format!("data:image/png;base64,{}", base64_str));
            }
            None
        }
        Err(_) => None,
    }
}

// 将 Icon 转换为 PNG 格式
pub fn icon_to_png(icon: &file_icon_provider::Icon) -> Result<Vec<u8>, String> {
    let img =
        RgbaImage::from_raw(icon.width, icon.height, icon.pixels.clone()).ok_or("创建图像失败")?;

    let mut png_data = Vec::new();
    let mut cursor = Cursor::new(&mut png_data);
    img.write_to(&mut cursor, ImageFormat::Png)
        .map_err(|e| format!("PNG编码失败: {}", e))?;

    Ok(png_data)
}

// 计算图标哈希
fn calculate_icon_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let hash = format!("{:x}", hasher.finalize());
    hash[..16].to_string()
}

// 保存应用图标到 app_icons 目录
pub fn save_app_icon(exe_path: &str) -> Option<String> {
    let icon = match get_file_icon(exe_path, 32) {
        Ok(icon) => icon,
        Err(_) => return None,
    };

    let png_data = match icon_to_png(&icon) {
        Ok(data) => data,
        Err(_) => return None,
    };

    let hash = calculate_icon_hash(&png_data);

    let data_dir = match crate::services::get_data_directory() {
        Ok(dir) => dir,
        Err(_) => return None,
    };

    let icons_dir = data_dir.join("app_icons");
    if !icons_dir.exists() {
        if std::fs::create_dir_all(&icons_dir).is_err() {
            return None;
        }
    }

    let icon_path = icons_dir.join(format!("{}.png", hash));
    if !icon_path.exists() {
        if std::fs::write(&icon_path, &png_data).is_err() {
            return None;
        }
    }

    Some(hash)
}
