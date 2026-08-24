//设置管理模块

import { invoke } from '@tauri-apps/api/core';

const STORAGE_KEY = 'pinImageSettings';

//默认设置
const DEFAULT_SETTINGS = {
    alwaysOnTop: false,
    shadow: false,
    lockPosition: false,
    pixelRender: false,
    opacity: 100,
    thumbnailMode: false,
    thumbnailRestoreMode: 'follow',
    savedThumbnailPosition: null
};

//获取当前主题设置
export async function getCurrentTheme() {
    try {
        return await invoke('reload_settings');
    } catch (error) {
        console.error('获取主题设置失败:', error);
        return { theme: 'auto' };
    }
}

//保存设置到 localStorage
export function saveSettings(settings) {
    try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
    } catch (error) {
        console.error('保存设置失败:', error);
    }
}

//从 localStorage 加载设置
export function loadSettings() {
    try {
        const saved = localStorage.getItem(STORAGE_KEY);
        if (saved) {
            return JSON.parse(saved);
        }
    } catch (error) {
        console.error('加载设置失败:', error);
    }
    return { ...DEFAULT_SETTINGS };
}

