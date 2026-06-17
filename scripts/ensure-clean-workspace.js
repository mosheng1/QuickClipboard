// 检测并恢复因 SIGKILL 等强制中断遗留的构建补丁。
// 社区构建时会移除 Cargo.toml 中 gpu-image-viewer / screenshot-suite
// 的依赖及 feature 声明。若进程被强制终止，补丁未还原则残留。
// 通过检测 Cargo.toml 是否缺少私有依赖行来识别，仅在确认残留时才恢复。
import { execSync } from 'child_process';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const TOML = path.join(ROOT, 'src-tauri', 'Cargo.toml');
const SCREENSHOT_CAP = path.join(ROOT, 'src-tauri', 'capabilities', 'screenshot.json');
const DEFAULT_CAP = path.join(ROOT, 'src-tauri', 'capabilities', 'default.json');
const CARGO_LOCK = path.join(ROOT, 'src-tauri', 'Cargo.lock');

export function ensureCleanWorkspace() {
  if (!fs.existsSync(TOML)) return;

  // 检测 Cargo.toml 是否包含未注释的私有插件依赖行。
  // 若依赖行存在 → 正常状态；若不存在 → 已处于补丁，需恢复
  const content = fs.readFileSync(TOML, 'utf8');
  const hasPrivate = content
    .split(/\r?\n/)
    .some((line) => {
      const t = line.trim();
      if (t.startsWith('#')) return false;
      return t.startsWith('screenshot-suite = {') || t.startsWith('gpu-image-viewer = {');
    });

  if (!hasPrivate) {
    try {
      // 记录补丁文件原始换行格式，git checkout 后原样恢复
      const wasCRLF = content.includes('\r\n');
      execSync(
        `git checkout -- "${TOML}" "${SCREENSHOT_CAP}" "${DEFAULT_CAP}" "${CARGO_LOCK}"`,
        { cwd: ROOT, stdio: 'pipe' }
      );
      if (wasCRLF) {
        for (const f of [TOML, SCREENSHOT_CAP, DEFAULT_CAP, CARGO_LOCK]) {
          if (fs.existsSync(f)) {
            const c = fs.readFileSync(f, 'utf8');
            if (!c.includes('\r\n')) {
              fs.writeFileSync(f, c.replace(/\n/g, '\r\n'), 'utf8');
            }
          }
        }
      }
      console.log('[build] 检测到上次构建中断残留，已自动恢复 Cargo.toml / capabilities / Cargo.lock');
    } catch {
      // 非 git 环境或无 checkout 权限，静默跳过
    }
  }
}
