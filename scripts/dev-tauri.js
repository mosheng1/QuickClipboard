#!/usr/bin/env node
// 开发模式前置脚本 - 链接截图插件源码后启动 vite dev，退出时自动清理
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

// 通过符号链接将截图插件源码链接到主项目
async function linkScreenshotSource() {
  if (!existsSync(SCREENSHOT_PLUGIN_SRC)) {
    return false
  }

  if (existsSync(SCREENSHOT_MAIN_PROJECT)) {
    await fs.rm(SCREENSHOT_MAIN_PROJECT, { recursive: true, force: true })
  }

  if (isWin) {
    await fs.symlink(SCREENSHOT_PLUGIN_SRC, SCREENSHOT_MAIN_PROJECT, 'junction')
  } else {
    await fs.symlink(SCREENSHOT_PLUGIN_SRC, SCREENSHOT_MAIN_PROJECT, 'dir')
  }

  return true
}

// 清理截图插件符号链接
async function unlinkScreenshotSource() {
  if (existsSync(SCREENSHOT_MAIN_PROJECT)) {
    await fs.rm(SCREENSHOT_MAIN_PROJECT, { recursive: true, force: true })
  }
}

let childProcess = null

async function main() {
  const isCommunity = process.env.QC_COMMUNITY === '1'
  const hasScreenshot = isScreenshotPluginAvailable(isCommunity)

  if (hasScreenshot) {
    await linkScreenshotSource()
  }

  childProcess = spawn(npmCmd, ['run', 'dev'], {
    cwd: rootDir,
    stdio: 'inherit',
    env: process.env,
    shell: true,
  })

  childProcess.on('error', (err) => {
    console.error(err)
    process.exit(1)
  })

  childProcess.on('exit', (code) => {
    if (code && code !== 0) {
      process.exit(code)
    }
  })
}

// 终止子进程并清理符号链接
function shutdown() {
  childProcess?.kill()
  unlinkScreenshotSource().catch(console.error)
}

process.on('SIGINT', () => {
  shutdown()
  process.exit(0)
})
process.on('SIGTERM', () => {
  shutdown()
  process.exit(0)
})

main().catch((err) => {
  console.error(err)
  shutdown()
  process.exit(1)
})
