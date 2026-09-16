import { spawn } from 'node:child_process'
import { fileURLToPath } from 'node:url'
const repository=fileURLToPath(new URL('../..',import.meta.url))
const args=process.argv.slice(2)
for (const [flag,value] of [['--workspace',repository],['--data',fileURLToPath(new URL('../../.zedflow',import.meta.url))],['--web',fileURLToPath(new URL('../../web/apps/client/dist',import.meta.url))]]) {
  if(!args.some(arg=>arg===flag||arg.startsWith(`${flag}=`)))args.push(flag,value)
}
const child = spawn('cargo', ['run', '--locked', '-p', 'zf-serve', '--bin', 'zedflow-daemon', '--', ...args], { cwd: fileURLToPath(new URL('../../rust', import.meta.url)), stdio: 'inherit', env: { ...process.env, CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR || '/tmp/zedflow-adk-target' } })
child.on('error', e => { console.error(e); process.exitCode = 1 })
child.on('exit', code => { process.exitCode = code ?? 1 })
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => child.kill(signal))
