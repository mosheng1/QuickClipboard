use serde::{Deserialize, Serialize};

pub const SETTINGS_MIGRATION_VERSION_V1: u32 = 1;
pub const SETTINGS_MIGRATION_VERSION_V2: u32 = 2;
pub const SETTINGS_MIGRATION_VERSION_V3: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppSettings {
    // 基础设置
    pub auto_start: bool,
    pub run_as_admin: bool,
    pub start_hidden: bool,
    pub show_tray_icon: bool,
    pub show_startup_notification: bool,
    pub tooltips_enabled: bool,
    pub auto_low_memory_enabled: bool,
    pub auto_low_memory_idle_minutes: u32,
    pub auto_exit_low_memory_mode: bool,
    #[serde(alias = "history_limit")]
    pub history_limit: u64,
    pub language: String,
    pub theme: String,
    pub light_theme_style: String,
    pub dark_theme_style: String,
    pub custom_font_enabled: bool,
    pub custom_font_type: String,
    pub custom_font_path: String,
    pub custom_font_url: String,
    pub custom_font_family: String,
    pub visible_optional_tabs: Vec<String>,
    pub opacity: f64,
    pub background_image_path: String,
    pub super_background_blur_scale: f64,
    pub toggle_shortcut: String,
    pub open_settings_shortcut: String,
    pub number_shortcuts: bool,
    pub number_shortcuts_modifier: String,
    pub clipboard_monitor: bool,
    pub ignore_duplicates: bool,
    pub save_images: bool,
    pub image_preview: bool,
    pub text_preview: bool,
    pub file_preview: bool,
    #[serde(default)]
    pub settings_migration_version: Option<u32>,
    pub display_priority_order: String,

    // 音效设置
    pub sound_enabled: bool,
    pub sound_volume: f64,
    pub copy_sound_path: String,
    pub paste_sound_path: String,
    pub copy_sound_timing: String,
    pub paste_sound_timing: String,

    // 图片显示限制
    pub image_max_size_mb: u32,
    pub image_max_width: u32,
    pub image_max_height: u32,

    // 截屏设置
    pub screenshot_enabled: bool,
    pub screenshot_shortcut: String,
    pub screenshot_quick_save_shortcut: String,
    pub screenshot_quick_pin_shortcut: String,
    pub screenshot_quick_ocr_shortcut: String,
    pub screenshot_quality: u8,
    pub screenshot_auto_save: bool,
    pub screenshot_show_hints: bool,
    pub screenshot_element_detection: String,
    pub screenshot_magnifier_enabled: bool,
    pub screenshot_hints_enabled: bool,
    pub screenshot_color_include_format: bool,
    pub screenshot_window_lifecycle_mode: String,
    pub screenshot_auto_dispose_minutes: u32,

    // 预览窗口设置
    pub quickpaste_enabled: bool,
    pub quickpaste_shortcut: String,
    pub transfer_shelf_create_shortcut: String,
    pub quickpaste_paste_on_modifier_release: bool,
    pub quickpaste_scroll_sound: bool,
    pub quickpaste_scroll_sound_path: String,
    pub quickpaste_window_width: u32,
    pub quickpaste_window_height: u32,

    // AI翻译设置
    pub ai_translation_enabled: bool,
    pub ai_api_key: String,
    pub ai_model: String,
    pub ai_base_url: String,
    pub ai_target_language: String,
    pub ai_translate_on_copy: bool,
    pub ai_translate_on_paste: bool,
    pub ai_translation_prompt: String,
    pub ai_input_speed: u32,
    pub ai_newline_mode: String,
    pub ai_output_mode: String,

    // 鼠标设置
    pub mouse_middle_button_enabled: bool,
    pub mouse_middle_button_modifier: String,
    pub mouse_middle_button_trigger: String,
    pub mouse_middle_button_long_press_ms: u32,

    // 动画设置
    pub clipboard_animation_enabled: bool,
    pub ui_animation_enabled: bool,

    // 列表外观设置
    pub row_height: String,
    pub auto_row_max_lines: u32,
    pub file_display_mode: String,
    pub list_style: String,
    pub card_spacing: u32,

    // 显示行为
    pub auto_scroll_to_top_on_show: bool,
    pub auto_clear_search: bool,

    // 应用过滤设置
    pub app_filter_enabled: bool,
    pub app_filter_blocklist: Vec<String>,
    #[serde(default, skip_serializing)]
    pub app_filter_mode: String,
    #[serde(default, skip_serializing)]
    pub app_filter_list: Vec<String>,
    pub app_filter_effect: String,

    // 窗口设置
    pub window_position_mode: String,
    pub remember_window_size: bool,
    pub saved_window_position: Option<(i32, i32)>,
    pub saved_window_size: Option<(u32, u32)>,

    // 贴边隐藏设置
    pub edge_hide_enabled: bool,
    pub edge_snap_position: Option<(i32, i32)>,
    pub edge_snap_edge: Option<String>,
    pub edge_snap_ratio: Option<f64>,
    pub edge_snap_monitor_id: Option<String>,
    pub edge_hide_offset: i32,

    // 窗口行为设置
    pub auto_focus_search: bool,

    // 标题栏设置
    pub title_bar_position: String,

    // 格式设置
    pub paste_with_format: bool,
    pub paste_shortcut_mode: String,
    pub modifier_click_multi_select: bool,
    
    pub paste_to_top: bool,
    pub show_list_shortcuts: bool,
    pub show_list_index: bool,
    pub show_badges: bool,
    pub show_source_icon: bool,
    pub update_check_interval: String,
    pub disable_update_popup: bool,
    pub include_beta_updates: Option<bool>,

    // 快捷键设置
    pub hotkeys_enabled: bool,
    pub navigate_up_shortcut: String,
    pub navigate_down_shortcut: String,
    pub tab_left_shortcut: String,
    pub tab_right_shortcut: String,
    pub focus_search_shortcut: String,
    pub hide_window_shortcut: String,
    pub paste_item_shortcut: String,
    pub previous_group_shortcut: String,
    pub next_group_shortcut: String,
    pub toggle_pin_shortcut: String,
    pub toggle_clipboard_monitor_shortcut: String,
    pub toggle_paste_with_format_shortcut: String,
    pub toggle_low_memory_mode_shortcut: String,
    pub paste_plain_text_shortcut: String,

    // 数据存储设置
    #[serde(alias = "custom_storage_path")]
    pub custom_storage_path: Option<String>,
    #[serde(alias = "use_custom_storage")]
    pub use_custom_storage: bool,

    // WebDAV Sync 设置
    pub webdav_enabled: bool,
    pub webdav_url: String,
    pub webdav_username: String,
    #[serde(default, skip_serializing)]
    pub webdav_password: String,
    pub webdav_root_path: String,
    pub webdav_auto_push: bool,
    pub webdav_push_delay_secs: u64,
    pub webdav_auto_pull: bool,
    pub webdav_auto_pull_on_window_show: bool,
    pub webdav_pull_interval_secs: u64,
    pub webdav_push_shortcut: String,
    pub webdav_pull_shortcut: String,
    pub webdav_sync_clipboard: bool,
    pub webdav_sync_favorites: bool,
    pub webdav_sync_images: bool,
    pub sync_transfer_active_mode: String,

}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            auto_start: false,
            run_as_admin: false,
            start_hidden: true,
            show_tray_icon: true,
            show_startup_notification: true,
            tooltips_enabled: true,
            auto_low_memory_enabled: false,
            auto_low_memory_idle_minutes: 15,
            auto_exit_low_memory_mode: false,
            history_limit: 100,
            language: "zh-CN".to_string(),
            theme: "light".to_string(),
            light_theme_style: "modern".to_string(),
            dark_theme_style: "classic".to_string(),
            custom_font_enabled: false,
            custom_font_type: "file".to_string(),
            custom_font_path: String::new(),
            custom_font_url: String::new(),
            custom_font_family: String::new(),
            visible_optional_tabs: vec![
                "favorites".to_string(),
                "emoji".to_string(),
            ],
            opacity: 0.9,
            background_image_path: String::new(),
            super_background_blur_scale: 1.0,
            toggle_shortcut: "Shift+Space".to_string(),
            open_settings_shortcut: String::new(),
            number_shortcuts: true,
            number_shortcuts_modifier: "Ctrl".to_string(),
            clipboard_monitor: true,
            ignore_duplicates: true,
            save_images: true,
            image_preview: true,
            text_preview: true,
            file_preview: true,
            settings_migration_version: Some(SETTINGS_MIGRATION_VERSION_V3),
            display_priority_order: "text,html,image".to_string(),

            sound_enabled: true,
            sound_volume: 50.0,
            copy_sound_path: String::new(),
            paste_sound_path: String::new(),
            copy_sound_timing: "success".to_string(),
            paste_sound_timing: "success".to_string(),

            image_max_size_mb: 15,
            image_max_width: 4096,
            image_max_height: 4096,

            screenshot_enabled: true,
            screenshot_shortcut: "Ctrl+Shift+A".to_string(),
            screenshot_quick_save_shortcut: String::new(),
            screenshot_quick_pin_shortcut: String::new(),
            screenshot_quick_ocr_shortcut: String::new(),
            screenshot_quality: 85,
            screenshot_auto_save: true,
            screenshot_show_hints: true,
            screenshot_element_detection: "all".to_string(),
            screenshot_magnifier_enabled: true,
            screenshot_hints_enabled: true,
            screenshot_color_include_format: true,
            screenshot_window_lifecycle_mode: "quick".to_string(),
            screenshot_auto_dispose_minutes: 10,

            quickpaste_enabled: true,
            quickpaste_shortcut: "Ctrl+`".to_string(),
            transfer_shelf_create_shortcut: String::new(),
            quickpaste_paste_on_modifier_release: true,
            quickpaste_scroll_sound: true,
            quickpaste_scroll_sound_path: "sounds/roll.mp3".to_string(),
            quickpaste_window_width: 300,
            quickpaste_window_height: 400,

            ai_translation_enabled: false,
            ai_api_key: String::new(),
            ai_model: "Qwen/Qwen2-7B-Instruct".to_string(),
            ai_base_url: "https://api.siliconflow.cn/v1".to_string(),
            ai_target_language: "auto".to_string(),
            ai_translate_on_copy: false,
            ai_translate_on_paste: true,
            ai_translation_prompt: "请将以下文本翻译成{target_language}，严格保持原文的所有格式、换行符、段落结构和空白字符，只返回翻译结果，不要添加任何解释或修改格式：".to_string(),
            ai_input_speed: 50,
            ai_newline_mode: "auto".to_string(),
            ai_output_mode: "stream".to_string(),

            mouse_middle_button_enabled: false,
            mouse_middle_button_modifier: "None".to_string(),
            mouse_middle_button_trigger: "short_press".to_string(),
            mouse_middle_button_long_press_ms: 300,

            clipboard_animation_enabled: true,
            ui_animation_enabled: true,

            row_height: "medium".to_string(),
            auto_row_max_lines: 18,
            file_display_mode: "detailed".to_string(),
            list_style: "card".to_string(),
            card_spacing: 8,

            auto_scroll_to_top_on_show: false,
            auto_clear_search: false,

            app_filter_enabled: false,
            app_filter_blocklist: vec![],
            app_filter_mode: "blacklist".to_string(),
            app_filter_list: vec![],
            app_filter_effect: "clipboard_only".to_string(),

            window_position_mode: "smart".to_string(),
            remember_window_size: true,
            saved_window_position: None,
            saved_window_size: None,

            edge_hide_enabled: true,
            edge_snap_position: None,
            edge_snap_edge: None,
            edge_snap_ratio: None,
            edge_snap_monitor_id: None,
            edge_hide_offset: 3,

            auto_focus_search: false,

            title_bar_position: "top".to_string(),

            paste_with_format: true,
            paste_shortcut_mode: "ctrl_v".to_string(),
            modifier_click_multi_select: true,
            paste_to_top: false,
            show_list_shortcuts: true,
            show_list_index: true,
            show_badges: true,
            show_source_icon: true,
            update_check_interval: "daily".to_string(),
            disable_update_popup: false,
            include_beta_updates: None,

            hotkeys_enabled: true,
            navigate_up_shortcut: "ArrowUp".to_string(),
            navigate_down_shortcut: "ArrowDown".to_string(),
            tab_left_shortcut: "ArrowLeft".to_string(),
            tab_right_shortcut: "ArrowRight".to_string(),
            focus_search_shortcut: "Tab".to_string(),
            hide_window_shortcut: "Escape".to_string(),
            paste_item_shortcut: "Enter".to_string(),
            previous_group_shortcut: "Ctrl+ArrowUp".to_string(),
            next_group_shortcut: "Ctrl+ArrowDown".to_string(),
            toggle_pin_shortcut: "Ctrl+P".to_string(),
            toggle_clipboard_monitor_shortcut: String::new(),
            toggle_paste_with_format_shortcut: String::new(),
            toggle_low_memory_mode_shortcut: String::new(),
            paste_plain_text_shortcut: String::new(),

            custom_storage_path: None,
            use_custom_storage: false,

            webdav_enabled: false,
            webdav_url: String::new(),
            webdav_username: String::new(),
            webdav_password: String::new(),
            webdav_root_path: "quickclipboard".to_string(),
            webdav_auto_push: false,
            webdav_push_delay_secs: 10,
            webdav_auto_pull: false,
            webdav_auto_pull_on_window_show: false,
            webdav_pull_interval_secs: 30,
            webdav_push_shortcut: String::new(),
            webdav_pull_shortcut: String::new(),
            webdav_sync_clipboard: true,
            webdav_sync_favorites: true,
            webdav_sync_images: false,
            sync_transfer_active_mode: "webdav".to_string(),

        }
    }
}

impl AppSettings {
    pub fn normalize_app_filter_blocklist(&mut self) -> bool {
        let mut changed = false;

        match self.app_filter_mode.as_str() {
            "whitelist" => {}
            _ if self.app_filter_blocklist.is_empty() && !self.app_filter_list.is_empty() => {
                self.app_filter_blocklist = self.app_filter_list.clone();
                changed = true;
            }
            _ => {}
        }

        if self.app_filter_mode != "blacklist" {
            self.app_filter_mode = "blacklist".to_string();
            changed = true;
        }

        if !self.app_filter_list.is_empty() {
            self.app_filter_list.clear();
            changed = true;
        }

        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_legacy_blacklist_to_blocklist() {
        let mut settings = AppSettings::default();
        settings.app_filter_blocklist = vec![];
        settings.app_filter_mode = "blacklist".to_string();
        settings.app_filter_list = vec!["chrome.exe".to_string()];

        assert!(settings.normalize_app_filter_blocklist());
        assert_eq!(settings.app_filter_blocklist, vec!["chrome.exe".to_string()]);
        assert!(settings.app_filter_list.is_empty());
        assert_eq!(settings.app_filter_mode, "blacklist");
    }

    #[test]
    fn does_not_invert_legacy_whitelist_into_blocklist() {
        let mut settings = AppSettings::default();
        settings.app_filter_blocklist = vec![];
        settings.app_filter_mode = "whitelist".to_string();
        settings.app_filter_list = vec!["chrome.exe".to_string()];

        assert!(settings.normalize_app_filter_blocklist());
        assert!(settings.app_filter_blocklist.is_empty());
        assert!(settings.app_filter_list.is_empty());
        assert_eq!(settings.app_filter_mode, "blacklist");
    }
}
