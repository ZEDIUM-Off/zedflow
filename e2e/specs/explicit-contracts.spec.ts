import { test, expect } from '@playwright/test'
import { mkdir } from 'node:fs/promises'
import { fixturePath, saveFlow, waitRun } from './helpers'
import { template } from './fixtures/templates'
import { createHash } from 'node:crypto'

test('types are discoverable without a local catalog and examples are scoped to complete schemas', async ({page,request})=>{
  const path=fixturePath(`type-examples-${crypto.randomUUID()}`);await mkdir(path,{recursive:true})
  const opened=await request.post('/api/workspaces',{data:{path}});expect(opened.ok()).toBeTruthy();const workspace=await opened.json()
  const catalog=await request.get(`/api/context-source-types?workspaceId=${workspace.id}`);expect(catalog.ok()).toBeTruthy()
  const result=await catalog.json();const route=result.entries.find((entry:any)=>entry.id==='builtin:route-result')
  expect(route.type).toEqual({kind:'named',name:'RouteResult'});expect(route.types.RouteResult).toEqual({kind:'text'})
  const schema={workspaceId:workspace.id,dataType:route.type,types:route.types}
  const saved=await request.post('/api/type-examples',{data:{...schema,label:'Documentation vérifiée',value:'Deux documents vérifiés.'}});expect(saved.ok(),await saved.text()).toBeTruthy();const example=await saved.json()
  const list=await request.post('/api/type-examples/query',{data:schema});expect((await list.json()).some((entry:any)=>entry.id===example.id)).toBeTruthy()
  const other=await request.post('/api/type-examples/query',{data:{...schema,types:{RouteResult:{kind:'number'}}}});expect((await other.json()).some((entry:any)=>entry.id===example.id)).toBeFalsy()
  await page.goto('/');await page.getByRole('button',{name:'Conception',exact:true}).click()
  await page.getByRole('navigation',{name:'Espace de conception',exact:true}).getByRole('button',{name:'Contexte',exact:true}).click()
  await page.getByLabel('Workspace des stratégies',{exact:true}).selectOption(workspace.id)
  await page.getByRole('button',{name:'Catalogues de types',exact:true}).click()
  const explorer=page.getByRole('region',{name:'Tous les types connus',exact:true})
  await expect(explorer.getByRole('button',{name:/Résultat de routage/})).toBeVisible()
  await expect(page.getByLabel('Identifiant du catalogue de types',{exact:true})).not.toBeVisible()
  await explorer.getByRole('button',{name:/Résultat de routage/}).click();await expect(explorer.locator('.type-detail')).toContainText('"kind": "text"')
})

test('v4 named ports compile and a fixture invocation downloads its original request', async ({request})=>{
  const flow=template(true);flow.name=`Ports v4 ${crypto.randomUUID()}`;flow.formatVersion=4
  for(const edge of flow.edges){const source=flow.nodes.find(node=>node.id===edge.source)!,target=flow.nodes.find(node=>node.id===edge.target)!;edge.sourceHandle=source.data.kind==='condition'?edge.sourceHandle:source.data.kind==='context'?'context':'state';edge.targetHandle=target.data.kind==='model'?'context':'state'}
  const analysis=await request.post('/api/graph-analysis',{data:flow});expect(analysis.ok()).toBeTruthy();expect((await analysis.json()).diagnostics).toEqual([])
  const incompatible=structuredClone(flow);const pair=incompatible.edges.find(edge=>edge.targetHandle==='context')!;pair.sourceHandle='state'
  const rejected=await request.post('/api/graph-analysis',{data:incompatible});expect((await rejected.json()).diagnostics.length).toBeGreaterThan(0)
  const file=await saveFlow(request,flow)
  const launched=await request.post('/api/runs',{data:{flowKey:file.key,flowHash:file.hash,input:{input:'Texte é🦀 avec "guillemets"\net une seconde ligne'}}});expect(launched.ok(),await launched.text()).toBeTruthy()
  const ack=await launched.json(),run=await waitRun(request,ack.id,'waiting')
  const invocation=run.contextSnapshots?.at(-1)?.invocationId;expect(invocation).toBeTruthy()
  expect(run.contextSnapshots?.at(-1)?.rawRef).toBeTruthy()
  expect(run.contextSnapshots?.at(-1)?.requestStatus).toBe('sent')
  const detailResponse=await request.get(`/api/runs/${ack.id}/requests/${invocation}`);expect(detailResponse.ok(),await detailResponse.text()).toBeTruthy();const detail=await detailResponse.json()
  expect(detail.capture.boundary).toBe('fixtureInput')
  const raw=await request.get(`/api/runs/${ack.id}/requests/${invocation}/raw`);expect(raw.ok()).toBeTruthy();const bytes=await raw.body()
  expect(createHash('sha256').update(bytes).digest('hex')).toBe(detail.capture.sha256);expect(bytes.length).toBe(detail.capture.byteLength)
  expect(JSON.parse(bytes.toString('utf8'))).toEqual(detail.manifest.request)
  expect(bytes.toString('utf8')).toContain('é🦀')
  const passage=run.activities.find((activity:any)=>activity.node==='model')
  expect(passage?.occurrenceId).toBeTruthy()
  const boundary=await request.get(`/api/runs/${ack.id}/boundaries/${passage.occurrenceId}`)
  expect(boundary.ok(),await boundary.text()).toBeTruthy()
  const state=await boundary.json();expect(state.occurrenceId).toBe(passage.occurrenceId);expect(state.stateRef).toBeTruthy()
  expect(JSON.stringify(state.state)).toContain('é🦀')
  expect((await request.get(`/api/runs/${ack.id}/boundaries/missing`)).status()).toBe(404)
})

test('a standalone raw preview uses its trial profile without saving model bindings in the strategy',async({page,request})=>{
  const path=fixturePath(`request-preview-${crypto.randomUUID()}`);await mkdir(path,{recursive:true})
  const opened=await request.post('/api/workspaces',{data:{path}});const workspace=await opened.json()
  const strategy={version:2,id:'raw-trial-ui',name:'Essai raw autonome',requirements:{},capabilities:[],program:[{kind:'emit',id:'message',role:'data',format:'text',value:{kind:'literal',dataType:{kind:'text'},value:'Essai é🦀'}}]}
  const saved=await request.post('/api/context-strategies',{data:{workspaceId:workspace.id,strategy}});expect(saved.ok(),await saved.text()).toBeTruthy()
  await page.goto('/');await page.getByRole('button',{name:'Conception',exact:true}).click()
  await page.getByRole('navigation',{name:'Espace de conception',exact:true}).getByRole('button',{name:'Contexte',exact:true}).click()
  await page.getByLabel('Workspace des stratégies',{exact:true}).selectOption(workspace.id)
  await page.getByRole('button',{name:/Essai raw autonome/}).click()
  const profile=page.getByRole('region',{name:'Profil de requête d’essai',exact:true})
  await profile.getByRole('button',{name:'Configurer l’aperçu',exact:true}).click()
  await profile.getByLabel('Frontière d’aperçu',{exact:true}).selectOption('fixture')
  await profile.getByLabel('Identifiant du modèle d’essai',{exact:true}).fill('fixture-preview-only')
  await expect(profile.getByText('Raw · requête préparée, jamais envoyée',{exact:true})).toBeVisible()
  await profile.getByText('Raw · requête préparée, jamais envoyée',{exact:true}).click()
  await expect(profile.locator('pre')).toContainText('fixture-preview-only')
  await expect(profile.locator('pre')).toContainText('Essai é🦀')
  const file=await request.get(`/api/context-strategies/raw-trial-ui?workspaceId=${workspace.id}`)
  expect((await file.json()).source).not.toContain('fixture-preview-only')
  const runs=await request.get(`/api/runs?workspaceId=${workspace.id}`);expect(await runs.json()).toEqual([])
})
