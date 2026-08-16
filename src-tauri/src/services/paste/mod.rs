pub mod clipboard_content;
pub mod keyboard;
pub mod merge;
pub mod options;
pub mod paste_handler;
pub mod text;

pub use clipboard_content::{
    set_clipboard_files, set_clipboard_from_item, set_clipboard_text, FilesData,
};
pub use merge::{copy_merged_items, paste_merged_items};
pub use options::PasteAction;
