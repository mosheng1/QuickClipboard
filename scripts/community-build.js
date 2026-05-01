#!/usr/bin/env node
// 社区版编译脚本 - 使用 --no-default-features 排除私有插件
import { spawn } from 'child_process';
import { fileURLToPath } from 'url';
import path from 'path';
import fs from 'fs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.join(__dirname, '..');

const isDev = process.argv.includes('--dev');
const isCommunity = process.argv.includes('--no-default-features') || !process.argv.includes('--full');
const command = isDev ? 'dev' : 'build';

const screenshotCapabilityPath = path.join(rootDir, 'src-tauri', 'capabilities', 'screenshot.json');
const defaultCapabilityPath = path.join(rootDir, 'src-tauri', 'capabilities', 'default.json');

function patchCapabilityFile(filePath) {
    if (!fs.existsSync(filePath)) return () => {};

    const original = fs.readFileSync(filePath, 'utf8');
    let json;
    try {
        json = JSON.parse(original);
    } catch {
        return () => {};
    }

    if (!Array.isArray(json.permissions)) return () => {};

    const nextPermissions = json.permissions.filter((p) => p !== 'screenshot-suite:default');
    if (nextPermissions.length === json.permissions.length) return () => {};

    json.permissions = nextPermissions;
    fs.writeFileSync(filePath, `${JSON.stringify(json, null, 2)}\n`, 'utf8');

    return () => {
        fs.writeFileSync(filePath, original, 'utf8');
    };
}

function patchCapabilitiesForCommunity() {
    if (!isCommunity) return () => {};

    const restoreScreenshot = patchCapabilityFile(screenshotCapabilityPath);
    const restoreDefault = patchCapabilityFile(defaultCapabilityPath);

    return () => {
        restoreScreenshot();
        restoreDefault();
    };
}

const args = ['run', 'tauri', '--', command];
if (isCommunity) {
    args.push('--', '--no-default-features', '--features', 'custom-protocol');
} else {
    args.push('--', '--features', 'custom-protocol');
}

const edition = isCommunity ? '社区版' : '完整版';
console.log(`[build] 版本: ${edition}`);
console.log(`[build] 模式: ${isDev ? '开发' : '生产'}`);
console.log(`[build] 执行: npm ${args.join(' ')}`);

let restored = false;
const restoreCapabilities = patchCapabilitiesForCommunity();
const restoreOnce = () => {
    if (restored) return;
    restored = true;
    try {
        restoreCapabilities();
    } catch {
    }
};

process.on('SIGINT', () => {
    restoreOnce();
    process.exit(130);
});

process.on('SIGTERM', () => {
    restoreOnce();
    process.exit(143);
});

const child = spawn('npm', args, { 
    stdio: 'inherit', 
    cwd: rootDir,
    shell: true,
    env: {
        ...process.env,
        QC_COMMUNITY: isCommunity ? '1' : '0'
    }
});

child.on('error', (err) => {
    restoreOnce();
    console.error(`[build] 启动失败: ${err.message}`);
    process.exit(1);
});

child.on('close', (code) => {
    restoreOnce();
    if (code !== 0) {
        console.error(`[build] 编译失败，退出码: ${code}`);
    } else {
        console.log(`[build] ${edition}编译完成`);
    }
    process.exit(code);
});
