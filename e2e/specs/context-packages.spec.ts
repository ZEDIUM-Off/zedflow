import { test, expect } from '@playwright/test'
import { readFile } from 'node:fs/promises'
import { template } from './fixtures/templates'
import { fixturePath, saveFlow, useDesign, waitRun } from './helpers'

test('a visual type catalogue binds to an inference and shares exact Rust with a strategy package',async({page,request})=>{
  const health=await(await request.get('/api/health')).json(),workspaceId=health.defaultWorkspaceId
  const target=await(await request.post('/api/workspaces',{data:{path:fixturePath('workspace-b')}})).json()
  const key=`catalogue-${crypto.randomUUID().slice(0,8)}`
  await page.goto('/');await page.getByRole('button',{name:'Conception',exact:true}).click()
  await page.getByRole('navigation',{name:'Espace de conception'}).getByRole('button',{name:'Contexte',exact:true}).click()
  await page.getByRole('button',{name:'Catalogues de types',exact:true}).click()
  const editor=page.getByRole('region',{name:'Éditeur de types'})
  await editor.getByRole('button', { name: 'Nouveau catalogue de types', exact: true }).click()
  await editor.getByLabel('Identifiant du catalogue de types',{exact:true}).fill(key)
  await editor.getByLabel('Nom du type du catalogue',{exact:true}).fill('Document');await editor.getByLabel('Nom du type du catalogue',{exact:true}).press('Enter')
  await editor.getByLabel('Définition de Document',{exact:true}).selectOption('text')
  await editor.getByRole('button',{name:'Enregistrer les types',exact:true}).click()
  await expect(editor.getByRole('status')).toContainText('enregistré en Rust')
  const saved=await(await request.get(`/api/context-types/${key}?workspaceId=${workspaceId}`)).json()
  expect(saved.types).toEqual({Document:{kind:'text'}});expect(saved.source).toContain('DataType::Text')
  const strategy={version:1,id:`strategy-${key}`,name:'Document typé',requirements:{document:{kind:'named',name:'Document'}},capabilities:[],program:[{kind:'emit',id:'document',role:'data',format:'text',value:{kind:'resource',name:'document'}}]}
  const response=await request.post('/api/context-strategies',{data:{workspaceId,strategy,types:saved.types}});expect(response.ok(),await response.text()).toBeTruthy();const stored=await response.json()
  const composition=template(false);composition.name=`Modèle ${key}`;const flow=await saveFlow(request,composition,workspaceId)
  await page.getByRole('navigation',{name:'Espace de conception'}).getByRole('button',{name:'Flows',exact:true}).click()
  await page.getByRole('button',{name:'Actualiser les flows',exact:true}).click()
  await page.locator(`[data-flow-key="${flow.key}"] .flow-file-open`).click();await page.locator('.design-space .flow-card.context').click()
  await page.getByLabel('Stratégie Rust',{exact:true}).selectOption(stored.key)
  await page.getByLabel('Source de document',{exact:true}).selectOption('state');await page.getByLabel('Champ d’état pour document',{exact:true}).fill('input')
  await page.locator('.ctx-binding-editor').getByLabel('Catalogue de types',{exact:true}).selectOption(key)
  await page.getByRole('button',{name:'Enregistrer',exact:true}).click();await expect(page.locator('.banner.notice')).toContainText('Flow enregistré')
  const updated=await(await request.get(`/api/flows/${flow.key}?workspaceId=${workspaceId}`)).json()
  expect(updated.composition.nodes.find((node:any)=>node.id==='context').data.config.contextTypesRef).toEqual({key,hash:saved.hash})
  const start=page.waitForResponse(response=>new URL(response.url()).pathname==='/api/runs'&&response.request().method()==='POST')
  await useDesign(page,'Document fourni explicitement');const ack=await(await start).json();const run=await waitRun(request,ack.id,'completed',workspaceId)
  const program=run.composition.nodes.find(node=>node.id==='context')!.data.config.contextProgram
  expect(program.typeSources).toEqual([{key,hash:saved.hash,source:saved.source}])

  await page.getByRole('button',{name:'Conception',exact:true}).click();await page.getByRole('button',{name:'Partager des définitions',exact:true}).click()
  const dialog=page.getByRole('dialog',{name:'Partager des définitions'})
  await dialog.getByRole('checkbox',{name:new RegExp(`^${key}`)}).check()
  await dialog.getByRole('checkbox',{name:new RegExp(`^${stored.key}`)}).check()
  const download=page.waitForEvent('download');await dialog.getByRole('button',{name:'Télécharger le package',exact:true}).click()
  const artifact=await download,path=await artifact.path();expect(path).toBeTruthy();const bytes=await readFile(path!);const bundle=JSON.parse(bytes.toString())
  expect(bundle.artifacts.find((item:any)=>item.kind==='types').source).toBe(saved.source)
  expect(bundle.artifacts.find((item:any)=>item.kind==='strategy').hash).toBe(stored.hash)
  await dialog.getByRole('button',{name:'Importer',exact:true}).click();await dialog.getByLabel('Workspace du transfert',{exact:true}).selectOption(target.id)
  await dialog.getByLabel('Fichier du package',{exact:true}).setInputFiles({name:'definitions.json',mimeType:'application/json',buffer:bytes})
  await expect(dialog.getByRole('status')).toContainText('Sources et empreintes valides')
  await dialog.getByRole('button',{name:'Importer les définitions',exact:true}).click();await expect(dialog).toContainText('2 fichiers importés')
  const imported=await(await request.get(`/api/context-types/${key}?workspaceId=${target.id}`)).json();expect(imported.hash).toBe(saved.hash)
  const altered=await request.post('/api/context-types',{data:{workspaceId:target.id,key,types:{Document:{kind:'number'}},expectedHash:imported.hash}});expect(altered.ok(),await altered.text()).toBeTruthy()
  await dialog.getByRole('button',{name:'Importer les définitions',exact:true}).click();await expect(dialog.getByRole('alert')).toBeVisible()
  const retained=await(await request.get(`/api/context-types/${key}?workspaceId=${target.id}`)).json();expect(retained.types.Document.kind).toBe('number')
})

test('a native reader is bound through its typed parameters and captures the file used by the model',async({page,request})=>{
  const workspaceId=(await(await request.get('/api/health')).json()).defaultWorkspaceId
  const strategy={version:1,id:`reader-${crypto.randomUUID()}`,name:'Document lu',requirements:{document:{kind:'text'}},capabilities:[],program:[{kind:'emit',id:'document',role:'data',format:'text',value:{kind:'resource',name:'document'}}]}
  const saved=await request.post('/api/context-strategies',{data:{workspaceId,strategy}});expect(saved.ok(),await saved.text()).toBeTruthy();const stored=await saved.json()
  const composition=template(false),file=await saveFlow(request,composition,workspaceId)
  await page.goto('/');await page.getByRole('button',{name:'Conception',exact:true}).click()
  await page.locator(`[data-flow-key="${file.key}"] .flow-file-open`).click();await page.locator('.design-space .flow-card.context').click()
  await page.getByLabel('Stratégie Rust',{exact:true}).selectOption(stored.key)
  const binding=page.locator('[data-context-binding="document"]')
  await binding.getByLabel('Source de document',{exact:true}).selectOption('reader')
  await binding.getByLabel('Lecteur de document',{exact:true}).selectOption('file.text')
  await binding.getByRole('group',{name:'Paramètres de document',exact:true}).getByLabel('path',{exact:true}).fill('workspace-name.txt')
  const creation=page.waitForResponse(response=>new URL(response.url()).pathname==='/api/runs'&&response.request().method()==='POST')
  await useDesign(page,'Lecture native explicite');const ack=await(await creation).json(),completed=await waitRun(request,ack.id,'completed',workspaceId)
  expect(completed.composition.nodes.find(node=>node.id==='context')!.data.config.contextProgram.bindings.document).toEqual({kind:'reader',reader:'file.text',input:{kind:'literal',value:{path:'workspace-name.txt'}}})
  const contents=await readFile(fixturePath('workspace-a','workspace-name.txt'),'utf8')
  expect(JSON.stringify(completed.contextSnapshots)).toContain(contents.trim())
})


test('a delayed default catalogue preserves the active type editor and its draft until explicit strategy navigation',async({page,request})=>{
  const health=await(await request.get('/api/health')).json()
  const key=`delayed-types-${crypto.randomUUID().slice(0,8)}`
  let release!:()=>void
  const gate=new Promise<void>(resolve=>{release=resolve})
  await page.route(/\/api\/context-strategies\?/,async route=>{
    const response=await route.fetch()
    await gate
    await route.fulfill({response})
  })
  try{
    await page.goto('/')
    await page.getByRole('button',{name:'Conception',exact:true}).click()
    await page.getByRole('navigation',{name:'Espace de conception'}).getByRole('button',{name:'Contexte',exact:true}).click()
    await page.getByRole('button',{name:'Catalogues de types',exact:true}).click()
    const editor=page.getByRole('region',{name:'Éditeur de types'})
    await editor.getByRole('button', { name: 'Nouveau catalogue de types', exact: true }).click()
    await editor.getByLabel('Identifiant du catalogue de types',{exact:true}).fill(key)
    await editor.getByLabel('Nom du type du catalogue',{exact:true}).fill('PendingDocument')
    await editor.getByLabel('Nom du type du catalogue',{exact:true}).press('Enter')
    await editor.getByLabel('Définition de PendingDocument',{exact:true}).selectOption('text')
    release()
    await expect(page.locator('[data-context-key="workspace-default"]')).toHaveClass(/chosen/)
    await expect(page.getByRole('button',{name:'Catalogues de types',exact:true})).toHaveAttribute('aria-pressed','true')
    await expect(editor.getByLabel('Identifiant du catalogue de types',{exact:true})).toHaveValue(key)
    await editor.getByRole('button',{name:'Enregistrer les types',exact:true}).click()
    await expect(editor.getByRole('status')).toContainText('enregistré en Rust')
    const saved=await(await request.get(`/api/context-types/${key}?workspaceId=${health.defaultWorkspaceId}`)).json()
    expect(saved.types).toEqual({PendingDocument:{kind:'text'}})
    // Opening even the already-selected default is an explicit navigation action.
    await page.locator('[data-context-key="workspace-default"]').click()
    await expect(page.getByRole('button',{name:'Stratégie',exact:true})).toHaveAttribute('aria-pressed','true')
    await page.getByRole('button',{name:'Catalogues de types',exact:true}).click()
    await expect(editor.getByLabel('Identifiant du catalogue de types',{exact:true})).toHaveValue(key)
  }finally{release()}
})

for (const catalogue of [
  { endpoint: 'context-types', label: 'Catalogue de types', property: 'contextTypesRef', selector: '.ctx-types-selection', definition: { types: { Document: { kind: 'text' } } } },
  { endpoint: 'context-libraries', label: 'Bibliothèque de fonctions', property: 'contextLibraryRef', selector: '.ctx-library-selection', definition: { library: { projections: {}, subprograms: {} } } },
]) {
  test(`${catalogue.label} pins the visible snapshot before a delayed read and preserves a newer choice`, async ({ page, request }) => {
    const workspaceId = (await (await request.get('/api/health')).json()).defaultWorkspaceId
    const files = []
    for (const suffix of ['first', 'second']) {
      const response = await request.post(`/api/${catalogue.endpoint}`, { data: { workspaceId, key: `pending-${suffix}-${crypto.randomUUID().slice(0, 8)}`, ...catalogue.definition } })
      expect(response.ok(), await response.text()).toBeTruthy()
      files.push(await response.json())
    }
    const [first, second] = files
    const flow = await saveFlow(request, template(false), workspaceId)
    await page.goto('/')
    await page.getByRole('button', { name: 'Conception', exact: true }).click()
    await page.locator(`[data-flow-key="${flow.key}"] .flow-file-open`).click()
    await page.locator('.design-space .flow-card.context').click()
    const editor = page.locator('.ctx-binding-editor')
    const select = editor.getByLabel(catalogue.label, { exact: true })
    let release!: () => void, arrived!: () => void
    const gate = new Promise<void>(resolve => { release = resolve })
    const requested = new Promise<void>(resolve => { arrived = resolve })
    await page.route(`**/api/${catalogue.endpoint}/${first.key}?*`, async route => {
      const response = await route.fetch()
      arrived()
      await gate
      await route.fulfill({ response })
    })
    const firstReadRequest = page.waitForRequest(request => new URL(request.url()).pathname === `/api/${catalogue.endpoint}/${first.key}`)
    async function persist() {
      const saving = page.waitForResponse(response => new URL(response.url()).pathname === '/api/flows' && response.request().method() === 'POST')
      await page.getByRole('button', { name: 'Enregistrer', exact: true }).click()
      const response = await saving
      expect(response.ok(), await response.text()).toBeTruthy()
      await expect(page.locator('.banner.notice')).toContainText('Flow enregistré')
      const stored = await (await request.get(`/api/flows/${flow.key}?workspaceId=${workspaceId}`)).json()
      return { sent: response.request().postDataJSON().composition.nodes.find((node: any) => node.id === 'context'), stored: stored.composition.nodes.find((node: any) => node.id === 'context') }
    }
    try {
      await select.selectOption(first.key)
      await requested
      const firstRequest = await firstReadRequest
      const read = page.waitForResponse(response => response.request() === firstRequest)
      // Saving while the read is held must include the choice already visible to the user.
      const initial = await persist()
      expect(initial.sent.data.config[catalogue.property]).toEqual({ key: first.key, hash: first.hash })
      expect(initial.stored.data.config[catalogue.property]).toEqual({ key: first.key, hash: first.hash })
      await select.selectOption(second.key)
      await expect(editor.locator(`${catalogue.selector} small`)).toContainText(second.hash.slice(0, 10))
      // Replacing an already resolved selection must stay visible during its reread.
      const revisiting = page.waitForRequest(request => new URL(request.url()).pathname === `/api/${catalogue.endpoint}/${first.key}`)
      await select.selectOption(first.key)
      const revisitedRequest = await revisiting
      const revisitedResponse = page.waitForResponse(response => response.request() === revisitedRequest)
      await expect(select).toHaveValue(first.key)
      await select.selectOption(second.key)
      await page.getByLabel('Nom du nœud', { exact: true }).fill('Choix plus récent')
      release()
      await (await read).finished()
      await (await revisitedResponse).finished()
      const replacement = await persist()
      expect(replacement.stored.data.config[catalogue.property]).toEqual({ key: second.key, hash: second.hash })
      expect(replacement.stored.data.label).toBe('Choix plus récent')
      await expect(select).toHaveValue(second.key)
      await select.selectOption('')
      const cleared = await persist()
      expect(cleared.stored.data.config[catalogue.property]).toBeUndefined()
    } finally {
      release()
    }
  })
}

test('a failed library catalogue in another workspace cannot offer the previous workspace snapshot', async ({ page, request }) => {
  const workspaceId = (await (await request.get('/api/health')).json()).defaultWorkspaceId
  const target = await (await request.post('/api/workspaces', { data: { path: fixturePath('workspace-b') } })).json()
  const key = `workspace-library-${crypto.randomUUID().slice(0, 8)}`
  const saved = await request.post('/api/context-libraries', { data: { workspaceId, key, library: { projections: {}, subprograms: {} } } })
  expect(saved.ok(), await saved.text()).toBeTruthy()
  await page.goto('/')
  await page.getByRole('button', { name: 'Conception', exact: true }).click()
  await page.getByRole('navigation', { name: 'Espace de conception' }).getByRole('button', { name: 'Contexte', exact: true }).click()
  await page.getByRole('button', { name: 'Créer une stratégie', exact: true }).click()
  const panel = page.locator('.context-studio:not(.bridge-studio)')
  await panel.locator('.ctx-studio-dependencies').locator('summary').click()
  const selection = panel.locator('.ctx-library-selection')
  await selection.getByLabel('Bibliothèque de fonctions', { exact: true }).selectOption(key)
  await page.route('**/api/context-libraries?*', async route => {
    if (new URL(route.request().url()).searchParams.get('workspaceId') === target.id) {
      await route.fulfill({ status: 503, contentType: 'application/json', body: JSON.stringify({ error: 'Catalogue B indisponible' }) })
    } else await route.continue()
  })
  await page.getByLabel('Workspace des stratégies', { exact: true }).selectOption(target.id)
  await expect(selection.locator('.ctx-error')).toContainText('Catalogue B indisponible')
  await expect(selection.locator(`option[value="${key}"]`)).toHaveCount(0)
  await expect(selection.getByLabel('Bibliothèque de fonctions', { exact: true })).toHaveValue('')
})
