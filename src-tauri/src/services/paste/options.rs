use regex::Regex;

use crate::services::database::{ClipboardDataItem, ClipboardItem, PasteOption};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasteAction {
    PlainText,
    Html,
    Rtf,
    AllFormats,
    ImageBundle,
    File,
}

impl PasteAction {
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "plain_text" => Some(Self::PlainText),
            "html" => Some(Self::Html),
            "rtf" => Some(Self::Rtf),
            "all_formats" => Some(Self::AllFormats),
            "image_bundle" => Some(Self::ImageBundle),
            "file" => Some(Self::File),
            _ => None,
        }
    }

    pub fn id(&self) -> &'static str {
        match self {
            Self::PlainText => "plain_text",
            Self::Html => "html",
            Self::Rtf => "rtf",
            Self::AllFormats => "all_formats",
            Self::ImageBundle => "image_bundle",
            Self::File => "file",
        }
    }

    pub fn kind(&self) -> &'static str {
        self.id()
    }
}

pub fn build_paste_options(
    item: &ClipboardItem,
    raw_formats: &[ClipboardDataItem],
) -> Vec<PasteOption> {
    if is_pure_image_item(item) {
        return vec![build_option(PasteAction::ImageBundle, None, true)];
    }

    let primary_type = primary_type(&item.content_type);
    if primary_type == "file" && !has_type(&item.content_type, "image") {
        return vec![build_option(PasteAction::File, None, true)];
    }

    let has_text = has_raw_format(raw_formats, "CF_UNICODETEXT")
        || has_raw_format(raw_formats, "CF_TEXT")
        || has_meaningful_plain_text(item);
    let has_html = has_raw_format(raw_formats, "HTML Format")
        || item
            .html_content
            .as_deref()
            .map(|html| !html.trim().is_empty())
            .unwrap_or(false);
    let has_rtf = has_raw_format(raw_formats, "Rich Text Format");
    let has_image = has_type(&item.content_type, "image");

    let mut semantic_options = Vec::new();

    if has_text {
        semantic_options.push(build_option(
            PasteAction::PlainText,
            None,
            is_primary_text(raw_formats, primary_type),
        ));
    }

    if has_html {
        semantic_options.push(build_option(
            PasteAction::Html,
            Some("HTML Format"),
            is_primary_raw(raw_formats, "HTML Format"),
        ));
    }

    if has_rtf {
        semantic_options.push(build_option(
            PasteAction::Rtf,
            Some("Rich Text Format"),
            is_primary_raw(raw_formats, "Rich Text Format"),
        ));
    }

    if has_image {
        semantic_options.push(build_option(
            PasteAction::ImageBundle,
            None,
            primary_type == "image",
        ));
    }

    if semantic_options.is_empty() {
        if !raw_formats.is_empty() {
            return vec![build_option(PasteAction::AllFormats, None, false)];
        }
        return Vec::new();
    }

    let mut options = Vec::new();
    if raw_formats.len() > 1 || semantic_options.len() > 1 {
        options.push(build_option(PasteAction::AllFormats, None, false));
    }
    options.extend(semantic_options);
    options
}

pub fn resolve_default_paste_action(
    item: &ClipboardItem,
    raw_formats: &[ClipboardDataItem],
) -> PasteAction {
    let primary_type = primary_type(&item.content_type);

    if primary_type == "image" || is_pure_image_item(item) {
        return PasteAction::ImageBundle;
    }

    if primary_type == "file" {
        return PasteAction::File;
    }

    if crate::services::get_settings().paste_with_format {
        if !raw_formats.is_empty() {
            return PasteAction::AllFormats;
        }
        if item
            .html_content
            .as_deref()
            .map(|html| !html.trim().is_empty())
            .unwrap_or(false)
        {
            return PasteAction::Html;
        }
    }

    PasteAction::PlainText
}

fn build_option(
    action: PasteAction,
    source_format_name: Option<&str>,
    is_primary: bool,
) -> PasteOption {
    PasteOption {
        id: action.id().to_string(),
        kind: action.kind().to_string(),
        source_format_name: source_format_name.map(str::to_string),
        is_primary,
    }
}

fn primary_type(content_type: &str) -> &str {
    content_type
        .split(',')
        .next()
        .unwrap_or(content_type)
        .trim()
}

fn has_type(content_type: &str, target: &str) -> bool {
    content_type.split(',').any(|item| item.trim() == target)
}

fn has_raw_format(raw_formats: &[ClipboardDataItem], format_name: &str) -> bool {
    raw_formats
        .iter()
        .any(|item| item.format_name == format_name)
}

fn is_primary_raw(raw_formats: &[ClipboardDataItem], format_name: &str) -> bool {
    raw_formats
        .iter()
        .any(|item| item.format_name == format_name && item.is_primary)
}

fn is_primary_text(raw_formats: &[ClipboardDataItem], primary_type: &str) -> bool {
    raw_formats.iter().any(|item| {
        item.is_primary && matches!(item.format_name.as_str(), "CF_UNICODETEXT" | "CF_TEXT")
    }) || primary_type == "text"
}

fn has_meaningful_plain_text(item: &ClipboardItem) -> bool {
    if item.content.starts_with("files:") {
        return false;
    }

    !item.content.trim().is_empty()
}

fn is_pure_image_item(item: &ClipboardItem) -> bool {
    if primary_type(&item.content_type) == "image" {
        return true;
    }

    if !has_type(&item.content_type, "image") {
        return false;
    }

    if item
        .image_id
        .as_deref()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
    {
        return false;
    }

    if !item.content.trim().is_empty() {
        return false;
    }

    is_image_only_html(item.html_content.as_deref())
}

fn is_image_only_html(html: Option<&str>) -> bool {
    let Some(html) = html else {
        return false;
    };

    if !html.contains("<img") {
        return false;
    }

    let tag_regex = Regex::new(r"<[^>]*>").unwrap();
    let entity_regex = Regex::new(r"&[a-zA-Z]+;").unwrap();

    let mut text = tag_regex.replace_all(html, " ").to_string();
    text = entity_regex.replace_all(&text, " ").to_string();
    text.trim().is_empty()
}
