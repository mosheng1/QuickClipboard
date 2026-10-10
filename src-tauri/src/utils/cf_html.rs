pub fn generate_cf_html(html: &str) -> String {
    let html_content = if !html.contains("<html") {
        format!(
            "<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n</head>\n<body>\n<!--StartFragment-->{}\n<!--EndFragment-->\n</body>\n</html>",
            html
        )
    } else if !html.contains("<!--StartFragment-->") {
        html.replace("<body>", "<body>\n<!--StartFragment-->")
            .replace("</body>", "<!--EndFragment-->\n</body>")
    } else {
        html.to_string()
    };

    let header = "Version:0.9\r\nStartHTML:0000000000\r\nEndHTML:0000000000\r\nStartFragment:0000000000\r\nEndFragment:0000000000\r\n";
    let start_html = header.len();
    let end_html = start_html + html_content.len();

    let start_fragment = start_html + html_content.find("<!--StartFragment-->").unwrap_or(0);
    let end_fragment = start_html
        + html_content
            .find("<!--EndFragment-->")
            .unwrap_or(html_content.len());

    format!(
        "Version:0.9\r\nStartHTML:{:010}\r\nEndHTML:{:010}\r\nStartFragment:{:010}\r\nEndFragment:{:010}\r\n{}",
        start_html, end_html, start_fragment, end_fragment, html_content
    )
}

pub fn normalize_clipboard_html(input: &str) -> String {
    let s = input;

    if s.contains("StartFragment") || s.contains("StartHTML") {
        if let Some(fragment) = extract_cf_html_by_markers(s) {
            return fragment;
        }
        if let Some(fragment) = extract_cf_html_by_offsets(s) {
            return fragment;
        }
    }

    s.to_string()
}

fn extract_cf_html_by_markers(s: &str) -> Option<String> {
    let start_marker = "<!--StartFragment-->";
    let end_marker = "<!--EndFragment-->";

    let start = s.find(start_marker)? + start_marker.len();
    let end = s.find(end_marker)?;
    if end <= start {
        return None;
    }

    Some(s[start..end].to_string())
}

fn extract_cf_html_by_offsets(s: &str) -> Option<String> {
    fn parse_offset(s: &str, key: &str) -> Option<usize> {
        let idx = s.find(key)?;
        let rest = &s[idx + key.len()..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            return None;
        }
        digits.parse::<usize>().ok()
    }

    let bytes = s.as_bytes();
    let len = bytes.len();

    let start_fragment =
        parse_offset(s, "StartFragment:").or_else(|| parse_offset(s, "StartHTML:"));
    let end_fragment = parse_offset(s, "EndFragment:").or_else(|| parse_offset(s, "EndHTML:"));

    let (start, end) = match (start_fragment, end_fragment) {
        (Some(a), Some(b)) if a < b && b <= len => (a, b),
        _ => return None,
    };

    std::str::from_utf8(&bytes[start..end])
        .ok()
        .map(|t| t.to_string())
}
