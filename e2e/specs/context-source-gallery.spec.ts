import { test, expect, type APIRequestContext, type Locator, type Page } from '@playwright/test'
import { mkdir, readFile } from 'node:fs/promises'
import type { ContextFile, ContextStrategy, ContextType } from '@zedflow/sdk'
import type { SourceCatalog } from '@zedflow/sdk'
import { fixturePath } from './helpers'

async function workspace(request: APIRequestContext, prefix: string) {
  const path = fixturePath(`${prefix}-${crypto.randomUUID()}`)
  await mkdir(path, { recursive: true })
  const response = await request.post('/api/workspaces', { data: { path } })
  expect(response.ok(), await response.text()).toBeTruthy()
  return response.json() as Promise<{ id: string; path: string }>
}
async function openStudio(page: Page, workspaceId: string, create = false) {
  await page.goto('/')
  await expect(page.getByText('Daemon connecté', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Conception', exact: true }).click()
  await page.getByRole('navigation', { name: 'Espace de conception', exact: true }).getByRole('button', { name: 'Contexte', exact: true }).click()
  await page.getByLabel('Workspace des stratégies', { exact: true }).selectOption(workspaceId)
  const panel = page.getByRole('region', { name: 'Studio de contexte', exact: true })
  await expect(panel).toHaveAttribute('data-context-workspace', workspaceId)
  if (create) await page.getByRole('button', { name: 'Créer une stratégie', exact: true }).click()
  return panel
}
async function gallery(page: Page, panel: Locator) {
  await panel.getByRole('button', { name: 'Ajouter des types', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: 'Ajouter des types de sources', exact: true })
  await expect(dialog.getByRole('checkbox', { name: 'Sélectionner Instructions', exact: true })).toBeVisible()
  return dialog
}
async function save(page: Page, panel: Locator): Promise<ContextFile> {
  await expect(panel.locator('.ctx-preview-scroll')).toHaveAttribute('aria-busy', 'false')
  const saved = page.waitForResponse(response => new URL(response.url()).pathname === '/api/context-strategies' && response.request().method() === 'POST')
  await panel.getByRole('button', { name: 'Enregistrer la stratégie', exact: true }).click()
  const response = await saved
  expect(response.ok(), await response.text()).toBeTruthy()
  return response.json()
}
async function storeTypes(request: APIRequestContext, workspaceId: string, key: string, types: Record<string, ContextType>) {
  const response = await request.post('/api/context-types', { data: { workspaceId, key, types } })
  expect(response.ok(), await response.text()).toBeTruthy()
  return response.json()
}
async function color(source: Locator) {
  return source.evaluate(element => getComputedStyle(element).getPropertyValue('--ctx-source-color').trim())
}
async function rememberFixture(page: Page, panel: Locator, name: string, resources?: Record<string, unknown>) {
  await panel.getByRole('button', { name: 'Voir les valeurs', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: 'Données d’essai du contexte', exact: true })
  if (resources) {
    await dialog.getByRole('button', { name: 'JSON', exact: true }).click()
    await dialog.getByLabel('Valeurs JSON des sources', { exact: true }).fill(JSON.stringify(resources))
    await dialog.getByRole('button', { name: 'Appliquer les valeurs JSON', exact: true }).click()
  }
  await dialog.getByLabel('Nom du jeu de données', { exact: true }).fill(name)
  await dialog.getByRole('button', { name: 'Conserver dans le brouillon', exact: true }).click()
  const id = await panel.getByLabel('Jeu de données d’essai', { exact: true }).inputValue()
  await dialog.getByRole('button', { name: 'Fermer', exact: true }).click()
  return id
}
async function fixtureValues(page: Page, panel: Locator) {
  await panel.getByRole('button', { name: 'Voir les valeurs', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: 'Données d’essai du contexte', exact: true })
  await dialog.getByRole('button', { name: 'JSON', exact: true }).click()
  const resources = JSON.parse(await dialog.getByLabel('Valeurs JSON des sources', { exact: true }).inputValue())
  await dialog.getByRole('button', { name: 'Fermer', exact: true }).click()
  return resources
}

// These cases exercise the real authoring API and local Rust files. They never create a model run.
test('the empty drawer selects source types by keyboard from all workspace catalogues and imports only their dependencies', async ({ page, request }) => {
  const target = await workspace(request, 'source-gallery')
  const types: Record<string, ContextType> = {
    GalleryExperiment: { kind: 'record', fields: { observations: { kind: 'named', name: 'GalleryObservation' }, score: { kind: 'number' } } },
    GalleryObservation: { kind: 'record', fields: { content: { kind: 'text' } } },
    UnusedCatalogueType: { kind: 'text' },
  }
  await storeTypes(request, target.id, 'research-types', types)
  await storeTypes(request, target.id, 'independent-types', { SecondCatalogueType: { kind: 'boolean' } })
  const response = await request.get(`/api/context-source-types?workspaceId=${target.id}`)
  expect(response.ok(), await response.text()).toBeTruthy()
  const catalog: SourceCatalog = await response.json()
  expect(catalog.entries.some(entry => entry.id === 'catalog:research-types:GalleryExperiment')).toBeTruthy()
  expect(catalog.entries.some(entry => entry.id === 'catalog:independent-types:SecondCatalogueType')).toBeTruthy()

  const panel = await openStudio(page, target.id, true)
  await expect(panel.locator('.ctx-source-drawer [data-resource]')).toHaveCount(0)
  await expect(panel.locator('.ctx-sources-empty')).toContainText('Aucun type déclaré')
  const dialog = await gallery(page, panel)
  const instruction = dialog.getByRole('checkbox', { name: 'Sélectionner Instructions', exact: true })
  await instruction.focus()
  await page.keyboard.press('Space')
  await expect(instruction).toBeChecked()
  await dialog.getByRole('button', { name: 'Annuler', exact: true }).click()
  await expect(panel.locator('[data-resource]')).toHaveCount(0)

  await gallery(page, panel)
  await dialog.getByRole('checkbox', { name: 'Sélectionner Instructions', exact: true }).focus()
  await page.keyboard.press('Space')
  await dialog.getByRole('navigation', { name: 'Catégories de sources' }).getByRole('button', { name: 'Personnalisés', exact: true }).click()
  await expect(dialog.getByRole('checkbox', { name: 'Sélectionner SecondCatalogueType', exact: true })).toBeVisible()
  await dialog.getByLabel('Rechercher un type ou un champ', { exact: true }).fill('observations')
  const experiment = dialog.getByRole('checkbox', { name: 'Sélectionner GalleryExperiment', exact: true })
  await expect(experiment).toBeVisible()
  await experiment.focus()
  await page.keyboard.press('Space')
  await expect(experiment).toBeChecked()
  await dialog.getByRole('button', { name: 'Ajouter 2 types', exact: true }).focus()
  await page.keyboard.press('Enter')
  await expect(dialog).not.toBeVisible()
  await expect(panel.locator('.ctx-source-drawer [data-resource]')).toHaveCount(2)
  await expect(panel.locator('[data-resource="instructions"]')).toBeVisible()
  await expect(panel.locator('[data-resource="gallery_experiment"]')).toBeVisible()
  await expect(panel.locator('[data-context-block]')).toHaveCount(0)

  const saved = await save(page, panel)
  expect(saved.strategy?.requirements).toEqual({ instructions: { kind: 'named', name: 'Instructions' }, gallery_experiment: { kind: 'named', name: 'GalleryExperiment' } })
  expect(Object.keys(saved.strategy?.types || {}).sort()).toEqual(['GalleryExperiment', 'GalleryObservation', 'Instructions'])
  expect(saved.strategy?.types?.GalleryObservation).toEqual(types.GalleryObservation)
  expect(saved.strategy?.program).toEqual([])
  expect(await readFile(saved.path, 'utf8')).toContain('.define_type(')
})

test('source aliases and type colors survive Rust reload and a standalone strategy package without retaining trial values', async ({ page, request }) => {
  const target = await workspace(request, 'source-alias')
  const panel = await openStudio(page, target.id, true)
  await panel.getByLabel('Nom de la stratégie', { exact: true }).fill('Documents avec alias')
  const dialog = await gallery(page, panel)
  await dialog.getByRole('checkbox', { name: 'Sélectionner Document', exact: true }).check()
  await dialog.getByRole('button', { name: 'Ajouter 1 type', exact: true }).click()
  let document = panel.locator('[data-resource="document"]')
  await document.getByRole('button', { name: 'Déplier la source document', exact: true }).click()
  const originalColor = await color(document)
  expect(originalColor).not.toBe('')
  await document.getByRole('button', { name: /^Insérer .*content$/ }).focus()
  await page.keyboard.press('Enter')
  const insertion = page.getByRole('dialog', { name: 'Insérer un champ dans le programme', exact: true })
  await insertion.getByRole('button', { name: 'Ajouter ce champ au contexte', exact: true }).click()
  await expect(panel.locator('[data-block-kind="emit"]')).toHaveCount(1)
  await document.locator('summary').filter({ hasText: 'Valeur manuelle d’aperçu' }).click()
  await document.getByLabel('Fournir document pour l’aperçu', { exact: true }).check()
  await document.getByLabel('content', { exact: true }).fill('SOURCE GALLERY PRIVATE TRIAL VALUE')
  const beforeRenameFixture = await rememberFixture(page, panel, 'Document avant renommage')
  await document.locator('summary').filter({ hasText: 'Alias et configuration' }).click()
  await document.getByLabel('Alias de document', { exact: true }).fill('paper')
  await document.getByRole('button', { name: 'Appliquer', exact: true }).click()
  await expect(panel.locator('[data-resource="document"]')).toHaveCount(0)
  document = panel.locator('[data-resource="paper"]')
  await expect(document).toBeVisible()
  await panel.getByLabel('Jeu de données d’essai', { exact: true }).selectOption('empty')
  await panel.getByLabel('Jeu de données d’essai', { exact: true }).selectOption(beforeRenameFixture)
  const renamedValues = await fixtureValues(page, panel)
  expect(Object.keys(renamedValues)).toEqual(['paper'])
  expect(renamedValues.paper.content).toBe('SOURCE GALLERY PRIVATE TRIAL VALUE')
  await document.locator('summary').filter({ hasText: 'Alias et configuration' }).click()
  await document.getByRole('button', { name: 'Déclarer une autre source de type Document', exact: true }).click()
  const other = panel.locator('[data-resource="paper_2"]')
  await expect(other).toBeVisible()
  expect(await color(other)).toBe(originalColor)
  expect(await color(document)).toBe(originalColor)
  const programToken = panel.locator('.ctx-program-panel .ctx-source-token').first()
  await expect(programToken).toContainText('paper')
  expect(await color(programToken)).toBe(originalColor)

  const bothFixture = await rememberFixture(page, panel, 'Deux documents', { ...renamedValues, paper_2: { title: 'Second', content: 'SECOND PRIVATE TRIAL VALUE', path: 'second.md' } })
  await other.locator('summary').filter({ hasText: 'Alias et configuration' }).click()
  await other.getByRole('button', { name: 'Retirer la ressource paper_2', exact: true }).click()
  await expect(other).toHaveCount(0)
  await panel.getByLabel('Jeu de données d’essai', { exact: true }).selectOption('empty')
  await panel.getByLabel('Jeu de données d’essai', { exact: true }).selectOption(bothFixture)
  expect(await fixtureValues(page, panel)).toEqual(renamedValues)
  await document.getByRole('button', { name: 'Déclarer une autre source de type Document', exact: true }).click()
  await expect(other).toBeVisible()

  await document.locator('summary').filter({ hasText: 'Valeur manuelle d’aperçu' }).click()
  await document.getByLabel('Fournir paper pour l’aperçu', { exact: true }).check()
  await document.getByLabel('content', { exact: true }).fill('SOURCE GALLERY PRIVATE TRIAL VALUE')
  await panel.getByRole('button', { name: 'Prévisualiser', exact: true }).click()
  await expect(panel.locator('.ctx-preview-panel')).toContainText('SOURCE GALLERY PRIVATE TRIAL VALUE')
  const saved = await save(page, panel)
  expect(saved.strategy?.requirements).toEqual({ paper: { kind: 'named', name: 'Document' }, paper_2: { kind: 'named', name: 'Document' } })
  expect(saved.strategy?.program[0]).toMatchObject({ kind: 'emit', value: { kind: 'field', value: { kind: 'resource', name: 'paper' }, field: 'content' } })
  expect(saved.strategy?.types?.Document).toBeDefined()
  const source = await readFile(saved.path, 'utf8')
  expect(source).toContain('.define_type(')
  expect(source).not.toContain('SOURCE GALLERY PRIVATE TRIAL VALUE')

  await openStudio(page, target.id)
  await page.locator(`[data-context-key="${saved.key}"]`).click()
  await expect(panel.locator('.ctx-source-drawer [data-resource]')).toHaveCount(2)
  expect(await color(panel.locator('[data-resource="paper"]'))).toBe(originalColor)
  expect(await color(panel.locator('[data-resource="paper_2"]'))).toBe(originalColor)
  expect(await color(panel.locator('.ctx-program-panel .ctx-source-token').first())).toBe(originalColor)
  await panel.locator('[data-resource="paper"]').getByRole('button', { name: 'Déplier la source paper', exact: true }).click()
  await panel.locator('[data-resource="paper"]').locator('summary').filter({ hasText: 'Valeur manuelle d’aperçu' }).click()
  await expect(panel.getByLabel('Fournir paper pour l’aperçu', { exact: true })).not.toBeChecked()

  const exported = await request.post('/api/context-packages/export', { data: { workspaceId: target.id, artifacts: [{ kind: 'strategy', key: saved.key }] } })
  expect(exported.ok(), await exported.text()).toBeTruthy()
  const bundle = await exported.json()
  expect(bundle.artifacts).toHaveLength(1)
  expect(bundle.artifacts[0].source).toBe(source)
  const destination = await workspace(request, 'source-package')
  const imported = await request.post('/api/context-packages/import', { data: { workspaceId: destination.id, package: bundle } })
  expect(imported.ok(), await imported.text()).toBeTruthy()
  const reloaded = await request.get(`/api/context-strategies/${saved.key}?workspaceId=${destination.id}`)
  expect(reloaded.ok(), await reloaded.text()).toBeTruthy()
  const copy: ContextFile = await reloaded.json()
  expect(copy.hash).toBe(saved.hash)
  expect(copy.strategy?.types).toEqual(saved.strategy?.types)
  expect(copy.strategy?.requirements).toEqual(saved.strategy?.requirements)
  const preview = await request.post('/api/context-strategies/preview', { data: { workspaceId: destination.id, selection: { kind: 'file', key: copy.key, hash: copy.hash }, resources: { paper: { title: 'Transferred', content: 'Preview from embedded schema', path: 'paper.md' } } } })
  expect(preview.ok(), await preview.text()).toBeTruthy()
  expect((await preview.json()).evaluation.items[0].value).toBe('Preview from embedded schema')
})

test('a condition rejects an incompatible source field in its menu and on drop, then accepts a numeric field from the drawer', async ({ page, request }) => {
  const target = await workspace(request, 'source-condition')
  const strategy: ContextStrategy = {
    version: 2, id: 'typed-source-condition', name: 'Condition numérique précise',
    types: { Measurement: { kind: 'record', fields: { score: { kind: 'number' }, label: { kind: 'text' } } } },
    requirements: { measurement: { kind: 'named', name: 'Measurement' } }, capabilities: [],
    program: [{ kind: 'if', id: 'compare-measurement', condition: { kind: 'eq', left: { kind: 'field', value: { kind: 'resource', name: 'measurement' }, field: 'score' }, right: { kind: 'literal', dataType: { kind: 'number' }, value: 5 } }, then: [{ kind: 'emit', id: 'equal-score', role: 'data', format: 'text', value: { kind: 'literal', dataType: { kind: 'text' }, value: 'Scores égaux' } }], else: [] }],
  }
  const stored = await request.post('/api/context-strategies', { data: { workspaceId: target.id, strategy } })
  expect(stored.ok(), await stored.text()).toBeTruthy()
  const file: ContextFile = await stored.json()
  const panel = await openStudio(page, target.id)
  await page.locator(`[data-context-key="${file.key}"]`).click()
  const source = panel.locator('[data-resource="measurement"]')
  await source.getByRole('button', { name: 'Déplier la source measurement', exact: true }).click()
  const right = panel.locator('[data-socket-label="Valeur de la condition"]')
  await right.getByRole('button', { name: 'Choisir un champ pour Valeur de la condition', exact: true }).click()
  await expect(page.getByRole('menuitem').filter({ hasText: 'measurement · label' })).toHaveAttribute('data-disabled', '')
  await expect(page.getByRole('menuitem').filter({ hasText: 'measurement · score' })).not.toHaveAttribute('data-disabled', '')
  await page.keyboard.press('Escape')
  const transfer = await page.evaluateHandle(() => {
    const data = new DataTransfer()
    data.setData('application/x-zedflow-context-source', JSON.stringify({ source: 'measurement', path: ['label'], type: { kind: 'number' }, typeId: 'Measurement', label: 'measurement · label' }))
    return data
  })
  await right.dispatchEvent('drop', { dataTransfer: transfer })
  await transfer.dispose()
  await expect(panel.getByRole('alert').filter({ hasText: 'Ce champ ne convient pas' })).toBeVisible()
  await expect(right.getByLabel('Valeur de la condition', { exact: true })).toHaveValue('5')
  await source.getByRole('button', { name: /^Insérer .*score$/ }).dragTo(right)
  await expect(right.locator('.ctx-source-token')).toContainText('score')
  await expect(panel.getByRole('alert').filter({ hasText: 'Ce champ ne convient pas' })).toHaveCount(0)
  const saved = await save(page, panel)
  expect(saved.strategy?.program[0]).toMatchObject({ kind: 'if', condition: { right: { kind: 'field', value: { kind: 'resource', name: 'measurement' }, field: 'score' } } })
  const preview = await request.post('/api/context-strategies/preview', { data: { workspaceId: target.id, selection: { kind: 'file', key: saved.key, hash: saved.hash }, resources: { measurement: { score: 5, label: 'No conversion from text' } } } })
  expect(preview.ok(), await preview.text()).toBeTruthy()
  expect((await preview.json()).evaluation.items[0].value).toBe('Scores égaux')
})

test('conflicting named schemas from two catalogues cannot silently replace a declared source type', async ({ page, request }) => {
  const target = await workspace(request, 'source-conflict')
  const numeric: ContextType = { kind: 'record', fields: { value: { kind: 'number' } } }
  await storeTypes(request, target.id, 'numeric-metric', { CatalogueMetric: numeric })
  await storeTypes(request, target.id, 'text-metric', { CatalogueMetric: { kind: 'record', fields: { value: { kind: 'text' } } } })
  const panel = await openStudio(page, target.id, true)
  let dialog = await gallery(page, panel)
  await dialog.getByLabel('Rechercher un type ou un champ', { exact: true }).fill('CatalogueMetric')
  const numericCard = dialog.locator('.ctx-gallery-type').filter({ hasText: 'Catalogue numeric-metric' })
  await numericCard.getByRole('checkbox').check()
  await dialog.getByRole('button', { name: 'Ajouter 1 type', exact: true }).click()
  dialog = await gallery(page, panel)
  await dialog.getByLabel('Rechercher un type ou un champ', { exact: true }).fill('CatalogueMetric')
  // Once selected, both the existing declaration and the competing schema are unavailable.
  await expect(dialog.getByRole('checkbox', { name: 'Sélectionner CatalogueMetric', exact: true })).toHaveCount(2)
  for (const checkbox of await dialog.getByRole('checkbox', { name: 'Sélectionner CatalogueMetric', exact: true }).all()) await expect(checkbox).toBeDisabled()
  await expect(dialog).toContainText('Schéma différent dans ce brouillon')
  await dialog.getByRole('button', { name: 'Annuler', exact: true }).click()
  await expect(panel.locator('[data-resource]')).toHaveCount(1)
  const saved = await save(page, panel)
  expect(saved.strategy?.types?.CatalogueMetric).toEqual(numeric)
  expect(saved.strategy?.requirements).toEqual({ catalogue_metric: { kind: 'named', name: 'CatalogueMetric' } })
})

test('the builtin and three embedded copies share one gallery card while retaining every provenance', async ({ page, request }) => {
  const target = await workspace(request, 'source-deduplication')
  const response = await request.get(`/api/context-source-types?workspaceId=${target.id}`)
  expect(response.ok(), await response.text()).toBeTruthy()
  const catalog: SourceCatalog = await response.json()
  const instructions = catalog.entries.find(entry => entry.id === 'builtin:instructions')!
  expect(instructions).toBeDefined()
  for (let index = 1; index <= 3; index++) {
    const strategy: ContextStrategy = {
      version: 2, id: `shared-instructions-${index}`, name: `Shared instructions ${index}`,
      types: { ...instructions.types, ...(index === 3 ? { UnrelatedEmbeddedType: { kind: 'number' as const } } : {}) },
      requirements: { instructions: instructions.type }, capabilities: [], program: [],
    }
    const saved = await request.post('/api/context-strategies', { data: { workspaceId: target.id, strategy } })
    expect(saved.ok(), await saved.text()).toBeTruthy()
  }
  const panel = await openStudio(page, target.id, true)
  const dialog = await gallery(page, panel)
  const checkbox = dialog.getByRole('checkbox', { name: 'Sélectionner Instructions', exact: true })
  await expect(checkbox).toHaveCount(1)
  const card = dialog.locator('.ctx-gallery-type').filter({ has: page.getByRole('checkbox', { name: 'Sélectionner Instructions', exact: true }) })
  await expect(card).toContainText('4 provenances')
  for (let index = 1; index <= 3; index++) await expect(card).toHaveAttribute('title', new RegExp(`Stratégie Shared instructions ${index}`))
  await dialog.getByLabel('Rechercher un type ou un champ', { exact: true }).fill('Shared instructions 3')
  await expect(checkbox).toHaveCount(1)
  await checkbox.check()
  await dialog.getByRole('button', { name: 'Ajouter 1 type', exact: true }).click()
  await expect(panel.locator('[data-resource]')).toHaveCount(1)
  const saved = await save(page, panel)
  expect(saved.strategy?.requirements).toEqual({ instructions: instructions.type })
  expect(saved.strategy?.types).toEqual(instructions.types)
  await gallery(page, panel)
  await expect(checkbox).toHaveCount(1)
  await expect(checkbox).toBeDisabled()
  await expect(dialog.locator('.ctx-gallery-type').filter({ has: page.getByRole('checkbox', { name: 'Sélectionner Instructions', exact: true }) })).toContainText('Déjà déclaré')
})

test('deep imported fields stay insertable and capability declarations do not offer a preview grant', async ({ page, request }) => {
  function nestedType(length: number): ContextType {
    let type: ContextType = { kind: 'text' }
    for (let index = length; index > 0; index--) type = { kind: 'record', fields: { [`level_${index}`]: type } }
    return type
  }
  const target = await workspace(request, 'source-deep-fields')
  const strategy: ContextStrategy = {
    version: 2, id: 'deep-source-fields', name: 'Champs profonds importés',
    requirements: { deep: nestedType(32) },
    capabilities: [{ id: 'read', input: { kind: 'record', fields: {} }, output: { kind: 'text' } }],
    program: [],
  }
  const stored = await request.post('/api/context-strategies', { data: { workspaceId: target.id, strategy } })
  expect(stored.ok(), await stored.text()).toBeTruthy()
  const file: ContextFile = await stored.json()
  const panel = await openStudio(page, target.id)
  await page.locator(`[data-context-key="${file.key}"]`).click()
  const source = panel.locator('[data-resource="deep"]')
  await source.getByRole('button', { name: 'Déplier la source deep', exact: true }).click()
  for (let index = 1; index < 32; index++) await source.getByRole('button', { name: `Déplier level_${index}`, exact: true }).click()
  const path = Array.from({ length: 32 }, (_, index) => `level_${index + 1}`)
  await source.getByRole('button', { name: `Insérer ${['deep', ...path].join(' · ')}`, exact: true }).click()
  await page.getByRole('dialog', { name: 'Insérer un champ dans le programme' }).getByRole('button', { name: 'Ajouter ce champ au contexte', exact: true }).click()
  await expect(panel.locator('.ctx-program-panel .ctx-source-token')).toContainText('level_32')
  const capabilities = panel.locator('details.ctx-sources-advanced').filter({ has: page.locator('summary').filter({ hasText: 'Capacités demandées' }) })
  await capabilities.locator('summary').click()
  await expect(capabilities).toContainText('La liaison au flow vérifie cette capacité.')
  await expect(capabilities.getByRole('checkbox')).toHaveCount(0)
  const saved = await save(page, panel)
  expect(saved.strategy?.capabilities).toEqual(strategy.capabilities)
  const expression = path.reduce((value, field) => ({ kind: 'field', value, field }), { kind: 'resource', name: 'deep' } as import('@zedflow/sdk').ContextExpr)
  expect(saved.strategy?.program[0]).toMatchObject({ kind: 'emit', value: expression })
  const value = path.reduceRight<unknown>((nested, name) => ({ [name]: nested }), 'Valeur au niveau 32')
  const preview = await request.post('/api/context-strategies/preview', { data: { workspaceId: target.id, selection: { kind: 'file', key: saved.key, hash: saved.hash }, resources: { deep: value } } })
  expect(preview.ok(), await preview.text()).toBeTruthy()
  expect((await preview.json()).evaluation.items[0].value).toBe('Valeur au niveau 32')
})
