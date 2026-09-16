import { fileURLToPath } from 'node:url'
import { mkdir } from 'node:fs/promises'
const repo=fileURLToPath(new URL('../../../..',import.meta.url))
import { createRequire } from 'node:module'
import { resolve } from 'node:path'
const require = createRequire(fileURLToPath(new URL('../package.json',import.meta.url)))
const tests=createRequire(resolve(repo,'e2e/package.json'))
const { _electron:electron }=tests('@playwright/test')
const executablePath = require('electron')
const application = await electron.launch({ executablePath, timeout: 30000, args: ['--no-sandbox', fileURLToPath(new URL('../main.mjs',import.meta.url))], env: { ...process.env, ZEDFLOW_CLIENT_URL: process.env.ZEDFLOW_CLIENT_URL || 'http://127.0.0.1:5173' } })
try {
  const window = await application.firstWindow()
  await window.getByText('Daemon connecté').waitFor({timeout:30000})
  const preferences = await application.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0].webContents.getLastWebPreferences())
  if (preferences.nodeIntegration || !preferences.contextIsolation || !preferences.sandbox) throw new Error('Renderer isolation mismatch')
  await mkdir(resolve(repo,'.agents/output/app-preview'),{recursive:true})
  await window.screenshot({path:resolve(repo,'.agents/output/app-preview/04-electron.png')})
  console.log('Electron: native window, Vue renderer, daemon connection and renderer isolation verified')
} finally { await application.close() }
