import { test, expect, type Page, type Locator, type APIRequestContext } from '@playwright/test'
import type { ContextFile, ContextPreview, ContextStrategy } from '@zedflow/sdk'

const keys = ['workspace-default', 'conversation-default', 'tools-default', 'working-system-context']
const namedSources = { instructions: 'WorkspaceInstructions', skills: 'SkillCatalog', files: 'SelectedFiles', input: 'UserInput' }
let workspaceId: string

test.beforeAll(async ({ request }) => {
  workspaceId = (await (await request.get('/api/health')).json()).defaultWorkspaceId
  // Install the fourth canonical starter in the isolated fixture workspace.
  const response = await request.post('/api/examples/working-system', { data: { workspaceId, workingDirectory: '../workspace-b' } })
  expect(response.ok(), await response.text()).toBeTruthy()
})

async function readStarter(request: APIRequestContext, key: string): Promise<ContextFile> {
  const response = await request.get(`/api/context-strategies/${key}?workspaceId=${workspaceId}`)
  expect(response.ok(), await response.text()).toBeTruthy()
  return response.json()
}
async function openStarter(page: Page, key: string) {
  await page.goto('/')
  await expect(page.getByText('Daemon connecté', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Conception', exact: true }).click()
  await page.getByRole('navigation', { name: 'Espace de conception', exact: true }).getByRole('button', { name: 'Contexte', exact: true }).click()
  await page.locator(`[data-context-key="${key}"]`).click()
  return page.getByRole('region', { name: 'Studio de contexte', exact: true })
}
async function choosePreset(page: Page, panel: Locator, key: string, preset: 'success' | 'failure' | 'empty'): Promise<ContextPreview> {
  const response = page.waitForResponse(response => {
    if (!response.url().endsWith('/api/context-strategies/preview')) return false
    const data = response.request().postDataJSON()
    return data.selection?.strategy?.id === key && (preset === 'empty' ? !Object.keys(data.resources || {}).length : data.resources?.history?.[3]?.parts?.[0]?.functionResponse?.response?.status === (preset === 'failure' ? 'erreur' : 'succès'))
  })
  await panel.getByLabel('Jeu de données d’essai', { exact: true }).selectOption(preset)
  const actual = await response
  expect(actual.ok(), await actual.text()).toBeTruthy()
  expect(actual.request().postDataJSON().grantedCapabilities).toEqual([])
  return actual.json()
}
async function fixtureValues(page: Page, panel: Locator): Promise<Record<string, any>> {
  await panel.getByRole('button', { name: 'Voir les valeurs', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: 'Données d’essai du contexte', exact: true })
  await dialog.getByRole('button', { name: 'JSON', exact: true }).click()
  const resources = JSON.parse(await dialog.getByLabel('Valeurs JSON des sources', { exact: true }).inputValue())
  await dialog.getByRole('button', { name: 'Fermer', exact: true }).click()
  return resources
}
async function applyFixtureValues(page: Page, panel: Locator, resources: Record<string, unknown>) {
  await panel.getByRole('button', { name: 'Voir les valeurs', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: 'Données d’essai du contexte', exact: true })
  await dialog.getByRole('button', { name: 'JSON', exact: true }).click()
  await dialog.getByLabel('Valeurs JSON des sources', { exact: true }).fill(JSON.stringify(resources))
  await dialog.getByRole('button', { name: 'Appliquer les valeurs JSON', exact: true }).click()
  await dialog.getByRole('button', { name: 'Fermer', exact: true }).click()
}
function historyBlock(strategy: ContextStrategy) {
  expect(strategy.program.map(block => block.id)).toEqual(['instructions', 'skills', 'files', 'conversation'])
  expect(strategy.program.slice(0, 3).map(block => block.kind)).toEqual(['emit', 'emit', 'emit'])
  const condition = strategy.program[3]
  if (condition.kind !== 'if') throw new Error('The starter must explicitly branch on history presence')
  expect(condition.condition).toEqual({ kind: 'present', value: { kind: 'resource', name: 'history' } })
  const loop = condition.then[0]
  if (loop.kind !== 'forEach') throw new Error('The starter must iterate over its typed conversation')
  expect(loop.value).toEqual({ kind: 'resource', name: 'history' })
  expect(loop.item).toBe('message')
  const block = loop.items[0]
  if (block.kind !== 'emit') throw new Error('Each conversation item must produce a context fragment')
  expect(block.format).toBe('adkMessages')
  expect(block.value).toEqual({ kind: 'list', itemType: { kind: 'named', name: 'ConversationMessage' }, items: [{ kind: 'variable', name: 'message' }] })
  expect(condition.else).toEqual([{ kind: 'emit', id: 'input', role: 'data', format: 'text', value: { kind: 'resource', name: 'input' } }])
  return block
}

for (const key of keys) test(`${key} exposes named sources and preserves each ADK message when reopened`, async ({ page, request }) => {
  const file = await readStarter(request, key)
  expect(file.diagnostics).toEqual([])
  const strategy = file.strategy!
  expect(strategy.id).toBe(key)
  for (const [source, name] of Object.entries(namedSources)) {
    expect(strategy.requirements[source]).toEqual({ kind: 'named', name })
    expect(strategy.types?.[name]).toEqual({ kind: 'text' })
  }
  expect(strategy.requirements.history).toEqual({ kind: 'list', item: { kind: 'named', name: 'ConversationMessage' } })
  expect(strategy.types?.ConversationMessage).toEqual({ kind: 'record', fields: { role: { kind: 'text' }, parts: { kind: 'list', item: { kind: 'record', fields: {} } } } })
  const message = historyBlock(strategy)
  const panel = await openStarter(page, key)
  await expect(panel.locator('[data-resource]')).toHaveCount(5)
  expect(await panel.locator('[data-resource]').evaluateAll(elements => elements.map(element => element.getAttribute('data-resource')))).toEqual(['instructions', 'skills', 'files', 'input', 'history'])
  await expect(panel.locator('.ctx-declared-title strong')).toHaveText(['Instructions du workspace', 'Catalogue et skills actifs', 'Fichiers sélectionnés', 'Saisie utilisateur', 'Messages de conversation'])
  const preview = await choosePreset(page, panel, key, 'failure')
  expect(preview.evaluation.complete).toBe(true)
  expect(preview.evaluation.diagnostics).toEqual([])
  expect(preview.evaluation.needs).toEqual([])
  expect(preview.evaluation.items).toHaveLength(7)
  const outputIds = preview.evaluation.items.map(item => item.id)
  expect(new Set(outputIds).size).toBe(7)
  expect(preview.evaluation.items.map(item => preview.evaluation.trace?.find(entry => entry.id === item.id)?.blockId || item.id)).toEqual(['instructions', 'skills', 'files', ...Array(4).fill(message.id)])
  await expect(panel.locator('[data-origin-block]')).toHaveCount(7)
  await expect(panel.locator('[data-tool-call-id="17"]')).toContainText('docs/terms.md')
  await expect(panel.locator('[data-tool-result-id="17"]')).toContainText('Fichier indisponible')
  await expect(panel.locator('.ctx-diagnostics')).toHaveCount(0)
  const resources = await fixtureValues(page, panel)
  expect(resources.history.map((item: { role: string }) => item.role)).toEqual(['user', 'model', 'model', 'function'])
  expect(resources.history[2].parts[0]).toEqual({ id: '17', name: 'read', args: { path: 'docs/terms.md' } })
  expect(resources.history[3].parts[0].id).toBe(resources.history[2].parts[0].id)

  const emission = panel.locator(`[data-context-block="${message.id}"]`)
  await emission.getByRole('button', { name: /^Configurer l’expression : Contenu du fragment/ }).click()
  await emission.getByRole('button', { name: 'Choisir un champ pour Élément 1', exact: true }).click()
  await expect(page.getByRole('menuitem', { name: /^message ConversationMessage$/ })).toBeEnabled()
  await expect(page.getByRole('menuitem', { name: /^message · role\b/ })).toBeDisabled()
  await expect(page.getByRole('menuitem', { name: /^message · parts\b/ })).toBeDisabled()
  await page.keyboard.press('Escape')
  await emission.getByRole('button', { name: 'Réduire les détails', exact: true }).click()
  await panel.locator(`[data-origin-block="${message.id}"]`).last().getByRole('button', { name: /^Voir le bloc du fragment/ }).click()
  await expect(emission).toHaveClass(/selected/)
  await expect(panel.locator('.ctx-layout')).toHaveAttribute('data-selected-source', 'history')

  await page.locator(`[data-context-key="${key === keys[0] ? keys[1] : keys[0]}"]`).click()
  await page.locator(`[data-context-key="${key}"]`).click()
  await expect(panel.getByLabel('Jeu de données d’essai', { exact: true })).toHaveValue('failure')
  await expect(panel.locator('[data-tool-result-id="17"]')).toContainText('Fichier indisponible')
  expect(await panel.locator('[data-fragment-id]').evaluateAll(elements => elements.map(element => element.getAttribute('data-fragment-id')))).toEqual(outputIds)
  expect(await fixtureValues(page, panel)).toEqual(resources)
  const reopened = await readStarter(request, key)
  expect(reopened.hash).toBe(file.hash)
  expect(reopened.source).toBe(file.source)
})

test('starter fixture presets preserve the distinction between absent and empty conversation history', async ({ page }) => {
  const panel = await openStarter(page, 'conversation-default')
  await choosePreset(page, panel, 'conversation-default', 'success')
  await expect(panel.locator('[data-tool-result-id="17"]')).toContainText('Trois définitions')
  const resources = await fixtureValues(page, panel)
  await applyFixtureValues(page, panel, { ...resources, history: [] })
  await expect(panel.locator('[data-origin-block]')).toHaveCount(3)
  await expect(panel.locator('[data-origin-block="input"]')).toHaveCount(0)
  await expect(panel.locator('.ctx-preview-footer')).toContainText('Aperçu à jour')
  expect((await fixtureValues(page, panel)).history).toEqual([])
  delete resources.history
  await applyFixtureValues(page, panel, resources)
  await expect(panel.locator('[data-origin-block]')).toHaveCount(4)
  await expect(panel.locator('[data-origin-block="input"]')).toContainText(resources.input)
  expect(Object.hasOwn(await fixtureValues(page, panel), 'history')).toBe(false)
  const empty = await choosePreset(page, panel, 'conversation-default', 'empty')
  expect(empty.evaluation.items).toEqual([])
  expect(empty.evaluation.needs.map(need => need.resource).sort()).toEqual(['files', 'input', 'instructions', 'skills'])
  expect(await fixtureValues(page, panel)).toEqual({})
})
