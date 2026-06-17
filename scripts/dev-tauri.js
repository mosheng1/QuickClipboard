import { spawn } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import path from 'node:path'
import fs from 'node:fs/promises'
import { existsSync, rmSync } from 'node:fs'
import { ensureCleanWorkspace } from './ensure-clean-workspace.js'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)
const rootDir = path.resolve(__dirname, '..')

const isWin = process.platform === 'win32'

function run(cwd, commandLine) {
  if (interrupted) throw new Error('中断')
  const child = isWin
    ? spawn('cmd.exe', ['/d', '/s', '/c', commandLine], {
        cwd,
        stdio: 'inherit',
        env: process.env,
      })
    : spawn('sh', ['-lc', commandLine], {
        cwd,
        stdio: 'inherit',
        env: process.env,
      })

  child.on('error', (err) => {
    console.error(err)
    process.exit(1)
  })

  return child
}

async function linkScreenshotSource() {
  const screenshotPluginSrc = path.join(rootDir, 'src-tauri', 'plugins', 'screenshot-suite', 'web', 'windows', 'screenshot')
  const mainProjectScreenshot = path.join(rootDir, 'src', 'windows', 'screenshot')

  if (!existsSync(screenshotPluginSrc)) {
    return false
  }

  if (existsSync(mainProjectScreenshot)) {
    await fs.rm(mainProjectScreenshot, { recursive: true, force: true })
  }

  if (process.platform === 'win32') {
    await fs.symlink(screenshotPluginSrc, mainProjectScreenshot, 'junction')
  } else {
    await fs.symlink(screenshotPluginSrc, mainProjectScreenshot, 'dir')
  }
  return true
}

async function unlinkScreenshotSource() {
  const mainProjectScreenshot = path.join(rootDir, 'src', 'windows', 'screenshot')

  if (existsSync(mainProjectScreenshot)) {
    await fs.rm(mainProjectScreenshot, { recursive: true, force: true })
  }
}

let host = null
let interrupted = false
let hasScreenshot = false

// 信号中断时 kill Vite 子进程，子进程 exit 后输出消息并退出
process.on('SIGINT', () => {
  if (interrupted) return
  interrupted = true
  try { host?.kill(); } catch {}
})

process.on('SIGTERM', () => {
  if (interrupted) return
  interrupted = true
  try { host?.kill(); } catch {}
})

process.on('exit', () => {
  if (hasScreenshot) {
    try { rmSync(path.join(rootDir, 'src', 'windows', 'screenshot'), { recursive: true, force: true }) } catch {}
  }
})

async function main() {
  const isCommunity = process.env.QC_COMMUNITY === '1'

  // 完整版构建前检测并恢复社区构建遗留的补丁文件
  if (!isCommunity) {
    ensureCleanWorkspace()
  }

  const screenshotWebDir = path.join(rootDir, 'src-tauri', 'plugins', 'screenshot-suite', 'web')
  hasScreenshot = !isCommunity && existsSync(path.join(screenshotWebDir, 'package.json'))

  if (hasScreenshot) {
    await linkScreenshotSource()
  }

  host = run(rootDir, 'npm run dev')

  host.on('exit', (code) => {
    if (interrupted) {
      console.error('[dev] 开发服务中断，退出码: 130')
      process.exit(130)
    } else if (code && code !== 0) {
      process.exit(code)
    }
  })
}

main().catch((err) => {
  if (interrupted) {
    console.error('[dev] 开发服务中断，退出码: 130')
  } else {
    console.error(err)
  }
  process.exit(interrupted ? 130 : 1)
})
