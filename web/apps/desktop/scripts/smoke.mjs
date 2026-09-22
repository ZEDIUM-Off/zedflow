import { fileURLToPath } from 'node:url'
import { mkdir, mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
const repo=fileURLToPath(new URL('../../../..',import.meta.url))
import { createRequire } from 'node:module'
import { resolve } from 'node:path'
const require = createRequire(fileURLToPath(new URL('../package.json',import.meta.url)))
const tests=createRequire(resolve(repo,'e2e/package.json'))
const { _electron:electron }=tests('@playwright/test')
const executablePath = require('electron')
if (!process.env.ZEDFLOW_CLIENT_URL) throw new Error('ZEDFLOW_CLIENT_URL must point to a dedicated fixture daemon')
const profile = await mkdtemp(resolve(tmpdir(), 'zedflow-electron-smoke-'))
let application
try {
  application = await electron.launch({ executablePath, timeout: 30000, args: ['--no-sandbox', `--user-data-dir=${profile}`, fileURLToPath(new URL('../main.mjs',import.meta.url))], env: { ...process.env } })
  const window = await application.firstWindow()
  await window.getByText('Daemon connecté').waitFor({timeout:30000})
  const preferences = await application.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0].webContents.getLastWebPreferences())
  if (preferences.nodeIntegration || !preferences.contextIsolation || !preferences.sandbox) throw new Error('Renderer isolation mismatch')
  await mkdir(resolve(repo,'.agents/output/app-preview'),{recursive:true})
  await window.screenshot({path:resolve(repo,'.agents/output/app-preview/04-electron.png')})
  console.log('Electron: native window, Vue renderer, daemon connection and renderer isolation verified')
} finally {
  try { await application?.close() }
  finally { await rm(profile, { recursive: true, force: true }) }
}
