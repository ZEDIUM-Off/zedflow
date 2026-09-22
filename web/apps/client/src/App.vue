<script setup lang="ts">
import { shallowRef } from 'vue'
import { flowPackageInventory, jsonValueSchema } from '@zedflow/sdk'
import { executedDefinitionRequest, executedSourceQuery, type InspectionSelection } from './composables/useExecutedDefinition'
import type { JsonValue } from '@zedflow/sdk'

const client=useClient()
import { useClient } from '@zedflow/vue'

import { useReloadRefs } from './composables/reloadState'
import { computed, ref, watch } from 'vue'
import { ArrowUpRight, Check, Code2, GitBranch, PanelLeft, PanelRight, Plus, Save, X, ChevronDown, Layers, Copy, SlidersHorizontal } from 'lucide-vue-next'
import type { Composition, FlowFile, RunSummary, SessionExportResponse, SessionImportResponse } from '@zedflow/sdk'
import { downloadCargoProject, type ExportFile } from './archive'
import AppFooter from './components/AppFooter.vue'
import ExecutionComposer from './components/ExecutionComposer.vue'
import AutonomousExecution from './components/AutonomousExecution.vue'
import AutonomousRunHistory from './components/AutonomousRunHistory.vue'
import { useZedflow, statuses } from './composables/useZedflow'
import WorkspaceSidebar from './components/WorkspaceSidebar.vue'
import FlowLibrary from './components/FlowLibrary.vue'
import FlowDesigner from './components/FlowDesigner.vue'
import ContextStudio from './components/context/ContextStudio.vue'
import ContextLibrary from './components/context/ContextLibrary.vue'
import ContextPackageDialog from './components/context/ContextPackageDialog.vue'
import DraftRunDialog from './components/DraftRunDialog.vue'
import BridgeStudio from './components/composition/BridgeStudio.vue'
import BridgeLibrary from './components/composition/BridgeLibrary.vue'
import RuntimePreparationDialog from './components/composition/RuntimePreparationDialog.vue'
import { useBridgeStudio, type RuntimePreparation } from './compositionEngine'

import { defaultContextValue, useContextStudio } from './contextEngine'

import DirectoryBrowser from './components/DirectoryBrowser.vue'
import SessionContextPopover from './components/SessionContextPopover.vue'
import SessionInspector from './components/SessionInspector.vue'
import ChatTimeline from './components/ChatTimeline.vue'
import AppDialog from './components/AppDialog.vue'
import GraphSettings from './components/GraphSettings.vue'
import { Conversation, ConversationContent, ConversationScrollButton } from './components/ai-elements/conversation'

const app=useZedflow()
const {mode,workspaces,workspaceId,workspace,flows,history,current,doc,designFile,designWorkspaceId,dirty,error,notice,busy,health,models,context,modelEntries,bindings,events,live,activeComposition,savedCompositions}=app
const designSection=ref<'flows'|'context'|'bridges'>('flows')
const contextStudio=useContextStudio(workspaceId,computed(()=>mode.value==='design'&&designSection.value==='context'))
const bridgeStudio=useBridgeStudio(workspaceId,computed(()=>mode.value==='design'&&designSection.value==='bridges'))
const compositionOpen=ref(false),packageOpen=ref(false),draftRunOpen=ref(false)
function selectComposition(preparation:RuntimePreparation){app.configureRuntime(preparation)}
async function launchComposition(preparation:RuntimePreparation,input:JsonValue){app.configureRuntime(preparation);await app.task('Lancement de la composition',()=>app.launchRuntime(input))}
const flowDesignMounted=ref(false)
watch([mode,designSection],()=>{if(mode.value==='design'&&designSection.value==='flows')flowDesignMounted.value=true},{immediate:true})
async function openContextStrategy(key?:string){
  if(designWorkspaceId.value&&designWorkspaceId.value!==workspaceId.value)await app.selectWorkspace(designWorkspaceId.value,false)
  mode.value='design';designSection.value='context'
  if(key){await contextStudio.refresh();const file=contextStudio.session.files.find(file=>file.key===key);if(file)await contextStudio.open(file)}
}
const sidebarOpen=ref(window.innerWidth>=760),browserOpen=ref(false),paletteOpen=ref(false),settingsOpen=ref(false),sourceOpen=ref(false),source=ref('')
const shareTitle=ref('Exporter la session')
const packageInventory=ref<Awaited<ReturnType<typeof flowPackageInventory>>>([])
const sourceFiles=shallowRef<ExportFile[]>([]),sourceRunId=ref<string>(),sourceTitle=ref('Rust du flow')
const sourceSelection=ref<{workspaceId:string;nodePath?:string;occurrenceId?:string;hash:string}>()
let sourceRequest=0
const propertiesOpen=ref(window.innerWidth>=760)
const inspectorOpen=ref(false),inspectorTab=ref<'activity'|'models'|'context'|'state'>('activity'),selectedPath=ref(''),focusRevision=ref(0)
const inspectorExpanded=ref(false),selectedOccurrence=ref<string>()
const drafts=ref<Record<string,string>>({})
const draftWaits=ref<Record<string,string|undefined>>({})
const draftKey=computed(()=>current.value?.id||`new:${workspaceId.value}:${app.executionFlow.value?activeComposition.value.id:'default'}`)
watch(draftKey,(next,previous)=>{
  if(previous?.startsWith('new::')&&next.startsWith('new:')&&drafts.value[previous]&&!drafts.value[next]){
    drafts.value[next]=drafts.value[previous];draftWaits.value[next]=draftWaits.value[previous]
    delete drafts.value[previous];delete draftWaits.value[previous]
  }
})
const autonomousInputs=ref<Record<string,JsonValue>>({})
useReloadRefs('composer',{drafts,draftWaits,autonomousInputs,designSection})
const autonomousKey=computed(()=>`${draftKey.value}:${app.runtimePreparation.value?.selection.entry||'legacy'}`)
const autonomousInput=computed({get:()=>autonomousInputs.value[autonomousKey.value]??defaultContextValue(app.runtimePreparation.value?.inputType||{kind:'text'},app.runtimePreparation.value?.overview.types),set:(value:JsonValue)=>{autonomousInputs.value[autonomousKey.value]=value}})
function newAutonomous(){const file=flows.value.find(file=>file.key===current.value?.flowRef?.key)||app.executionFlow.value;if(file)app.chooseFlow(file);else openDefinition()}
const currentTextWait=()=>current.value?.status==='waiting'&&current.value.wait?.kind!=='model_selection'?current.value.wait?.id:undefined
const chatDraft=computed({get:()=>drafts.value[draftKey.value]||'',set:(value:string)=>{if(!drafts.value[draftKey.value]?.trim())draftWaits.value[draftKey.value]=currentTextWait();drafts.value[draftKey.value]=value;if(!value.trim())draftWaits.value[draftKey.value]=undefined}})
const chatDraftWait=computed({get:()=>draftWaits.value[draftKey.value],set:(value:string|undefined)=>{draftWaits.value[draftKey.value]=value}})
async function sendChat(text:string,kind:'steering'|'followup',waitId?:string,nodePath?:string){const key=draftKey.value;try{await app.respond(text,kind,waitId,nodePath);if(drafts.value[key]===text)drafts.value[key]=''}catch(cause){if(!drafts.value[key]){drafts.value[key]=text;draftWaits.value[key]=waitId}throw cause}}
watch(()=>current.value?.id,()=>{selectedPath.value='';selectedOccurrence.value=undefined;focusRevision.value=0})
function details(tab:'activity'|'models'|'context'|'state'='activity'){inspectorTab.value=tab;inspectorOpen.value=true}
function inspect(selection:string|InspectionSelection,reveal=true,keepTab=false){
  const target=typeof selection==='string'?{nodePath:selection}:selection
  selectedPath.value=target.nodePath
  selectedOccurrence.value=target.occurrenceId||current.value?.activities?.filter(activity=>(activity.path||activity.node)===target.nodePath).at(-1)?.occurrenceId
  if(reveal)focusRevision.value++
  inspectorOpen.value=true;if(!keepTab)inspectorTab.value='activity'
}
const shareOpen=ref(false),shareResult=shallowRef<SessionExportResponse|null>(null),importOpen=ref(false),importPath=ref(''),importWorkspace=ref('')
async function exportSession(run:RunSummary){if(run.status==='running'||run.runtimeActive)return;shareTitle.value=run.interactive===false?'Exporter l’exécution':'Exporter la session';shareResult.value=null;shareOpen.value=true;await app.task('Export de la session',async()=>{shareResult.value=await client.sessions.export({workspaceId:run.workspaceId||workspaceId.value,sessionIds:[run.id]})})}
function showImport(){importWorkspace.value=workspaceId.value;importPath.value='';importOpen.value=true}
async function downloadSession(){
  const exported=shareResult.value;if(!exported)return
  await app.task('Téléchargement de la session',async()=>{
    const bytes=await client.sessions.downloadExport(exported.downloadUrl)
    const url=URL.createObjectURL(new Blob([new Uint8Array(bytes)],{type:'application/zip'}))
    try{const link=document.createElement('a');link.href=url;link.download='zedflow-sessions.zip';link.click()}finally{URL.revokeObjectURL(url)}
  })
}
async function importSession(){await app.task('Import de la session',async()=>{const result=await client.sessions.import({workspaceId:importWorkspace.value,path:importPath.value});await app.refresh();if(result.runs[0])await app.openSession(result.runs[0]);notice.value=result.imported?'Session importée. La reprise reste une action explicite.':'Cette session est déjà importée.';importOpen.value=false})}
async function openFolder(path:string){await app.openWorkspace(path);if(!error.value){browserOpen.value=false;if(window.innerWidth<760)sidebarOpen.value=false}}
async function openSession(run:RunSummary){await app.openSession(run);if(window.innerWidth<760)sidebarOpen.value=false}
async function newSession(id?:string){await app.newSession(id);if(window.innerWidth<760)sidebarOpen.value=false}
watch(mode,()=>{if(window.innerWidth<760)sidebarOpen.value=false})
async function showRust(compile=false,runId?:string){
  const request=++sourceRequest
  packageInventory.value=[];sourceOpen.value=true;source.value='';sourceFiles.value=[];sourceRunId.value=runId;sourceSelection.value=undefined
  sourceTitle.value=runId?'Rust exécuté':'Rust du flow'
  await app.task(compile?'Compilation Rust':'Chargement du Rust',async()=>{
    if(runId&&!compile){
      const active=current.value?.id===runId?current.value:undefined,run=active||history.value.find(run=>run.id===runId)
      const path=active?selectedPath.value:'',occurrence=active&&path?(selectedOccurrence.value||active.activities?.filter(item=>(item.path||item.node)===path).at(-1)?.occurrenceId):undefined
      const owner=run?.workspaceId||workspaceId.value,value=await client.definitions.load(executedDefinitionRequest({...run,id:runId,name:run?.name||'',status:run?.status||'',workspaceId:owner},path,occurrence))
      if(!value?.exact)throw new Error(value&&!value.exact?value.diagnostic.message:'La source exacte de cette occurrence est indisponible.')
      if(request===sourceRequest){sourceTitle.value=path?'Rust exécuté':'Rust initial du run';source.value=value.source;sourceSelection.value={workspaceId:owner,...executedSourceQuery(path,path?(value.occurrenceId??occurrence):undefined),hash:value.definitionRevision??value.hash};notice.value=path?'Source exacte du passage sélectionné':'Définition racine et graphe initial figés à l’admission'}
      return
    }
    const result=await app.generate(compile,runId)
    if(request!==sourceRequest)return
    sourceFiles.value=result.files
    if(!runId&&designFile.value?.package)packageInventory.value=await flowPackageInventory(designFile.value.package)
    source.value=result.files.map(file=>`// ${file.path}${file.encoding==='base64'?' · contenu binaire encodé en base64':''}\n${file.content}`).join('\n\n')
    if(compile&&!result.success)throw new Error(typeof result.output==='string'?result.output:JSON.stringify(result.output))
    notice.value=compile?'Compilation Cargo réussie':'Source Rust du flow et projet exportable'
  })
}
async function downloadRust(){
  const runId=sourceRunId.value,request=sourceRequest,selection=sourceSelection.value
  await app.task('Préparation du projet Cargo',async()=>{
    const files=sourceFiles.value.length?sourceFiles.value:(await app.generate(false,runId,selection)).files
    if(request===sourceRequest)sourceFiles.value=files
    downloadCargoProject(files,runId?`zedflow-session-${runId}.zip`:'zedflow-flow.zip')
  })
}

function openDefinition(){
  designSection.value='flows'
  const key=current.value?.flowRef?.key||(!current.value?app.executionFlow.value?.key:undefined)
  const file=key?flows.value.find(file=>file.key===key):flows.value.find(file=>file.composition?.id===activeComposition.value.id)
  if(file){editFlow(file);return}
  mode.value='design'
  notice.value=current.value?.hasFlowSource||current.value?.flowSource
    ?'Le fichier de ce flow n’est plus disponible. Le brouillon est conservé ; le Rust exécuté reste accessible dans les détails de la session.'
    :'Ce flow n’est pas disponible dans la bibliothèque. Le brouillon de conception est conservé.'
}
const actionOpen=ref(false),actionName=ref(''),actionScope=ref<'workspace'|'global'>('workspace')
type Action={kind:'saveas'}|{kind:'rename'|'duplicate'|'delete'|'convert';file:FlowFile}|{kind:'session';run:RunSummary}|{kind:'switch';file:FlowFile}|{kind:'create';template:'harness'|'tools'|'interactive'|'autonomous'}
const action=shallowRef<Action|null>(null)
const actionTitle=computed(()=>action.value?.kind==='convert'?'Convertir en package Rust':action.value?.kind==='saveas'?'Enregistrer sous':action.value?.kind==='duplicate'?'Dupliquer le flow':action.value?.kind==='delete'?'Supprimer le flow':action.value?.kind==='session'?'Renommer la session':['switch','create'].includes(action.value?.kind||'')?'Conserver le brouillon ?':'Renommer le flow')
function openAction(value:Action){action.value=value;actionName.value=value.kind==='session'?value.run.name:value.kind==='saveas'?doc.value.name:'file' in value?value.file.name:'';if(value.kind==='duplicate')actionName.value+=' · copie';actionScope.value='workspace';actionOpen.value=true}
function editFlow(file:FlowFile){designSection.value='flows';if(dirty.value){if(designFile.value?.key===file.key){mode.value='design';return}openAction({kind:'switch',file});return}app.openDesign(file)}
function createFlow(kind:'harness'|'tools'|'interactive'|'autonomous'){designSection.value='flows';if(dirty.value){openAction({kind:'create',template:kind});return}app.createFlow(kind)}
async function performAction(){const value=action.value;if(!value)return;if(value.kind==='saveas')await app.save(actionScope.value,true,actionName.value);else if(value.kind==='duplicate')await app.duplicateFlow(value.file,actionScope.value,actionName.value);else if(value.kind==='rename')await app.renameFlow(value.file,actionName.value);else if(value.kind==='delete')await app.removeFlow(value.file);else if(value.kind==='convert')await app.convertPackage(value.file);else if(value.kind==='session')await app.renameSession(value.run,actionName.value);else if(value.kind==='switch'||value.kind==='create'){await app.save();if(!error.value){if(value.kind==='switch')app.openDesign(value.file);else app.createFlow(value.template)}}if(!error.value)actionOpen.value=false}
function discardAndOpen(){if(action.value?.kind==='switch')app.openDesign(action.value.file);else if(action.value?.kind==='create')app.createFlow(action.value.template);actionOpen.value=false}
function convertedFlow(value:Composition){
  doc.value=value;designFile.value=null;app.designWorkspaceId.value ||= workspaceId.value;
  app.notice.value='Nouvelle copie Contexte → Modèle. Enregistrez-la pour conserver ce flow.';
}
const exampleOpen=ref(false), exampleCwd=ref('../docs')
async function installExample(){await app.task('Création de l’exemple Working System',async()=>{
  const result=await client.flows.installWorkingSystemExample({workspaceId:workspaceId.value,workingDirectory:exampleCwd.value});
  await app.refresh();await contextStudio.refresh();await bridgeStudio.refresh();
  exampleOpen.value=false;editFlow(result.root);
  app.notice.value='Exemple prêt : dans Utiliser, activez le bridge working-system et choisissez les deux modèles.';
})}
async function refreshLibrary(){await app.task('Actualisation',()=>app.refresh())}

</script>
<template>
<div class="app-shell redesigned-shell">
 <ContextPackageDialog v-model:open="packageOpen" :workspace-id="workspaceId" :workspaces="workspaces" @imported="app.refresh();contextStudio.refresh();bridgeStudio.refresh()"/>
 <DraftRunDialog v-model:open="draftRunOpen" :composition="doc" :models="models" :busy="!!busy" :launch="app.testDraft"/>
 <div class="workspace-body">
  <WorkspaceSidebar :design-label="designSection==='context'?'Contexte':designSection==='bridges'?'Bridges':'Flows'" v-if="sidebarOpen" :mode="mode" :workspaces="workspaces" :runs="history" :workspace-id="workspaceId" :current-id="current?.id" @mode="mode=$event" @open="browserOpen=true" @new-session="newSession" @session="openSession" @rename="openAction({kind:'session',run:$event})" @export="exportSession" @import="showImport" @close="app.closeWorkspace" @hide="sidebarOpen=false">
    <template #design>
      <nav class="ctx-design-tabs" aria-label="Espace de conception"><button :aria-pressed="designSection==='flows'" @click="designSection='flows'">Flows</button><button :aria-pressed="designSection==='context'" @click="designSection='context'">Contexte</button><button :aria-pressed="designSection==='bridges'" @click="designSection='bridges'">Bridges</button></nav>
      <FlowLibrary v-show="designSection==='flows'" :flows="flows" :workspaces="workspaces" :workspace-id="workspaceId" :selected-key="designFile?.key" :busy="!!busy" @open="editFlow" @convert="openAction({kind:'convert',file:$event})" @create="createFlow" @rename="openAction({kind:'rename',file:$event})" @duplicate="openAction({kind:'duplicate',file:$event})" @delete="openAction({kind:'delete',file:$event})" @refresh="refreshLibrary" @workspace="app.task('Changement de workspace',()=>app.selectWorkspace($event,false))"/>
      <ContextLibrary v-show="designSection==='context'" :studio="contextStudio" :workspaces="workspaces" @workspace="app.task('Changement de workspace',()=>app.selectWorkspace($event,false))"/>
      <BridgeLibrary v-show="designSection==='bridges'" :studio="bridgeStudio" :workspaces="workspaces" @workspace="app.task('Changement de workspace',()=>app.selectWorkspace($event,false))"/>
    </template>
    <template #executions>
      <AutonomousRunHistory :runs="history" :workspaces="workspaces" :workspace-id="workspaceId" :current-id="current?.id" :busy="!!busy" @open="openSession" @refresh="app.refresh" @export="exportSession"/>
    </template>
  </WorkspaceSidebar>
  <button v-if="sidebarOpen" class="sidebar-scrim" aria-label="Fermer la navigation" @click="sidebarOpen=false"/>
  <main class="main">
    <header class="workspace-topbar"><div class="session-heading"><span v-if="mode==='design'">Conception</span><template v-else><span>{{current?.name||(app.interactive.value?'Nouvelle session':'Nouvelle exécution')}}</span><small>{{workspace?.name}}</small></template></div><div class="topbar-actions"><button v-if="mode==='design'" @click="exampleOpen=true">Exemple multiflow</button><button v-if="mode==='design'" @click="packageOpen=true">Partager des définitions</button><span v-if="current&&mode==='execution'" :class="['session-status',current.status]">{{statuses[current.status]||current.status}}</span><SessionContextPopover v-if="mode==='execution'" :workspace="workspace" :composition="activeComposition" :run="current" :bindings="bindings" @details="details"/><button v-if="mode==='execution'" class="icon-button" :aria-pressed="inspectorOpen" aria-label="Afficher les détails" @click="inspectorOpen=!inspectorOpen"><PanelRight :size="18"/></button></div></header>
    <div v-if="error" role="alert" class="banner error">{{error}}<button aria-label="Fermer l’erreur" @click="error=''">×</button></div><div v-if="notice" role="status" class="banner notice">{{notice}}<button aria-label="Fermer la notification" @click="notice=''">×</button></div>
    <section v-show="mode==='design'&&designSection==='flows'" class="design-space">
      <div class="design-editor"><header class="design-toolbar"><div class="design-title"><input v-model="doc.name" class="title-input" aria-label="Nom de composition"/><small :title="designFile?.path">{{dirty?'Modifications non enregistrées':designFile?.scope==='global'?'Flow global':'Flow du workspace'}} · {{doc.nodes.length}} nœuds</small></div><div class="actions"><button v-if="(doc.formatVersion||1)<2" :disabled="!!busy" title="Créer une copie avec des capacités explicites" @click="app.convertDesign()">Convertir en v2</button><button :aria-pressed="paletteOpen" aria-label="Ajouter des nœuds" title="Ajouter des nœuds" @click="paletteOpen=!paletteOpen"><Plus :size="15"/><span>Nœuds</span></button><button aria-label="Propriétés du nœud" title="Propriétés du nœud" :aria-pressed="propertiesOpen" @click="propertiesOpen=!propertiesOpen"><SlidersHorizontal :size="15"/></button><button aria-label="Paramètres ADK" title="Paramètres ADK" @click="settingsOpen=true"><Layers :size="15"/></button><button aria-label="Afficher le Rust" title="Afficher le Rust" :disabled="!!busy" @click="showRust()"><Code2 :size="15"/></button><button aria-label="Compiler le flow" title="Compiler le flow" :disabled="!!busy" @click="showRust(true)"><Check :size="15"/></button><button aria-label="Enregistrer sous" title="Enregistrer sous" :disabled="!!busy" @click="openAction({kind:'saveas'})"><Copy :size="15"/></button><button :disabled="!!busy" @click="app.save()"><Save :size="15"/><span>Enregistrer</span></button><button :disabled="!!busy" @click="draftRunOpen=true">Tester le brouillon</button><button class="primary" :disabled="!!busy" @click="app.launchDesign()"><ArrowUpRight :size="15"/><span>Utiliser</span></button></div></header>
        <FlowDesigner @convert="convertedFlow" :active="mode==='design'&&designSection==='flows'" v-if="flowDesignMounted" :key="doc.id" v-model="doc" v-model:properties-open="propertiesOpen" :saved="savedCompositions" :context="context" :palette-open="paletteOpen" :workspace-id="designWorkspaceId||workspaceId" @context="openContextStrategy"/>
      </div>
    </section>
    <ContextStudio v-show="mode==='design'&&designSection==='context'" :studio="contextStudio" :active="mode==='design'&&designSection==='context'"/>
    <BridgeStudio v-show="mode==='design'&&designSection==='bridges'" :studio="bridgeStudio" :flows="flows" :active="mode==='design'&&designSection==='bridges'"/>
    <div v-if="mode==='execution'&&current?.preview" class="banner notice preview-run-banner"><span>Workspace temporaire · {{current.workspacePath}}</span><button @click="app.returnPreviewDraft().then(()=>{designSection='flows'})">Retour au brouillon</button></div>
    <section v-show="mode==='execution'" class="execution-space" :class="{'inspector-expanded':inspectorOpen&&inspectorExpanded}">
      <AutonomousExecution v-if="!app.interactive.value" v-show="!inspectorOpen||!inspectorExpanded" v-model:input="autonomousInput" :run="current" :composition="activeComposition" :flows="flows" :flow-key="app.executionFlow.value?.key" :prepared="app.runtimePreparation.value" :models="models" :nodes="modelEntries" :bindings="bindings" :busy="!!busy" :initialized="app.initialized.value" @launch="app.task('Lancement de l’exécution',()=>app.launchAutonomous($event))" @flow="app.chooseFlow" @composition="current?details():compositionOpen=true" @definition="openDefinition" @inspect="inspect" @details="details" @change="app.changeModel" @answer="(value,waitId)=>app.task('Réponse',()=>app.answerWait(value,waitId))" @command="app.runCommand" @earlier="app.loadEarlierTimeline" @new="newAutonomous"/>
      <div v-else v-show="!inspectorOpen||!inspectorExpanded" class="conversation-pane"><Conversation :key="current?.id||workspaceId" initial="instant"><ConversationContent class="conversation-content">
        <div v-if="!current" class="chat-welcome"><GitBranch :size="28"/><h1>Que souhaitez-vous faire ?</h1><p>Travaillez dans {{workspace?.name||'votre workspace'}} avec {{activeComposition.name}}.</p><button :disabled="!!busy||!workspaceId" @click="app.task('Lancement',()=>app.respond('Explorer les possibilités du workspace','steering'))">Explorer ce workspace <ArrowUpRight :size="14"/></button></div>
        <button v-if="current?.timelineHasMore&&current.timelineBefore!=null" class="load-earlier" :disabled="!!busy" @click="app.loadEarlierTimeline()">Charger les messages précédents</button>
        <ChatTimeline v-memo="[current?.id,current?.timeline,current?.activities]" :run="current" @inspect="inspect"/>
        <div v-if="current?.status==='running'" class="activity-inline"><span class="live-dot"/> En cours…</div><div v-if="current?.error" class="banner error">{{current.error}}</div>
        <div v-if="current&&['stopped','interrupted'].includes(current.status)" class="stopped-card"><strong>{{current.status==='stopped'?'Session arrêtée':'Session interrompue'}}</strong><p>Les résultats acquis et les messages en attente sont conservés.</p><button :disabled="!!busy" @click="app.runCommand('resume')">Reprendre le travail</button></div>
      </ConversationContent><ConversationScrollButton aria-label="Aller à la dernière réponse"/></Conversation>
      <ExecutionComposer :composed="!!(current?.runtimeGraphSummary||app.runtimePreparation.value)" @composition="current?details():compositionOpen=true" :key="draftKey" :initializing="!app.initialized.value" v-model:draft="chatDraft" v-model:draft-wait="chatDraftWait" :run="current" :workspace="workspace" :composition="activeComposition" :flows="flows" :models="models" :nodes="modelEntries" :bindings="bindings" :context="context" :busy="!!busy" :submit="sendChat" @flow="app.chooseFlow" @definition="openDefinition" @details="details" @change="app.changeModel" @answer="(value,waitId)=>app.task('Réponse',()=>app.answerWait(value,waitId))" @remove="app.removeMessage" @command="app.runCommand" @new-session="app.newSession()"/>
      </div>
      <SessionInspector @copy-context="(program,block)=>{contextStudio.fromFrozen(program,block);inspectorOpen=false;mode='design';designSection='context'}" v-model:expanded="inspectorExpanded" v-model:open="inspectorOpen" v-model:tab="inspectorTab" :composition="activeComposition" :run="current" :nodes="modelEntries" :models="models" :bindings="bindings" :context="context" :events="events" :selected-path="selectedPath" :selected-occurrence="selectedOccurrence" :focus-revision="focusRevision" :busy="!!busy" @inspect="inspect" @change="app.changeModel" @activate="app.activateCapability" @source="showRust(false,current?.id)"/>
    </section>
  </main>
 </div>
 <AppFooter :mode="mode" :hostname="app.daemon.hostname.value" :connected="app.daemon.connected.value" :health="health" :live="live" :busy="busy" :path="workspace?.path" @mode="mode=$event" @navigation="sidebarOpen=true" @retry="app.daemon.check().catch(()=>{})" @resync="current&&app.openSession(current)"/>
  <RuntimePreparationDialog v-model:open="compositionOpen" :workspace-id="workspaceId" :flows="flows" :models="models" :flow-key="app.executionFlow.value?.key" :selection="app.runtimePreparation.value" @use="selectComposition" @launch="launchComposition"/>
  <AppDialog v-model:open="shareOpen" :title="shareTitle"><p v-if="busy" class="muted">Création de l’export complet…</p><p v-if="error" role="alert" class="field-error">{{error}}</p><div v-for="item in shareResult?.exports||[]" :key="item.sessionId" class="session-share-result"><p>Les résultats, le flow exécuté, les checkpoints et les fichiers associés ont été exportés.</p><code>{{item.path}}</code></div><footer><button v-if="shareResult" class="button-link" @click="downloadSession">Télécharger le ZIP</button><button @click="shareOpen=false">Fermer</button></footer></AppDialog>
  <AppDialog v-model:open="importOpen" title="Importer une session"><form class="session-import-form" @submit.prevent="importSession"><label>Workspace cible<select v-model="importWorkspace" aria-label="Workspace cible"><option v-for="item in workspaces" :key="item.id" :value="item.id">{{item.name}} · {{item.path}}</option></select></label><label>Chemin de l’export sur le daemon<input v-model="importPath" aria-label="Chemin de l’export" placeholder="/chemin/.zedflow/sessions/session-id" required/></label><small>Dossier exporté ou archive ZIP accessible sur la machine du daemon.</small><p v-if="error" role="alert" class="field-error">{{error}}</p><footer><button type="button" @click="importOpen=false">Annuler</button><button class="primary" :disabled="!!busy||!importPath.trim()||!importWorkspace">Importer</button></footer></form></AppDialog>
  <DirectoryBrowser v-model:open="browserOpen" :initial-path="workspace?.path" :busy="!!busy" @select="openFolder"/>
  <AppDialog v-model:open="exampleOpen" title="Exemple Working System"><form class="flow-action-form" @submit.prevent="installExample"><p>Crée deux flows indépendants et un bridge : le harness transmet la demande au flow documentaire, attend son résultat puis prépare la réponse.</p><label>Répertoire du flow Working System<input v-model="exampleCwd" required placeholder="../docs"/></label><small>Chemin sur la machine du daemon, relatif au workspace ou absolu. Chaque flow conserve ses propres outils et son contexte.</small><p v-if="error" role="alert" class="field-error">{{error}}</p><footer><button type="button" @click="exampleOpen=false">Annuler</button><button class="primary" :disabled="!!busy||!exampleCwd.trim()" type="submit">Créer l’exemple</button></footer></form></AppDialog>
  <AppDialog v-model:open="settingsOpen" :title="`Paramètres ADK · ${doc.name}`" wide><GraphSettings :doc="doc"/></AppDialog>
  <AppDialog v-model:open="sourceOpen" :title="sourceTitle" wide><p v-if="busy" class="banner">{{busy}}…</p><p v-if="error" role="alert" class="banner error">{{error}}</p><details v-for="pkg in packageInventory" :key="pkg.revision" :open="pkg.root"><summary>{{pkg.manifest.name}} · entrée {{pkg.manifest.entry}}</summary><p>Révision du package : <code>{{pkg.revision}}</code></p><p v-if="pkg.root&&designFile?.sourceHash">Hash de l’entrée Rust : <code>{{designFile.sourceHash}}</code></p><ul><li v-for="file in pkg.files" :key="file.path"><code>{{file.path}}</code> · {{file.byteLength}} octets · SHA-256 <code>{{file.sha256}}</code></li></ul></details><pre class="source-preview">{{source}}</pre><footer><small>{{sourceRunId?'Le projet Cargo conserve la version exacte affichée.':'Source conservée et modules du projet Cargo.'}}</small><button :disabled="!!busy||(!sourceFiles.length&&!source)" @click="downloadRust">Télécharger le projet Cargo</button></footer></AppDialog>
  <AppDialog v-model:open="actionOpen" :title="actionTitle"><form class="flow-action-form" @submit.prevent="performAction"><template v-if="action?.kind==='convert'"><p>Convertir « {{action.file.name}} » en package Rust ?</p><p>Le fichier historique sera retiré et ses références dans les bridges enregistrés seront actualisées. Les sessions existantes conservent leurs sources figées. Le brouillon ouvert reste conservé.</p><small>{{action.file.path}}</small></template><template v-else-if="action?.kind==='delete'"><p>Supprimer « {{action.file.name}} » de la bibliothèque ? Les sessions existantes conservent leur version.</p><small>{{action.file.path}}</small></template><p v-else-if="['switch','create'].includes(action?.kind||'')">Le flow courant contient des modifications non enregistrées.</p><label v-else>Nom<input v-model="actionName" required autofocus maxlength="240"/></label><label v-if="action?.kind==='saveas'||action?.kind==='duplicate'">Emplacement<select v-model="actionScope"><option value="workspace">Workspace · .zedflow/flows</option><option value="global">Global · ~/.zedflow/flows</option></select></label><p v-if="error" role="alert" class="field-error">{{error}}</p><footer><button v-if="['switch','create'].includes(action?.kind||'')" type="button" @click="discardAndOpen">Ouvrir sans enregistrer</button><button v-else type="button" @click="actionOpen=false">Annuler</button><button :class="action?.kind==='delete'?'danger':'primary'" :disabled="!!busy||(!['delete','convert','switch','create'].includes(action?.kind||'')&&!actionName.trim())" type="submit">{{action?.kind==='convert'?'Convertir en package':action?.kind==='delete'?'Supprimer':['switch','create'].includes(action?.kind||'')?'Enregistrer et ouvrir':'Enregistrer'}}</button></footer></form></AppDialog>
</div>
</template>
