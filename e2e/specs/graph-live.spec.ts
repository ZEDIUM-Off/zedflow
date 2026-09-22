import { test, expect } from '@playwright/test'
import { inspector, openSession, fixturePath } from './helpers'
import { writeFile } from 'node:fs/promises'

const node = (id: string, kind: string, config: Record<string, unknown>, x: number) => ({
  id, type: 'flow', position: { x, y: 160 }, data: { label: id, kind, config },
})

test('the execution graph advances and preserves completed nodes without navigating or resetting the viewport', async ({ page, request }) => {
  const errors: string[] = []
  page.on('pageerror', error => errors.push(error.message))
  await page.goto('/')
  await expect(page.getByText('Daemon connecté')).toBeVisible()
  const gates = [fixturePath(`graph-first-${crypto.randomUUID()}`), fixturePath(`graph-second-${crypto.randomUUID()}`)]
  const command = (path: string) => `until test -f '${path.replaceAll("'", "'\\''")}'; do sleep 0.05; done`
  const composition = {
    id: crypto.randomUUID(), name: `Graphe live ${crypto.randomUUID().slice(0, 8)}`, revision: 0,
    nodes: [
      node('start', 'start', {}, 0),
      node('first-delay', 'tool', { tool: 'exec', arguments: { command: command(gates[0]!) }, field: 'firstResult' }, 280),
      node('second-delay', 'tool', { tool: 'exec', arguments: { command: command(gates[1]!) }, field: 'secondResult' }, 560),
      node('answer', 'input', { field: 'input', prompt: 'Reprendre le graphe ?', responseType: 'text' }, 840),
      node('end', 'end', {}, 1120),
    ],
    edges: [
      { id: 'a', source: 'start', target: 'first-delay' },
      { id: 'b', source: 'first-delay', target: 'second-delay' },
      { id: 'c', source: 'second-delay', target: 'answer' },
      { id: 'd', source: 'answer', target: 'end' },
    ],
  }
  try {
  const response = await request.post('/api/runs', { data: { composition, input: {} } })
  expect(response.ok()).toBeTruthy()
  const run = await response.json()
  await openSession(page, run.id)
  await inspector(page)

  const graphNode = (id: string) => page.locator(`.run-canvas .vue-flow__node[data-id="${id}"] .flow-card`)
  await expect(graphNode('first-delay')).toHaveAttribute('data-execution-status', 'running')
  await expect(graphNode('first-delay')).toContainText('En cours')
  await expect(page.locator('[data-node=first-delay][data-status=running]')).toBeVisible()
  // User-controlled zoom must survive the stream of fresh run documents.
  await page.locator('.run-canvas .vue-flow__controls-zoomin').click()
  const viewport = page.locator('.run-canvas .vue-flow__transformationpane')
  const transform = await viewport.getAttribute('style')
  await writeFile(gates[0]!, 'continue')
  await expect(graphNode('first-delay')).toHaveAttribute('data-execution-status', 'completed')
  await expect(graphNode('second-delay')).toHaveAttribute('data-execution-status', 'running')
  await expect(graphNode('first-delay')).toContainText('Terminé')
  await writeFile(gates[1]!, 'continue')
  await expect(graphNode('second-delay')).toHaveAttribute('data-execution-status', 'completed')
  await expect(graphNode('answer')).toHaveAttribute('data-execution-status', 'waiting')
  await expect(graphNode('answer')).toContainText('Réponse attendue')
  await expect(page.locator('.composer-wait-prompt')).toBeVisible()
  await expect(viewport).toHaveAttribute('style', transform!)
  await expect(page.locator('.run-canvas .flow-card.active')).toHaveCount(0)
  expect(errors).toEqual([])
  } finally { await Promise.all(gates.map(path => writeFile(path, 'continue'))) }
})
