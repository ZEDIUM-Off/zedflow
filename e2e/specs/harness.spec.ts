import { modelCatalogSchema, runSchema, runSyncSchema } from '@zedflow/sdk'
import { test, expect, type Page } from '@playwright/test'
import type { Composition, FlowNode, Run, FlowFile } from '@zedflow/sdk'
import { legacyHarnessTemplate as harnessTemplate } from './fixtures/templates'

test.afterEach(async ({ page }) => { await page.goto('about:blank') })

const node = (id: string, kind: FlowNode['data']['kind'], config: Record<string, unknown> = {}, x = 0): FlowNode => ({ id, type: 'flow', position: { x, y: 100 }, data: { label: id, kind, config } })
const flow = (id: string, nodes: FlowNode[]): Composition => ({ id, name: id, revision: 1, nodes:[...nodes,node('inbox','input',{field:'input',prompt:'Suite de la session'},700)], edges: [] })

async function harnessServer(page: Page, compositions: Composition[] = [], modelWait = false) {
  let run: Run | undefined, revision = 1, fingerprint=''
  const files:FlowFile[]=[...compositions,harnessTemplate()].map(composition=>({key:composition.id,id:composition.id,name:composition.name,composition,path:`/workspace/.zedflow/flows/${composition.id}.rs`,scope:'workspace',workspaceId:'workspace',hash:`hash-${composition.id}`,diagnostics:[]}))
  const requests: { path: string; method: string; body: any }[] = []
  const projectMessages=()=>{if(run)run.timeline=run.messages.map((message,index)=>({...message,id:`message:${message.id||index}`,seq:index,kind:'message' as const}))}
  const context = { instructions: [{ path: '/workspace/AGENTS.md', content: 'Vérifier le travail.', hash: 'abc123' }], skills: [{ name: 'review', description: 'Examiner les changements du workspace', path: '/workspace/.agents/skills/review/SKILL.md', manualOnly: true }], diagnostics: [] }
  await page.route('**/api/**', async route => {
    const request = route.request(), url = new URL(request.url()), path = url.pathname.replace('/api', ''), method = request.method()
    const body = request.postDataJSON()
    if (method !== 'GET') requests.push({ path, method, body })
    const respond = (value: unknown, status = 200) => route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(value) })
    if (path === '/health') return respond({ adk: '2.2.0',defaultWorkspaceId:'workspace', workspace: { id:'workspace',host: 'fixture', path: '/workspace' } })
    if (path === '/workspaces') return respond([{id:'workspace',name:'Workspace fixture',path:'/workspace',open:true}])
    if (path === '/flows' && method === 'GET') return respond(files)
    if (path === '/flows' && method === 'POST') {const file:FlowFile={key:body.key||body.composition.id,id:body.composition.id,name:body.composition.name,composition:body.composition,path:`/workspace/.zedflow/flows/${body.composition.id}.rs`,scope:body.scope||'workspace',workspaceId:'workspace',hash:`hash-${++revision}`,diagnostics:[]};const index=files.findIndex(item=>item.key===file.key);if(index>=0)files[index]=file;else files.push(file);return respond(file)}
    if (path.startsWith('/flows/')) return respond(files.find(file=>path.endsWith(file.key)))
    if (path === '/context') return respond(context)
    if (path === '/models') return respond(modelCatalogSchema.parse({ providers: [{ id: 'fixture', label: 'Fixture' }, { id: 'codex', label: 'Codex fixture (no live connection)' }], models: [{ provider: 'fixture', id: 'fixture', label: 'Fixture', reasoningLevels: [] }, { provider: 'codex', id: 'test-model', label: 'Modèle de test', reasoningLevels: ['low', 'high'] }] }))
    if (path === '/rtc/config' || path.endsWith('/events')) return respond({ error: 'HTTP fixture' }, 503)
    if (path === '/runs' && method === 'GET') {
      // Loading past runs must not block a newly available composer.
      if (run) await new Promise(resolve => setTimeout(resolve, 1500))
      return respond(run ? [run] : [])
    }
    if (path === '/runs' && method === 'POST') {
      const file=files.find(file=>file.key===body.flowKey)!
      run = { id: 'harness-run-1', workspaceId:'workspace', workspacePath:'/workspace',flowRef:{key:file.key,path:file.path,hash:file.hash},name: file.name, composition: file.composition!, status: 'waiting', state: {}, messages: [{ role: 'user', text: body.input.input }], modelBindings: body.modelBindings || {}, modelRevision: 0, queue: [], context, wait: modelWait ? { id: 'choose-model', kind: 'model_selection', node: 'child/second', nodePath: 'child/second', config: { prompt: 'Choisissez le modèle du second nœud' } } : { id: 'input-wait', kind: 'input', node: 'inbox', config: { prompt: 'Sur quoi continuer ?', responseType: 'text' } } }
      projectMessages()
      return respond({id:run.id,workspaceId:run.workspaceId,revision})
    }
    if (method === 'GET' && /^\/runs\/[^/]+$/.test(path)) {
      if (!run || path !== `/runs/${run.id}` || (url.searchParams.get('workspaceId') ?? 'workspace') !== run.workspaceId) return respond({ error: 'Unknown request' }, 404)
      return respond(runSchema.parse(run))
    }
    if (!run) return respond({ error: 'Unknown request' }, 404)
    if (path.endsWith('/definition')) return respond({exact:false,runId:run.id,nodePath:new URL(request.url()).searchParams.get('nodePath')||'',diagnostic:{code:'legacy_definition_origin_missing',message:'Cette fixture ne conserve pas l’origine exacte des passages.'}})
    if (path.endsWith('/revisions')) return respond({runId:run.id,instances:[]})
    if (path.endsWith('/messages') && body?.text === 'Rejet simulé') return respond({ error: 'Message temporairement refusé' }, 409)
    if (path.endsWith('/snapshot')) {const next=JSON.stringify(run);if(next!==fingerprint){revision++;fingerprint=next}return respond(runSyncSchema.parse({type:'bootstrap',run,revision,cursor:revision}))}
    if (path.endsWith('/answer')) {
      if (run.wait?.kind === 'model_selection') { run.modelBindings!['child/second'] = body.value; run.status = 'running'; run.wait = null }
      else { run.messages.push({ role: 'user', text: body.value }); run.wait = { ...run.wait!, id: 'next-input' } }
    } else if (path.endsWith('/models')) { run.modelBindings![body.nodePath] = body.selection; run.modelRevision!++ }
    else if (path.endsWith('/messages') && method === 'POST') run.queue!.push({ ...body, status: 'pending' })
    else if (method === 'DELETE') run.queue!.find(message => path.endsWith(message.id))!.status = 'cancelled'
    else if (path.endsWith('/abort')) { run.status = 'stopped'; run.wait = null }
    else if (path.endsWith('/resume')) { run.status = 'running'; if (body.text) run.messages.push({ role: 'user', text: body.text }) }
    else return respond({ error: 'Unknown request' }, 404)
    projectMessages()
    revision++
    return respond({id:run.id,workspaceId:run.workspaceId,revision})
  })
  return { requests, current: () => run, files }
}

test('workspace harness exposes flow, model, context and explicit skill invocation without a live model', async ({ page }) => {
  const server = await harnessServer(page)
  await page.goto('/')
  await expect(page.getByText('Daemon connecté')).toBeVisible()
  await page.locator('button.flow-picker').click()
  await page.getByLabel('Rechercher un flow').fill('Harness')
  await page.locator('.flow-menu').getByRole('button', { name: /Harness de workspace/ }).click()
  await expect(page.locator('.flow-picker')).not.toHaveAttribute('open', '')
  await page.getByRole('button',{name:'Réglages des modèles',exact:true}).click()
  await page.locator('.model-menu').getByLabel('Fournisseur du modèle').selectOption('fixture')
  await expect(page.locator('.model-menu').getByLabel('Modèle', { exact: true })).toHaveValue('fixture')
  await expect(page.locator('.model-menu').getByLabel('Réflexion', { exact: true })).toBeDisabled()
  await page.keyboard.press('Escape')
  await page.getByRole('button', { name: 'Contexte', exact: true }).click()
  await expect(page.getByLabel('Capacités et contexte de l’agent')).toContainText('Instructions du workspace')
  await expect(page.getByLabel('Capacités et contexte de l’agent')).toContainText('À la demande')
  const composer = page.locator('.composer textarea')
  await composer.fill('/skill:rev')
  await page.getByRole('button', { name: '/skill:review Examiner les changements du workspace' }).click()
  await expect(composer).toHaveValue('/skill:review ')
  await page.screenshot({ path:test.info().outputPath('04-harness.png') })
  await composer.fill('/skill:review Vérifier la modification')
  await composer.press('Enter')
  await expect(page.locator('.composer-wait-prompt')).toContainText('Sur quoi continuer ?')
  const start = server.requests.find(request => request.path === '/runs')!.body
  expect(start.modelBindings).toEqual({ model: { provider: 'fixture', model: 'fixture' } })
  const selected=server.files.find(file=>file.key===start.flowKey)!.composition!
  expect(selected.nodes.find((node: FlowNode) => node.id === 'model')!.data.config.attachments.tools.items.map((item:any)=>item.name)).toEqual(['read', 'write', 'edit', 'exec'])
  expect(selected.nodes.find((node: FlowNode) => node.id === 'tools')!.data.config.tool).toEqual('execute_next_call')
  await composer.fill('Continuer la vérification')
  await composer.press('Enter')
  await expect(page.locator('.is-user').last()).toContainText('Continuer la vérification')
  expect(server.requests.find(request => request.path.endsWith('/answer'))?.body).toEqual({ waitId: 'input-wait', value: 'Continuer la vérification' })
})

test('nested model selection, steering, queue, stop and changing flow preserve their distinct run commands', async ({ page }) => {
  const child = flow('Sous-flow', [node('first', 'agent', { modelBinding: 'runtime' }), node('second', 'agent', { modelBinding: 'runtime' }, 270)])
  const composition = flow('Travail multi-modèle', [node('fixed', 'agent', { provider: 'fixture', model: 'fixture' }), node('child', 'subgraph', { composition: child }, 280)])
  const server = await harnessServer(page, [composition], true)
  await page.goto('/')
  await expect(page.getByText('Daemon connecté')).toBeVisible()
  await page.getByRole('button', { name: 'Afficher les détails', exact: true }).click()
  await page.getByRole('tab', { name: 'Modèles', exact: true }).click()
  await expect(page.locator('[data-model-path="fixed"] input')).toBeDisabled()
  const first = page.locator('[data-model-path="child/first"]')
  await first.getByRole('button').click()
  await expect(page.locator('.graph-breadcrumbs')).toContainText('child')
  await expect(page.locator('.run-canvas [data-id="first"] .flow-card')).toHaveClass(/selected/)
  await first.getByLabel('Fournisseur du modèle').selectOption('codex')
  await first.getByLabel('Réflexion', { exact: true }).selectOption('high')
  const composer = page.locator('.composer textarea')
  await composer.fill('Examiner le workspace')
  await composer.press('Enter')
  await expect(page.locator('.composer-wait-prompt')).toContainText('Choisissez le modèle')
  await page.locator('.model-wait-composer').getByLabel('Fournisseur du modèle').selectOption('fixture')
  await page.getByRole('button', { name: 'Choisir et reprendre' }).click()
  // Queue mode is already visible during model selection; wait for its answer to be applied.
  await expect(page.locator('.model-wait-composer')).not.toBeVisible()
  await expect(page.getByLabel('Mode d’envoi')).toBeVisible()
  expect(server.requests.find(request => request.path === '/runs')!.body.modelBindings['child/first']).toEqual({ provider: 'codex', model: 'test-model', reasoningEffort: 'high' })
  expect(server.requests.find(request => request.path.endsWith('/answer'))!.body).toEqual({ waitId: 'choose-model', value: { provider: 'fixture', model: 'fixture' } })
  await first.getByLabel('Réflexion', { exact: true }).selectOption('low')
  await expect.poll(() => server.current()?.modelRevision).toBe(1)
  expect(server.requests.find(request => request.path.endsWith('/models'))!.body).toEqual({ nodePath: 'child/first', selection: { provider: 'codex', model: 'test-model', reasoningEffort: 'low' }, revision: 0 })
  await composer.fill('Rejet simulé')
  await composer.press('Enter')
  await expect(page.getByRole('alert')).toContainText('Message temporairement refusé')
  await expect(composer).toHaveValue('Rejet simulé')
  await composer.fill('Commencer par les tests')
  await composer.press('Enter')
  await expect(page.getByLabel('Messages en attente')).toContainText('Réorientation')
  await page.getByLabel('Mode d’envoi').selectOption('followup')
  await composer.fill('Préparer le rapport ensuite')
  await composer.press('Enter')
  await expect(page.getByLabel('Messages en attente')).toContainText('Préparer le rapport ensuite')
  await page.getByRole('button', { name: 'Retirer Préparer le rapport ensuite' }).click()
  await expect(page.getByLabel('Messages en attente')).not.toContainText('Préparer le rapport ensuite')
  await page.getByRole('button', { name: '■ Arrêter' }).click()
  await expect(page.locator('.stopped-card')).toContainText('Session arrêtée')
  await page.getByRole('button', { name: 'Reprendre le travail' }).click()
  await expect(page.getByLabel('Mode d’envoi')).toBeVisible()
  await page.locator('button.flow-picker').click()
  await page.locator('.flow-menu').getByRole('button', { name: /Harness de workspace/ }).click()
  await expect(page.locator('button.flow-picker')).toContainText('Harness de workspace')
  expect(server.current()?.composition.name).toBe('Travail multi-modèle')
  expect(server.current()?.status).toBe('running')
  expect(server.requests.filter(request => request.path.endsWith('/abort'))).toHaveLength(1)
})


test('the initial composer waits for the workspace catalogue before accepting its first draft', async ({page})=>{
  const server=await harnessServer(page)
  let release!:()=>void,held=false
  const gate=new Promise<void>(resolve=>release=resolve)
  await page.route('**/api/flows?*',async route=>{if(!held){held=true;await gate}await route.fallback()})
  await page.goto('/')
  await expect(page.getByText('Daemon connecté',{exact:true})).toBeVisible()
  const composer=page.locator('.composer textarea')
  await expect(composer).not.toBeEditable()
  const filling=composer.fill('Première demande conservée')
  release();await filling
  await composer.press('Enter')
  await expect(page.locator('.composer-wait-prompt')).toBeVisible()
  expect(server.current()?.messages[0]?.text).toBe('Première demande conservée')
})

test('tool activity renders command output and file changes with provenance', async ({ page }) => {
  const server = await harnessServer(page)
  await page.goto('/')
  await expect(page.getByText('Daemon connecté')).toBeVisible()
  await page.locator('.composer textarea').fill('Vérifier un fichier')
  await page.locator('.composer textarea').press('Enter')
  await expect(page.locator('.composer-wait-prompt')).toBeVisible()
  server.current()!.toolActivities = [
    { callId: 'exec-1', nodePath: 'tools', name: 'exec', arguments: { command: 'cargo check' }, status: 'completed', output: 'Checking workspace…', result: { content: 'Vérification réussie', exitCode: 0, fullOutputPath: '/workspace/.lab/tool-output/result.log', truncated: true } },
    { callId: 'edit-1', nodePath: 'tools', name: 'edit', arguments: { path: 'src/main.rs' }, status: 'completed', result: { path: '/workspace/src/main.rs', diff: '-ancien\n+nouveau' } },
  ]
  server.current()!.timeline!.push(...server.current()!.toolActivities!.map((activity,index)=>({id:`tool:${activity.callId}`,seq:index+1,kind:'tool' as const,activity})))
  // An explicit refresh uses the same snapshot flow as reconnection.
  await page.getByRole('button',{name:'Connexion au daemon',exact:true}).click()
  await page.locator('.transport-badge').click()
  await page.locator('.tool-group-summary').click()
  await page.locator('[data-tool="exec"] > summary').click()
  await page.locator('[data-tool="edit"] > summary').click()
  await expect(page.locator('[data-tool="exec"]')).toContainText('cargo check')
  await expect(page.locator('[data-tool="exec"]')).toContainText('Checking workspace…')
  await expect(page.locator('[data-tool="exec"]')).toContainText('Code de sortie 0')
  await expect(page.locator('[data-tool="exec"]')).toContainText('/workspace/.lab/tool-output/result.log')
  await expect(page.locator('[data-tool="edit"] .tool-diff .added')).toHaveText('+nouveau\n')
})

test('flow search opens its definition and model rows reveal nodes outside the viewport', async ({ page }) => {
  const composition = flow('Flow à explorer', [node('fixed', 'agent', { provider: 'fixture', model: 'fixture' }), node('distant', 'agent', { modelBinding: 'runtime' }, 2500)])
  const server = await harnessServer(page, [composition])
  await page.goto('/')
  await expect(page.getByText('Daemon connecté')).toBeVisible()
  await page.getByRole('button', { name: 'Afficher les détails', exact: true }).click()
  await page.getByRole('tab', { name: 'Modèles', exact: true }).click()
  const row = page.locator('[data-model-path="distant"]')
  const graphNode = page.locator('.run-canvas [data-id="distant"] .flow-card')
  const canvas = (await page.locator('.run-canvas').boundingBox())!
  const centered = async () => {
    const box = await graphNode.boundingBox()
    return !!box && Math.abs(box.x + box.width / 2 - (canvas.x + canvas.width / 2)) < 15
  }
  await row.getByRole('button').click()
  await expect.poll(centered).toBe(true)
  await page.mouse.move(canvas.x + 60, canvas.y + 25)
  await page.mouse.down()
  await page.mouse.move(canvas.x - 380, canvas.y + 25, { steps: 8 })
  await page.mouse.up()
  await expect(graphNode).not.toBeInViewport()
  await row.getByRole('button').click()
  await expect.poll(centered).toBe(true)
  await page.getByRole('button', { name: 'Fermer les modèles' }).click()
  await graphNode.click()
  await expect(page.getByRole('tab',{name:'Parcours',exact:true})).toHaveAttribute('aria-selected','true')
  await expect(page.locator('.passage-selection')).toContainText('distant')
  await page.getByRole('button',{name:'Réglages des modèles',exact:true}).click()
  await page.locator('.model-menu-entry').filter({has:page.getByRole('heading',{name:'distant',exact:true})}).getByLabel('Fournisseur du modèle').selectOption('fixture')
  await page.keyboard.press('Escape')
  await page.locator('.composer textarea').fill('Examiner le flow')
  await page.locator('.composer textarea').press('Enter')
  await expect(page.locator('.composer-wait-prompt')).toBeVisible()
  await page.locator('button.flow-picker').click()
  await page.getByLabel('Rechercher un flow').fill('explorer')
  await expect(page.locator('.flow-menu').getByRole('button', { name: /Harness de workspace/ })).toHaveCount(0)
  await page.getByRole('button', { name: 'Ouvrir la définition', exact: true }).click()
  await expect(page.getByRole('textbox', { name: 'Nom de composition' })).toHaveValue('Flow à explorer')
  await page.getByRole('textbox', { name: 'Nom de composition' }).fill('Brouillon modifié')
  await page.getByRole('button', { name: 'Exécution', exact: true }).click()
  await expect(page.locator('.session-heading')).toContainText('Flow à explorer')
  expect(server.current()?.composition.name).toBe('Flow à explorer')
})

test('a draft for an old input wait cannot answer the next wait without explicit reuse',async({page})=>{
  const server=await harnessServer(page)
  await page.goto('/');await expect(page.getByText('Daemon connecté')).toBeVisible()
  const composer=page.locator('.composer textarea')
  await composer.fill('Commencer');await composer.press('Enter')
  await expect(page.locator('.composer-wait-prompt')).toContainText('Sur quoi continuer ?')
  await composer.fill('Réponse à la première attente')
  const run=server.current()!
  run.wait={...run.wait!,id:'replacement-wait',config:{prompt:'Nouvelle question',responseType:'text'}}
  await page.getByRole('button',{name:'Connexion au daemon',exact:true}).click()
  await page.locator('.transport-badge').click()
  await expect(page.locator('.composer-wait-prompt')).toContainText('Nouvelle question')
  await expect(page.getByText('Ce brouillon répondait à une attente précédente.')).toBeVisible()
  await composer.press('Enter')
  expect(server.requests.filter(request=>request.path.endsWith('/answer'))).toHaveLength(0)
  await expect(composer).toHaveValue('Réponse à la première attente')
  await page.getByRole('button',{name:'Conception',exact:true}).click()
  await page.getByRole('button',{name:'Exécution',exact:true}).click()
  await expect(page.getByText('Ce brouillon répondait à une attente précédente.')).toBeVisible()
  await page.getByRole('button',{name:'Réutiliser le brouillon pour cette étape',exact:true}).click()
  await composer.press('Enter')
  await expect.poll(()=>server.requests.filter(request=>request.path.endsWith('/answer')).length).toBe(1)
  expect(server.requests.find(request=>request.path.endsWith('/answer'))!.body).toEqual({waitId:'replacement-wait',value:'Réponse à la première attente'})
})

test('relative attached skills remain suggested for their agent and an output selection does not hide agent context',async({page})=>{
  const composition=harnessTemplate()
  composition.name='Skill relatif'
  composition.nodes.find(node=>node.id==='model')!.data.config.attachments.skills={items:[{id:'relative',source:{kind:'file',path:'.agents/skills/review/SKILL.md'},activation:'explicit'}]}
  await harnessServer(page,[composition])
  await page.goto('/');await expect(page.getByText('Daemon connecté')).toBeVisible()
  await page.locator('.composer textarea').fill('/skill:rev')
  await expect(page.getByRole('button',{name:'/skill:review Examiner les changements du workspace'})).toBeVisible()
  await page.getByRole('button',{name:'Afficher les détails',exact:true}).click()
  await page.locator('.run-canvas [data-id="response"] .flow-card').click()
  await page.getByRole('tab',{name:'Contexte',exact:true}).click()
  await expect(page.getByLabel('Agent à inspecter',{exact:true})).toHaveValue('model')
  await expect(page.getByLabel('Capacités et contexte de l’agent')).not.toContainText('Ce flow ne contient pas d’appel modèle')
})
