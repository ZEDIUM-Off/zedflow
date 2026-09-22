import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createRenderer, nextTick, ref } from 'vue'
import { clientKey } from '@zedflow/vue'
import { createClient, type ContextStrategy } from '@zedflow/sdk'
import { useContextStudio } from '../src/contextEngine'

// Same no-DOM Vue renderer seam as runDetails.test.ts; SDK responses remain schema-checked.
const renderer=createRenderer<object,object>({
  insert(){},remove(){},createElement(){return {}},createText(){return {}},createComment(){return {}},
  setText(){},setElementText(){},parentNode(){return null},nextSibling(){return null},patchProp(){},
})
const flush = async () => { for (let i=0;i<20;i++) await nextTick() }
function deferred<T>() { let resolve!: (value:T)=>void; const promise=new Promise<T>(r=>resolve=r); return {promise,resolve} }
const json=(body:unknown,status=200)=>new Response(JSON.stringify(body),{status})
const file=(strategy:ContextStrategy)=>({key:strategy.id,path:`/${strategy.id}.rs`,hash:'saved-hash',source:'// saved exact',strategy,diagnostics:[]})
const preview=(strategy:ContextStrategy)=>({selection:{strategy,source:'// preview',hash:'preview-hash'},evaluation:{complete:true,items:[],capabilities:[],needs:[],diagnostics:[]}})
function setup(t: import('node:test').TestContext) {
  t.mock.timers.enable({apis:['setTimeout']})
  Object.defineProperty(globalThis,'window',{configurable:true,value:new EventTarget()})
  const saves: {strategy:ContextStrategy; reply:ReturnType<typeof deferred<Response>>}[]=[], previews:typeof saves=[]
  const client=createClient({baseUrl:'https://fixture.invalid/api',protocol:1,fetch:async(url,init)=>{
    const path=new URL(url).pathname
    if(init?.method!=='POST')return json([])
    const body=JSON.parse(String(init.body))
    if(path.endsWith('/convert'))return json({valid:true,strategy:{...body.strategy,id:'converted',version:2},diagnostics:[]})
    const request={strategy:body.strategy||body.selection.strategy,reply:deferred<Response>()}
    ;(path.endsWith('/preview')?previews:saves).push(request)
    return request.reply.promise
  }})
  const workspace=ref('A'),active=ref(true)
  let studio!:ReturnType<typeof useContextStudio>
  const app=renderer.createApp({setup(){studio=useContextStudio(workspace,active);return ()=>null}})
  app.provide(clientKey,client);app.mount({})
  t.after(()=>{app.unmount();client.dispose();delete (globalThis as {window?:unknown}).window})
  studio.createExample('instructions')
  return {studio,workspace,saves,previews}
}

test('useContextStudio: save notice survives the 450ms automatic preview, not an explicit preview',async t=>{
  const {studio,saves,previews}=setup(t)
  const saving=studio.save();await flush();saves[0].reply.resolve(json(file(saves[0].strategy)));await saving;await flush()
  assert.equal(studio.current.notice,'Stratégie enregistrée en Rust')
  t.mock.timers.tick(449);await flush();assert.equal(previews.length,0)
  t.mock.timers.tick(1);await flush();assert.equal(previews.length,1)
  assert.equal(studio.current.notice,'Stratégie enregistrée en Rust')
  previews[0].reply.resolve(json(preview(previews[0].strategy)));await flush()
  assert.equal(studio.current.notice,'Stratégie enregistrée en Rust')
  const explicit=studio.preview();await flush();assert.equal(studio.current.notice,'')
  previews[1].reply.resolve(json(preview(previews[1].strategy)));await explicit
})

test('useContextStudio: edits before and after ACK cannot claim current changes were saved',async t=>{
  const {studio,saves}=setup(t)
  const saving=studio.save();await flush();studio.current.strategy.name='Edited during ACK'
  saves[0].reply.resolve(json(file(saves[0].strategy)));await saving;await flush()
  assert.equal(studio.dirty,true);assert.equal(studio.current.file?.hash,'saved-hash');assert.equal(studio.current.notice,'')
  const again=studio.save();await flush();saves[1].reply.resolve(json(file(saves[1].strategy)));await again
  assert.equal(studio.dirty,false);assert.equal(studio.current.notice,'Stratégie enregistrée en Rust')
  studio.current.strategy.name='Later edit';await flush();assert.equal(studio.current.notice,'')
})

test('useContextStudio: automatic preview errors and 409 clear confirmation and remain visible',async t=>{
  const {studio,saves,previews}=setup(t)
  const saving=studio.save();await flush();saves[0].reply.resolve(json(file(saves[0].strategy)));await saving;await flush()
  t.mock.timers.tick(450);await flush();previews[0].reply.resolve(json({error:'preview conflict'},409));await flush()
  assert.equal(studio.current.notice,'');assert.equal(studio.current.conflict,true);assert.match(studio.current.error,/preview conflict/)
})

test('useContextStudio: late save in A cannot confirm a different document or workspace',async t=>{
  const {studio,workspace,saves}=setup(t)
  const old=studio.current,saving=studio.save();await flush();studio.create();workspace.value='B';await flush()
  saves[0].reply.resolve(json(file(saves[0].strategy)));await saving
  assert.equal(studio.current.notice,'');assert.equal(old.notice,'');assert.equal(old.file?.hash,'saved-hash')
})

test('useContextStudio: conversion notice survives auto preview, then save replaces it',async t=>{
  const {studio,saves,previews}=setup(t)
  await studio.convert({});await flush();assert.match(studio.current.notice,/Copie v2 créée/)
  t.mock.timers.tick(450);await flush();assert.match(studio.current.notice,/Copie v2 créée/)
  previews[0].reply.resolve(json(preview(previews[0].strategy)));await flush()
  const saving=studio.save();await flush();saves[0].reply.resolve(json(file(saves[0].strategy)));await saving
  assert.equal(studio.current.notice,'Stratégie enregistrée en Rust')
})

test('useContextStudio: obsolete preview cannot restore confirmation or overwrite a changed draft',async t=>{
  const {studio,previews}=setup(t)
  await flush();t.mock.timers.tick(450);await flush()
  studio.current.strategy.name='Changed';studio.create();const other=studio.current
  previews[0].reply.resolve(json(preview(previews[0].strategy)));await flush()
  assert.equal(studio.current,other);assert.equal(other.preview,undefined);assert.equal(other.notice,'')
})

for(const change of ['document','workspace'] as const)test(`useContextStudio: late save after only a ${change} switch cannot confirm the old draft`,async t=>{
  const {studio,workspace,saves}=setup(t),old=studio.current,saving=studio.save();await flush()
  if(change==='document')studio.create();else workspace.value='B'
  await flush();saves[0].reply.resolve(json(file(saves[0].strategy)));await saving
  assert.equal(studio.current.notice,'');assert.equal(old.notice,'');assert.equal(old.file?.hash,'saved-hash')
})

test('useContextStudio: save 409 retains dirty changes and exposes the conflict without confirmation',async t=>{
  const {studio,saves}=setup(t),saving=studio.save();await flush()
  saves[0].reply.resolve(json({error:'source changed externally'},409));await saving
  assert.equal(studio.dirty,true);assert.equal(studio.current.notice,'');assert.equal(studio.current.conflict,true)
  assert.match(studio.current.error,/source changed externally/)
})

test('useContextStudio: explicit source clears confirmation; late preview errors cannot stain a newer revision or workspace',async t=>{
  const {studio,workspace,saves,previews}=setup(t),saving=studio.save();await flush()
  saves[0].reply.resolve(json(file(saves[0].strategy)));await saving
  const source=studio.source();await flush();assert.equal(studio.current.notice,'')
  previews[0].reply.resolve(json(preview(previews[0].strategy)));await source
  studio.current.strategy.name='New preview';await flush();t.mock.timers.tick(450);await flush()
  const old=studio.current;old.strategy.name='Newer revision';workspace.value='B';await flush()
  previews[1].reply.resolve(json({error:'obsolete preview'},409));await flush()
  assert.equal(old.error,'');assert.equal(old.notice,'');assert.equal(studio.current.error,'');assert.equal(studio.current.preview,undefined)
})

for(const diagnostic of [true,false])test(`useContextStudio: HTTP 200 preview diagnostic=${diagnostic} after save preserves persistence, but only needs preserve confirmation`,async t=>{
  const {studio,saves,previews}=setup(t),saving=studio.save();await flush()
  const saved=file(saves[0].strategy);saves[0].reply.resolve(json(saved));await saving;await flush()
  const baseline=studio.current.saved,noticeSignature=studio.current.noticeSignature
  t.mock.timers.tick(449);await flush();assert.equal(previews.length,0)
  t.mock.timers.tick(1);await flush();assert.equal(previews.length,1)
  assert.equal(studio.current.notice,'Stratégie enregistrée en Rust')
  const diagnostics=diagnostic?[{code:'value_type',path:'resources.instructions',message:'Expected text, got number'}]:[]
  previews[0].reply.resolve(json({
    ...preview(previews[0].strategy),selection:{strategy:saved.strategy,source:saved.source,hash:saved.hash},
    evaluation:{complete:false,items:[],capabilities:[],needs:diagnostic?[]:[{resource:'instructions',dataType:{kind:'text'},requiredBy:['instructions']}],diagnostics},
  }));await flush()
  assert.deepEqual(studio.current.diagnostics,diagnostics)
  assert.equal(studio.current.preview?.evaluation.complete,false)
  assert.equal(studio.current.notice,diagnostic?'':'Stratégie enregistrée en Rust')
  assert.equal(studio.current.noticeSignature,diagnostic?undefined:noticeSignature)
  assert.deepEqual(studio.current.file,saved);assert.equal(studio.current.saved,baseline)
  assert.equal(studio.current.source,saved.source);assert.equal(studio.current.sourceSignature,baseline)
  assert.equal(studio.dirty,false)
})

for(const change of ['document','workspace'] as const)test(`useContextStudio: obsolete HTTP 200 diagnostic never clears the saved confirmation in the new ${change}`,async t=>{
  const {studio,workspace,saves,previews}=setup(t)
  await flush();t.mock.timers.tick(450);await flush();assert.equal(previews.length,1)
  if(change==='document')studio.create();else workspace.value='B'
  await flush();const saving=studio.save();await flush();saves[0].reply.resolve(json(file(saves[0].strategy)));await saving
  const current=studio.current,noticeSignature=current.noticeSignature
  previews[0].reply.resolve(json({...preview(previews[0].strategy),evaluation:{complete:false,items:[],capabilities:[],needs:[],diagnostics:[{code:'value_type',path:'resources.instructions',message:'Obsolete error'}]}}));await flush()
  assert.equal(studio.current,current);assert.equal(current.notice,'Stratégie enregistrée en Rust')
  assert.equal(current.noticeSignature,noticeSignature);assert.deepEqual(current.diagnostics,[]);assert.equal(studio.dirty,false)
})

test('useContextStudio: HTTP 200 diagnostic for an older revision is not applied',async t=>{
  const {studio,previews}=setup(t)
  await flush();t.mock.timers.tick(450);await flush();assert.equal(previews.length,1)
  studio.current.strategy.name='New revision'
  previews[0].reply.resolve(json({...preview(previews[0].strategy),evaluation:{complete:false,items:[],capabilities:[],needs:[],diagnostics:[{code:'value_type',path:'resources.instructions',message:'Obsolete error'}]}}));await flush()
  assert.deepEqual(studio.current.diagnostics,[]);assert.equal(studio.current.preview,undefined)
  assert.equal(studio.current.strategy.name,'New revision')
})
