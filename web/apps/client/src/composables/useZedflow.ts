
import type { JsonValue, WorkspaceContext, ModelEntry } from '@zedflow/sdk'

import { useClient, useRun, type RunTarget } from '@zedflow/vue'
import { useReloadPart } from './reloadState'
import { computed, onMounted, onUnmounted, ref, shallowRef, watch } from 'vue'
import type { Composition, FlowFile, ModelSelection, Run, RunSummary, Workspace } from '@zedflow/sdk'
import { editableComposition, runSummarySchema, createBrowserRunTransport, createBrowserConnectivity, type StartRunInput } from '@zedflow/sdk'
import { useDaemonConnection } from './useDaemonConnection'
import { provideRunDetails } from './runDetails'
import { harnessTemplate, template, toolTemplate } from '../templates'
import { flowIsInteractive, runtimeModelNodes, type RuntimePreparation } from '../compositionEngine'

import { modelNodes } from '../harness'

// SDK documents contain JSON, never nested Vue refs. Set the initial value through
// Vue's ref setter so nested edits remain reactive without expanding recursive JSON types.
export function documentRef<T>(value:T):import('vue').Ref<T>{return Object.assign(ref<T>(),{value})}
const clone = <T>(value:T):T => JSON.parse(JSON.stringify(value))
// Vue Flow enriches bound nodes with selection and geometry. Those transient
// fields must neither dirty the authoring document nor enter the source file.
function documentValue(doc:Composition):Composition {
  return {...doc,nodes:doc.nodes.map(({id,type,position,data})=>({id,type,position:{...position},data})),edges:doc.edges.map(({id,source,target,sourceHandle,targetHandle,label})=>({id,source,target,...(sourceHandle?{sourceHandle}:{}),...(targetHandle?{targetHandle}:{}),...(label?{label}:{})}))}
}
const documentText=(doc:Composition)=>JSON.stringify(documentValue(doc))
export const statuses:Record<string,string>={running:'En cours',waiting:'Réponse attendue',completed:'Terminée',error:'Échec',interrupted:'Interrompue',stopped:'Arrêtée'}
export function useZedflow() {
  const client=useClient()
  const mode = ref<'execution'|'design'>('execution'), initialized=ref(false)
  const builtinHarness=harnessTemplate()
  const workspaces = shallowRef<Workspace[]>([]), workspaceId = ref(''), flows = shallowRef<FlowFile[]>([]), history = shallowRef<RunSummary[]>([])
  const runtimePreparation=shallowRef<RuntimePreparation|null>(null)
  const current = shallowRef<Run|null>(null), executionFlow = shallowRef<FlowFile|null>(null)
  const doc = documentRef<Composition>(clone(builtinHarness))
  const designFile = shallowRef<FlowFile|null>(null), designWorkspaceId = ref('')
  const baseline = ref(documentText(doc.value)), error = ref(''), notice = ref(''), busy = ref('')
  const daemon=useDaemonConnection(),health=daemon.health
  provideRunDetails(()=>current.value)
  const models = shallowRef<ModelEntry[]>([]), workspaceContext = shallowRef<WorkspaceContext|null>(null)
  const draftBindings = ref<Record<string,ModelSelection>>({}), events = ref<{seq:number;event:unknown}[]>([])
  const target = shallowRef<RunTarget|null>(null)
  const subscription = useRun(client,target,{
    stream:createBrowserRunTransport({baseUrl:new URL('/api',window.location.href).href,runs:client.runs,daemon:client.daemon}),
    connectivity:createBrowserConnectivity(),seen:daemon.seen,
    receive:value=>{current.value=value.run;rememberSummary(value.run)},
  })
  const live = subscription.status
  let refreshVersion=0, capabilitiesVersion=0, taskVersion=0, workspaceIntent=0, sessionIntent=0, designIntent=0
  const workspace = computed(()=>workspaces.value.find(item=>item.id===workspaceId.value))
  const dirty = computed(()=>documentText(doc.value)!==baseline.value)
  const activeComposition = computed(()=>current.value?.composition || executionFlow.value?.composition || builtinHarness)
  const modelEntries = computed(()=>current.value?.runtimeGraphSummary?runtimeModelNodes(current.value.runtimeGraphSummary):!current.value&&runtimePreparation.value?.workspaceId===workspaceId.value?runtimePreparation.value.models:modelNodes(activeComposition.value))
  const interactive=computed(()=>current.value?.interactive??(current.value?.runtimeGraphSummary?(current.value.runtimeGraphSummary.interactive??current.value.runtimeGraphSummary.instances[current.value.runtimeGraphSummary.entry.instance]?.interactive):runtimePreparation.value?(runtimePreparation.value.overview.interactive??runtimePreparation.value.overview.instances[runtimePreparation.value.overview.entry.instance]?.interactive):flowIsInteractive(activeComposition.value))??true)
  const bindings = computed(()=>current.value?.modelBindings || draftBindings.value)
  const context = computed(()=>current.value?.context || workspaceContext.value)
  const recovery=useReloadPart('flow-draft',()=>({workspaceId:workspaceId.value,mode:mode.value,doc:documentValue(doc.value),baseline:baseline.value,designFile:designFile.value,designWorkspaceId:designWorkspaceId.value,executionFlow:executionFlow.value,bindings:draftBindings.value}))
  const savedCompositions = computed(()=>flows.value.flatMap(file=>file.composition ? [file.composition] : []))
  async function task(label:string, action:()=>Promise<unknown>) {
    const version=++taskVersion
    busy.value=label;error.value='';notice.value=''
    try{return await action()}catch(cause){if(version===taskVersion)error.value=cause instanceof Error?cause.message:String(cause)}finally{if(version===taskVersion)busy.value=''}
  }
  async function refresh() {
    const version=++refreshVersion, id=workspaceId.value
    const nextWorkspaces=await client.workspaces.list()
    const [historyGroups,nextFlows]=await Promise.all([Promise.all(nextWorkspaces.filter(item=>item.open).map(item=>client.runs.list({workspaceId:item.id}))),id?client.flows.list({workspaceId:id}):Promise.resolve([])])
    const nextHistory=historyGroups.flat()
    if(version!==refreshVersion||id!==workspaceId.value)return
    workspaces.value=nextWorkspaces;history.value=nextHistory;flows.value=nextFlows
    // The draft has its own baseline: refreshing a catalogue never overwrites edits.
    if(!executionFlow.value && !current.value)executionFlow.value=nextFlows.find(file=>file.composition&&flowIsInteractive(file.composition))||null
    else if(executionFlow.value&&!current.value)executionFlow.value=nextFlows.find(file=>file.key===executionFlow.value?.key)||executionFlow.value
  }
  async function capabilities() {
    const id=workspaceId.value, version=++capabilitiesVersion
    const results=await Promise.allSettled([client.models.list({workspaceId:id}),client.context.workspace({workspaceId:id})])
    if(id!==workspaceId.value||version!==capabilitiesVersion)return
    if(results[0].status==='fulfilled')models.value=results[0].value.models
    if(results[1].status==='fulfilled')workspaceContext.value=results[1].value
  }
  function resetRun(){runtimePreparation.value=null;sessionIntent++;target.value=null;current.value=null;events.value=[];draftBindings.value={};live.value={transport:'offline'}}
  async function selectWorkspace(id:string, newSession=true){workspaceIntent++;if(id!==workspaceId.value){workspaceId.value=id;executionFlow.value=null;models.value=[];workspaceContext.value=null;if(newSession)resetRun();await Promise.all([refresh(),capabilities()])}else if(newSession){resetRun();await Promise.all([refresh(),capabilities()])}}
  async function newSession(id=workspaceId.value){mode.value='execution';await task('Ouverture',async()=>{await selectWorkspace(id);executionFlow.value=flows.value.find(file=>file.composition&&flowIsInteractive(file.composition))||null})}
  async function openWorkspace(path:string){workspaceIntent++;sessionIntent++;await task('Ouverture du workspace',async()=>{const value=await client.workspaces.open({path});workspaces.value=workspaces.value.some(item=>item.id===value.id)?workspaces.value.map(item=>item.id===value.id?value:item):[...workspaces.value,value];await selectWorkspace(value.id);mode.value='execution'})}
  async function closeWorkspace(id:string){await task('Fermeture du workspace',async()=>{await client.workspaces.update(id,{open:false});await refresh()})}
  function rememberSummary(run:RunSummary){
    const summary=runSummarySchema.parse(JSON.parse(JSON.stringify({runtimeActive:run.runtimeActive,interactive:run.interactive,id:run.id,name:run.name,workspaceId:run.workspaceId,workspacePath:run.workspacePath,status:run.status,createdAt:run.createdAt,updatedAt:run.updatedAt,flowRef:run.flowRef,error:run.error})))
    const index=history.value.findIndex(item=>item.id===run.id)
    if(index>=0&&JSON.stringify(history.value[index])===JSON.stringify(summary))return
    const list=[...history.value];if(index<0)list.unshift(summary);else list[index]=summary
    history.value=list
  }
  function subscribe(run:Run){
    if(!run.workspaceId)throw new Error('Le workspace du run est requis.')
    sessionIntent++;current.value=run;events.value=[];mode.value='execution'
    target.value={runId:run.id,workspaceId:run.workspaceId}
  }
  async function openSession(run:RunSummary){
    const intent=++sessionIntent
    await task('Ouverture de la session',async()=>{
      if(run.workspaceId)await selectWorkspace(run.workspaceId,false)
      if(intent!==sessionIntent||run.workspaceId&&run.workspaceId!==workspaceId.value)return
      const loadedRun=await client.runs.read(run.id,{workspaceId:run.workspaceId||workspaceId.value})
      if(intent===sessionIntent&&(!run.workspaceId||run.workspaceId===workspaceId.value))subscribe(loadedRun)
    })
  }
  async function renameSession(run:RunSummary,name:string){await task('Renommage',async()=>{await client.runs.rename(run.id,{name},{workspaceId:run.workspaceId||workspaceId.value});if(current.value?.id===run.id)await subscription?.refresh();else await refresh()})}
  async function loadEarlierTimeline(){
    const run=current.value,before=run?.timelineBefore
    if(!run||before===undefined||before===null)return
    await task('Chargement de la conversation',async()=>{
      const page=await client.runs.timeline(run.id,{workspaceId:run.workspaceId||workspaceId.value,before})
      if(current.value?.id!==run.id||current.value?.workspaceId!==run.workspaceId)return
      subscription.prependTimeline({runId:run.id,workspaceId:run.workspaceId||workspaceId.value},page)
    })
  }

  function openDesign(file:FlowFile){if(!file.composition){error.value=file.diagnostics.join('\n')||'Ce fichier ne peut pas être chargé dans le canvas.';return}let editable:Composition;try{editable=editableComposition(file.composition)}catch(cause){error.value=`Ce flow ne peut pas être interprété par les éditeurs : ${cause instanceof Error?cause.message:String(cause)}`;return}designIntent++;designFile.value=clone(file);doc.value=editable;designWorkspaceId.value=workspaceId.value;baseline.value=documentText(doc.value);mode.value='design'}
  function createFlow(kind:'harness'|'tools'|'interactive'|'autonomous'='harness'){designIntent++;doc.value=kind==='harness'?harnessTemplate():kind==='tools'?toolTemplate():template(kind==='interactive');designFile.value=null;designWorkspaceId.value=workspaceId.value;baseline.value='';mode.value='design'}
  async function persistDesign(scope:'workspace'|'global'='workspace',duplicate=false,name=doc.value.name){
    const intent=designIntent, initialText=documentText(doc.value), initialName=doc.value.name
    const composition=clone(documentValue(doc.value))
    const targetWorkspace=duplicate?workspaceId.value:designWorkspaceId.value||workspaceId.value
    composition.name=name
    if(duplicate){composition.id=crypto.randomUUID();composition.revision=0}
    const file=await client.flows.save({workspaceId:targetWorkspace,composition,scope,...(!duplicate&&designFile.value?{key:designFile.value.key,expectedHash:designFile.value.hash}:{})})
    if(intent===designIntent){
      const saved=editableComposition(file.composition||composition), unchanged=documentText(doc.value)===initialText
      designFile.value=clone(file);designWorkspaceId.value=targetWorkspace;baseline.value=documentText(saved)
      // The response acknowledges the submitted version. Edits made while it
      // was in flight remain a dirty draft against the new on-disk hash.
      doc.value=unchanged?saved:{...doc.value,id:saved.id,revision:saved.revision,...(doc.value.name===initialName?{name:saved.name}:{})}
    }
    await refresh();notice.value='Flow enregistré';return file
  }
  async function save(scope:'workspace'|'global'='workspace',duplicate=false,name=doc.value.name){return task('Enregistrement',()=>persistDesign(scope,duplicate,name))}
  async function removeFlow(file:FlowFile){await task('Suppression du flow',async()=>{await client.flows.remove(file.key,{workspaceId:workspaceId.value,expectedHash:file.hash});if(executionFlow.value?.key===file.key)executionFlow.value=null;if(designFile.value?.key===file.key){designFile.value=null;baseline.value=''}await refresh()})}
  async function duplicateFlow(file:FlowFile,scope:'workspace'|'global',name:string){if(!file.composition)return;await task('Copie du flow',async()=>{const composition={...clone(file.composition!),id:crypto.randomUUID(),revision:0,name};const value=await client.flows.save({workspaceId:workspaceId.value,scope,composition});await refresh();openDesign(value)})}
  async function renameFlow(file:FlowFile,name:string){if(!file.composition)return;await task('Renommage',async()=>{const value=await client.flows.save({workspaceId:workspaceId.value,key:file.key,expectedHash:file.hash,composition:{...clone(file.composition!),name}});await refresh();if(designFile.value?.key===file.key&&!dirty.value)openDesign(value)})}
  function chooseFlow(file:FlowFile){if(!file.composition)return;resetRun();executionFlow.value=file;mode.value='execution'}
  async function launchDesign(){await task('Préparation de la session',async()=>{const file=dirty.value||!designFile.value?await persistDesign():designFile.value;await selectWorkspace(designWorkspaceId.value||workspaceId.value);chooseFlow(file)})}
  async function testDraft(input:Record<string,JsonValue>,modelBindings:Record<string,ModelSelection>){
    const intent=++sessionIntent,targetWorkspace=designWorkspaceId.value||workspaceId.value,composition=clone(documentValue(doc.value))
    const acknowledgement=await client.runs.preview({workspaceId:targetWorkspace,composition,input,modelBindings:clone(modelBindings)})
    if(intent!==sessionIntent)return
    const loadedRun=await client.runs.read(acknowledgement.id,{workspaceId:acknowledgement.workspaceId||workspaceId.value})
    if(intent!==sessionIntent)return
    rememberSummary(loadedRun);subscribe(loadedRun)
    notice.value='Test du brouillon lancé dans un workspace temporaire.'
  }
  async function returnPreviewDraft(){
    const run=current.value;if(!run?.preview)return
    const intent=designIntent, initialText=documentText(doc.value)
    await task('Retour au brouillon',async()=>{
      const source=!dirty.value?await client.runs.previewSource(run.id,{workspaceId:run.workspaceId||workspaceId.value}):undefined
      if(current.value?.id!==run.id||intent!==designIntent)return
      await selectWorkspace(run.preview!.sourceWorkspaceId,false)
      if(current.value?.id!==run.id||intent!==designIntent)return
      if(source&&documentText(doc.value)===initialText&&!dirty.value){
        const matches=flows.value.filter(file=>file.composition?.id===source.composition.id)
        designIntent++;doc.value=editableComposition(source.composition);designWorkspaceId.value=source.sourceWorkspaceId
        designFile.value=matches.length===1?clone(matches[0]):null
        baseline.value=designFile.value?.composition?documentText(designFile.value.composition):''
      }
      mode.value='design'
    })
  }
  async function ensureExecutionFlow(targetWorkspace:string,intent:number){
    if(executionFlow.value?.composition)return executionFlow.value
    const file=await client.flows.save({workspaceId:targetWorkspace,scope:'workspace',composition:harnessTemplate()})
    if(intent===sessionIntent&&workspaceId.value===targetWorkspace)executionFlow.value=file
    await refresh();return file
  }
  function configureRuntime(preparation:RuntimePreparation){
    if(preparation.workspaceId!==workspaceId.value)throw new Error('Le workspace a changé. Préparez à nouveau la composition dans le workspace actif.')
    const file=flows.value.find(file=>file.key===preparation.selection.flow)
    if(!file?.composition)throw new Error('Le flow d’entrée n’est plus disponible.')
    resetRun();runtimePreparation.value=preparation;executionFlow.value=file;draftBindings.value=clone(preparation.bindings);mode.value='execution'
  }
  async function startInput(input:JsonValue,nodePath?:string){
    const intent=++sessionIntent,targetWorkspace=workspaceId.value,modelBindings=clone(draftBindings.value),prepared=runtimePreparation.value
    let payload:Pick<StartRunInput,'runtimeSelection'|'flowKey'|'flowHash'|'input'>
    if(prepared){
      if(prepared.workspaceId!==targetWorkspace)throw new Error('Cette composition appartient à un autre workspace.')
      payload={runtimeSelection:prepared.selection,input:{[prepared.inputField]:input}}
    }else{
      const file=await ensureExecutionFlow(targetWorkspace,intent)
      if(intent!==sessionIntent||targetWorkspace!==workspaceId.value)return
      payload={flowKey:file.key,flowHash:file.hash,input:{input}}
    }
    const acknowledgement=await client.runs.start({workspaceId:targetWorkspace,...payload,modelBindings,...(nodePath?{nodePath}:{})})
    if(intent!==sessionIntent||workspaceId.value!==targetWorkspace){await refresh();return}
    const loadedRun=await client.runs.read(acknowledgement.id,{workspaceId:acknowledgement.workspaceId||workspaceId.value})
    rememberSummary(loadedRun)
    if(intent===sessionIntent&&workspaceId.value===targetWorkspace){subscribe(loadedRun)}
  }
  async function launchAutonomous(input:JsonValue){if(current.value)throw new Error('Préparez une nouvelle exécution avant de la lancer.');await startInput(input)}
  async function start(text:string,nodePath?:string){await startInput(text,nodePath)}
  async function launchRuntime(input:JsonValue){if(!runtimePreparation.value||current.value)throw new Error('Préparez une nouvelle composition avant de la lancer.');await startInput(input)}
  async function answerWait(value:JsonValue,expectedWaitId?:string,nodePath?:string){const run=current.value;if(!run?.wait)throw new Error('Cette attente n’est plus active.');if(expectedWaitId&&expectedWaitId!==run.wait.id)throw new Error('Cette attente a changé. Vérifiez la nouvelle question avant de répondre.');await client.runs.answer(run.id,{waitId:run.wait.id,value,...(nodePath?{nodePath}:{})},{workspaceId:run.workspaceId||workspaceId.value});if(current.value?.id===run.id)await subscription?.refresh()}
  async function respond(text:string,kind:'steering'|'followup',expectedWaitId?:string,nodePath?:string){
    if(!text.trim())return
    busy.value='Envoi';error.value=''
    try{const run=current.value;if(expectedWaitId&&run?.wait?.id!==expectedWaitId)throw new Error('Cette attente a changé. Vérifiez la nouvelle question avant de répondre.');if(!run)await start(text,nodePath);else if(['stopped','interrupted'].includes(run.status)){await client.runs.resume(run.id,{text,...(nodePath?{nodePath}:{})},{workspaceId:run.workspaceId||workspaceId.value});await subscription?.refresh()}else if(run.status==='running'||run.wait?.kind==='model_selection'){await client.runs.queueMessage(run.id,{id:crypto.randomUUID(),kind,text,...(nodePath?{nodePath}:{})},{workspaceId:run.workspaceId||workspaceId.value});await subscription?.refresh()}else if(run.wait)await answerWait(text,expectedWaitId,nodePath)}catch(cause){error.value=cause instanceof Error?cause.message:String(cause);throw cause}finally{busy.value=''}
  }
  async function changeModel(path:string,selection:ModelSelection){if(!current.value){draftBindings.value={...draftBindings.value,[path]:selection};if(runtimePreparation.value)runtimePreparation.value={...runtimePreparation.value,bindings:draftBindings.value};return}await task('Configuration du modèle',async()=>{await client.runs.selectModel(current.value!.id,{nodePath:path,selection,revision:current.value!.modelRevision||0},{workspaceId:current.value!.workspaceId||workspaceId.value});await subscription?.refresh()})}
  async function runCommand(action:'abort'|'resume'){await task(action==='abort'?'Arrêt':'Reprise',async()=>{if(!current.value)return;await (action==='abort'?client.runs.abort(current.value.id,{workspaceId:current.value.workspaceId||workspaceId.value}):client.runs.resume(current.value.id,{},{workspaceId:current.value.workspaceId||workspaceId.value}));await subscription?.refresh()})}
  async function removeMessage(id:string){await task('Retrait du message',async()=>{await client.runs.removeMessage(current.value!.id,id,{workspaceId:current.value!.workspaceId||workspaceId.value});await subscription?.refresh()})}
  async function convertPackage(file:FlowFile){await task('Conversion en package Rust',async()=>{
    const owner=workspaceId.value
    const result=await client.flows.convertPackage({workspaceId:owner,key:file.key,expectedHash:file.hash})
    // Rebind catalogue identities without discarding the authored draft or run snapshots.
    if(designWorkspaceId.value===owner&&designFile.value?.key===result.oldKey){
      const unchanged=!dirty.value
      designFile.value=clone(result.flow)
      if(unchanged&&result.flow.composition){doc.value=editableComposition(result.flow.composition);baseline.value=documentText(doc.value)}
    }
    if(workspaceId.value===owner&&executionFlow.value?.key===result.oldKey)executionFlow.value=clone(result.flow)
    if(runtimePreparation.value?.workspaceId===owner)runtimePreparation.value=null
    await refresh()
    if(workspaceId.value===owner)notice.value=`Package créé : ${result.flow.name} · ${result.changedConsumers.length} références de bridges actualisées. Le brouillon ouvert est conservé.`
  })}
  async function convertDesign(){await task('Conversion du flow',async()=>{const composition=await client.flows.convert(documentValue(doc.value));doc.value=editableComposition(composition);designFile.value=null;baseline.value='';notice.value='Copie convertie en v2. Enregistrez-la pour conserver ce nouveau flow.'})}
  async function activateCapability(nodePath:string,itemId:string,active:boolean,skillName?:string){if(!current.value)return;await task('Activation du contexte',async()=>{await client.runs.activateCapability(current.value!.id,{nodePath,itemId,active,...(skillName?{skillName}:{})},{workspaceId:current.value!.workspaceId||workspaceId.value});await subscription?.refresh()})}
  async function generate(compile=false,runId?:string,selection?:{workspaceId?:string;nodePath?:string;occurrenceId?:string;hash?:string}){if(runId)return client.generation[compile?'build':'generate']({runId,workspaceId:current.value?.id===runId?current.value.workspaceId:history.value.find(run=>run.id===runId)?.workspaceId||workspaceId.value,...selection});const file=dirty.value||!designFile.value?await persistDesign():designFile.value;return client.generation[compile?'build':'generate']({workspaceId:designWorkspaceId.value||workspaceId.value,flowKey:file.key,flowHash:file.hash})}
  watch([workspaceId,()=>current.value?.id,mode],()=>{
    if(mode.value==='execution'&&workspaceId.value){try{localStorage.setItem('zedflow.last-session',JSON.stringify({workspaceId:current.value?.workspaceId||workspaceId.value,runId:current.value?.id||null}))}catch{/* Browser storage can be disabled. */}}
  })
  watch(mode,next=>{if(next==='execution'&&current.value?.workspaceId&&current.value.workspaceId!==workspaceId.value)void task('Retour à la session',()=>selectWorkspace(current.value!.workspaceId!,false))})
  const wake=()=>{if(document.visibilityState==='visible'&&!busy.value)void refresh().catch(cause=>{error.value=cause instanceof Error?cause.message:String(cause)})}
  onMounted(()=>{
    // Bootstrap may finish after the user creates a flow, edits its title, or
    // opens a different workspace. Only hydrate state that is still untouched.
    const initialDoc=doc.value, initialDocument=documentText(doc.value)
    const initialWorkspaceIntent=workspaceIntent, initialSessionIntent=sessionIntent
    let previous:{workspaceId?:string;runId?:string}={}
    try{previous=JSON.parse(localStorage.getItem('zedflow.last-session')||'{}')}catch{/* Ignore invalid local preferences. */}
    void task('Connexion',async()=>{
      await daemon.check()
      const initialWorkspaces=await client.workspaces.list()
      if(workspaceIntent===initialWorkspaceIntent||!workspaces.value.length)workspaces.value=initialWorkspaces
      if(!workspaceId.value){
        workspaceId.value=initialWorkspaces.find(item=>item.open&&item.id===previous.workspaceId)?.id||health.value?.defaultWorkspaceId||initialWorkspaces.find(item=>item.open)?.id||initialWorkspaces[0]?.id||''
      }
      if(!designWorkspaceId.value)designWorkspaceId.value=workspaceId.value
      const initialWorkspace=workspaceId.value
      await Promise.all([refresh(),capabilities()])
      if(workspaceId.value!==initialWorkspace)return
      const first=flows.value.find(file=>file.composition)
      if(first&&doc.value===initialDoc&&documentText(doc.value)===initialDocument&&!designFile.value){
        designFile.value=clone(first);doc.value=editableComposition(first.composition!);baseline.value=documentText(doc.value)
      }
      if(sessionIntent===initialSessionIntent&&workspaceIntent===initialWorkspaceIntent&&mode.value==='execution'&&!current.value){
        const previousRun=history.value.find(run=>run.id===previous.runId&&run.workspaceId===workspaceId.value)
        if(previousRun)await openSession(previousRun)
        else if(previous.runId&&previous.workspaceId&&initialWorkspaces.some(item=>item.id===previous.workspaceId)){
          // Temporary tests stay out of open-workspace history, but an explicit
          // last-run preference can still reopen their immutable result.
          try{const loadedRun=await client.runs.read({id:previous.runId,workspaceId:previous.workspaceId}.id,{workspaceId:{id:previous.runId,workspaceId:previous.workspaceId}.workspaceId||workspaceId.value})
            if(loadedRun.preview&&sessionIntent===initialSessionIntent&&workspaceIntent===initialWorkspaceIntent&&mode.value==='execution'){
              subscribe(loadedRun)
            }
          }catch{/* A removed temporary test is no longer restorable. */}
        }
      }
    }).finally(()=>{
      if(recovery&&workspaces.value.some(item=>item.id===recovery.workspaceId)){
        workspaceId.value=recovery.workspaceId;doc.value=editableComposition(recovery.doc);baseline.value=recovery.baseline
        designFile.value=recovery.designFile;designWorkspaceId.value=recovery.designWorkspaceId
        executionFlow.value=recovery.executionFlow;draftBindings.value=recovery.bindings;mode.value=recovery.mode
      }
      initialized.value=true
    })
    window.addEventListener('focus',wake);document.addEventListener('visibilitychange',wake)
  })
  onUnmounted(()=>{target.value=null;window.removeEventListener('focus',wake);document.removeEventListener('visibilitychange',wake)})
  return {returnPreviewDraft,testDraft,interactive,launchAutonomous,runtimePreparation,configureRuntime,launchRuntime,mode,initialized,workspaces,workspaceId,workspace,flows,history,current,executionFlow,doc,designFile,designWorkspaceId,dirty,error,notice,busy,health,daemon,models,context,modelEntries,bindings,events,live,activeComposition,savedCompositions,task,refresh,newSession,openWorkspace,closeWorkspace,openSession,renameSession,openDesign,createFlow,save,removeFlow,duplicateFlow,renameFlow,chooseFlow,launchDesign,respond,answerWait,changeModel,runCommand,removeMessage,generate,selectWorkspace,convertPackage,convertDesign,activateCapability,loadEarlierTimeline}
}
