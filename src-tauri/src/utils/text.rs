// 文本截断工具函数

const SEARCH_CONTEXT_BEFORE_CHARS: usize = 16;
const ELLIPSIS: &str = "...";

pub fn is_textual_content_type(content_type: &str) -> bool {
    content_type
        .split(',')
        .map(str::trim)
        .any(|kind| matches!(kind, "text" | "rich_text" | "link"))
}

pub fn truncate_string(s: String, max_len: usize) -> String {
    if s.is_empty() || s.len() <= max_len {
        return s;
    }

    let mut truncate_point = max_len.saturating_sub(50);
    while truncate_point > 0 && !s.is_char_boundary(truncate_point) {
        truncate_point -= 1;
    }

    if truncate_point == 0 {
        return "...(内容过长已截断)".to_string();
    }

    match s.get(..truncate_point) {
        Some(slice) => format!("{}...(内容过长已截断)", slice),
        None => "...(内容过长已截断)".to_string(),
    }
}

fn original_byte_index_at_lowercase_offset(text: &str, target_offset: usize) -> Option<usize> {
    if target_offset == 0 {
        return Some(0);
    }

    let mut lowercase_offset = 0;
    for (byte_index, ch) in text.char_indices() {
        if lowercase_offset == target_offset {
            return Some(byte_index);
        }

        lowercase_offset += ch.to_lowercase().map(char::len_utf8).sum::<usize>();
        if lowercase_offset > target_offset {
            return None;
        }
    }

    (lowercase_offset == target_offset).then_some(text.len())
}

fn find_keyword_range(text: &str, keyword: &str) -> Option<(usize, usize)> {
    if let Some(start) = text.find(keyword) {
        return Some((start, start + keyword.len()));
    }

    let lowercase_text = text.to_lowercase();
    let lowercase_keyword = keyword.to_lowercase();
    let lowercase_start = lowercase_text.find(&lowercase_keyword)?;
    let lowercase_end = lowercase_start + lowercase_keyword.len();
    let start = original_byte_index_at_lowercase_offset(text, lowercase_start)?;
    let end = original_byte_index_at_lowercase_offset(text, lowercase_end)?;
    Some((start, end))
}

fn byte_index_before_chars(text: &str, end: usize, count: usize) -> usize {
    if count == 0 {
        return end;
    }

    text[..end]
        .char_indices()
        .rev()
        .nth(count - 1)
        .map(|(index, _)| index)
        .unwrap_or(0)
}

// 截取关键词附近的搜索摘要，并让关键词靠近摘要开头。
pub fn truncate_around_keyword(s: String, keyword: &str, max_len: usize) -> String {
    if s.is_empty() || keyword.is_empty() || s.len() <= max_len {
        return if s.len() <= max_len {
            s
        } else {
            truncate_string(s, max_len)
        };
    }

    let (keyword_start, keyword_end) = match find_keyword_range(&s, keyword) {
        Some(range) => range,
        None => return truncate_string(s, max_len),
    };

    let mut start = byte_index_before_chars(&s, keyword_start, SEARCH_CONTEXT_BEFORE_CHARS);
    let prefix_len = if start > 0 { ELLIPSIS.len() } else { 0 };
    let mut slice_len = max_len.saturating_sub(prefix_len + ELLIPSIS.len());

    if keyword_end.saturating_sub(start) > slice_len {
        start = keyword_start;
        slice_len = max_len.saturating_sub(ELLIPSIS.len() * 2);
    }

    let mut end = start.saturating_add(slice_len).min(s.len());
    while end > start && !s.is_char_boundary(end) {
        end -= 1;
    }

    if end <= start || end < keyword_end {
        return truncate_string(s, max_len);
    }

    let slice = match s.get(start..end) {
        Some(slice) => slice,
        None => return truncate_string(s, max_len),
    };

    let mut result = String::with_capacity(max_len);

    if start > 0 {
        result.push_str(ELLIPSIS);
    }

    result.push_str(slice);

    if end < s.len() {
        result.push_str(ELLIPSIS);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_compound_text_content_types() {
        assert!(is_textual_content_type("text,link"));
        assert!(is_textual_content_type("rich_text,link,image"));
        assert!(is_textual_content_type(" link , image "));
        assert!(!is_textual_content_type("image,file"));
    }

    #[test]
    fn search_excerpt_keeps_keyword_near_the_start() {
        let content = format!("{}目标关键词{}", "前文".repeat(1000), "后文".repeat(1000));
        let excerpt = truncate_around_keyword(content, "目标关键词", 1600);
        let keyword_index = excerpt.find("目标关键词").expect("摘要应包含关键词");

        assert!(excerpt.len() <= 1600);
        assert!(excerpt.starts_with(ELLIPSIS));
        assert!(excerpt[..keyword_index].chars().count() <= SEARCH_CONTEXT_BEFORE_CHARS + 3);
    }

    #[test]
    fn search_excerpt_matches_ascii_case_insensitively() {
        let content = format!("{}Needle{}", "before ".repeat(500), " after".repeat(500));
        let excerpt = truncate_around_keyword(content, "needle", 800);

        assert!(excerpt.len() <= 800);
        assert!(excerpt.contains("Needle"));
    }

    #[test]
    fn search_excerpt_preserves_utf8_boundaries() {
        let content = format!("{}关键字{}", "甲".repeat(1000), "乙".repeat(1000));
        let excerpt = truncate_around_keyword(content, "关键字", 257);

        assert!(excerpt.len() <= 257);
        assert!(excerpt.contains("关键字"));
    }
}
