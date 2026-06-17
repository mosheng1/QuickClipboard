import { execSync } from 'child_process';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const TOML = path.join(ROOT, 'src-tauri', 'Cargo.toml');

export function ensureCleanCargoToml() {
  if (!fs.existsSync(TOML)) return;

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
      execSync(`git checkout -- "${TOML}"`, { cwd: ROOT, stdio: 'pipe' });
      console.log('[build] 检测到上次构建中断残留，已自动恢复 Cargo.toml');
    } catch {
      // 非 git 环境或无 checkout 权限，静默跳过
    }
  }
}
