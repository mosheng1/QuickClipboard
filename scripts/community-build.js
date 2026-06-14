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
const cargoTomlPath = path.join(rootDir, 'src-tauri', 'Cargo.toml');

// 需要从 Cargo.toml 中移除的私有依赖行前缀
const PRIVATE_DEPENDENCY_PREFIXES = ['screenshot-suite = {', 'gpu-image-viewer = {'];
// 需要从 [features] 中移除的私有 feature 名称（仅移除含 dep: 引用的行，保留虚拟 feature）
const PRIVATE_FEATURE_NAMES = ['gpu-image-viewer', 'screenshot-suite'];

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

function patchCargoToml() {
    if (!fs.existsSync(cargoTomlPath)) return () => {};

    const original = fs.readFileSync(cargoTomlPath, 'utf8');
    const modified = original
        .split(/\r?\n/)
        .filter((line) => {
            const trimmed = line.trim();
            // 移除私有依赖声明行
            if (PRIVATE_DEPENDENCY_PREFIXES.some((prefix) => trimmed.startsWith(prefix))) {
                return false;
            }
            // 移除引用已删除 dep: 的私有 feature 行，保留虚拟 feature（如 screenshot-suite = []）
            if (PRIVATE_FEATURE_NAMES.some((name) => {
                const pattern = new RegExp(`^${name}\\s*=\\s*\\[`);
                return pattern.test(trimmed) && trimmed.includes('dep:');
            })) {
                return false;
            }
            return true;
        })
        .map((line) => (line.trim().startsWith('default') ? 'default = []' : line))
        .join('\n');

    fs.writeFileSync(cargoTomlPath, modified, 'utf8');

    return () => {
        fs.writeFileSync(cargoTomlPath, original, 'utf8');
    };
}

function patchForCommunity() {
    if (!isCommunity) return () => {};

    const restoreCargoToml = patchCargoToml();
    const restoreScreenshot = patchCapabilityFile(screenshotCapabilityPath);
    const restoreDefault = patchCapabilityFile(defaultCapabilityPath);

    return () => {
        restoreCargoToml();
        restoreScreenshot();
        restoreDefault();
    };
}

const args = ['run', 'tauri', '--', command];
if (isCommunity) {
    args.push('--', '--no-default-features');
}

const edition = isCommunity ? '社区版' : '完整版';
console.log(`[build] 版本: ${edition}`);
console.log(`[build] 模式: ${isDev ? '开发' : '生产'}`);
console.log(`[build] 执行: npm ${args.join(' ')}`);

let restored = false;
const restorePatches = patchForCommunity();
const restoreOnce = () => {
    if (restored) return;
    restored = true;
    try {
        restorePatches();
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
