import { spawn } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import path from 'node:path'
import fs from 'node:fs/promises'
import { existsSync } from 'node:fs'
import { ensureCleanCargoToml } from './ensure-clean-cargo-toml.js'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)
const rootDir = path.resolve(__dirname, '..')

const isWin = process.platform === 'win32'

function run(cwd, commandLine) {
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

  child.on('exit', (code) => {
    if (code && code !== 0) {
      process.exit(code)
    }
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

async function main() {
  const isCommunity = process.env.QC_COMMUNITY === '1'
  const screenshotWebDir = path.join(rootDir, 'src-tauri', 'plugins', 'screenshot-suite', 'web')
  const hasScreenshotPlugin = !isCommunity && existsSync(path.join(screenshotWebDir, 'package.json'))

  if (!isCommunity) {
    ensureCleanCargoToml()
  }

  if (hasScreenshotPlugin) {
    await linkScreenshotSource()
  }

  host = run(rootDir, 'npm run dev')
}

function shutdown() {
  host?.kill()
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
