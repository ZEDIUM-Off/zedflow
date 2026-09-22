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
