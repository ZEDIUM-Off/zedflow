import { test, expect, type Page, type Locator } from '@playwright/test'
import type { ContextStrategy } from '@zedflow/sdk'

const text = (value: string) => ({ kind: 'literal' as const, dataType: { kind: 'text' as const }, value })

async function dragBlock(page: Page, source: Locator, target: Locator) {
  await target.scrollIntoViewIfNeeded()
  await source.hover()
  const from = await source.boundingBox(), to = await target.boundingBox()
  expect(from).not.toBeNull(); expect(to).not.toBeNull()
  await page.mouse.move(from!.x + from!.width / 2, from!.y + from!.height / 2)
  await page.mouse.down()
  // Cross the native drag threshold before travelling to the destination.
  await page.mouse.move(from!.x + from!.width / 2 + 12, from!.y + from!.height / 2 + 12, { steps: 4 })
  await page.mouse.move(to!.x + to!.width / 2, to!.y + to!.height / 2, { steps: 12 })
  await page.mouse.move(to!.x + to!.width / 2, to!.y + to!.height / 2 + 1)
  await page.mouse.up()
}

test('typed field sockets preserve incompatible drops, local scopes and nested program order', async ({ page, request }) => {
  const workspaceId = (await (await request.get('/api/health')).json()).defaultWorkspaceId
  const strategy: ContextStrategy = {
    version: 2, id: `structured-program-${crypto.randomUUID()}`, name: 'Programme structuré vérifié',
    requirements: {
      document: { kind: 'record', fields: { title: { kind: 'text' }, count: { kind: 'number' } } },
      documents: { kind: 'list', item: { kind: 'record', fields: { title: { kind: 'text' } } } },
    }, capabilities: [], program: [
      { kind: 'emit', id: 'heading', role: 'instruction', format: 'text', value: text('Ancien titre') },
      { kind: 'if', id: 'choice', condition: { kind: 'present', value: { kind: 'resource', name: 'document' } }, then: [
        { kind: 'emit', id: 'detail', role: 'data', format: 'text', value: text('Dans Alors') },
      ], else: [] },
      { kind: 'forEach', id: 'documents-loop', value: { kind: 'resource', name: 'documents' }, item: 'documentItem', items: [
        { kind: 'emit', id: 'document-title', role: 'data', format: 'text', value: { kind: 'field', value: { kind: 'variable', name: 'documentItem' }, field: 'title' } },
      ] },
    ],
  }
  const saved = await request.post('/api/context-strategies', { data: { workspaceId, strategy } })
  expect(saved.ok(), await saved.text()).toBeTruthy()
  const file = await saved.json()
  await page.goto('/')
  await expect(page.getByText('Daemon connecté', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Conception', exact: true }).click()
  await page.getByRole('navigation', { name: 'Espace de conception' }).getByRole('button', { name: 'Contexte', exact: true }).click()
  await page.locator(`[data-context-key="${file.key}"]`).click()
  const panel = page.getByRole('region', { name: 'Studio de contexte', exact: true })
  await panel.getByRole('button', { name: 'Voir les valeurs', exact: true }).click()
  const fixtures = page.getByRole('dialog', { name: 'Données d’essai du contexte', exact: true })
  await fixtures.getByRole('button', { name: 'JSON', exact: true }).click()
  await fixtures.getByLabel('Valeurs JSON des sources', { exact: true }).fill(JSON.stringify({ document: { title: 'Titre source', count: 4 }, documents: [{ title: 'Alpha' }, { title: 'Bêta' }] }))
  await fixtures.getByRole('button', { name: 'Appliquer les valeurs JSON', exact: true }).click()
  await fixtures.getByRole('button', { name: 'Fermer', exact: true }).click()

  const heading = panel.locator('[data-context-block="heading"]')
  const source = panel.locator('[data-resource="document"]')
  await source.getByRole('button', { name: 'Déplier la source document', exact: true }).click()
  const fieldChoice = heading.getByRole('button', { name: /^Choisir un champ pour Contenu du fragment/ })
  await fieldChoice.focus()
  await page.keyboard.press('Enter')
  await expect(page.getByRole('menuitem', { name: /^document · count\b/ })).toBeDisabled()
  await page.getByRole('menuitem', { name: /^document · title\b/ }).focus()
  await page.keyboard.press('Enter')
  await expect(heading.locator('.ctx-source-token')).toHaveText('document · title')
  await source.getByRole('button', { name: 'Insérer document · count', exact: true }).dragTo(heading.locator('.ctx-expression-socket'))
  await expect(heading.getByRole('alert')).toContainText('ne convient pas')
  await expect(heading.locator('.ctx-source-token')).toHaveText('document · title')

  const detail = panel.locator('[data-context-block="detail"]')
  await source.getByRole('button', { name: 'Insérer document · title', exact: true }).dragTo(detail.locator('.ctx-expression-socket'))
  await expect(detail.locator('.ctx-source-token')).toHaveText('document · title')
  const loop = panel.locator('[data-context-block="documents-loop"]')
  const item = loop.locator('[data-context-block="document-title"]')
  await item.getByRole('button', { name: /^Choisir un champ pour Contenu du fragment/ }).click()
  await expect(page.getByRole('menuitem', { name: /^documentItem Objet$/ })).toBeDisabled()
  await expect(page.getByRole('menuitem', { name: /^documentItem · title\b/ })).toBeEnabled()
  await page.keyboard.press('Escape')

  const program = panel.locator('.ctx-program-panel > fieldset > .ctx-block-list')
  await dragBlock(page, detail.getByRole('button', { name: 'Déplacer le bloc 1', exact: true }), program.locator(':scope > .ctx-program-end'))
  await expect(panel.locator('[data-context-block="choice"] [data-context-block="detail"]')).toHaveCount(0)
  await expect(program.locator(':scope > [data-context-block]')).toHaveCount(4)
  await detail.locator('.ctx-block-title').focus()
  await page.keyboard.press('Alt+ArrowUp')
  await expect(program.locator(':scope > [data-context-block]').nth(2)).toHaveAttribute('data-context-block', 'detail')
  // A loop cannot be moved into its own body, which would create a cyclic program.
  await dragBlock(page, loop.locator(':scope > .ctx-block-order > .ctx-block-grip'), loop.locator('.ctx-block-list > .ctx-program-end'))
  await expect(program.locator(':scope > [data-context-block]')).toHaveCount(4)
  await expect(program.locator('[data-context-block="documents-loop"]')).toHaveCount(1)

  await panel.getByRole('button', { name: 'Prévisualiser', exact: true }).click()
  await expect(panel.locator('.ctx-preview-fragment pre')).toHaveText(['Titre source', 'Titre source', 'Alpha', 'Bêta'])
  await panel.getByRole('button', { name: 'Enregistrer la stratégie', exact: true }).click()
  await expect(panel.locator('.ctx-alert[role=status]')).toContainText('Stratégie enregistrée en Rust')
  const persisted = await (await request.get(`/api/context-strategies/${file.key}?workspaceId=${workspaceId}`)).json()
  expect(persisted.strategy.program.map((block: { id: string }) => block.id)).toEqual(['heading', 'choice', 'detail', 'documents-loop'])
  expect(persisted.strategy.program[0].value).toEqual({ kind: 'field', value: { kind: 'resource', name: 'document' }, field: 'title' })
  expect(persisted.strategy.program[1].then).toEqual([])
  expect(persisted.strategy.program[3].items[0].value).toEqual(strategy.program[2].kind === 'forEach' && strategy.program[2].items[0].kind === 'emit' ? strategy.program[2].items[0].value : undefined)
  expect(persisted.source).toContain('ContextBlock::for_each')
  expect(persisted.source).not.toContain('Titre source')
})
