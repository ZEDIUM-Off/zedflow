import { spawn } from 'node:child_process'
import { fileURLToPath } from 'node:url'
const children = [
  spawn(process.execPath, [fileURLToPath(new URL('./daemon.mjs', import.meta.url))], { stdio: 'inherit' }),
  spawn('pnpm', ['--filter', '@zedflow/client', 'dev'], { cwd: fileURLToPath(new URL('../../web', import.meta.url)), stdio: 'inherit' }),
]
let stopping = false
function stop(code = 0) { if (stopping) return; stopping = true; for (const child of children) child.kill('SIGTERM'); process.exitCode = code }
for (const child of children) { child.on('error', (e) => { console.error(e); stop(1) }); child.on('exit', (code) => stop(code ?? 1)) }
process.on('SIGINT', () => stop())
process.on('SIGTERM', () => stop())
