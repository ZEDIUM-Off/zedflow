import { test, expect, type APIRequestContext } from '@playwright/test'
import { readFile } from 'node:fs/promises'
import { waitRun } from './helpers'

function flow(id:string,root:boolean) {
  const port={input:{kind:'text'},output:{kind:'text'}}
  const exports={contract:{entries:{main:port},branches:root?{work:{contract:port,invocations:['node']}}:{},data:{},requires:{},inferenceNodes:{}},types:{},entries:{main:{node:'start',inputField:'input',outputField:'response'}},branches:root?{work:'action'}:{},data:{},requires:{},interactive:false}
  return {formatVersion:3,id,name:root?'Entrée composée':'Worker composé',revision:0,nodes:[
    {id:'start',type:'flow',position:{x:0,y:0},data:{kind:'start',label:'Départ',config:{exports}}},
    {id:'action',type:'flow',position:{x:0,y:180},data:root?{kind:'route',label:'Déléguer le travail',config:{branch:'work',invocation:'node',inputField:'input',field:'output'}}:{kind:'set',label:'Produire',config:{field:'output',value:'Résultat du worker composé'}}},
    {id:'publish',type:'flow',position:{x:0,y:340},data:{kind:'output',label:'Publier',config:{inputField:'output'}}},
    {id:'end',type:'flow',position:{x:0,y:480},data:{kind:'end',label:'Fin',config:{}}},
  ],edges:[{id:'a',source:'start',target:'action'},{id:'b',source:'action',target:'publish'},{id:'c',source:'publish',target:'end'}]}
}
async function fixtures(request:APIRequestContext){
  const health=await(await request.get('/api/health')).json(),workspaceId=health.defaultWorkspaceId
  const files=[]
  for(const root of [true,false]){const response=await request.post('/api/flows',{data:{workspaceId,composition:flow(crypto.randomUUID(),root)}});expect(response.ok(),await response.text()).toBeTruthy();files.push(await response.json())}
  return{workspaceId,parent:files[0],child:files[1]}
}

test('visual bridge saves Rust, prepares only selected instances and executes the resolved route',async({page,request})=>{
  const {workspaceId,parent,child}=await fixtures(request)
  await page.goto('/')
  await expect(page.getByText('Daemon connecté',{exact:true})).toBeVisible()
  await page.getByRole('button',{name:'Conception',exact:true}).click()
  await page.getByRole('navigation',{name:'Espace de conception'}).getByRole('button',{name:'Bridges',exact:true}).click()
  const studio=page.getByRole('region',{name:'Studio de bridges'})
  const key=`ui-bridge-${Date.now()}`
  await studio.getByLabel('Identifiant du bridge',{exact:true}).fill(key)
  await studio.getByLabel('Flow racine pour les ports',{exact:true}).selectOption(parent.key)
  await studio.getByLabel('Alias du flow importé',{exact:true}).fill('worker')
  await studio.getByLabel('Alias du flow importé',{exact:true}).press('Enter')
  await studio.getByLabel('Définition de worker',{exact:true}).selectOption(child.key)
  await studio.getByLabel('Nom de la connexion',{exact:true}).fill('delegate')
  await studio.getByLabel('Nom de la connexion',{exact:true}).press('Enter')
  await studio.getByLabel('Port source delegate',{exact:true}).selectOption('work')
  await studio.getByLabel('Entrée cible delegate',{exact:true}).selectOption('main')
  await studio.getByLabel('Identifiant du bridge',{exact:true}).focus()
  await page.keyboard.press('Control+s')
  await expect(studio.getByRole('status')).toContainText('Bridge enregistré en Rust')
  const file=await(await request.get(`/api/bridges/${key}?workspaceId=${workspaceId}`)).json()
  expect(await readFile(file.path,'utf8')).toContain('BridgeDefinition::new')
  expect(file.bridge.connections.delegate.from).toEqual({instance:'root',port:'work'})
  await studio.getByRole('button',{name:'Afficher le Rust du bridge',exact:true}).click()
  await expect(page.getByRole('dialog',{name:'Rust du bridge'})).toContainText('delegate')
  await page.keyboard.press('Escape')
  await page.getByRole('button',{name:'Exécution',exact:true}).click()
  await page.getByRole('button',{name:'Composer',exact:true}).click()
  const dialog=page.getByRole('dialog',{name:'Composer une exécution'})
  await dialog.getByLabel('Flow d’entrée',{exact:true}).selectOption(parent.key)
  await dialog.getByRole('checkbox',{name:new RegExp(key)}).check()
  await dialog.getByRole('button',{name:'Préparer le graphe résolu',exact:true}).click()
  await expect(dialog.locator('.runtime-graph-preview')).toContainText('Worker composé')
  await expect(dialog.locator('.runtime-graph-preview')).toContainText(`${key}/delegate`)
  await page.screenshot({path:test.info().outputPath('composition-prepared.png')})
  await dialog.getByLabel('Donnée d’entrée',{exact:true}).fill('Question du flow racine')
  const created=page.waitForResponse(response=>response.url().endsWith('/api/runs')&&response.request().method()==='POST')
  await dialog.getByRole('button',{name:'Lancer avec cette donnée',exact:true}).click()
  const response=await created;expect(response.ok(),await response.text()).toBeTruthy();const ack=await response.json()
  const completed=await waitRun(request,ack.id,'completed',workspaceId)
  expect(completed.status,completed.error).toBe('completed')
  expect(completed.state.response).toBe('Résultat du worker composé')
  expect(completed.activities.some((activity:any)=>activity.path===`${key}/worker/action`)).toBeTruthy()
  const sent=response.request().postDataJSON()
  expect(sent.runtimeSelection.bridgeHashes[key]).toBe(file.hash)
  expect(sent.runtimeSelection.flowHashes[parent.key]).toBe(parent.hash)
  await page.getByRole('button',{name:'Afficher les détails',exact:true}).click()
  await expect(page.locator('.runtime-inspector-graph')).toContainText('Worker composé')
  await expect(page.locator('.runtime-inspector-graph')).toContainText(`${key}/delegate`)
  await page.reload()
  await expect(page.locator('.session-heading')).toContainText('Question du flow racine')
  await page.getByRole('button',{name:'Afficher les détails',exact:true}).click()
  await expect(page.locator('.runtime-inspector-graph')).toContainText(`${key}/delegate`)
})

test('public flow ports are edited through typed controls and retain the Rust file as their source',async({page,request})=>{
  const {workspaceId,parent}=await fixtures(request)
  await page.goto('/');await expect(page.getByText('Daemon connecté',{exact:true})).toBeVisible()
  await page.getByRole('button',{name:'Conception',exact:true}).click()
  await page.locator(`[data-flow-key="${parent.key}"] .flow-file-open`).click()
  await page.locator('.design-editor [data-id="start"]').click()
  const editor=page.getByRole('region',{name:'Contrats publics du flow'})
  await editor.getByLabel('Nom du contrat public',{exact:true}).fill('second')
  await editor.getByLabel('Nom du contrat public',{exact:true}).press('Enter')
  await editor.getByLabel('Canal de sortie second',{exact:true}).selectOption('response')
  await page.locator('.design-toolbar').getByRole('button',{name:'Enregistrer',exact:true}).click()
  await expect(page.getByRole('status')).toContainText('Flow enregistré')
  const file=await(await request.get(`/api/flows/${parent.key}?workspaceId=${workspaceId}`)).json()
  const exports=file.composition.nodes.find((node:any)=>node.id==='start').data.config.exports
  expect(exports.entries.second).toEqual({node:'start',inputField:'input',outputField:'response'})
  expect(exports.contract.entries.second.input).toEqual({kind:'text'})
  expect(await readFile(`${file.path}/flow.rs`,'utf8')).toContain('second')
})

test('runtime preparation selects an exact strategy per inference and requires preparing its changed contracts',async({page,request})=>{
  const workspaceId=(await(await request.get('/api/health')).json()).defaultWorkspaceId
  const composition=flow(crypto.randomUUID(),false)
  composition.nodes[1].data={kind:'agent',label:'Inférence préparée',config:{provider:'fixture',model:'fixture-echo',inputField:'input',field:'output'}} as typeof composition.nodes[1]['data']
  const save=await request.post('/api/flows',{data:{workspaceId,composition}});expect(save.ok(),await save.text()).toBeTruthy();const file=await save.json()
  const strategy={version:1,id:`chosen-${crypto.randomUUID()}`,name:'Contexte choisi au lancement',requirements:{},capabilities:[],program:[{kind:'emit',id:'selected',role:'instruction',format:'text',value:{kind:'literal',dataType:{kind:'text'},value:'Contexte propre à cette inférence'}}]}
  const saved=await request.post('/api/context-strategies',{data:{workspaceId,strategy}});expect(saved.ok(),await saved.text()).toBeTruthy();const context=await saved.json()
  await page.goto('/');await page.getByRole('button',{name:'Composer',exact:true}).click()
  const dialog=page.getByRole('dialog',{name:'Composer une exécution'})
  await dialog.getByLabel('Flow d’entrée',{exact:true}).selectOption(file.key)
  await dialog.getByRole('button',{name:'Préparer le graphe résolu',exact:true}).click()
  await dialog.getByLabel('Stratégie de root/action',{exact:true}).selectOption(context.key)
  await expect(dialog.getByRole('button',{name:'Lancer avec cette donnée',exact:true})).toBeDisabled()
  await dialog.getByRole('button',{name:'Préparer le graphe résolu',exact:true}).click()
  await expect(dialog.getByRole('button',{name:'Lancer avec cette donnée',exact:true})).toBeEnabled()
  await dialog.getByLabel('Stratégie de root/action',{exact:true}).scrollIntoViewIfNeeded()
  await page.screenshot({path:test.info().outputPath('context-engine-preparation.png')})
  await dialog.getByLabel('Donnée d’entrée',{exact:true}).fill('Entrée de cette instance')
  const started=page.waitForResponse(response=>new URL(response.url()).pathname==='/api/runs'&&response.request().method()==='POST')
  await dialog.getByRole('button',{name:'Lancer avec cette donnée',exact:true}).click()
  const response=await started;expect(response.ok(),await response.text()).toBeTruthy();const ack=await response.json()
  expect(response.request().postDataJSON().runtimeSelection.contexts).toEqual({'root/action':{key:context.key,hash:context.hash}})
  const completed=await waitRun(request,ack.id,'completed',workspaceId)
  expect(JSON.stringify(completed.contextSnapshots)).toContain('Contexte propre à cette inférence')
  const unchanged=await(await request.get(`/api/flows/${file.key}?workspaceId=${workspaceId}`)).json()
  expect(unchanged.hash).toBe(file.hash)
})

test('an autonomous entry routed to an interactive child opens a session and keeps its answer composer',async({page,request})=>{
  const workspaceId=(await(await request.get('/api/health')).json()).defaultWorkspaceId
  const parent=flow(crypto.randomUUID(),true),child=flow(crypto.randomUUID(),false)
  child.nodes[0].data.config.exports.interactive=true
  child.nodes[1].data={kind:'input',label:'Question enfant',config:{field:'output',prompt:'Votre réponse au sous-flow',responseType:'text'}} as typeof child.nodes[1]['data']
  const files=[]
  for(const composition of [parent,child]){const result=await request.post('/api/flows',{data:{workspaceId,composition}});expect(result.ok(),await result.text()).toBeTruthy();files.push(await result.json())}
  const key=`interactive-${crypto.randomUUID()}`
  const bridge=await request.post('/api/bridges',{data:{workspaceId,key,bridge:{imports:{worker:{flow:files[1].key}},connections:{call:{from:{instance:'root',port:'work'},to:{instance:'worker',port:'main'},invocation:'node',mode:'callAwait'}}}}});expect(bridge.ok(),await bridge.text()).toBeTruthy()
  await page.goto('/');await page.getByRole('button',{name:'Composer',exact:true}).click()
  const dialog=page.getByRole('dialog',{name:'Composer une exécution'})
  await dialog.getByLabel('Flow d’entrée',{exact:true}).selectOption(files[0].key)
  await dialog.getByRole('checkbox',{name:new RegExp(key)}).check()
  await dialog.getByRole('button',{name:'Préparer le graphe résolu',exact:true}).click()
  await dialog.getByRole('button',{name:'Utiliser cette composition',exact:true}).click()
  await expect(page.locator('.autonomous-launch')).toHaveCount(0)
  const start=page.waitForResponse(response=>new URL(response.url()).pathname==='/api/runs'&&response.request().method()==='POST')
  await page.locator('.composer textarea').fill('Démarrer la route');await page.locator('.composer textarea').press('Enter')
  const ack=await(await start).json(),waiting=await waitRun(request,ack.id,'waiting',workspaceId)
  expect(waiting.interactive).toBe(true)
  await expect(page.locator(`[data-session-id="${ack.id}"]`)).toBeVisible()
  await expect(page.locator('.composer-wait-prompt')).toContainText('Votre réponse au sous-flow')
  await page.locator('.composer textarea').fill('Réponse du flow enfant');await page.locator('.composer textarea').press('Enter')
  const completed=await waitRun(request,ack.id,'completed',workspaceId)
  expect(completed.state.response).toBe('Réponse du flow enfant')
})
