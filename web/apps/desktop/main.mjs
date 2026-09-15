import { app, BrowserWindow } from 'electron'
const url = process.env.ZEDFLOW_CLIENT_URL || (process.argv.includes('--dev') ? 'http://127.0.0.1:5173' : 'http://127.0.0.1:3142')
const parsed = new URL(url)
if (!['http:', 'https:'].includes(parsed.protocol)) throw new Error('Unsupported client protocol')
if (parsed.protocol === 'http:' && !['127.0.0.1', 'localhost', '[::1]'].includes(parsed.hostname)) throw new Error('Remote clients require HTTPS')
function open() {
  const win = new BrowserWindow({ width: 1500, height: 980, minWidth: 1000, minHeight: 680, backgroundColor: '#181818', title: 'Zedflow', autoHideMenuBar: true, webPreferences: { nodeIntegration: false, contextIsolation: true, sandbox: true } })
  win.webContents.setWindowOpenHandler(() => ({ action: 'deny' }))
  win.webContents.on('will-navigate', (event, target) => { if (new URL(target).origin !== parsed.origin) event.preventDefault() })
  win.loadURL(url)
}
app.whenReady().then(open)
app.on('activate', () => { if (!BrowserWindow.getAllWindows().length) open() })
app.on('window-all-closed', () => { if (process.platform !== 'darwin') app.quit() })
