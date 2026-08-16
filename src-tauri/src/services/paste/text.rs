use super::clipboard_content::set_clipboard_text;
use clipboard_rs::ClipboardContext;

// 粘贴纯文本
pub fn paste_text(ctx: &ClipboardContext, text: &str) -> Result<(), String> {
    set_clipboard_text(ctx, text)
}
