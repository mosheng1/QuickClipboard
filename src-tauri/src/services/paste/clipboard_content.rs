//! 剪贴板内容设置的共用逻辑

use crate::utils::cf_html::generate_cf_html;
use clipboard_rs::{Clipboard, ClipboardContent, ClipboardContext};
use std::path::Path;

// 文件信息结构
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileInfo {
    pub path: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub is_directory: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_data: Option<String>,
    #[serde(default)]
    pub file_type: String,
    #[serde(default)]
    pub exists: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

// 文件剪贴板数据
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FilesData {
    pub files: Vec<FileInfo>,
    #[serde(default)]
    pub operation: String,
}

// 解析 files: 格式的内容，返回文件路径列表
pub fn parse_files_content(content: &str) -> Result<Vec<String>, String> {
    if !content.starts_with("files:") {
        return Err("无效的文件内容格式".to_string());
    }

    let json_str = &content[6..];
    let file_data: FilesData =
        serde_json::from_str(json_str).map_err(|e| format!("解析文件数据失败: {}", e))?;

    let paths: Vec<String> = file_data
        .files
        .iter()
        .map(|f| crate::services::resolve_stored_path(&f.path))
        .collect();

    Ok(paths)
}

// 解析 files: 格式的内容，只返回存在的文件路径
pub fn parse_files_content_existing(content: &str) -> Result<Vec<String>, String> {
    let paths = parse_files_content(content)?;
    let existing: Vec<String> = paths
        .into_iter()
        .filter(|p| Path::new(p).exists())
        .collect();

    if existing.is_empty() {
        return Err("所有文件都不存在".to_string());
    }

    Ok(existing)
}

// 设置剪贴板为文件列表
pub fn set_clipboard_files(ctx: &ClipboardContext, paths: Vec<String>) -> Result<(), String> {
    let _guard = crate::services::clipboard::pause_clipboard_monitor_for(500);
    ctx.set_files(paths)
        .map_err(|e| format!("设置文件到剪贴板失败: {}", e))
}

// 设置剪贴板为图片，同时保留文件路径格式
pub fn set_clipboard_image_file(path: &str) -> Result<(), String> {
    set_clipboard_image_file_inner(path, true)
}

// 设置新图片到系统剪贴板，需要让剪贴板监听器正常入库。
pub fn set_clipboard_image_file_recordable(path: &str) -> Result<(), String> {
    set_clipboard_image_file_inner(path, false)
}

fn set_clipboard_image_file_inner(path: &str, pause_monitor: bool) -> Result<(), String> {
    let resolved_path = crate::services::resolve_stored_path(path);
    if !Path::new(&resolved_path).exists() {
        return Err(format!("图片文件不存在: {}", resolved_path));
    }

    if pause_monitor {
        let _guard = crate::services::clipboard::pause_clipboard_monitor_for(500);
        set_clipboard_image_file_impl(&resolved_path)
    } else {
        set_clipboard_image_file_impl(&resolved_path)
    }
}

#[cfg(target_os = "windows")]
fn set_clipboard_image_file_impl(path: &str) -> Result<(), String> {
    use image::ImageFormat;
    use std::io::Cursor;
    use std::mem;
    use std::ptr;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE, GMEM_ZEROINIT,
    };
    use windows::Win32::UI::Shell::DROPFILES;

    const CF_DIB: u32 = 8;
    const CF_HDROP: u32 = 15;

    fn alloc_global_bytes(data: &[u8]) -> Result<HGLOBAL, String> {
        if data.is_empty() {
            return Err("剪贴板数据为空".to_string());
        }

        let handle = unsafe { GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, data.len()) }
            .map_err(|e| format!("分配剪贴板内存失败: {}", e))?;
        let ptr = unsafe { GlobalLock(handle) } as *mut u8;

        if ptr.is_null() {
            let _ = unsafe { GlobalFree(Some(handle)) };
            return Err("锁定剪贴板内存失败".to_string());
        }

        unsafe {
            ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len());
            let _ = GlobalUnlock(handle);
        }

        Ok(handle)
    }

    fn set_clipboard_handle(format: u32, handle: HGLOBAL, name: &str) -> Result<(), String> {
        match unsafe { SetClipboardData(format, Some(HANDLE(handle.0))) } {
            Ok(_) => Ok(()),
            Err(e) => {
                let _ = unsafe { GlobalFree(Some(handle)) };
                Err(format!("设置 {} 到剪贴板失败: {}", name, e))
            }
        }
    }

    fn build_hdrop_data(path: &str) -> Vec<u8> {
        let mut wide_path: Vec<u16> = path.encode_utf16().collect();
        wide_path.push(0);
        wide_path.push(0);

        let header_size = mem::size_of::<DROPFILES>();
        let mut data = vec![0u8; header_size + wide_path.len() * 2];

        let dropfiles = DROPFILES {
            pFiles: header_size as u32,
            pt: Default::default(),
            fNC: false.into(),
            fWide: true.into(),
        };

        unsafe {
            ptr::copy_nonoverlapping(
                &dropfiles as *const DROPFILES as *const u8,
                data.as_mut_ptr(),
                header_size,
            );
            ptr::copy_nonoverlapping(
                wide_path.as_ptr() as *const u8,
                data.as_mut_ptr().add(header_size),
                wide_path.len() * 2,
            );
        }

        data
    }

    struct ClipboardSession;

    impl ClipboardSession {
        fn open() -> Result<Self, String> {
            unsafe { OpenClipboard(None) }.map_err(|e| format!("打开剪贴板失败: {}", e))?;
            Ok(Self)
        }
    }

    impl Drop for ClipboardSession {
        fn drop(&mut self) {
            let _ = unsafe { CloseClipboard() };
        }
    }

    let image = image::open(path).map_err(|e| format!("读取图片失败: {}", e))?;

    let mut png_data = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png_data), ImageFormat::Png)
        .map_err(|e| format!("编码 PNG 失败: {}", e))?;

    let dib_data = build_dib_data(&image)?;
    let hdrop_data = build_hdrop_data(path);

    let png_format = {
        let mut name: Vec<u16> = "PNG".encode_utf16().collect();
        name.push(0);
        unsafe { RegisterClipboardFormatW(PCWSTR(name.as_ptr())) }
    };
    if png_format == 0 {
        return Err("注册 PNG 剪贴板格式失败".to_string());
    }

    let _clipboard = ClipboardSession::open()?;
    unsafe { EmptyClipboard() }.map_err(|e| format!("清空剪贴板失败: {}", e))?;

    let dib_handle = alloc_global_bytes(&dib_data)?;
    set_clipboard_handle(CF_DIB, dib_handle, "CF_DIB")?;

    let png_handle = alloc_global_bytes(&png_data)?;
    set_clipboard_handle(png_format, png_handle, "PNG")?;

    let hdrop_handle = alloc_global_bytes(&hdrop_data)?;
    set_clipboard_handle(CF_HDROP, hdrop_handle, "CF_HDROP")?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn build_dib_data(image: &image::DynamicImage) -> Result<Vec<u8>, String> {
    let rgba = image.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    if height > i32::MAX as u32 || width > i32::MAX as u32 {
        return Err("图片尺寸过大，无法生成 DIB 数据".to_string());
    }
    let pixel_data_size = width
        .checked_mul(height)
        .and_then(|value| value.checked_mul(4))
        .ok_or_else(|| "图片尺寸过大，无法生成 DIB 数据".to_string())?
        as usize;
    let mut data = Vec::with_capacity(40 + pixel_data_size);

    data.extend_from_slice(&40u32.to_le_bytes());
    data.extend_from_slice(&(width as i32).to_le_bytes());
    data.extend_from_slice((-(height as i32)).to_le_bytes().as_slice());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&32u16.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&(pixel_data_size as u32).to_le_bytes());
    data.extend_from_slice(&0i32.to_le_bytes());
    data.extend_from_slice(&0i32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());

    for pixel in rgba.pixels() {
        data.push(pixel[2]);
        data.push(pixel[1]);
        data.push(pixel[0]);
        data.push(pixel[3]);
    }

    Ok(data)
}

#[cfg(not(target_os = "windows"))]
fn set_clipboard_image_file_impl(path: &str) -> Result<(), String> {
    let ctx = ClipboardContext::new().map_err(|e| format!("创建剪贴板上下文失败: {}", e))?;
    ctx.set_files(vec![path.to_string()])
        .map_err(|e| format!("设置图片到剪贴板失败: {}", e))
}

// 设置剪贴板为纯文本
pub fn set_clipboard_text(ctx: &ClipboardContext, text: &str) -> Result<(), String> {
    let _guard = crate::services::clipboard::pause_clipboard_monitor_for(500);
    ctx.set_text(text.to_string())
        .map_err(|e| format!("设置文本到剪贴板失败: {}", e))
}

// 设置剪贴板为富文本（文本 + HTML）
pub fn set_clipboard_rich_text(
    ctx: &ClipboardContext,
    text: &str,
    html: &str,
) -> Result<(), String> {
    let _guard = crate::services::clipboard::pause_clipboard_monitor_for(500);
    let cf_html = generate_cf_html(html);
    ctx.set(vec![
        ClipboardContent::Text(text.to_string()),
        ClipboardContent::Html(cf_html),
    ])
    .map_err(|e| format!("设置剪贴板内容失败: {}", e))
}

// 设置剪贴板为任意内容组合
pub fn set_clipboard_contents(
    ctx: &ClipboardContext,
    contents: Vec<ClipboardContent>,
) -> Result<(), String> {
    let _guard = crate::services::clipboard::pause_clipboard_monitor_for(500);
    ctx.set(contents)
        .map_err(|e| format!("设置剪贴板内容失败: {}", e))
}

// 根据内容类型设置剪贴板（不触发粘贴，用于复制操作）
pub fn set_clipboard_from_item(
    content_type: &str,
    content: &str,
    html_content: &Option<String>,
    skip_record: bool,
) -> Result<(), String> {
    let ctx = ClipboardContext::new().map_err(|e| format!("创建剪贴板上下文失败: {}", e))?;

    let primary_type = content_type.split(',').next().unwrap_or(content_type);

    match primary_type {
        "image" | "file" => {
            let paths = parse_files_content(content)?;
            if paths.is_empty() {
                return Err("无法解析文件内容".to_string());
            }

            if skip_record {
                crate::services::clipboard::set_last_hash_files(content);
            }

            set_clipboard_files(&ctx, paths)
        }
        _ => {
            // 文本类型（text, rich_text, link 等）
            let text = if content.starts_with("files:") {
                content[6..].to_string()
            } else {
                content.to_string()
            };

            if skip_record {
                crate::services::clipboard::set_last_hash_text(&text);
            }

            if let Some(html) = html_content {
                set_clipboard_rich_text(&ctx, &text, html)
            } else {
                set_clipboard_text(&ctx, &text)
            }
        }
    }
}
