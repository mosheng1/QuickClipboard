#!/usr/bin/env node
// 生产构建前置脚本 - 链接截图插件源码后执行 vite build，构建完成后自动清理
import { spawn } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import path from 'node:path'
import fs from 'node:fs/promises'
import { existsSync } from 'node:fs'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)
const rootDir = path.resolve(__dirname, '..')
const isWin = process.platform === 'win32'
const npmCmd = isWin ? 'npm.cmd' : 'npm'

// 截图插件路径常量
const SCREENSHOT_PLUGIN_SRC = path.join(
  rootDir, 'src-tauri', 'plugins', 'screenshot-suite', 'web', 'windows', 'screenshot'
)
const SCREENSHOT_MAIN_PROJECT = path.join(rootDir, 'src', 'windows', 'screenshot')
const SCREENSHOT_PLUGIN_PACKAGE_JSON = path.join(
  rootDir, 'src-tauri', 'plugins', 'screenshot-suite', 'web', 'package.json'
)

// 检查截图插件是否可用（社区版不可用）
function isScreenshotPluginAvailable(isCommunity) {
  return !isCommunity && existsSync(SCREENSHOT_PLUGIN_PACKAGE_JSON)
}

// 通过符号链接将截图插件源码链接到主项目（比文件复制快得多）
async function linkScreenshotSource() {
  if (!existsSync(SCREENSHOT_PLUGIN_SRC)) {
    throw new Error(`未找到截图插件源码: ${SCREENSHOT_PLUGIN_SRC}`)
  }

  if (existsSync(SCREENSHOT_MAIN_PROJECT)) {
    await fs.rm(SCREENSHOT_MAIN_PROJECT, { recursive: true, force: true })
  }

  if (isWin) {
    await fs.symlink(SCREENSHOT_PLUGIN_SRC, SCREENSHOT_MAIN_PROJECT, 'junction')
  } else {
    await fs.symlink(SCREENSHOT_PLUGIN_SRC, SCREENSHOT_MAIN_PROJECT, 'dir')
  }
}

// 清理截图插件符号链接
async function unlinkScreenshotSource() {
  if (existsSync(SCREENSHOT_MAIN_PROJECT)) {
    await fs.rm(SCREENSHOT_MAIN_PROJECT, { recursive: true, force: true })
  }
}

// 执行 npm 子命令
function run(cwd, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(npmCmd, args, {
      cwd,
      stdio: 'inherit',
      env: process.env,
      shell: true,
    })

    child.on('exit', (code) => {
      if (code === 0) resolve()
      else reject(new Error(`${args.join(' ')} 失败，退出码 ${code}`))
    })
  })
}

async function main() {
  const isCommunity = process.env.QC_COMMUNITY === '1'
  const hasScreenshot = isScreenshotPluginAvailable(isCommunity)

  try {
    if (hasScreenshot) {
      await linkScreenshotSource()
    }

    await run(rootDir, ['run', 'build'])
  } finally {
    if (hasScreenshot) {
      await unlinkScreenshotSource()
    }
  }
}

main().catch((err) => {
  console.error(err)
  process.exit(1)
})
