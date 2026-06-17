#!/usr/bin/env node
// 社区版编译脚本 - 使用 --no-default-features 排除私有插件
import { spawn } from 'child_process';
import { fileURLToPath } from 'url';
import path from 'path';
import fs from 'fs';
import { ensureCleanWorkspace } from './ensure-clean-workspace.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.join(__dirname, '..');

const isDev = process.argv.includes('--dev');
const isCommunity = process.argv.includes('--no-default-features') || !process.argv.includes('--full');
const command = isDev ? 'dev' : 'build';

const screenshotCapabilityPath = path.join(rootDir, 'src-tauri', 'capabilities', 'screenshot.json');
const defaultCapabilityPath = path.join(rootDir, 'src-tauri', 'capabilities', 'default.json');
const cargoTomlPath = path.join(rootDir, 'src-tauri', 'Cargo.toml');
const cargoLockPath = path.join(rootDir, 'src-tauri', 'Cargo.lock');

const PRIVATE_DEPENDENCY_PREFIXES = ['screenshot-suite = {', 'gpu-image-viewer = {'];
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

    ensureCleanWorkspace();

    const original = fs.readFileSync(cargoTomlPath, 'utf8');
    // 保持原始换行符（CRLF/LF），避免 Windows 上格式变化
    const eol = original.includes('\r\n') ? '\r\n' : '\n';
    const modified = original
        .split(/\r?\n/)
        .filter((line) => {
            const trimmed = line.trim();
            if (PRIVATE_DEPENDENCY_PREFIXES.some((prefix) => trimmed.startsWith(prefix))) {
                return false;
            }
            if (PRIVATE_FEATURE_NAMES.some((name) => {
                const pattern = new RegExp(`^${name}\\s*=\\s*\\[`);
                return pattern.test(trimmed) && trimmed.includes('dep:');
            })) {
                return false;
            }
            return true;
        })
        .map((line) => (line.trim().startsWith('default') ? 'default = []' : line))
        .join(eol);

    if (modified === original) return () => {};

    fs.writeFileSync(cargoTomlPath, modified, 'utf8');

    return () => {
        fs.writeFileSync(cargoTomlPath, original, 'utf8');
    };
}

// 备份 Cargo.lock，社区构建完成后恢复原状
function backupCargoLock() {
    if (!fs.existsSync(cargoLockPath)) return () => {};

    const original = fs.readFileSync(cargoLockPath, 'utf8');

    return () => {
        fs.writeFileSync(cargoLockPath, original, 'utf8');
    };
}

// 修补所有社区构建需要修改的文件，返回闭包用于恢复
function patchForCommunity() {
    if (!isCommunity) return () => {};

    const restoreCargoToml = patchCargoToml();
    const restoreScreenshot = patchCapabilityFile(screenshotCapabilityPath);
    const restoreDefault = patchCapabilityFile(defaultCapabilityPath);
    const restoreCargoLock = backupCargoLock();

    return () => {
        restoreCargoToml();
        restoreScreenshot();
        restoreDefault();
        restoreCargoLock();
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

// 修补前先检测并恢复上次中断残留的补丁文件
ensureCleanWorkspace();

let restored = false;
const restorePatches = patchForCommunity();
const restoreOnce = () => {
    if (restored) return;
    restored = true;
    try {
        restorePatches();
    } catch (err) {
        console.error('[build] 还原失败:', err.message);
    }
};

let interrupted = false;

const child = spawn('npm', args, { 
    stdio: 'inherit', 
    cwd: rootDir,
    shell: true,
    env: {
        ...process.env,
        QC_COMMUNITY: isCommunity ? '1' : '0'
    }
});

// 信号中断时 kill 子进程，等 close 事件后再 restore，
// 避免 cargo 退出时覆盖已恢复的 Cargo.lock
process.on('SIGINT', () => {
    if (interrupted) return;
    interrupted = true;
    try { child.kill('SIGINT'); } catch {}
    // 不在此恢复，等 child close 事件确保子进程已完全退出
});

process.on('SIGTERM', () => {
    if (interrupted) return;
    interrupted = true;
    try { child.kill('SIGTERM'); } catch {}
});

child.on('error', (err) => {
    restoreOnce();
    console.error(`[build] 启动失败: ${err.message}`);
    process.exit(1);
});

child.on('close', (code) => {
    restoreOnce();
    const exitCode = interrupted ? 130 : (code ?? 1);
    if (exitCode !== 0) {
        console.error(`[build] 编译${interrupted ? '中断' : '失败'}，退出码: ${exitCode}`);
    } else {
        console.log(`[build] ${edition}编译完成`);
    }
    process.exit(exitCode);
});
