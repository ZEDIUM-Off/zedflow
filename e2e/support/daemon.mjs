import { spawn } from 'node:child_process'
import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = process.env.ZEDFLOW_E2E_ROOT
if (!root) throw new Error('ZEDFLOW_E2E_ROOT must be supplied by Playwright')
const port = process.env.ZEDFLOW_E2E_DAEMON_PORT || '3157'
const flowHome = join(root, 'home')
const workspaceA = join(root, 'workspace-a')
const workspaceB = join(root, 'workspace-b')
for (const directory of [flowHome, workspaceA, workspaceB]) {
  await mkdir(join(directory, '.zedflow/flows'), { recursive: true })
  await mkdir(join(directory, '.agents/flows'), { recursive: true })
}
for (const [workspace, name] of [[workspaceA, 'A'], [workspaceB, 'B']]) {
  await writeFile(join(workspace, 'AGENTS.md'), `# Workspace ${name}\nUse the ADK fixture model for these isolated browser tests.\n`)
  await writeFile(join(workspace, 'workspace-name.txt'), name)
}
console.log(`Isolated Zedflow E2E fixtures: ${root}`)
const child = spawn('cargo', [
  'run', '--locked', '-p', 'zf-serve', '--bin', 'zedflow-daemon', '--',
  '--web', fileURLToPath(new URL('../../web/apps/client/dist/', import.meta.url)),
  '--listen', `127.0.0.1:${port}`, '--data', join(root, 'data'),
  '--workspace', workspaceA, '--flow-home', flowHome, '--context-home', flowHome,
], {
  cwd: fileURLToPath(new URL('../../rust/', import.meta.url)),
  stdio: 'inherit',
  env: { ...process.env, CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR || '/tmp/zedflow-adk-target' },
})
let stopping = false
let stopTimer
function stop() {
  if (stopping || !child.pid) return
  stopping = true
  const signal = value => {
    try {
      child.kill(value)
    } catch (error) { if (error.code !== 'ESRCH') throw error }
  }
  signal('SIGINT')
  stopTimer = setTimeout(() => signal('SIGKILL'), 5000)
  stopTimer.unref()
}
child.on('error', error => { console.error(error); process.exitCode = 1 })
child.on('exit', code => { clearTimeout(stopTimer); process.exitCode = stopping ? 0 : (code ?? 1) })
process.on('SIGINT', stop)
process.on('SIGTERM', stop)
