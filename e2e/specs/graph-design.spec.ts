import { test, expect, type Page } from '@playwright/test'
import type { Composition, FlowFile, Run } from '@zedflow/sdk'
import { legacyHarnessTemplate as harnessTemplate, legacyTemplate as template, harnessTemplate as currentHarnessTemplate } from './fixtures/templates'

test.afterEach(async({page})=>{await page.goto('about:blank')})

async function designFixture(page: Page, composition: Composition) {
  let saved = structuredClone(composition), run: Run | undefined
  const file = (): FlowFile => ({key: saved.id, id: saved.id, name: saved.name, path: `/fixture/.zedflow/flows/${saved.id}.rs`, scope: 'workspace', workspaceId: 'graph-workspace', hash: 'fixture-hash', composition: saved, diagnostics: []})
  await page.route('**/api/**', async route => {
    const request = route.request(), path = new URL(request.url()).pathname.replace('/api', '')
    const respond = (value: unknown) => route.fulfill({contentType: 'application/json', body: JSON.stringify(value)})
    if (path === '/health') return respond({defaultWorkspaceId:'graph-workspace', workspace:{host:'fixture',path:'/fixture'}})
    if (path === '/workspaces') return respond([{id:'graph-workspace',name:'Graph fixture',path:'/fixture',open:true}])
    if (path === '/runs') return respond(run?[run]:[])
    if (run && path === `/runs/${run.id}/snapshot`) return respond({run,revision:1,cursor:1,events:[]})
    if (run && path === `/runs/${run.id}`) return respond(run)
    if (path === '/rtc/config' || path.endsWith('/events')) return route.fulfill({status:503,body:'HTTP fixture'})
    if (path === '/context') return respond({instructions:[],skills:[{name:'review',path:'/fixture/.agents/skills/review/SKILL.md',description:'Relire les changements'}],diagnostics:[]})
    if (path === '/context-strategies' || path === '/context-libraries') return respond([])
    if (path === '/models') return respond({models:[{id:'fixture',provider:'fixture',label:'Fixture',reasoningLevels:[]}]})
    if (path === '/flows' && request.method() === 'POST') { saved = request.postDataJSON().composition; return respond(file()) }
    if (path === '/flows') return respond([file()])
    if (path.startsWith('/flows/')) return respond(file())
    return respond({})
  })
  await page.goto('/')
  await expect(page.getByText('Daemon connecté')).toBeVisible()
  await page.getByRole('button', {name:'Conception',exact:true}).click()
  await expect(page.locator('.flow-library [data-flow-key]')).toHaveCount(1)
  await page.locator('.flow-file-open').click()
  await expect(page.locator('.canvas .vue-flow__node')).toHaveCount(composition.nodes.length)
  return Object.assign(() => saved, {session: (value:Run) => {run=value}})
}

async function intersections(page: Page, composition: Composition, selector = '.canvas') {
  return page.locator(selector).evaluate((canvas, edges) => {
    const problems: string[] = []
    for (const edge of edges) {
      const group = [...canvas.querySelectorAll<SVGGElement>('.vue-flow__edge')].find(element => element.dataset.id === edge.id)
      const path = group?.querySelector<SVGPathElement>('.vue-flow__edge-path')
      if (!path?.getAttribute('d')) { problems.push(`${edge.id}:missing`); continue }
      const matrix = path.getScreenCTM()!
      const values = (path.getAttribute('d')!.match(/-?\d+(?:\.\d+)?/g) || []).map(Number)
      const points = Array.from({length:values.length/2},(_,index)=>new DOMPoint(values[index*2],values[index*2+1]).matrixTransform(matrix))
      for (let index=1;index<points.length;index++) {
        const a=points[index-1],b=points[index]
        if (Math.abs(a.x-b.x)>0.1&&Math.abs(a.y-b.y)>0.1) problems.push(`${edge.id}:not-orthogonal`)
        for (const node of canvas.querySelectorAll<HTMLElement>('.vue-flow__node')) {
          if ([edge.source,edge.target].includes(node.dataset.id!)) continue
          const rect=node.getBoundingClientRect(), epsilon=1
          const horizontal=Math.abs(a.y-b.y)<0.1&&a.y>rect.top+epsilon&&a.y<rect.bottom-epsilon&&Math.max(a.x,b.x)>rect.left+epsilon&&Math.min(a.x,b.x)<rect.right-epsilon
          const vertical=Math.abs(a.x-b.x)<0.1&&a.x>rect.left+epsilon&&a.x<rect.right-epsilon&&Math.max(a.y,b.y)>rect.top+epsilon&&Math.min(a.y,b.y)<rect.bottom-epsilon
          if(horizontal||vertical)problems.push(`${edge.id}:${node.dataset.id}`)
        }
      }
    }
    return problems
  }, composition.edges)
}

test('orthogonal routes avoid measured nodes and attachments, including after manual movement', async ({page}) => {
  const composition=template(false)
  composition.nodes.find(node=>node.id==='model')!.position={x:250,y:120}
  composition.nodes.find(node=>node.id==='response')!.position={x:1120,y:180}
  composition.nodes.find(node=>node.id==='end')!.position={x:1120,y:450}
  composition.nodes.push({id:'obstacle',type:'flow',position:{x:720,y:130},data:{kind:'agent',label:'Obstacle et pièces',config:{provider:'fixture',attachments:{}}}})
  const current=await designFixture(page,composition)
  await expect(page.getByRole('checkbox',{name:'Aligner sur la grille'})).toBeChecked()
  await page.getByRole('checkbox',{name:'Aligner sur la grille'}).uncheck()
  await page.getByRole('checkbox',{name:'Aligner sur la grille'}).check()
  await expect.poll(()=>intersections(page,composition)).toEqual([])
  await expect(page.locator('.graph-routing-error')).toHaveCount(0)
  const obstacle=page.locator('.canvas .vue-flow__node[data-id="obstacle"] .node-heading')
  const box=(await obstacle.boundingBox())!
  await page.mouse.move(box.x+box.width/2,box.y+box.height/2)
  await page.mouse.down();await page.mouse.move(box.x+box.width/2-25,box.y+box.height/2+45,{steps:8});await page.mouse.up()
  await expect.poll(()=>intersections(page,composition)).toEqual([])
  await page.getByRole('button',{name:'Enregistrer',exact:true}).click()
  await expect.poll(()=>current().nodes.find(node=>node.id==='obstacle')!.position.y).not.toBe(130)
  expect(current().nodes.find(node=>node.id==='model')!.position).toEqual({x:250,y:120})
  expect(current().nodes.find(node=>node.id==='obstacle')!.position.x%16).toBe(0)
  expect(current().nodes.find(node=>node.id==='obstacle')!.position.y%16).toBe(0)
  await page.screenshot({path:test.info().outputPath('graph-obstacle-routing.png')})
})

test('context sources edit ordered resources without creating executable nodes, and starters use a dismissible menu', async ({page}) => {
  const composition=currentHarnessTemplate(), current=await designFixture(page,composition)
  const model=page.locator('.canvas .vue-flow__node[data-id="model"]')
  await expect(model.locator('.attachment-piece')).toHaveCount(0)
  await page.locator('.canvas .vue-flow__node[data-id="context"] .node-heading').click()
  await page.locator('.context-sources summary').click()
  await page.locator('.context-sources .attachment-tabs').getByRole('tab',{name:/Fichiers/}).click()
  await page.getByRole('button',{name:'Ajouter un fichier',exact:true}).click()
  await page.getByLabel('Chemin du fichier',{exact:true}).fill('src/main.rs')
  await page.getByLabel('Première ligne',{exact:true}).fill('3')
  await page.getByLabel('Dernière ligne',{exact:true}).fill('17')
  await page.getByLabel('Limite de caractères',{exact:true}).fill('2000')
  await page.getByRole('button',{name:'Enregistrer',exact:true}).click()
  await expect.poll(()=>current().nodes.find(node=>node.id==='context')!.data.config.attachments.files.items.length).toBe(1)
  expect(current().nodes).toHaveLength(composition.nodes.length)
  expect(current().nodes.find(node=>node.id==='context')!.data.config.attachments.files.items[0]).toMatchObject({path:'src/main.rs',startLine:3,endLine:17,maxChars:2000})
  await page.getByRole('button',{name:'Actions du flow Harness de workspace',exact:true}).click()
  await expect(page.getByRole('menuitem',{name:'Supprimer Harness de workspace',exact:true})).toBeVisible()
  await page.keyboard.press('Escape')
  await page.getByRole('button',{name:'Créer un flow',exact:true}).click()
  await expect(page.getByRole('menuitem',{name:/Boucle interactive/})).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(page.getByRole('menu')).toHaveCount(0)
  await expect(page.getByRole('button',{name:'Créer un flow',exact:true})).toBeFocused()
})

test('binary conditions preserve typed grouped predicates and label both outgoing routes',async({page})=>{
  const composition=harnessTemplate(),current=await designFixture(page,composition)
  await expect(page.locator('.canvas [data-edge-label="route-tools"]')).toHaveText('Oui')
  await expect(page.locator('.canvas [data-edge-label="route-response"]')).toHaveText('Non')
  await page.locator('.canvas .vue-flow__node[data-id="route"] .node-heading').click()
  await page.getByRole('combobox',{name:'Combinaison',exact:true}).selectOption('all')
  await page.getByRole('button',{name:'Ajouter un critère',exact:true}).click()
  const criteria=page.locator('.graph-properties .predicate-child')
  await criteria.nth(1).getByLabel('Champ d’état',{exact:true}).fill('/result/count')
  await criteria.nth(1).getByRole('combobox',{name:'Comparaison',exact:true}).selectOption('gte')
  await criteria.nth(1).getByRole('combobox',{name:'Type de valeur',exact:true}).selectOption('number')
  await criteria.nth(1).getByLabel('Valeur numérique',{exact:true}).fill('2')
  await criteria.nth(1).getByLabel('Valeur numérique',{exact:true}).blur()
  await page.getByRole('button',{name:'Enregistrer',exact:true}).click()
  await expect.poll(()=>current().nodes.find(node=>node.id==='route')!.data.config.predicate.kind).toBe('all')
  expect(current().nodes.find(node=>node.id==='route')!.data.config.predicate).toEqual({kind:'all',items:[{kind:'compare',field:'hasToolCalls',operator:'eq',value:true},{kind:'compare',field:'/result/count',operator:'gte',value:2}]})
  await expect.poll(()=>intersections(page,composition)).toEqual([])
})


test('the execution inspector routes the frozen graph around cards with occurrence badges',async({page})=>{
  const composition=harnessTemplate(),current=await designFixture(page,composition)
  const run:Run={id:'graph-run',name:'Améliorer le chargement des flows',workspaceId:'graph-workspace',workspacePath:'/fixture',composition,status:'waiting',state:{},messages:[],wait:{id:'inbox-wait',kind:'input',node:'inbox',nodePath:'inbox',config:{prompt:'Quelle amélioration souhaitez-vous traiter ensuite ?',responseType:'text'}},modelBindings:{model:{provider:'fixture',model:'fixture'}},activities:[
    {occurrenceId:'model-1',node:'model',path:'model',label:'Agent du workspace',kind:'agent',step:1,status:'completed',startedAt:1,endedAt:2,startedSeq:1,endedSeq:2},
    {occurrenceId:'tool-1',node:'tools',path:'tools',label:'Exécuter un outil',kind:'tool',step:2,status:'completed',startedAt:2,endedAt:3,startedSeq:3,endedSeq:6},
    {occurrenceId:'model-2',node:'model',path:'model',label:'Agent du workspace',kind:'agent',step:3,status:'completed',startedAt:3,endedAt:4,startedSeq:7,endedSeq:8},
    {occurrenceId:'response-1',node:'response',path:'response',label:'Réponse',kind:'output',step:4,status:'completed',startedAt:4,endedAt:5,startedSeq:9,endedSeq:10},
    {occurrenceId:'inbox-1',node:'inbox',path:'inbox',label:'Suite du travail',kind:'inbox',step:5,status:'waiting',startedAt:5,startedSeq:11}],timeline:[
    {id:'user-1',seq:0,kind:'message',role:'user',text:'Corrige le chargement des flows, puis vérifie la modification.'},
    {id:'tool-read',seq:3,kind:'tool',origin:{nodePath:'tools',occurrenceId:'tool-1'},activity:{callId:'read-1',nodePath:'tools',name:'read',status:'completed',arguments:{path:'src/flows.rs'},result:{content:'fn load() {}'}}},
    {id:'tool-edit',seq:4,kind:'tool',origin:{nodePath:'tools',occurrenceId:'tool-1'},activity:{callId:'edit-1',nodePath:'tools',name:'edit',status:'completed',arguments:{path:'src/flows.rs'},result:{path:'src/flows.rs',diff:'- load().unwrap()\n+ load()?'}}},
    {id:'tool-exec',seq:5,kind:'tool',origin:{nodePath:'tools',occurrenceId:'tool-1'},activity:{callId:'exec-1',nodePath:'tools',name:'exec',status:'completed',arguments:{command:'cargo test flows'},result:{content:'3 tests passed',exitCode:0}}},
    {id:'assistant-1',seq:9,kind:'message',role:'assistant',origin:{nodePath:'response',occurrenceId:'response-1'},text:'Le chargement des flows conserve maintenant le brouillon ouvert.\n\n- Les fichiers invalides restent visibles avec leur diagnostic.\n- Les modifications externes déclenchent un conflit explicite.\n- Les trois tests de chargement passent.\n\nLa modification est dans `src/flows.rs`.'}]}

  current.session(run)
  await page.reload()
  await page.locator('[data-session-id="graph-run"]').getByRole('button').first().click()
  await expect(page.locator('.assistant-markdown')).toContainText('brouillon ouvert')
  await page.screenshot({path:test.info().outputPath('execution-audit-chat.png')})
  await page.getByRole('button',{name:'Afficher les détails',exact:true}).click()
  await expect(page.locator('.run-canvas .vue-flow__node[data-id="model"]')).toContainText('Passage 2')
  await expect.poll(()=>intersections(page,composition,'.run-canvas')).toEqual([])
  await expect(page.locator('.graph-routing-error')).toHaveCount(0)
  const compactWidth=(await page.locator('.run-canvas .flow-card.agent').boundingBox())!.width
  await page.getByRole('button',{name:'Agrandir le graphe',exact:true}).click()
  await expect.poll(async()=>(await page.locator('.run-canvas .flow-card.agent').boundingBox())!.width).toBeGreaterThan(compactWidth*1.3)
  await expect(page.locator('.session-inspector.expanded')).toBeVisible()
  await page.screenshot({path:test.info().outputPath('execution-audit-fullgraph.png')})
  await page.getByRole('button',{name:'Revenir au chat',exact:true}).click()
  await expect.poll(async()=>Math.abs((await page.locator('.run-canvas .flow-card.agent').boundingBox())!.width-compactWidth)).toBeLessThan(1)
  await page.getByRole('button',{name:'Fermer les détails',exact:true}).click()
  await page.setViewportSize({width:390,height:844})
  await page.screenshot({path:test.info().outputPath('execution-audit-mobile-navigation.png')})
  await page.getByRole('button',{name:'Masquer la navigation',exact:true}).click()
  await page.screenshot({path:test.info().outputPath('execution-audit-mobile.png')})
})
