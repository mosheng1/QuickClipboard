import { spawn } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import path from 'node:path'
import fs from 'node:fs/promises'
import { existsSync } from 'node:fs'
import { ensureCleanCargoToml } from './ensure-clean-cargo-toml.js'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)
const rootDir = path.resolve(__dirname, '..')

const npmCmd = process.platform === 'win32' ? 'npm.cmd' : 'npm'

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

async function linkScreenshotSource() {
  const screenshotPluginSrc = path.join(rootDir, 'src-tauri', 'plugins', 'screenshot-suite', 'web', 'windows', 'screenshot')
  const mainProjectScreenshot = path.join(rootDir, 'src', 'windows', 'screenshot')
  
  if (!existsSync(screenshotPluginSrc)) {
    throw new Error(`未找到截图插件源码: ${screenshotPluginSrc}`)
  }
  
  if (existsSync(mainProjectScreenshot)) {
    await fs.rm(mainProjectScreenshot, { recursive: true, force: true })
  }
  
  await fs.cp(screenshotPluginSrc, mainProjectScreenshot, { recursive: true })
}

async function unlinkScreenshotSource() {
  const mainProjectScreenshot = path.join(rootDir, 'src', 'windows', 'screenshot')
  
  if (existsSync(mainProjectScreenshot)) {
    await fs.rm(mainProjectScreenshot, { recursive: true, force: true })
  }
}

async function main() {
  const isCommunity = process.env.QC_COMMUNITY === '1'
  const hasScreenshotPlugin = !isCommunity && existsSync(
    path.join(rootDir, 'src-tauri', 'plugins', 'screenshot-suite', 'web', 'package.json')
  )

  ensureCleanCargoToml()

  try {
    if (hasScreenshotPlugin) {
      await linkScreenshotSource()
    }

    await run(rootDir, ['run', 'build'])
    
  } finally {
    if (hasScreenshotPlugin) {
      await unlinkScreenshotSource()
    }
  }
}

main().catch((err) => {
  console.error(err)
  process.exit(1)
})
