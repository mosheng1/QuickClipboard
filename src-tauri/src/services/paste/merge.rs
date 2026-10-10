use std::io::Cursor;

use base64::{engine::general_purpose, Engine as _};
use clipboard_rs::ClipboardContext;
use image::ImageFormat;

use crate::services::database::ClipboardItem;
use crate::services::paste::clipboard_content::{
    parse_files_content_existing, set_clipboard_files, set_clipboard_rich_text, set_clipboard_text,
    FilesData,
};
use crate::services::paste::keyboard::simulate_paste;
use crate::utils::cf_html::normalize_clipboard_html;

const MERGE_TEXT_SEPARATOR: &str = "\n\n";

enum MergePayload {
    Files { paths: Vec<String> },
    Text { text: String },
    RichText { text: String, html: String },
}

enum MergeSource {
    File { paths: Vec<String> },
    Text { text: String, html: Option<String> },
    Image { text: String, html: String },
}

pub fn copy_merged_items(items: &[ClipboardItem]) -> Result<(), String> {
    let payload = build_merge_payload(items)?;
    let _monitor_guard = crate::services::clipboard::pause_clipboard_monitor_for(500);
    apply_merge_payload(&payload, true)?;
    Ok(())
}

pub fn paste_merged_items(items: &[ClipboardItem], app: &tauri::AppHandle) -> Result<(), String> {
    let payload = build_merge_payload(items)?;
    let _monitor_guard = crate::services::clipboard::pause_clipboard_monitor_for(1000);
    apply_merge_payload(&payload, true)?;

    crate::services::mark_paste_operation();

    if !crate::get_window_state().is_pinned {
        if let Some(window) = crate::get_main_window(app) {
            crate::hide_main_window(&window);
        }
    }

    std::thread::sleep(std::time::Duration::from_millis(50));
    simulate_paste()?;
    std::thread::sleep(std::time::Duration::from_millis(100));
    crate::AppSounds::play_paste_on_success();

    Ok(())
}

fn apply_merge_payload(payload: &MergePayload, skip_record: bool) -> Result<(), String> {
    let ctx = ClipboardContext::new().map_err(|e| format!("创建剪贴板上下文失败: {}", e))?;

    match payload {
        MergePayload::Files { paths } => {
            if skip_record {
                crate::services::clipboard::set_last_hash_files(&build_files_hash_payload(paths)?);
            }
            set_clipboard_files(&ctx, paths.clone())
        }
        MergePayload::Text { text } => {
            if skip_record {
                crate::services::clipboard::set_last_hash_text(text);
            }
            set_clipboard_text(&ctx, text)
        }
        MergePayload::RichText { text, html } => {
            if skip_record {
                crate::services::clipboard::set_last_hash_text(text);
            }
            set_clipboard_rich_text(&ctx, text, html)
        }
    }
}

fn build_merge_payload(items: &[ClipboardItem]) -> Result<MergePayload, String> {
    if items.is_empty() {
        return Err("至少需要选择一项内容".to_string());
    }

    let sources = items
        .iter()
        .map(normalize_item_to_source)
        .collect::<Result<Vec<_>, _>>()?;

    determine_merge_payload(sources)
}

fn determine_merge_payload(sources: Vec<MergeSource>) -> Result<MergePayload, String> {
    let has_file = sources
        .iter()
        .any(|source| matches!(source, MergeSource::File { .. }));
    let has_non_file = sources
        .iter()
        .any(|source| !matches!(source, MergeSource::File { .. }));

    if has_file && has_non_file {
        return Err("文件类型不能与其他类型混合合并".to_string());
    }

    if has_file {
        let mut paths = Vec::new();
        for source in sources {
            if let MergeSource::File {
                paths: source_paths,
            } = source
            {
                paths.extend(source_paths);
            }
        }

        if paths.is_empty() {
            return Err("没有可用于合并的文件".to_string());
        }

        return Ok(MergePayload::Files { paths });
    }

    let requires_rich_text = sources.iter().any(|source| {
        matches!(source, MergeSource::Image { .. })
            || matches!(source, MergeSource::Text { html: Some(_), .. })
    });

    if !requires_rich_text {
        let text = sources
            .into_iter()
            .filter_map(|source| match source {
                MergeSource::Text { text, .. } => Some(text),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(MERGE_TEXT_SEPARATOR);
        return Ok(MergePayload::Text { text });
    }

    let mut plain_segments = Vec::new();
    let mut html_segments = Vec::new();

    for source in sources {
        match source {
            MergeSource::Text { text, html } => {
                plain_segments.push(text.clone());
                html_segments.push(wrap_merge_html_block(
                    &html.unwrap_or_else(|| plain_text_to_html(&text)),
                ));
            }
            MergeSource::Image { text, html } => {
                plain_segments.push(text);
                html_segments.push(wrap_merge_html_block(&html));
            }
            MergeSource::File { .. } => {
                return Err("文件类型不能与其他类型混合合并".to_string());
            }
        }
    }

    Ok(MergePayload::RichText {
        text: plain_segments.join(MERGE_TEXT_SEPARATOR),
        html: html_segments.join(""),
    })
}

fn normalize_item_to_source(item: &ClipboardItem) -> Result<MergeSource, String> {
    let primary_type = item
        .content_type
        .split(',')
        .next()
        .unwrap_or(item.content_type.as_str());

    match primary_type {
        "file" => Ok(MergeSource::File {
            paths: parse_files_content_existing(&item.content)?,
        }),
        "image" => {
            let image_html = build_image_html_for_item(item)?;
            let fallback_text = build_image_fallback_text(item);
            Ok(MergeSource::Image {
                text: fallback_text,
                html: image_html,
            })
        }
        "text" | "link" | "rich_text" => Ok(MergeSource::Text {
            text: item.content.clone(),
            html: item
                .html_content
                .as_ref()
                .map(|html| normalize_clipboard_html(html)),
        }),
        other => Err(format!("不支持合并的内容类型: {}", other)),
    }
}

fn build_image_html_for_item(item: &ClipboardItem) -> Result<String, String> {
    let content = normalize_image_content(item)?;
    let image_paths = parse_files_content_existing(&content)?;

    let mut images_html = String::new();
    for path in image_paths {
        let data_url = image_path_to_data_url_fast(&path)?;
        let file_name = std::path::Path::new(&path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("image");

        images_html.push_str(
            format!(
                "<img src=\"{}\" alt=\"{}\" style=\"max-width:100%;height:auto;display:block;\" />",
                data_url,
                escape_html(file_name)
            )
            .as_str(),
        );
    }

    if images_html.is_empty() {
        return Err("没有可用于合并的图片".to_string());
    }

    Ok(images_html)
}

fn normalize_image_content(item: &ClipboardItem) -> Result<String, String> {
    if item.content.starts_with("files:") {
        return Ok(item.content.clone());
    }

    let image_id = item
        .image_id
        .as_deref()
        .or_else(|| item.content.strip_prefix("image:"))
        .ok_or_else(|| format!("无法获取图片数据: {}", item.id))?;

    let image_path = crate::services::get_data_directory()?
        .join("clipboard_images")
        .join(format!("{}.png", image_id));

    if !image_path.exists() {
        return Err(format!("图片文件不存在: {}", image_path.display()));
    }

    let file_data = serde_json::json!({
        "files": [{
            "path": image_path.to_string_lossy(),
            "name": format!("{}.png", image_id),
            "size": std::fs::metadata(&image_path).map(|meta| meta.len()).unwrap_or(0),
            "is_directory": false,
            "file_type": "PNG"
        }],
        "operation": "copy"
    });

    Ok(format!("files:{}", file_data))
}

fn image_path_to_data_url(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("读取图片失败 [{}]: {}", path, e))?;
    let image = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("识别图片格式失败 [{}]: {}", path, e))?
        .decode()
        .map_err(|e| format!("解码图片失败 [{}]: {}", path, e))?;

    let mut png_data = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png_data), ImageFormat::Png)
        .map_err(|e| format!("编码图片失败 [{}]: {}", path, e))?;

    Ok(format!(
        "data:image/png;base64,{}",
        general_purpose::STANDARD.encode(png_data)
    ))
}

fn image_path_to_data_url_fast(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("读取图片失败 [{}]: {}", path, e))?;
    let format =
        image::guess_format(&bytes).map_err(|e| format!("识别图片格式失败 [{}]: {}", path, e))?;
    let mime =
        image_format_to_mime(format).ok_or_else(|| format!("暂不支持的图片格式 [{}]", path))?;

    Ok(format!(
        "data:{};base64,{}",
        mime,
        general_purpose::STANDARD.encode(bytes)
    ))
}

fn image_format_to_mime(format: image::ImageFormat) -> Option<&'static str> {
    match format {
        image::ImageFormat::Png => Some("image/png"),
        image::ImageFormat::Jpeg => Some("image/jpeg"),
        image::ImageFormat::Gif => Some("image/gif"),
        image::ImageFormat::WebP => Some("image/webp"),
        _ => None,
    }
}

fn build_image_fallback_text(item: &ClipboardItem) -> String {
    let file_name = item
        .content
        .strip_prefix("files:")
        .and_then(|json| serde_json::from_str::<FilesData>(json).ok())
        .and_then(|data| data.files.into_iter().next())
        .map(|file| {
            if file.name.trim().is_empty() {
                "[图片]".to_string()
            } else {
                format!("[图片: {}]", file.name)
            }
        });

    file_name.unwrap_or_else(|| "[图片]".to_string())
}

fn build_files_hash_payload(paths: &[String]) -> Result<String, String> {
    let files = paths
        .iter()
        .map(|path| {
            let name = std::path::Path::new(path)
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_string();
            serde_json::json!({
                "path": path,
                "name": name,
            })
        })
        .collect::<Vec<_>>();

    let payload = serde_json::json!({
        "files": files,
        "operation": "copy"
    });

    Ok(format!("files:{}", payload))
}

fn wrap_merge_html_block(fragment: &str) -> String {
    format!("<div style=\"margin:0 0 12px 0;\">{}</div>", fragment)
}

fn plain_text_to_html(text: &str) -> String {
    let mut html = escape_html(text);
    html = html.replace("\r\n", "\n");
    html = html.replace('\r', "\n");
    html.replace('\n', "<br />")
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_mixed_file_and_text_merge() {
        let result = determine_merge_payload(vec![
            MergeSource::File {
                paths: vec!["C:\\demo.txt".to_string()],
            },
            MergeSource::Text {
                text: "hello".to_string(),
                html: None,
            },
        ]);

        assert!(
            matches!(result, Err(message) if message.contains("文件类型不能与其他类型混合合并"))
        );
    }

    #[test]
    fn merges_plain_text_with_blank_line_separator() {
        let result = determine_merge_payload(vec![
            MergeSource::Text {
                text: "第一段".to_string(),
                html: None,
            },
            MergeSource::Text {
                text: "第二段".to_string(),
                html: None,
            },
        ])
        .expect("纯文本合并应该成功");

        match result {
            MergePayload::Text { text } => {
                assert_eq!(text, "第一段\n\n第二段");
            }
            _ => panic!("期望得到纯文本合并结果"),
        }
    }

    #[test]
    fn rich_merge_wraps_each_segment_as_block() {
        let result = determine_merge_payload(vec![
            MergeSource::Text {
                text: "纯文本".to_string(),
                html: None,
            },
            MergeSource::Text {
                text: "富文本".to_string(),
                html: Some("<strong>富文本</strong>".to_string()),
            },
            MergeSource::Image {
                text: "[图片]".to_string(),
                html: "<img src=\"data:image/png;base64,abc\" />".to_string(),
            },
        ])
        .expect("混合富文本合并应该成功");

        match result {
            MergePayload::RichText { text, html } => {
                assert_eq!(text, "纯文本\n\n富文本\n\n[图片]");
                assert!(html.contains("<div style=\"margin:0 0 12px 0;\">纯文本</div>"));
                assert!(html.contains("<strong>富文本</strong>"));
                assert!(html.contains("data:image/png;base64,abc"));
            }
            _ => panic!("期望得到富文本合并结果"),
        }
    }
}
