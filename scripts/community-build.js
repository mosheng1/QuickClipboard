#!/usr/bin/env node
// 社区版编译脚本 - 使用 --no-default-features 排除私有插件
// 临时修补 Cargo.toml 移除私有依赖声明，构建后自动恢复
import { spawn } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import path from 'node:path'
import fs from 'node:fs'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)
const rootDir = path.resolve(__dirname, '..')
const srcTauriDir = path.join(rootDir, 'src-tauri')
const npmCmd = process.platform === 'win32' ? 'npm.cmd' : 'npm'

// 需要从 Cargo.toml 中移除的私有依赖前缀
const CARGO_TOML_PATH = path.join(srcTauriDir, 'Cargo.toml')
const COMMUNITY_CONFIG_PATH = path.join(srcTauriDir, 'community.conf.json')
const PRIVATE_DEPENDENCY_PREFIXES = ['screenshot-suite', 'gpu-image-viewer']

const isDev = process.argv.includes('--dev')
const isCommunity = process.argv.includes('--community')
const edition = isCommunity ? '社区版' : '完整版'
const mode = isDev ? '开发' : '生产'

// 清理栈：按注册逆序执行，确保资源可靠释放
const cleanupStack = []

function registerCleanup(fn) {
  cleanupStack.push(fn)
}

function runCleanup() {
  while (cleanupStack.length > 0) {
    const fn = cleanupStack.pop()
    try { fn() } catch (err) { console.error('[build] 清理失败:', err.message) }
  }
}

// 信号处理
process.on('SIGINT', () => { runCleanup(); process.exit(130) })
process.on('SIGTERM', () => { runCleanup(); process.exit(143) })

// 修补 Cargo.toml：移除私有依赖和 full feature，将 default feature 置空
// Cargo 在 feature 解析前就会 fetch 所有 git 依赖（包括 optional），
// 因此社区构建仍需临时移除私有依赖声明
function patchCargoToml() {
  if (!fs.existsSync(CARGO_TOML_PATH)) {
    throw new Error(`未找到 Cargo.toml: ${CARGO_TOML_PATH}`)
  }

  const original = fs.readFileSync(CARGO_TOML_PATH, 'utf8')
  const modified = original
    .split(/\r?\n/)
    .filter((line) => {
      const trimmed = line.trim()
      return !PRIVATE_DEPENDENCY_PREFIXES.some((prefix) => trimmed.startsWith(prefix))
        && !trimmed.startsWith('full =')
    })
    .map((line) => (line.trim().startsWith('default') ? 'default = []' : line))
    .join('\n')

  fs.writeFileSync(CARGO_TOML_PATH, modified, 'utf8')
  registerCleanup(() => fs.writeFileSync(CARGO_TOML_PATH, original, 'utf8'))
}

// 生成社区版临时配置：禁用更新产物生成
function createCommunityConfig() {
  const config = { bundle: { createUpdaterArtifacts: false } }
  fs.writeFileSync(COMMUNITY_CONFIG_PATH, JSON.stringify(config, null, 2), 'utf8')
  registerCleanup(() => {
    try { fs.unlinkSync(COMMUNITY_CONFIG_PATH) } catch {}
  })
}

// 执行 npm 子命令
function run(cwd, args, env = process.env) {
  return new Promise((resolve, reject) => {
    const child = spawn(npmCmd, args, {
      cwd,
      stdio: 'inherit',
      env,
      shell: true,
    })

    child.on('exit', (code) => {
      if (code === 0) resolve()
      else reject(new Error(`${args.join(' ')} 失败，退出码 ${code}`))
    })
  })
}

async function main() {
  try {
    if (isCommunity) {
      patchCargoToml()
      createCommunityConfig()
    }

    const args = ['run', 'tauri', '--', isDev ? 'dev' : 'build']

    if (isCommunity) {
      args.push('--config', COMMUNITY_CONFIG_PATH, '--', '--no-default-features')
    }

    console.log(`[build] 版本: ${edition}`)
    console.log(`[build] 模式: ${mode}`)
    console.log(`[build] 执行: npm ${args.join(' ')}`)

    await run(rootDir, args, {
      ...process.env,
      QC_COMMUNITY: isCommunity ? '1' : '0',
    })

    console.log(`[build] ${edition}编译完成`)
  } catch (err) {
    console.error(`[build] 编译失败: ${err.message}`)
    process.exitCode = 1
  } finally {
    runCleanup()
  }
}

main()
