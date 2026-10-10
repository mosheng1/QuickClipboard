// HTML 处理工具函数

pub fn truncate_html(html: String, max_visible_len: usize) -> String {
    if html.is_empty() {
        return html;
    }

    if max_visible_len == 0 {
        return "...(内容过长已截断)".to_string();
    }

    let mut visible_count: usize = 0;
    let mut in_tag = false;

    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => {
                visible_count = visible_count.saturating_add(1);
                if visible_count > max_visible_len {
                    break;
                }
            }
            _ => {}
        }
    }

    if visible_count <= max_visible_len {
        return html;
    }

    let mut result = String::with_capacity(html.len().min(max_visible_len * 10));
    visible_count = 0;
    in_tag = false;
    let mut open_tags: Vec<String> = Vec::with_capacity(16);
    let mut current_tag = String::with_capacity(32);
    let mut is_closing_tag = false;
    let mut tag_started = false;

    for c in html.chars() {
        if c == '<' {
            in_tag = true;
            tag_started = false;
            is_closing_tag = false;
            current_tag.clear();
            result.push(c);
        } else if c == '>' {
            in_tag = false;
            result.push(c);

            if !current_tag.is_empty() {
                let tag_name = current_tag.to_lowercase();
                let is_self_closing = matches!(
                    tag_name.as_str(),
                    "br" | "hr"
                        | "img"
                        | "input"
                        | "meta"
                        | "link"
                        | "area"
                        | "base"
                        | "col"
                        | "embed"
                        | "source"
                        | "track"
                        | "wbr"
                );

                if !is_self_closing {
                    if is_closing_tag {
                        if let Some(pos) = open_tags.iter().rposition(|t| t == &tag_name) {
                            open_tags.remove(pos);
                        }
                    } else {
                        if open_tags.len() < 100 {
                            open_tags.push(tag_name);
                        }
                    }
                }
            }
        } else if in_tag {
            result.push(c);

            if c == '/' && !tag_started {
                is_closing_tag = true;
            } else if c.is_alphanumeric() && !tag_started {
                tag_started = true;
                if current_tag.len() < 50 {
                    current_tag.push(c);
                }
            } else if tag_started && (c.is_alphanumeric() || c == '-') {
                if current_tag.len() < 50 {
                    current_tag.push(c);
                }
            } else if tag_started {
                tag_started = false;
            }
        } else {
            visible_count = visible_count.saturating_add(1);
            if visible_count > max_visible_len {
                break;
            }
            result.push(c);
        }
    }

    for tag in open_tags.iter().rev().take(50) {
        result.push_str("</");
        result.push_str(tag);
        result.push('>');
    }
    result.push_str("...(内容过长已截断)");

    result
}
