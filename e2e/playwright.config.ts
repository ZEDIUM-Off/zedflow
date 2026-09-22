import { defineConfig } from '@playwright/test'
import { mkdtempSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

// The test runner, its workers, and both servers share one isolated fixture root.
const fixtureRoot = process.env.ZEDFLOW_E2E_ROOT || mkdtempSync(join(tmpdir(), 'zedflow-e2e-'))
process.env.ZEDFLOW_E2E_ROOT = fixtureRoot
const daemonPort = Number(process.env.ZEDFLOW_E2E_DAEMON_PORT || 3157)
const webPort = Number(process.env.ZEDFLOW_E2E_WEB_PORT || 5177)
const daemonUrl = `http://127.0.0.1:${daemonPort}`
const staticClient = process.env.ZEDFLOW_E2E_STATIC === '1'
const webUrl = `http://127.0.0.1:${webPort}`

export default defineConfig({
  testDir: './specs', outputDir: '../.agents/output/e2e/test-results', timeout: 90000, workers: 1,
  webServer: [
    {
      command: 'node support/daemon.mjs', url: `${daemonUrl}/api/health`,
      env: { ZEDFLOW_E2E_ROOT: fixtureRoot, ZEDFLOW_E2E_DAEMON_PORT: String(daemonPort) },
      reuseExistingServer: false, timeout: 180000,
      gracefulShutdown: { signal: 'SIGINT', timeout: 6000 },
    },
    ...(staticClient ? [] : [{
      command: 'pnpm --dir ../web dev', url: webUrl,
      env: { ZEDFLOW_DAEMON_URL: daemonUrl, ZEDFLOW_WEB_PORT: String(webPort) },
      reuseExistingServer: false, timeout: 60000,
    }]),
  ],
  use: {
    baseURL: staticClient ? daemonUrl : webUrl, viewport: { width: 1512, height: 982 },
    actionTimeout: 15000,
    launchOptions: { executablePath: process.env.CHROMIUM_PATH, args: ['--no-sandbox'] },
    screenshot: 'only-on-failure',
  },
})
