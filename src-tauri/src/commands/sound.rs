use crate::services::sound::{AppSounds, SoundPlayer};
use std::path::Path;

#[tauri::command]
pub fn play_sound(path: String, volume: f32) -> Result<(), String> {
    SoundPlayer::play(Path::new(&path), volume);
    Ok(())
}

#[tauri::command]
pub fn play_beep(frequency: f32, duration_ms: u64, volume: f32) -> Result<(), String> {
    SoundPlayer::play_beep(frequency, duration_ms, volume);
    Ok(())
}

#[tauri::command]
pub fn play_copy_sound() {
    AppSounds::play_copy();
}

#[tauri::command]
pub fn play_paste_sound() {
    AppSounds::play_paste();
}

#[tauri::command]
pub fn play_scroll_sound() {
    AppSounds::play_scroll();
}
