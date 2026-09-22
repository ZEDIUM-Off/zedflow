import { modelCatalogSchema, runSchema, runSyncSchema } from '@zedflow/sdk'
import { test,expect,type Page } from '@playwright/test'
import type { Run,RunDelta,RunSync } from '@zedflow/sdk'
import { legacyHarnessTemplate as harnessTemplate,legacyTemplate as template } from './fixtures/templates'
import { RunProjection } from '@zedflow/sdk'

function fixtureRun():Run {
  const composition=template(true)
  return {id:'sync-run',name:'Synchronisation compacte',workspaceId:'sync-workspace',workspacePath:'/fixture',flowSourceRef:'source-initial',compositionRef:'composition-initial',createdAt:1,composition,status:'waiting',hasFlowSource:true,state:{},messages:[],wait:{id:'wait-1',kind:'input',node:'input',config:{prompt:'Votre réponse',responseType:'text'}},timeline:[{id:'answer-1',seq:1,kind:'message',role:'assistant',text:'Réponse canonique'},{id:'tool-1',seq:2,kind:'tool',activity:{callId:'read-1',name:'read',nodePath:'tools',status:'completed',argumentsPreview:{path:'src/main.rs'},resultRef:'result-1'}}],activities:[{occurrenceId:'model-1',path:'model',node:'model',label:'Appel modèle',kind:'agent',step:1,status:'completed',startedAt:1,startedSeq:1,inputRef:'input-1',outputRef:'output-1'}],contextSnapshots:[{invocationId:'context-1',nodePath:'model',origin:{nodePath:'model',occurrenceId:'model-1'},contentRef:'context-body',resources:[],skillCatalog:[]}]}
}

async function compactServer(page:Page,configure?:(run:Run)=>void){
  const run=fixtureRun(),requests:string[]=[],responses:RunSync[]=[]
  configure?.(run)
  let revision=1,healthy=true
  await page.addInitScript(()=>{
    ;(window as any).__syncStreams=[]
    window.EventSource=class extends EventTarget {
      onopen=null;onerror=null;onmessage=null;readyState=1
      constructor(public url:string){super();(window as any).__syncStreams.push(this)}
      close(){this.readyState=2}
    } as unknown as typeof EventSource
  })
  await page.route('**/api/**',async route=>{
    const url=new URL(route.request().url()),path=url.pathname.replace('/api','');requests.push(path)
    const respond=(value:unknown,status=200)=>route.fulfill({status,contentType:'application/json',body:JSON.stringify(value)})
    if(path==='/health')return respond(healthy?{daemon:{id:'daemon-1',host:'fixture-dgx',version:'test'},defaultWorkspaceId:run.workspaceId,workspace:{host:'fixture-dgx',path:'/fixture'}}:{error:'offline'},healthy?200:503)
    if(path==='/workspaces')return respond([{id:run.workspaceId,name:'Workspace fixture',path:'/fixture',open:true}])
    if(path==='/flows')return respond([{id:run.composition.id,key:'fixture-flow',name:run.composition.name,composition:run.composition,hash:'flow-1',scope:'workspace',workspaceId:run.workspaceId,path:'/fixture/.zedflow/flows/flow.rs',diagnostics:[]}])
    if(path==='/models')return respond(modelCatalogSchema.parse({models:[],providers:[]}))
    if(path==='/context')return respond({instructions:[],skills:[],diagnostics:[]})
    if(path==='/runs')return respond([{id:run.id,name:run.name,workspaceId:run.workspaceId,workspacePath:run.workspacePath,status:run.status,interactive:run.interactive}])
    if(route.request().method()==='GET'&&/^\/runs\/[^/]+$/.test(path)){
      if(path!==`/runs/${run.id}`||(url.searchParams.get('workspaceId')??run.workspaceId)!==run.workspaceId)return respond({error:'not found'},404)
      return respond(runSchema.parse(run))
    }
    if(path.endsWith('/tools/read-1/output')){
      if(path!==`/runs/${run.id}/tools/read-1/output`||url.searchParams.get('workspaceId')!==run.workspaceId)return respond({error:'not found'},404)
      return route.fulfill({contentType:'application/octet-stream',headers:{'content-disposition':'attachment; filename="read-output.bin"'},body:Buffer.from([0,65,10,195,169,255])})
    }
    if(path.endsWith('/snapshot')){
      const value:RunSync=url.searchParams.has('after')?{type:'heartbeat',runId:run.id,workspaceId:run.workspaceId,revision,cursor:revision}:{type:'bootstrap',run:structuredClone(run),revision,cursor:revision}
      responses.push(value);return respond(runSyncSchema.parse(value))
    }
    if(path.endsWith('/tools/read-1'))return respond({callId:'read-1',name:'read',arguments:{path:'src/main.rs'},status:'completed',result:{content:'Contenu chargé uniquement à l’ouverture',fullOutputRef:'full-content-1'}})
    if(path.endsWith('/activities/model-1'))return respond({...run.activities![0],input:{input:'Demande initiale'},output:{modelResponse:{provider:'fixture',model:'fixture',contextSnapshotId:'context-1'}}})
    if(path.endsWith('/context/context-1'))return respond({...run.contextSnapshots![0],resources:[{id:'instructions',kind:'instructions',hash:'ctx-1',content:'Instructions canoniques de cet appel'}]})
    if(path.endsWith('/definition'))return respond({exact:true,runId:run.id,instance:'',key:'fixture-flow',nodePath:url.searchParams.get('nodePath')||'',occurrenceId:url.searchParams.get('occurrenceId'),composition:run.composition,source:'// Source Rust figée\nfn flow() {}',hash:'flow-1'})
    if(path==='/generate')return respond({files:[{path:'flows/instance-0/flow.rs',content:'// Source Rust figée\nfn flow() {}'},{path:'Cargo.toml',content:'[package]\nname = \"fixture\"'}]})
    if(path.endsWith('/metrics'))return respond({batches:[{events:3,writerWaitMs:0.5,encodeMs:0.8,commitMs:2,publishMs:0.3,seq:revision}]})
    if(path.endsWith('/state'))return respond({state:{answer:'État demandé explicitement'},checkpoint:'checkpoint-1',revision})
    if(path.endsWith('/event-history'))return respond({events:[{seq:1,event:{type:'fixture_event'}}],cursor:1,hasMore:false})
    if(path==='/rtc/config')return respond({error:'SSE fixture'},503)
    return respond({})
  })
  await page.goto('/')
  await expect(page.getByText('Daemon connecté',{exact:true})).toBeVisible()
  if(run.interactive===false){await page.locator('.autonomous-history summary').click();await page.locator(`[data-execution-id="${run.id}"]`).getByRole('button').first().click()}else await page.locator(`[data-session-id="${run.id}"]`).getByRole('button').first().click()
  await expect(page.locator('.assistant-markdown')).toHaveText('Réponse canonique')
  const send=async(frame:RunSync)=>{
    revision=frame.revision
    await page.evaluate(value=>{const stream=(window as any).__syncStreams.filter((stream:any)=>stream.readyState===1).at(-1);stream.dispatchEvent(new MessageEvent('sync',{data:JSON.stringify(value)}))},frame)
  }
  return {run,requests,responses,send,health:(value:boolean)=>{healthy=value}}
}

test('skill catalogues load only on demand for the selected agent and share the context cache',async({page})=>{
  const server=await compactServer(page,run=>{
    run.composition=harnessTemplate()
    const agent=run.composition.nodes.find(node=>node.id==='model')!
    agent.data.label='Premier agent'
    agent.data.config.attachments.skills={items:[{id:'attached',source:{kind:'file',path:'skills/alpha/SKILL.md'},activation:'explicit'}]}
    const second=structuredClone(agent);second.id='second';second.data.label='Second agent';second.position.x+=400
    second.data.config.attachments.skills.items[0].source.path='skills/beta/SKILL.md'
    run.composition.nodes.push(second)
    run.contextSnapshots=[
      {invocationId:'old-context',agentPath:'model',contentRef:'old-body',skillCatalogRef:'old-catalogue'},
      {invocationId:'context-alpha',origin:{nodePath:'model',occurrenceId:'model-1'},contentRef:'alpha-body',skillCatalogRef:'alpha-catalogue'},
      {invocationId:'context-beta',nodePath:'second',contentRef:'beta-body',skillCatalogRef:'beta-catalogue'},
    ]
  })
  const requests:{id:string;workspaceId:string|null}[]=[]
  let betaName='beta'
  let releaseAlpha!:()=>void,startedAlpha!:()=>void
  const held=new Promise<void>(resolve=>{releaseAlpha=resolve}),started=new Promise<void>(resolve=>{startedAlpha=resolve})
  await page.route('**/api/runs/sync-run/context/**',async route=>{
    const url=new URL(route.request().url()),id=url.pathname.split('/').at(-1)!
    requests.push({id,workspaceId:url.searchParams.get('workspaceId')})
    if(id==='context-alpha'){startedAlpha();await held}
    const name=id==='context-alpha'?'alpha':betaName
    await route.fulfill({contentType:'application/json',body:JSON.stringify({invocationId:id,resources:[],skillCatalog:[{itemId:'attached',name,description:`Catalogue ${name}`,path:`/fixture/skills/${name}/SKILL.md`,activationKey:`attached::${name}`}]})})
  })
  expect(requests).toEqual([])
  await page.locator('.composer textarea').fill('/skill:')
  await started
  await expect(page.getByRole('status').filter({hasText:'Chargement du catalogue'})).toBeVisible()
  await page.getByLabel('Agent destinataire du skill',{exact:true}).selectOption('second')
  await expect(page.getByRole('button',{name:'/skill:beta Catalogue beta',exact:true})).toBeVisible()
  const response=page.waitForResponse(response=>new URL(response.url()).pathname.endsWith('/context/context-alpha'))
  releaseAlpha();await (await response).finished()
  await expect(page.getByRole('button',{name:'/skill:alpha Catalogue alpha',exact:true})).toHaveCount(0)
  await page.getByLabel('Agent destinataire du skill',{exact:true}).selectOption('model')
  await expect(page.getByRole('button',{name:'/skill:alpha Catalogue alpha',exact:true})).toBeVisible()
  await page.locator('.composer-context-button').click()
  const panel=page.getByLabel('Capacités et contexte de l’agent',{exact:true})
  await expect(panel.getByLabel('Agent à inspecter',{exact:true})).toHaveValue('model')
  await expect(panel.getByLabel('Activer /skill:alpha pour Premier agent',{exact:true})).toBeVisible()
  await expect(panel.locator('.captured-context')).toContainText('alpha')
  await panel.getByLabel('Agent à inspecter',{exact:true}).selectOption('second')
  await expect(panel.getByLabel('Activer /skill:beta pour Second agent',{exact:true})).toBeVisible()
  expect(requests).toEqual([{id:'context-alpha',workspaceId:'sync-workspace'},{id:'context-beta',workspaceId:'sync-workspace'}])
  betaName='beta-updated'
  await server.send({type:'delta',runId:server.run.id,workspaceId:server.run.workspaceId,baseRevision:1,revision:2,cursor:2,ops:[{collection:'contextSnapshots',id:'context-beta',value:{...server.run.contextSnapshots![2],contentRef:'beta-body-2',skillCatalogRef:'beta-catalogue-2'}}]})
  await expect(panel.getByLabel('Activer /skill:beta-updated pour Second agent',{exact:true})).toBeVisible()
  expect(requests.filter(request=>request.id==='context-beta')).toHaveLength(2)
})

test('the global footer and dismissible rail coexist with shared details loaded on demand',async({page})=>{
  const server=await compactServer(page)
  await expect(page.locator('.workspace-sidebar .brand')).toHaveCount(0)
  await expect(page.locator('.app-footer')).toContainText('fixture-dgx')
  await expect(page.locator('.app-footer').getByRole('button',{name:'Conception',exact:true})).toBeVisible()
  expect(server.requests.some(path=>/\/(activities|tools|state|event-history)\//.test(path))).toBe(false)
  const rail=page.getByRole('button',{name:'1 étapes du graphe',exact:true})
  await rail.click();await expect(page.locator('.rail-passage-popover')).toBeVisible()
  await page.locator('.session-heading').click();await expect(page.locator('.rail-passage-popover')).toHaveCount(0)
  await rail.click();await page.keyboard.press('Escape');await expect(rail).toBeFocused()
  const tool=page.locator('[data-tool="read"]>summary')
  await tool.click();await expect(page.locator('[data-tool="read"]')).toContainText('Contenu chargé uniquement à l’ouverture')
  await tool.click();await tool.click()
  expect(server.requests.filter(path=>path.endsWith('/tools/read-1'))).toHaveLength(1)
  await rail.click();await page.locator('.rail-passage-popover button').click()
  await expect(page.getByRole('combobox',{name:'Passage du nœud'})).toHaveValue('model-1')
  await expect.poll(()=>server.requests.filter(path=>path.endsWith('/activities/model-1')).length).toBe(1)
  expect(server.requests.filter(path=>path.endsWith('/context/context-1'))).toHaveLength(0)
  await page.getByText('Contexte réellement chargé',{exact:true}).click()
  await expect(page.locator('.captured-context')).toContainText('Instructions canoniques de cet appel')
  await page.getByRole('tab',{name:'Contexte',exact:true}).click()
  await expect(page.locator('.captured-context')).toContainText('Instructions canoniques de cet appel')
  expect(server.requests.filter(path=>path.endsWith('/context/context-1'))).toHaveLength(1)
  await page.getByRole('tab',{name:'État',exact:true}).click()
  await expect(page.locator('.state-row')).toContainText('État demandé explicitement')
  expect(server.requests.filter(path=>path.endsWith('/event-history'))).toHaveLength(0)
  await page.locator('.event-log>summary').click();await expect(page.locator('.event-log')).toContainText('fixture_event')
  await page.getByRole('button',{name:'Fermer les détails',exact:true}).click()
  await page.getByRole('button',{name:'Masquer la navigation',exact:true}).click()
  await expect(page.locator('.app-footer').getByRole('button',{name:'Conception',exact:true})).toBeVisible()
  expect(server.requests.filter(path=>path.endsWith('/metrics'))).toHaveLength(0)
  await page.getByRole('button',{name:'Connexion au daemon',exact:true}).click()
  await page.getByText('Mesures de synchronisation',{exact:true}).click()
  await expect(page.locator('.transport-metrics')).toContainText('Dernier lot du daemon · 3 événements')
  await expect(page.locator('.transport-metrics')).toContainText('Commit SQLite')
  expect(server.requests.filter(path=>path.endsWith('/metrics'))).toHaveLength(1)
})

test('frozen Rust is fetched once and the Cargo project is requested only for download',async({page})=>{
  const server=await compactServer(page)
  await page.getByRole('button',{name:'Afficher les détails',exact:true}).click()
  await page.getByRole('button',{name:'Rust initial du run',exact:true}).click()
  const dialog=page.getByRole('dialog',{name:'Rust initial du run',exact:true})
  await expect(dialog.locator('.source-preview')).toHaveText('// Source Rust figée\nfn flow() {}')
  expect(server.requests.filter(path=>path==='/generate')).toHaveLength(0)
  await page.keyboard.press('Escape')
  await page.getByRole('button',{name:'Rust initial du run',exact:true}).click()
  await expect(dialog.locator('.source-preview')).toContainText('Source Rust figée')
  expect(server.requests.filter(path=>path.endsWith('/definition'))).toHaveLength(1)
  const downloading=page.waitForEvent('download')
  await dialog.getByRole('button',{name:'Télécharger le projet Cargo',exact:true}).click()
  expect((await downloading).suggestedFilename()).toBe('zedflow-session-sync-run.zip')
  expect(server.requests.filter(path=>path==='/generate')).toHaveLength(1)
})

test('full tool output exposes a scoped download without fetching its body automatically',async({page})=>{
  const server=await compactServer(page)
  await page.locator('[data-tool="read"]>summary').click()
  const output=page.getByRole('button',{name:'Télécharger la sortie complète',exact:true})
  await expect(output).toBeVisible()
  expect(server.requests.filter(path=>path.endsWith('/tools/read-1/output'))).toHaveLength(0)
  const requested=page.waitForRequest(request=>new URL(request.url()).pathname.endsWith('/tools/read-1/output'))
  const downloading=page.waitForEvent('download')
  await output.click()
  const request=await requested,url=new URL(request.url())
  expect(`${url.pathname}${url.search}`).toBe(`/api/runs/${server.run.id}/tools/read-1/output?workspaceId=${server.run.workspaceId}`)
  expect(request.method()).toBe('GET')
  const download=await downloading,stream=await download.createReadStream(),chunks:Buffer[]=[]
  for await(const chunk of stream!)chunks.push(chunk)
  expect(Buffer.concat(chunks)).toEqual(Buffer.from([0,65,10,195,169,255]))
  expect(server.requests.filter(path=>path.endsWith('/tools/read-1/output'))).toHaveLength(1)

})

test('a heartbeat and duplicate delta leave rendered messages stable',async({page})=>{
  const bootstrap=page.waitForResponse(response=>{const url=new URL(response.url());return url.pathname.endsWith('/snapshot')&&!url.searchParams.has('after')})
  const server=await compactServer(page),run=server.run
  await (await bootstrap).finished()
  await page.getByRole('button',{name:'Connexion au daemon',exact:true}).click()
  await page.getByText('Mesures de synchronisation',{exact:true}).click()
  await expect(page.locator('.transport-metrics dt:has-text("Bootstrap / deltas") + dd')).toHaveText('1 / 0')
  expect(server.requests.filter(path=>path.endsWith('/snapshot'))).toHaveLength(1)
  await page.locator('.assistant-markdown').evaluate(node=>{(window as any).__canonicalMessage=node})
  await server.send({type:'heartbeat',runId:run.id,workspaceId:run.workspaceId,revision:1,cursor:1})
  const frame:RunDelta={type:'delta',runId:run.id,workspaceId:run.workspaceId,baseRevision:1,revision:2,cursor:2,ops:[{collection:'timeline',id:'answer-2',value:{id:'answer-2',seq:3,kind:'message',role:'assistant',text:'Nouvelle réponse'}}]}
  await server.send(frame);await server.send(frame)
  await expect(page.locator('.assistant-markdown')).toHaveCount(2)
  expect(await page.locator('.assistant-markdown').first().evaluate(node=>node===(window as any).__canonicalMessage)).toBe(true)
  await expect(page.locator('.transport-metrics dt:has-text("Heartbeats / doublons ignorés") + dd')).toHaveText('1 / 1')
  // A current heartbeat and a duplicate do not create a gap to fetch again.
  expect(server.requests.filter(path=>path.endsWith('/snapshot'))).toHaveLength(1)
})

test('daemon presence expires and recovers independently of an open session',async({page})=>{
  await page.clock.install()
  const server=await compactServer(page)
  await page.getByRole('button',{name:'Nouvelle session',exact:true}).click()
  server.health(false)
  await page.clock.fastForward(13000)
  await expect(page.getByText('Daemon déconnecté',{exact:true})).toBeVisible()
  server.health(true)
  await page.clock.fastForward(5000)
  await expect(page.getByText('Daemon connecté',{exact:true})).toBeVisible()
})

test.afterEach(async({page})=>{await page.goto('about:blank')})


test('autonomous snapshots retain errors and results in their own history without conversation controls',async({page})=>{
  const server=await compactServer(page,run=>{run.interactive=false;run.status='error';run.wait=null;run.error='Échec autonome explicite';run.composition=template(false)})
  await expect(page.getByRole('region',{name:'Exécution autonome',exact:true})).toBeVisible()
  await expect(page.getByLabel('Résultats de l’exécution')).toContainText('Réponse canonique')
  await expect(page.getByRole('alert').filter({hasText:'Échec autonome explicite'})).toBeVisible()
  await expect(page.locator('.composer')).toHaveCount(0)
  await expect(page.locator(`[data-session-id="${server.run.id}"]`)).toHaveCount(0)
  await page.getByRole('button',{name:'Inspecter',exact:true}).focus();await page.keyboard.press('Enter')
  await expect(page.getByRole('heading',{name:'Détails de l’exécution',exact:true})).toBeVisible()
  await page.getByRole('button',{name:'Fermer les détails',exact:true}).click()
  await page.getByRole('button',{name:'Conception',exact:true}).click()
  await expect(page.locator('.autonomous-history')).toHaveCount(0)
  await page.getByRole('button',{name:'Exécution',exact:true}).click()
  await page.locator('.autonomous-history summary').focus();await page.keyboard.press('Enter')
  await expect(page.locator(`[data-execution-id="${server.run.id}"]`)).toContainText('Échec')
  await page.locator(`[data-execution-id="${server.run.id}"] button`).first().focus();await page.keyboard.press('Enter')
  await expect(page.getByLabel('Résultats de l’exécution')).toContainText('Réponse canonique')
})
