#!/usr/bin/env node
// 社区版 Rust 检查/测试脚本 - 临时移除私有插件依赖后运行 cargo 命令
// 用法:
//   node scripts/community-check.js check   - cargo check
//   node scripts/community-check.js clippy  - cargo clippy
//   node scripts/community-check.js test    - cargo test
import { spawn } from 'child_process';
import { fileURLToPath } from 'url';
import path from 'path';
import fs from 'fs';
import { ensureCleanWorkspace } from './ensure-clean-workspace.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.join(__dirname, '..');

const subcommand = process.argv[2];
const validCommands = {
  check: ['check', '--no-default-features'],
  clippy: ['clippy', '--no-default-features'],
  test: ['test', '--no-default-features'],
};

if (!validCommands[subcommand]) {
  console.error(`用法: node scripts/community-check.js <check|clippy|test>`);
  process.exit(1);
}

const cargoArgs = validCommands[subcommand];

const screenshotCapabilityPath = path.join(rootDir, 'src-tauri', 'capabilities', 'screenshot.json');
const defaultCapabilityPath = path.join(rootDir, 'src-tauri', 'capabilities', 'default.json');
const cargoTomlPath = path.join(rootDir, 'src-tauri', 'Cargo.toml');
const cargoLockPath = path.join(rootDir, 'src-tauri', 'Cargo.lock');

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
  // 保持原始换行符（CRLF/LF），避免 Windows 上格式变化
  const eol = original.includes('\r\n') ? '\r\n' : '\n';
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
    .map((line) => {
      const trimmed = line.trim();
      if (!trimmed.startsWith('default =')) return line;
      // 仅从 default 中移除私有 feature，保留其他公共 feature
      const m = line.match(/^(\s*)default\s*=\s*\[(.*)\]/);
      if (!m) return line;
      const kept = m[2].split(',').map(s => s.trim()).filter(Boolean)
        .filter(f => !PRIVATE_FEATURE_NAMES.some(n => f === `"${n}"`));
      return `${m[1]}default = [${kept.join(', ')}]`;
    })
    .join(eol);

  if (modified === original) return () => {};

  fs.writeFileSync(cargoTomlPath, modified, 'utf8');

  return () => {
    fs.writeFileSync(cargoTomlPath, original, 'utf8');
  };
}

// 备份 Cargo.lock，命令完成后恢复原状
function backupCargoLock() {
  if (!fs.existsSync(cargoLockPath)) return () => {};

  const original = fs.readFileSync(cargoLockPath, 'utf8');

  return () => {
    fs.writeFileSync(cargoLockPath, original, 'utf8');
  };
}

// 修补所有需要修改的文件，返回闭包用于恢复
function patchForCommunity() {
  ensureCleanWorkspace();

  const restoreCargoToml = patchCargoToml();
  const restoreScreenshot = patchCapabilityFile(screenshotCapabilityPath);
  const restoreDefault = patchCapabilityFile(defaultCapabilityPath);
  const restoreCargoLock = backupCargoLock();

  let restored = false;
  return () => {
    if (restored) return;
    restored = true;
    restoreCargoToml();
    restoreScreenshot();
    restoreDefault();
    restoreCargoLock();
  };
}

console.log(`[check] 执行: cargo ${cargoArgs.join(' ')} (社区版，已排除私有插件)`);

let restored = false;
const restorePatches = patchForCommunity();
const restoreOnce = () => {
  if (restored) return;
  restored = true;
  try {
    restorePatches();
  } catch (err) {
    console.error('[check] 还原失败:', err.message);
  }
};

let interrupted = false;

const child = spawn('cargo', cargoArgs, {
  stdio: 'inherit',
  cwd: path.join(rootDir, 'src-tauri'),
  shell: true,
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
  console.error(`[check] 启动失败: ${err.message}`);
  process.exit(1);
});

child.on('close', (code) => {
  restoreOnce();
  const exitCode = interrupted ? 130 : (code ?? 1);
  if (exitCode !== 0) {
    console.error(`[check] ${subcommand} ${interrupted ? '中断' : '失败'}，退出码: ${exitCode}`);
  } else {
    console.log(`[check] ${subcommand} 完成`);
  }
  process.exit(exitCode);
});
