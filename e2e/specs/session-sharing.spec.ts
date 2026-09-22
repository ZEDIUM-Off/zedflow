import { test, expect, request as playwrightRequest } from '@playwright/test'
import { spawn } from 'node:child_process'
import { once } from 'node:events'
import { mkdir, readFile, writeFile, access } from 'node:fs/promises'
import { join } from 'node:path'
import { unzipSync } from 'fflate'
import { fixturePath, waitRun } from './helpers'
import type { Composition, FlowNode, Run, Workspace } from '@zedflow/sdk'

const node=(id:string,kind:FlowNode['data']['kind'],config:Record<string,unknown>={},x=0):FlowNode=>({id,type:'flow',position:{x,y:100},data:{label:id,kind,config}})

test('a complete session is exported, imported into another daemon workspace and explicitly resumed without repeating an effect',async({page,request})=>{
  const root=fixturePath(`sharing-${crypto.randomUUID()}`),project=join(root,'project'),home=join(root,'home')
  await mkdir(project,{recursive:true});await mkdir(home,{recursive:true})
  await writeFile(join(project,'AGENTS.md'),'# Export fixture\nOnly use fixture models.\n')
  const child=spawn(join(process.env.CARGO_TARGET_DIR||'/tmp/zedflow-adk-target','debug','zedflow-daemon'),['--listen','127.0.0.1:0','--data',join(root,'data'),'--workspace',project,'--flow-home',home,'--context-home',home],{stdio:['ignore','pipe','pipe']})
  let errorOutput=''
  child.stderr.on('data',chunk=>{errorOutput=(errorOutput+String(chunk)).slice(-4000)})
  let source:Awaited<ReturnType<typeof playwrightRequest.newContext>>|undefined
  try {
    const address=await new Promise<string>((resolve,reject)=>{
      const timer=setTimeout(()=>reject(new Error(`Source fixture daemon did not start: ${errorOutput}`)),20000)
      let output=''
      child.stdout.on('data',chunk=>{output+=String(chunk);const match=output.match(/Zedflow daemon (http:\/\/127\.0\.0\.1:\d+)/);if(match){clearTimeout(timer);resolve(match[1]!)}})
      child.once('error',error=>{clearTimeout(timer);reject(error)})
      child.once('exit',code=>{clearTimeout(timer);reject(new Error(`Source fixture exited ${code}: ${errorOutput}`))})
    })
    source=await playwrightRequest.newContext({baseURL:address})
    const sourceWorkspace=(await(await source.get('/api/health')).json()).defaultWorkspaceId
    const nested:Composition={formatVersion:2,id:crypto.randomUUID(),name:'Sous-graphe partagé',revision:0,nodes:[node('start','start'),node('effect','tool',{tool:'exec',arguments:{command:'printf once >> shared-effect.txt'},field:'effect'},250),node('approval','input',{prompt:'Reprendre le travail partagé ?',responseType:'confirmation',field:'approved'},500),node('end','end',{},750)],edges:[{id:'a',source:'start',target:'effect'},{id:'b',source:'effect',target:'approval'},{id:'c',source:'approval',target:'end'}]}
    const composition:Composition={formatVersion:2,id:crypto.randomUUID(),name:'Session à partager',revision:0,nodes:[node('start','start'),node('child','subgraph',{composition:nested},250),node('output','output',{text:'Session reprise',field:'response'},550),node('end','end',{},850)],edges:[{id:'a',source:'start',target:'child'},{id:'b',source:'child',target:'output'},{id:'c',source:'output',target:'end'}]}
    const creation=await source.post('/api/runs',{data:{workspaceId:sourceWorkspace,composition,input:{input:'Partager une session complète'}}})
    expect(creation.ok(),await creation.text()).toBeTruthy()
    const initial:Run=await creation.json(),waiting=await waitRun(source,initial.id,'waiting')
    expect(await readFile(join(project,'shared-effect.txt'),'utf8')).toBe('once')
    const exportedResponse=await source.post('/api/sessions/export',{data:{workspaceId:sourceWorkspace,sessionIds:[initial.id]}})
    expect(exportedResponse.ok(),await exportedResponse.text()).toBeTruthy()
    const exported=await exportedResponse.json(),archivePath=exported.exports[0].path
    expect(await readFile(join(archivePath,'session.jsonl'),'utf8')).toContain(waiting.checkpoint)
    const zip=await source.get(exported.downloadUrl)
    expect(zip.ok()).toBeTruthy();expect(zip.headers()['content-type']).toContain('zip')
    const target:Workspace=await(await request.post('/api/workspaces',{data:{path:fixturePath('workspace-b')}})).json()
    await page.goto('/');await expect(page.getByText('Daemon connecté')).toBeVisible()
    await page.getByRole('button',{name:'Importer une session',exact:true}).click()
    const dialog=page.getByRole('dialog',{name:'Importer une session',exact:true})
    await dialog.getByLabel('Workspace cible').selectOption(target.id)
    await dialog.getByLabel('Chemin de l’export',{exact:true}).fill(archivePath)
    await dialog.getByRole('button',{name:'Importer',exact:true}).click()
    await expect(dialog).toHaveCount(0)
    await expect(page.locator('.confirmation-composer')).toContainText('Reprendre le travail partagé ?')
    const imported:Run=await(await request.get(`/api/runs/${initial.id}?workspaceId=${target.id}`)).json()
    expect(imported.workspaceId).toBe(target.id);expect(imported.import?.resumeBlocked).toEqual([])
    expect(imported.status).toBe('waiting');expect(imported.flowSource).toBe(waiting.flowSource)
    expect(imported.activities?.map(activity=>activity.occurrenceId)).toEqual(waiting.activities?.map(activity=>activity.occurrenceId))
    const wrongWorkspace=await request.get(`/api/runs/${initial.id}?workspaceId=${sourceWorkspace}`)
    expect(wrongWorkspace.ok()).toBe(false)
    const again=await request.post('/api/sessions/import',{data:{workspaceId:target.id,path:archivePath}})
    expect(again.ok(),await again.text()).toBeTruthy();expect((await again.json()).unchanged).toBe(1)
    await page.getByRole('button',{name:'Confirmer',exact:true}).click()
    await expect(page.locator('.assistant-markdown')).toContainText('Session reprise')
    await waitRun(request,initial.id,'completed',target.id)
    expect(await readFile(join(project,'shared-effect.txt'),'utf8')).toBe('once')
    expect(await access(fixturePath('workspace-b','shared-effect.txt')).then(()=>true,()=>false)).toBe(false)
    await page.getByRole('button',{name:'Actions de Partager une session complète',exact:true}).click()
    await page.getByRole('menuitem',{name:'Exporter la session',exact:true}).click()
    await expect(page.getByRole('dialog',{name:'Exporter la session',exact:true})).toContainText('.zedflow/sessions')
    const downloading=page.waitForEvent('download')
    await page.getByRole('button',{name:'Télécharger le ZIP',exact:true}).click()
    const download=await downloading,stream=await download.createReadStream(),chunks:Buffer[]=[]
    for await(const chunk of stream!)chunks.push(chunk)
    const files=unzipSync(Buffer.concat(chunks))
    const entry=Object.keys(files).find(path=>path.endsWith('/session.jsonl')||path==='session.jsonl')
    expect(entry).toBeDefined()
    const session=Buffer.from(files[entry!]!).toString('utf8')
    const completed:Run=await(await request.get(`/api/runs/${initial.id}?workspaceId=${target.id}`)).json()
    expect(session).toContain(initial.id)
    expect(session).toContain(completed.checkpoint)
    expect(completed.status).toBe('completed')
  } finally {
    await source?.dispose()
    if(child.exitCode===null){const exited=once(child,'exit');child.kill('SIGINT');const timer=setTimeout(()=>child.kill('SIGKILL'),5000);await exited;clearTimeout(timer)}
  }
})
