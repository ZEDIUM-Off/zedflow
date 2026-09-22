import { test } from 'node:test'
import assert from 'node:assert/strict'
import { RunProjection, type Run, type RunDelta } from '../dist/index.js'
function fixtureRun():Run {
  const composition={"formatVersion":2,"id":"legacyInteractive","name":"Assistant de workspace","revision":0,"nodes":[{"id":"start","type":"flow","position":{"x":60,"y":175},"data":{"kind":"start","label":"D\u00e9but","config":{}}},{"id":"model","type":"flow","position":{"x":300,"y":100},"data":{"kind":"agent","label":"Appel mod\u00e8le","config":{"fanIn":"any","provider":"fixture","inputField":"input","field":"output","attachments":{"instructions":{"items":[{"id":"instructions","source":{"kind":"text","text":"R\u00e9ponds de mani\u00e8re concise en fran\u00e7ais."},"mode":"literal","activation":"always"}]}}}}},{"id":"response","type":"flow","position":{"x":750,"y":165},"data":{"kind":"output","label":"R\u00e9ponse","config":{"text":"{{output}}"}}},{"id":"input","type":"flow","position":{"x":750,"y":410},"data":{"kind":"input","label":"Attendre une r\u00e9ponse","config":{"field":"input","prompt":"Sur quoi continuer ?","responseType":"text"}}}],"edges":[{"id":"e1","source":"start","target":"model"},{"id":"e2","source":"model","target":"response"},{"id":"e3","source":"response","target":"input"},{"id":"e4","source":"input","target":"model"}]}
  return {id:'sync-run',name:'Synchronisation compacte',workspaceId:'sync-workspace',workspacePath:'/fixture',composition,status:'waiting',hasFlowSource:true,state:{},messages:[],wait:{id:'wait-1',kind:'input',node:'input',config:{prompt:'Votre réponse',responseType:'text'}},timeline:[{id:'answer-1',seq:1,kind:'message',role:'assistant',text:'Réponse canonique'},{id:'tool-1',seq:2,kind:'tool',activity:{callId:'read-1',name:'read',nodePath:'tools',status:'completed',argumentsPreview:{path:'src/main.rs'},resultRef:'result-1'}}],activities:[{occurrenceId:'model-1',path:'model',node:'model',label:'Appel modèle',kind:'agent',step:1,status:'completed',startedAt:1,startedSeq:1,inputRef:'input-1',outputRef:'output-1'}],contextSnapshots:[{invocationId:'context-1',nodePath:'model',origin:{nodePath:'model',occurrenceId:'model-1'},contentRef:'context-body',resources:[],skillCatalog:[]}]}
}

test('canonical deltas preserve untouched identities, reject gaps and apply a replay only once',()=>{
  const run=fixtureRun(),store=new RunProjection(run.id,run.workspaceId)
  store.apply({type:'bootstrap',run,revision:1,cursor:1})
  const timeline=store.state!.run.timeline!,activity=store.state!.run.activities![0]
  const meta:RunDelta={type:'delta',runId:run.id,workspaceId:run.workspaceId,baseRevision:1,revision:2,cursor:2,ops:[{collection:'meta',value:{status:'running',wait:null}}]}
  assert.deepEqual(store.apply(meta), {changed:true,gap:false})
  assert.equal(store.state!.run.timeline, timeline)
  assert.equal(store.state!.run.activities![0], activity)
  assert.deepEqual(store.apply(meta), {changed:false,gap:false})
  assert.deepEqual(store.apply({...meta,baseRevision:4,revision:5,cursor:5}), {changed:false,gap:true})
  assert.equal(store.state!.revision, 2)
  const tool={...timeline[1]!,activity:{callId:'read-1',name:'read',status:'completed',resultRef:'result-2'}}
  store.apply({...meta,baseRevision:2,revision:3,cursor:3,ops:[{collection:'timeline',id:'tool-1',value:tool}]})
  assert.equal((store.state!.run.timeline).length, 2)
  assert.equal(store.state!.run.timeline![0], timeline[0])
  assert.deepEqual(store.state!.run.timeline![1], tool)
})

test('loading older timeline entries retains canonical order through subsequent live updates',()=>{
  const run=fixtureRun(),store=new RunProjection(run.id,run.workspaceId)
  store.apply({type:'bootstrap',run,revision:5,cursor:5})
  store.prependTimeline({runId:run.id,workspaceId:run.workspaceId}, {entries:[{id:'older-1',seq:0,kind:'message',role:'user',text:'Avant'},run.timeline![0]!],before:null,hasMore:false})
  store.apply({type:'delta',runId:run.id,workspaceId:run.workspaceId,baseRevision:5,revision:6,cursor:6,ops:[{collection:'timeline',id:'next',value:{id:'next',seq:7,kind:'message',role:'assistant',text:'Après'}}]})
  assert.deepEqual(store.state!.run.timeline!.map(entry=>entry.id), ['older-1','answer-1','tool-1','next'])
})
