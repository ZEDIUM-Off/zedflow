import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'
import { fileURLToPath, URL } from 'node:url'
import { createRequire } from 'node:module'
import { dirname, join } from 'node:path'
import { clientBuild } from '../../../tooling/releases/src/lib/build-info.mjs'
const build = clientBuild()
const require = createRequire(import.meta.url)
// libavoid publishes the binary but does not export its npm subpath.
const avoidWasm = join(dirname(require.resolve('libavoid-js')), 'libavoid.wasm')
export default defineConfig(({command}) => ({ base: command === 'build' ? `/_client/${build.buildId}/` : '/', define: { __ZEDFLOW_CLIENT_BUILD__: JSON.stringify(build) }, plugins: [vue(), tailwindcss(), { name: 'zedflow-build-identity', generateBundle(){this.emitFile({type:'asset',fileName:'client-version.json',source:JSON.stringify(build)})} }], resolve: { alias: { '@': fileURLToPath(new URL('./src', import.meta.url)), '@libavoid-assets': dirname(avoidWasm) } }, server: { port: Number(process.env.ZEDFLOW_WEB_PORT || 5173), strictPort: true, proxy: { '/api': process.env.ZEDFLOW_DAEMON_URL || 'http://127.0.0.1:3142' } } }))
