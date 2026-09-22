import { test, expect, type Page, type APIRequestContext, type Locator } from '@playwright/test'
import type { ContextBlock, ContextStrategy, ContextType } from '@zedflow/sdk'

const textType: ContextType = { kind: 'text' }
const literal = (value: string) => ({ kind: 'literal' as const, dataType: textType, value })
function gate() {
  let release!: () => void
  const promise = new Promise<void>(resolve => { release = resolve })
  return { promise, release }
}
async function openStrategy(page: Page, request: APIRequestContext, strategy: ContextStrategy) {
  const workspaceId = (await (await request.get('/api/health')).json()).defaultWorkspaceId
  const response = await request.post('/api/context-strategies', { data: { workspaceId, strategy } })
  expect(response.ok(), await response.text()).toBeTruthy()
  const file = await response.json()
  await page.goto('/')
  await expect(page.getByText('Daemon connecté', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Conception', exact: true }).click()
  await page.getByRole('navigation', { name: 'Espace de conception' }).getByRole('button', { name: 'Contexte', exact: true }).click()
  await page.locator(`[data-context-key="${file.key}"]`).click()
  return { panel: page.getByRole('region', { name: 'Studio de contexte', exact: true }), workspaceId, file }
}
async function setFixtures(page: Page, panel: Locator, resources: object) {
  await panel.getByRole('button', { name: 'Voir les valeurs', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: 'Données d’essai du contexte', exact: true })
  await dialog.getByRole('button', { name: 'JSON', exact: true }).click()
  await dialog.getByLabel('Valeurs JSON des sources', { exact: true }).fill(JSON.stringify(resources))
  await dialog.getByRole('button', { name: 'Appliquer les valeurs JSON', exact: true }).click()
  await dialog.getByRole('button', { name: 'Fermer', exact: true }).click()
}
function simpleStrategy(name: string): ContextStrategy {
  return { version: 2, id: `preview-${crypto.randomUUID()}`, name, requirements: {}, capabilities: [], program: [{ kind: 'emit', id: 'editable', role: 'data', format: 'text', value: literal('Aperçu validé') }] }
}

test('late automatic previews preserve the last valid result, edits and the active catalogue', async ({ page, request }) => {
  const strategy = simpleStrategy('Aperçu automatique ordonné')
  const { panel } = await openStrategy(page, request, strategy)
  await expect(panel.locator('.ctx-preview-fragment')).toContainText('Aperçu validé')
  const first = gate(), second = gate(), firstStarted = gate(), secondStarted = gate()
  // Delay real daemon responses; their status and contents remain unchanged.
  await page.route('**/api/context-strategies/preview', async route => {
    const data = route.request().postDataJSON()
    const value = data.selection?.strategy?.program?.[0]?.value?.value
    if (data.selection?.strategy?.id !== strategy.id || !['Intermédiaire A', 'Édition B'].includes(value)) { await route.continue(); return }
    const response = await route.fetch()
    if (value === 'Intermédiaire A') { firstStarted.release(); await first.promise }
    else { secondStarted.release(); await second.promise }
    await route.fulfill({ response })
  })
  try {
    const input = panel.locator('[data-context-block="editable"]').getByRole('textbox', { name: /^Contenu du fragment/ })
    await input.fill('Intermédiaire A')
    await firstStarted.promise
    await expect(input).toBeEditable()
    await input.fill('Édition B')
    await expect(panel.locator('.ctx-preview-fragment')).toContainText('Aperçu validé')
    await panel.getByRole('button', { name: 'Catalogues de types', exact: true }).click()
    const editor = panel.getByRole('region', { name: 'Éditeur de types', exact: true })
    await editor.getByRole('button', { name: 'Nouveau catalogue de types', exact: true }).click()
    await editor.getByLabel('Identifiant du catalogue de types', { exact: true }).fill('catalogue-encore-en-edition')
    first.release()
    await secondStarted.promise
    await expect(panel.getByRole('button', { name: 'Catalogues de types', exact: true })).toHaveAttribute('aria-pressed', 'true')
    await expect(editor.getByLabel('Identifiant du catalogue de types', { exact: true })).toHaveValue('catalogue-encore-en-edition')
    await panel.getByRole('button', { name: 'Stratégie', exact: true }).click()
    await expect(input).toHaveValue('Édition B')
    await expect(panel.locator('.ctx-preview-fragment')).toContainText('Aperçu validé')
    await expect(panel.locator('.ctx-preview-fragment')).not.toContainText('Intermédiaire A')
    second.release()
    await expect(panel.locator('.ctx-preview-fragment')).toContainText('Édition B')
    await expect(panel.locator('.ctx-preview-footer')).toContainText('Aperçu à jour')
  } finally { first.release(); second.release() }
})

test('reloading a draft cancels a save waiting for its obsolete preview', async ({ page, request }) => {
  const strategy = simpleStrategy('Rechargement pendant aperçu')
  const { panel, file, workspaceId } = await openStrategy(page, request, strategy)
  await expect(panel.locator('.ctx-preview-fragment')).toContainText('Aperçu validé')
  const responseGate = gate(), started = gate()
  await page.route('**/api/context-strategies/preview', async route => {
    const data = route.request().postDataJSON()
    if (data.selection?.strategy?.id !== strategy.id || data.selection?.strategy?.program?.[0]?.value?.value !== 'Brouillon abandonné') { await route.continue(); return }
    const response = await route.fetch()
    started.release(); await responseGate.promise
    await route.fulfill({ response })
  })
  try {
    const input = panel.locator('[data-context-block="editable"]').getByRole('textbox', { name: /^Contenu du fragment/ })
    await input.fill('Brouillon abandonné')
    await started.promise
    await panel.getByRole('button', { name: 'Enregistrer la stratégie', exact: true }).click()
    await panel.getByRole('button', { name: 'Recharger la stratégie depuis le disque', exact: true }).click()
    await page.getByRole('dialog', { name: 'Recharger la stratégie', exact: true }).getByRole('button', { name: 'Recharger depuis le disque', exact: true }).click()
    await expect(input).toHaveValue('Aperçu validé')
    responseGate.release()
    await expect(panel.locator('.ctx-preview-fragment')).toContainText('Aperçu validé')
    await expect(panel.locator('.ctx-preview-footer')).toContainText('Aperçu à jour')
    const retained = await (await request.get(`/api/context-strategies/${file.key}?workspaceId=${workspaceId}`)).json()
    expect(retained.hash).toBe(file.hash)
    expect(retained.strategy.program[0].value).toEqual(literal('Aperçu validé'))
  } finally { responseGate.release() }
})

test('reloading clears deletion undo and retains valid blocks nested beyond 24 levels', async ({ page, request }) => {
  let nested: ContextBlock = { kind: 'emit', id: 'deep-text', role: 'data', format: 'text', value: literal('Contenu profond conservé') }
  for (let depth = 25; depth >= 0; depth--) nested = { kind: 'group', id: `level-${depth}`, label: `Niveau ${depth + 1}`, items: [nested] }
  const strategy = simpleStrategy('Rechargement de blocs imbriqués')
  strategy.program.push(nested)
  const { panel } = await openStrategy(page, request, strategy)
  await expect(panel.locator('[data-context-block="deep-text"]')).toHaveCount(1)
  await panel.locator('[data-context-block="editable"]').getByRole('button', { name: 'Actions du bloc 1', exact: true }).click()
  await page.getByRole('menuitem', { name: 'Supprimer ce bloc', exact: true }).click()
  await expect(panel.locator('[data-context-block="editable"]')).toHaveCount(0)
  const undo = panel.getByRole('button', { name: 'Annuler la suppression du bloc', exact: true })
  await expect(undo).toHaveCount(1)
  await panel.getByRole('button', { name: 'Recharger la stratégie depuis le disque', exact: true }).click()
  await page.getByRole('dialog', { name: 'Recharger la stratégie', exact: true }).getByRole('button', { name: 'Recharger depuis le disque', exact: true }).click()
  await expect(undo).toHaveCount(0)
  await expect(panel.locator('[data-context-block="editable"]')).toHaveCount(1)
  await expect(panel.locator('[data-context-block="deep-text"]')).toHaveCount(1)
  const ids = await panel.locator('[data-context-block]').evaluateAll(elements => elements.map(element => element.getAttribute('data-context-block')))
  expect(new Set(ids).size).toBe(ids.length)
})

test('a late manual preview respects mobile navigation and loop occurrences select their source block', async ({ page, request }) => {
  const strategy: ContextStrategy = {
    version: 2, id: `preview-navigation-${crypto.randomUUID()}`, name: 'Navigation des occurrences',
    requirements: { question: textType, documents: { kind: 'list', item: textType } }, capabilities: [], program: [
      { kind: 'emit', id: 'question', role: 'data', format: 'text', value: { kind: 'resource', name: 'question' } },
      { kind: 'forEach', id: 'loop', value: { kind: 'resource', name: 'documents' }, item: 'document', items: [
        { kind: 'emit', id: 'document-text', role: 'data', format: 'text', value: { kind: 'variable', name: 'document' } },
      ] },
    ],
  }
  const { panel } = await openStrategy(page, request, strategy)
  await setFixtures(page, panel, { question: 'Question active', documents: ['Premier document', 'Second document'] })
  await expect(panel.locator('[data-origin-block="document-text"]')).toHaveCount(2)
  await page.setViewportSize({ width: 720, height: 900 })
  await page.getByRole('button', { name: 'Fermer la navigation', exact: true }).click()
  const tabs = panel.getByRole('navigation', { name: 'Panneaux du studio', exact: true })
  await tabs.getByRole('button', { name: 'Aperçu', exact: true }).click()
  const responseGate = gate(), started = gate()
  await page.route('**/api/context-strategies/preview', async route => {
    if (route.request().postDataJSON().selection?.strategy?.id !== strategy.id) { await route.continue(); return }
    const response = await route.fetch()
    started.release(); await responseGate.promise
    await route.fulfill({ response })
  }, { times: 1 })
  try {
    await panel.getByRole('button', { name: 'Prévisualiser', exact: true }).click()
    await started.promise
    await tabs.getByRole('button', { name: 'Sources', exact: true }).click()
    responseGate.release()
    await expect(panel.locator('.ctx-preview-scroll')).toHaveAttribute('aria-busy', 'false')
    await expect(tabs.getByRole('button', { name: 'Sources', exact: true })).toHaveAttribute('aria-pressed', 'true')
    await panel.locator('[data-resource="question"]').getByRole('button', { name: 'Déplier la source question', exact: true }).click()
    await expect(panel.locator('.ctx-layout')).toHaveAttribute('data-selected-source', 'question')
    await tabs.getByRole('button', { name: 'Aperçu', exact: true }).click()
    await panel.locator('[data-origin-block="document-text"]').last().getByRole('button', { name: /^Voir le bloc du fragment/ }).click()
    await expect(tabs.getByRole('button', { name: 'Programme', exact: true })).toHaveAttribute('aria-pressed', 'true')
    await expect(panel.locator('[data-context-block="document-text"]')).toHaveClass(/selected/)
    await expect(panel.locator('.ctx-layout')).toHaveAttribute('data-selected-source', 'documents')
    const ids = await panel.locator('[data-origin-block="document-text"]').evaluateAll(elements => elements.map(element => element.getAttribute('data-fragment-id')))
    expect(new Set(ids).size).toBe(2)
  } finally { responseGate.release() }
})

test('message projection and tool exchange compose real typed messages without grants or execution', async ({ page, request }) => {
  const object: ContextType = { kind: 'record', fields: {} }
  const strategy: ContextStrategy = {
    version: 2, id: `message-composition-${crypto.randomUUID()}`, name: 'Messages composés visuellement',
    types: {
      ToolCall: { kind: 'record', fields: { id: textType, name: textType, arguments: object } },
      ToolResult: { kind: 'record', fields: { callId: textType, status: textType, content: textType } },
    },
    requirements: { question: textType, question2: textType, call: { kind: 'named', name: 'ToolCall' }, call2: { kind: 'named', name: 'ToolCall' }, result: { kind: 'named', name: 'ToolResult' }, result2: { kind: 'named', name: 'ToolResult' } },
    capabilities: [{ id: 'read', input: object, output: textType }],
    program: [{ kind: 'emit', id: 'question', role: 'data', format: 'text', value: { kind: 'resource', name: 'question' } }],
  }
  const { panel, workspaceId, file } = await openStrategy(page, request, strategy)
  await setFixtures(page, panel, { question: 'Quel est le contenu du document ?', question2: 'Nouvelle question insérée', call: { id: 'call-17', name: 'read', arguments: { path: 'docs/example.md' } }, result: { callId: 'call-17', status: 'erreur', content: 'Document indisponible' }, call2: { id: 'call-27', name: 'read', arguments: { path: 'docs/replacement.md' } }, result2: { callId: 'call-27', status: 'ok', content: 'Document remplacé' } })
  await expect(panel.locator('.ctx-preview-fragment')).toContainText('Quel est le contenu du document ?')
  const fragment = panel.locator('[data-context-block="question"]')
  await fragment.getByRole('button', { name: 'Configurer le rôle et la représentation', exact: true }).click()
  const projection = fragment.getByLabel('Projection du texte en message', { exact: true })
  await projection.selectOption('user')
  await expect(panel.locator('.ctx-result-message')).toContainText('Utilisateur')
  await expect(panel.locator('.ctx-result-message')).toContainText('Quel est le contenu du document ?')
  await projection.selectOption('model')
  await expect(panel.locator('.ctx-result-message')).toContainText('Assistant')
  await projection.selectOption('text')
  await expect(panel.locator('.ctx-result-message')).toHaveCount(0)
  await expect(panel.locator('.ctx-preview-fragment')).toContainText('Quel est le contenu du document ?')
  await projection.selectOption('user')
  await fragment.getByRole('button', { name: /^Choisir un champ pour Contenu du fragment/ }).click()
  await expect(page.getByRole('menuitem', { name: /^call ToolCall$/ })).toBeDisabled()
  await page.getByRole('menuitem', { name: /^question2\b/ }).focus()
  await page.keyboard.press('Enter')
  await expect(panel.locator('.ctx-result-message')).toContainText('Nouvelle question insérée')
  await expect(panel.locator('.ctx-result-message')).toContainText('Utilisateur')
  await panel.locator('.ctx-program-panel > .ctx-panel-heading').getByRole('button', { name: 'Ajouter un bloc', exact: true }).click()
  await page.getByRole('menuitem', { name: /^Échange d’outil/ }).click()
  const exchange = page.getByRole('dialog', { name: 'Composer un échange d’outil', exact: true })
  await exchange.getByLabel('Source de l’appel enregistré', { exact: true }).selectOption('call')
  await exchange.getByLabel('Source du résultat enregistré', { exact: true }).selectOption('result')
  await exchange.getByRole('button', { name: 'Ajouter l’échange au contexte', exact: true }).click()
  await expect(panel.locator('[data-tool-call-id="call-17"]')).toContainText('docs/example.md')
  await expect(panel.locator('[data-tool-result-id="call-17"]')).toContainText('Document indisponible')
  await expect(panel.locator('.ctx-diagnostics')).toHaveCount(0)
  const toolSockets = panel.locator('.ctx-tool-sockets .ctx-expression-socket')
  await expect(toolSockets).toHaveCount(2)
  await toolSockets.first().getByRole('button', { name: /^Choisir un champ pour Appel enregistré/ }).click()
  await expect(page.getByRole('menuitem', { name: /^result2 ToolResult$/ })).toBeDisabled()
  await page.getByRole('menuitem', { name: /^call2 ToolCall$/ }).focus()
  await page.keyboard.press('Enter')
  await panel.locator('[data-resource="result2"] .ctx-source-disclosure').dragTo(toolSockets.last())
  await expect(panel.locator('[data-tool-call-id="call-27"]')).toContainText('docs/replacement.md')
  await expect(panel.locator('[data-tool-result-id="call-27"]')).toContainText('Document remplacé')
  await expect(panel.locator('.ctx-diagnostics')).toHaveCount(0)
  const previewRequest = page.waitForRequest(request => request.url().endsWith('/api/context-strategies/preview') && request.postDataJSON().selection?.strategy?.id === strategy.id)
  await panel.getByRole('button', { name: 'Prévisualiser', exact: true }).click()
  expect((await previewRequest).postDataJSON().grantedCapabilities).toEqual([])
  await panel.getByRole('button', { name: 'Enregistrer la stratégie', exact: true }).click()
  await expect(panel.locator('.ctx-alert[role=status]')).toContainText('Stratégie enregistrée en Rust')
  const persisted = await (await request.get(`/api/context-strategies/${file.key}?workspaceId=${workspaceId}`)).json()
  expect(persisted.strategy.capabilities).toEqual(strategy.capabilities)
  expect(persisted.strategy.types).toEqual(strategy.types)
  expect(persisted.strategy.program[0].format).toBe('adkMessages')
  expect(persisted.strategy.program[0].value.items[0].fields.parts.items[0].fields.text).toEqual({ kind: 'resource', name: 'question2' })
  expect(persisted.strategy.program[1].format).toBe('adkMessages')
  expect(persisted.strategy.program[1].value.items[0].fields.parts.items[0].fields.name.value).toEqual({ kind: 'resource', name: 'call2' })
  expect(persisted.strategy.program[1].value.items[1].fields.parts.items[0].fields.id.value).toEqual({ kind: 'resource', name: 'result2' })
  expect(persisted.source).not.toContain('Document indisponible')
  expect(persisted.source).not.toContain('docs/example.md')
  expect(persisted.source).not.toContain('Document remplacé')
  expect(persisted.source).not.toContain('docs/replacement.md')
})
