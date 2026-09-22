import { modelCatalogSchema } from '@zedflow/sdk'
import { test, expect, type Page, type Response, type Request } from '@playwright/test'
import type { Composition, FlowFile, Run, Workspace } from '@zedflow/sdk'
import { legacyTemplate as template } from './fixtures/templates'

function deferred() {
  let resolve!: () => void
  const promise = new Promise<void>(done => { resolve = done })
  return { promise, resolve }
}

function responseGate() {
  const started = deferred(), release = deferred()
  return { ready: started.promise, release: release.resolve, wait() { started.resolve(); return release.promise } }
}

async function afterResponse(response: Response) {
  await response.finished()
  // Let fetch's body continuation and Vue's DOM flush finish before checking
  // that a late acknowledgement did not replace the latest selection.
  await response.request().frame().page().evaluate(() => new Promise<void>(resolve => {
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
  }))
}

async function fixtures(page: Page) {
  const workspaces: Workspace[] = [
    { id: 'workspace-a', name: 'Workspace A', path: '/fixture/a', open: true },
    { id: 'workspace-b', name: 'Workspace B', path: '/fixture/b', open: true },
  ]
  const compositions: Record<string, Composition> = Object.fromEntries(workspaces.map(workspace => {
    const composition = template(true)
    composition.name = `Flow ${workspace.name.at(-1)}`
    return [workspace.id, composition]
  }))
  const hashes: Record<string, number> = { 'workspace-a': 0, 'workspace-b': 0 }
  const file = (id: string): FlowFile => ({
    key: `file-${id}`, id: compositions[id]!.id, name: compositions[id]!.name,
    path: `${workspaces.find(workspace => workspace.id === id)!.path}/.zedflow/flows/flow.rs`,
    scope: 'workspace', workspaceId: id, hash: `hash-${hashes[id]}`, diagnostics: [], composition: structuredClone(compositions[id]!),
  })
  const run = (workspaceId: string, id = `run-${workspaceId}`): Run => ({
    id, name: `Session ${workspaceId.at(-1)!.toUpperCase()}`, workspaceId,
    workspacePath: workspaces.find(workspace => workspace.id === workspaceId)!.path,
    composition: structuredClone(compositions[workspaceId]!), status: 'waiting', state: {}, messages: [], activities: [],
    timeline: [{ id: `${id}-message`, seq: 1, kind: 'message', role: 'assistant', text: `Réponse ${workspaceId.at(-1)!.toUpperCase()}` }],
    wait: { id: `${id}-wait`, kind: 'input', node: 'input', nodePath: 'input', config: { responseType: 'text', prompt: 'Votre réponse' } },
  })
  const runs = workspaces.map(workspace => run(workspace.id))
  const revisions=new Map<string,{fingerprint:string;revision:number}>()
  const gates: { save?: ReturnType<typeof responseGate>; flowsB?: ReturnType<typeof responseGate>; createRun?: ReturnType<typeof responseGate>; answer?: ReturnType<typeof responseGate> } = {}
  const saves: { workspaceId: string; expectedHash?: string; composition: Composition }[] = []
  const starts: Record<string, unknown>[] = [], answers: Record<string, unknown>[] = [], snapshots: string[] = []
  await page.route('**/api/**', async route => {
    const request = route.request(), url = new URL(request.url()), path = url.pathname.replace('/api', '')
    const workspaceId = url.searchParams.get('workspaceId') || 'workspace-a'
    const respond = (value: unknown) => route.fulfill({ contentType: 'application/json', body: JSON.stringify(value) })
    if (path === '/health') return respond({ defaultWorkspaceId: 'workspace-a', workspace: { host: 'fixture', path: '/fixture/a' } })
    if (path === '/workspaces') return respond(workspaces)
    if (path === '/context') return respond({ instructions: [], skills: [], diagnostics: [] })
    if (path === '/models') return respond(modelCatalogSchema.parse({ providers: [{ id: 'fixture', label: 'Fixture' }], models: [{ id: 'fixture', provider: 'fixture', label: 'Fixture', reasoningLevels: [] }] }))
    if (path === '/flows' && request.method() === 'POST') {
      const value = request.postDataJSON()
      saves.push(value)
      compositions[value.workspaceId] = structuredClone(value.composition)
      hashes[value.workspaceId] = (hashes[value.workspaceId] || 0) + 1
      const saved = file(value.workspaceId), gate = gates.save
      gates.save = undefined
      if (gate) await gate.wait()
      return respond(saved)
    }
    if (path === '/flows') {
      if (workspaceId === 'workspace-b' && gates.flowsB) {
        const gate = gates.flowsB; gates.flowsB = undefined
        await gate.wait()
      }
      return respond([file(workspaceId)])
    }
    if (path === '/runs' && request.method() === 'POST') {
      const value = request.postDataJSON(); starts.push(value)
      const created = run(value.workspaceId, 'created-run'); created.name = 'Nouvelle session créée'
      runs.push(created)
      if (gates.createRun) await gates.createRun.wait()
      return respond({id:created.id,workspaceId:created.workspaceId,revision:1})
    }
    if (path === '/runs') return respond(runs.filter(item => item.workspaceId === workspaceId))
    if (path.endsWith('/answer') && request.method() === 'POST') {
      answers.push({ workspaceId, ...request.postDataJSON() })
      if (gates.answer) await gates.answer.wait()
      return route.fulfill({ status: 409, contentType: 'application/json', body: JSON.stringify({ error: 'Cette attente a déjà reçu une réponse.' }) })
    }
    const item = runs.find(item => path === `/runs/${item.id}/snapshot` || path === `/runs/${item.id}`)
    if (item) {
      if (path.endsWith('/snapshot')) { snapshots.push(item.id);const fingerprint=JSON.stringify(item),previous=revisions.get(item.id);const revision=previous?(previous.fingerprint===fingerprint?previous.revision:previous.revision+1):1;revisions.set(item.id,{fingerprint,revision});return respond({type:'bootstrap',run:item,revision,cursor:revision}) }
      return respond(item)
    }
    if (path === '/rtc/config' || path.endsWith('/events')) return route.fulfill({ status: 503, body: 'HTTP fixture' })
    return respond({})
  })
  await page.goto('/')
  await expect(page.getByRole('button', { name: 'Choisir un flow', exact: true })).toContainText('Flow A')
  return { gates, saves, starts, answers, snapshots, runs }
}

test.afterEach(async ({ page }) => { await page.goto('about:blank') })

test('opening the first session waits for its identity before accepting a draft', async ({ page }) => {
  await fixtures(page)
  const gate = responseGate()
  await page.route(/\/api\/runs\/run-workspace-a(?:\?.*)?$/, async route => {
    if(new URL(route.request().url()).searchParams.get('workspaceId')!=='workspace-a')return route.fallback()
    await gate.wait()
    await route.fallback()
  })
  const session = page.locator('[data-session-id="run-workspace-a"]')
  await session.getByRole('button').first().click()
  await gate.ready
  const composer = page.locator('.composer textarea')
  await expect(composer).not.toBeEditable()
  const fill = composer.fill('Brouillon lié à la session A')
  gate.release()
  await fill
  await expect(session).toHaveClass(/chosen/)
  await expect(page.locator('.assistant-markdown')).toHaveText('Réponse A')
  await page.getByRole('button', { name: 'Conception', exact: true }).click()
  await page.getByRole('button', { name: 'Exécution', exact: true }).click()
  await expect(composer).toHaveValue('Brouillon lié à la session A')
})

test('a late scoped run identity cannot replace a later workspace session or its draft', async ({ page }) => {
  await fixtures(page)
  const gate = responseGate()
  let delayedRequest!: Request
  await page.route(/\/api\/runs\/run-workspace-a(?:\?.*)?$/, async route => {
    if(new URL(route.request().url()).searchParams.get('workspaceId')!=='workspace-a')return route.fallback()
    delayedRequest = route.request()
    await gate.wait()
    await route.fallback()
  })
  await page.locator('[data-session-id="run-workspace-a"]').getByRole('button').first().click()
  await gate.ready
  await expect(page.locator('.composer textarea')).not.toBeEditable()
  const sessionB = page.locator('[data-session-id="run-workspace-b"]')
  await sessionB.getByRole('button').first().click()
  await expect(page.locator('.assistant-markdown')).toHaveText('Réponse B')
  await page.locator('.composer textarea').fill('Brouillon du workspace B')
  const settled = delayedRequest.response()
  gate.release()
  const response = await settled
  if(response)await afterResponse(response)
  await expect(sessionB).toHaveClass(/chosen/)
  await expect(page.locator('.workspace-heading.active')).toContainText('Workspace B')
  await expect(page.locator('.assistant-markdown')).toHaveText('Réponse B')
  await expect(page.locator('.composer textarea')).toHaveValue('Brouillon du workspace B')
  await expect(page.locator('.composer textarea')).toBeEditable()
})

test('a delayed save preserves newer edits and advances the hash for the next save', async ({ page }) => {
  const fixture = await fixtures(page)
  await page.getByRole('button', { name: 'Conception', exact: true }).click()
  await page.locator('.flow-file-open').click()
  const title = page.getByRole('textbox', { name: 'Nom de composition', exact: true })
  await title.fill('Version envoyée')
  const gate = responseGate(); fixture.gates.save = gate
  await page.getByRole('button', { name: 'Enregistrer', exact: true }).click()
  await gate.ready
  await title.fill('Modification pendant la sauvegarde')
  const response = page.waitForResponse(response => response.url().endsWith('/api/flows') && response.request().method() === 'POST')
  gate.release(); await afterResponse(await response)
  await expect(title).toHaveValue('Modification pendant la sauvegarde')
  await expect(page.locator('.design-title')).toContainText('Modifications non enregistrées')
  await expect(page.getByRole('button', { name: 'Enregistrer', exact: true })).toBeEnabled()
  await page.getByRole('button', { name: 'Enregistrer', exact: true }).click()
  await expect.poll(() => fixture.saves.length).toBe(2)
  expect(fixture.saves[0]!.composition.name).toBe('Version envoyée')
  expect(fixture.saves[1]!.composition.name).toBe('Modification pendant la sauvegarde')
  expect(fixture.saves[1]!.expectedHash).toBe('hash-1')
})

test('opening a slower workspace session cannot replace a later session selection', async ({ page }) => {
  const fixture = await fixtures(page)
  const sessionA = page.locator('[data-session-id="run-workspace-a"]')
  await sessionA.getByRole('button').first().click()
  await expect(page.locator('.assistant-markdown')).toHaveText('Réponse A')
  const gate = responseGate(); fixture.gates.flowsB = gate
  await page.locator('[data-session-id="run-workspace-b"]').getByRole('button').first().click()
  await gate.ready
  await sessionA.getByRole('button').first().click()
  await expect(page.locator('.workspace-heading.active')).toContainText('Workspace A')
  await expect(page.locator('.statusbar')).not.toContainText('Ouverture de la session')
  const response = page.waitForResponse(response => new URL(response.url()).pathname === '/api/flows' && new URL(response.url()).searchParams.get('workspaceId') === 'workspace-b')
  gate.release(); await afterResponse(await response)
  await expect(sessionA).toHaveClass(/chosen/)
  await expect(page.locator('.assistant-markdown')).toHaveText('Réponse A')
  expect(fixture.snapshots).not.toContain('run-workspace-b')
})

test('a delayed run creation keeps its original workspace without reopening it after navigation', async ({ page }) => {
  const fixture = await fixtures(page)
  const gate = responseGate(); fixture.gates.createRun = gate
  await page.locator('.composer textarea').fill('Créer la session dans A')
  await page.locator('.composer textarea').press('Enter')
  await gate.ready
  await page.locator('[data-session-id="run-workspace-b"]').getByRole('button').first().click()
  await expect(page.locator('.assistant-markdown')).toHaveText('Réponse B')
  const response = page.waitForResponse(response => new URL(response.url()).pathname === '/api/runs' && response.request().method() === 'POST')
  gate.release(); await afterResponse(await response)
  await expect(page.locator('[data-session-id="run-workspace-b"]')).toHaveClass(/chosen/)
  await expect(page.locator('.workspace-heading.active')).toContainText('Workspace B')
  await expect(page.locator('.assistant-markdown')).toHaveText('Réponse B')
  expect(fixture.starts[0]!.workspaceId).toBe('workspace-a')
  expect(fixture.snapshots).not.toContain('created-run')
})

test('an answer rejected after leaving and reopening a session restores its draft and original wait', async ({ page }) => {
  const fixture = await fixtures(page)
  const sessionA = page.locator('[data-session-id="run-workspace-a"]')
  await sessionA.getByRole('button').first().click()
  await expect(page.locator('.assistant-markdown')).toHaveText('Réponse A')
  const gate = responseGate(); fixture.gates.answer = gate
  const text = 'Réponse conservée malgré le conflit et la navigation'
  await page.locator('.composer textarea').fill(text)
  await page.locator('.composer textarea').press('Enter')
  await gate.ready
  expect(fixture.answers).toEqual([{ workspaceId: 'workspace-a', waitId: 'run-workspace-a-wait', value: text }])
  await page.locator('[data-session-id="run-workspace-b"]').getByRole('button').first().click()
  await expect(page.locator('.assistant-markdown')).toHaveText('Réponse B')
  await page.locator('.composer textarea').fill('Brouillon indépendant de B')
  // Another client answered A while this tab was away. Its pending request
  // must restore the old draft, without silently associating it to this wait.
  fixture.runs[0]!.wait = { ...fixture.runs[0]!.wait!, id: 'replacement-wait', config: { responseType: 'text', prompt: 'Nouvelle question dans A' } }
  await sessionA.getByRole('button').first().click()
  await expect(page.locator('.composer-wait-prompt')).toHaveText('Nouvelle question dans A')
  await expect(page.locator('.composer textarea')).toHaveValue('')
  const response = page.waitForResponse(response => new URL(response.url()).pathname === '/api/runs/run-workspace-a/answer')
  gate.release(); const rejected = await response
  expect(rejected.status()).toBe(409)
  await afterResponse(rejected)
  await expect(page.locator('.composer textarea')).toHaveValue(text)
  await expect(page.locator('.stale-draft-notice')).toContainText('Ce brouillon répondait à une attente précédente.')
  await expect(page.locator('.composer button[type="submit"]')).toBeDisabled()
  await page.locator('[data-session-id="run-workspace-b"]').getByRole('button').first().click()
  await expect(page.locator('.composer textarea')).toHaveValue('Brouillon indépendant de B')
})
