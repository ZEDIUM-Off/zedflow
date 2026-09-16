<script setup lang="ts">
import type { HistoricalModelSelection } from '@zedflow/sdk'
import type { InspectionSelection } from '../composables/useExecutedDefinition'
import type { RunContext, ModelEntry } from '@zedflow/sdk'

import { computed, onMounted, onUnmounted, ref, shallowRef, watch } from 'vue'
import { DialogRoot, DialogContent, DialogTitle } from 'reka-ui'
import { Code2, GitBranch, Maximize2, Minimize2, X } from 'lucide-vue-next'
import type { Composition, ModelSelection, Run, RunStateDetail, RunEventPage } from '@zedflow/sdk'
import type { ModelNode } from '../harness'
import { passagesByNode } from '../runIndexes'
import { useRunDetails } from '../composables/runDetails'
import RunGraph from './RunGraph.vue'
import RuntimeGraphPreview from './composition/RuntimeGraphPreview.vue'
import ModelBindingsPanel from './ModelBindingsPanel.vue'
import ContextPanel from './ContextPanel.vue'
import AgentContextPanel from './AgentContextPanel.vue'
import type { ContextProgram } from '@zedflow/sdk'
import ExecutionActivity from './ExecutionActivity.vue'
import RunRevisionPanel from './RunRevisionPanel.vue'
import { useExecutedDefinition } from '../composables/useExecutedDefinition'
const expanded=defineModel<boolean>('expanded',{default:false})
const open=defineModel<boolean>('open',{required:true}), tab=defineModel<'activity'|'models'|'context'|'state'>('tab',{required:true})
const props=defineProps<{composition:Composition;run:Run|null;nodes:ModelNode[];models:ModelEntry[];bindings:Record<string,HistoricalModelSelection>;context:RunContext|null|undefined;events:{seq:number;event:unknown}[];selectedPath:string;selectedOccurrence?:string;focusRevision:number;busy:boolean}>()
const emit=defineEmits<{copyContext:[program:ContextProgram,blockId:string];inspect:[selection:string|InspectionSelection,reveal?:boolean,keepTab?:boolean];activate:[nodePath:string,itemId:string,active:boolean,skillName?:string];change:[path:string,selection:ModelSelection];source:[]}>()
const width=ref(440),narrow=ref(false)
const passages=computed(()=>props.selectedPath?passagesByNode(props.run?.activities).get(props.selectedPath)||[]:props.run?.activities||[])
const selected=computed(()=>passages.value.find(activity=>activity.occurrenceId===props.selectedOccurrence)||(props.selectedPath?passages.value.at(-1):undefined))
const revisionView=useExecutedDefinition(()=>props.run,()=>open.value,()=>props.selectedPath,()=>props.selectedOccurrence)
const exactDefinition=computed(()=>revisionView.definition.value?.exact?revisionView.definition.value:undefined)
const inspectedComposition=computed(()=>exactDefinition.value?.composition||props.composition)
const inspectedOverview=computed(()=>exactDefinition.value?.runtime||props.run?.runtimeGraphSummary)
function selectPass(id:string){const activity=passages.value.find(activity=>activity.occurrenceId===id);if(activity)emit('inspect',{nodePath:activity.path||activity.node,occurrenceId:id},false)}
const details=useRunDetails(),stateDetail=shallowRef<RunStateDetail>(),stateError=ref(''),stateLoading=ref(false)
const eventPage=shallowRef<RunEventPage>({events:[],cursor:0,hasMore:false}),eventsOpen=ref(false),eventsLoading=ref(false)
let detailRequest=0
const state=computed(()=>Object.fromEntries(Object.entries(stateDetail.value?.state||props.run?.state||{}).filter(([key])=>!key.startsWith('__zedflow:'))))
async function loadState(){
  const run=props.run;if(!run)return
  const request=++detailRequest;stateLoading.value=true;stateError.value=''
  try{const value=await details.load({...details.scope(),kind:'state',revision:run.revision});if(request===detailRequest&&props.run?.id===run.id)stateDetail.value=value}catch(error){if(request===detailRequest)stateError.value=error instanceof Error?error.message:String(error)}finally{if(request===detailRequest)stateLoading.value=false}
}
async function loadEvents(){
  const run=props.run;if(!run||eventsLoading.value)return
  eventsLoading.value=true
  try{const page:RunEventPage=await details.load({...details.scope(),kind:'event-history',revision:run.revision,query:{after:eventPage.value.cursor}});if(props.run?.id===run.id){const known=new Set(eventPage.value.events.map(event=>event.seq));eventPage.value={...page,events:[...eventPage.value.events,...page.events.filter(event=>!known.has(event.seq))]}}}finally{eventsLoading.value=false}
}
function toggleEvents(event:Event){eventsOpen.value=(event.target as HTMLDetailsElement).open;if(eventsOpen.value&&!eventPage.value.events.length)void loadEvents().catch(()=>{})}
watch(()=>props.run?.id,()=>{detailRequest++;stateDetail.value=undefined;eventPage.value={events:[],cursor:0,hasMore:false};eventsOpen.value=false;stateLoading.value=false})
watch(()=>[open.value,tab.value,props.run?.id],()=>{if(open.value&&tab.value==='state')void loadState()},{immediate:true})

function navigateTabs(event:KeyboardEvent){
  const choices=['activity','models','context','state'] as const
  const index=choices.indexOf(tab.value)
  const next=event.key==='ArrowRight'?(index+1)%choices.length:event.key==='ArrowLeft'?(index+choices.length-1)%choices.length:event.key==='Home'?0:event.key==='End'?choices.length-1:-1
  if(next<0)return
  event.preventDefault();tab.value=choices[next]!
  ;(event.currentTarget as HTMLElement).querySelectorAll<HTMLButtonElement>('[role=tab]')[next]?.focus()
}
function measure(){narrow.value=window.innerWidth<1000}
let stopResize=()=>{}
function resize(event:PointerEvent){if(narrow.value)return;event.preventDefault();const startX=event.clientX,startWidth=width.value;const move=(next:PointerEvent)=>{width.value=Math.max(310,Math.min(850,window.innerWidth-450,startWidth+startX-next.clientX))};const stop=()=>{window.removeEventListener('pointermove',move);window.removeEventListener('pointerup',stop)};stopResize=stop;window.addEventListener('pointermove',move);window.addEventListener('pointerup',stop,{once:true})}
function resizeKey(event:KeyboardEvent){if(['ArrowLeft','ArrowRight'].includes(event.key)){event.preventDefault();width.value=Math.max(310,Math.min(850,window.innerWidth-450,width.value+(event.key==='ArrowLeft'?30:-30)))}}
onMounted(()=>{measure();window.addEventListener('resize',measure)});onUnmounted(()=>{stopResize();window.removeEventListener('resize',measure)})
</script>
<template>
  <DialogRoot v-model:open="open" :modal="narrow"><div v-if="open&&narrow" class="inspector-scrim" @click="open=false"/><DialogContent class="session-inspector" :class="{expanded}" :style="{width:expanded?'100%':`${width}px`}" :aria-describedby="undefined" @interact-outside="!narrow&&$event.preventDefault()" @open-auto-focus="!narrow&&$event.preventDefault()">
    <div v-if="!narrow&&!expanded" role="separator" tabindex="0" aria-label="Redimensionner le panneau de détails" aria-orientation="vertical" :aria-valuenow="width" :aria-valuemin="310" :aria-valuemax="850" class="inspector-resizer" @pointerdown="resize" @keydown="resizeKey"/>
    <header class="inspector-header"><DialogTitle><GitBranch :size="16"/> {{run?.interactive===false?'Détails de l’exécution':'Détails de la session'}}</DialogTitle><button v-if="run?.hasFlowSource||run?.flowSource" class="inspector-source" title="Afficher le Rust de la version exécutée sélectionnée" @click="emit('source')"><Code2 :size="14"/> Rust exécuté</button><button class="icon-button" :aria-label="expanded?(run?.interactive===false?'Revenir aux résultats':'Revenir au chat'):'Agrandir le graphe'" :title="expanded?(run?.interactive===false?'Revenir aux résultats':'Revenir au chat'):'Agrandir le graphe'" @click="expanded=!expanded"><Minimize2 v-if="expanded" :size="17"/><Maximize2 v-else :size="17"/></button><button class="icon-button" aria-label="Fermer les détails" @click="open=false"><X :size="17"/></button></header>
    <p v-if="run?.runtimeActive&&run.status!=='running'" class="runtime-active-hint" role="status">Une exécution enfant reste active. L’archive sera disponible après son arrêt ou sa pause.</p>
    <RunRevisionPanel v-if="run" :definition="revisionView.definition.value" :revisions="revisionView.revisions.value" :loading="revisionView.loading.value" :error="revisionView.error.value" @refresh="revisionView.refresh"/>
    <div v-if="inspectedOverview&&!selectedPath" class="runtime-inspector-graph"><RuntimeGraphPreview :overview="inspectedOverview" :view-key="run?.id" @inspect="emit('inspect',$event,false)"/></div><div v-else class="run-canvas"><RunGraph :composition="inspectedComposition" :path-prefix="exactDefinition?.instance||''" :run="run" :selected-path="selectedPath" :selected-occurrence="selectedOccurrence" :focus-revision="focusRevision" :expanded="expanded" @inspect="emit('inspect',$event,false)"/></div>
    <div class="inspector-tabs" role="tablist" aria-label="Détails" @keydown="navigateTabs"><button v-for="item in [{id:'activity',label:'Parcours'},{id:'models',label:'Modèles'},{id:'context',label:'Contexte'},{id:'state',label:'État'}] as const" :key="item.id" role="tab" :tabindex="tab===item.id?0:-1" :aria-selected="tab===item.id" :class="{chosen:tab===item.id}" @click="tab=item.id">{{item.label}}</button></div>
    <ModelBindingsPanel v-if="tab==='models'" :nodes="nodes" :models="models" :bindings="bindings" :selected-path="selectedPath" :busy="busy" :running="!!run" @close="tab='activity'" @select="emit('inspect',$event,true,true)" @change="(path,selection)=>emit('change',path,selection)"/>
    <AgentContextPanel :selected-occurrence="selectedOccurrence" @copy="(program,block)=>emit('copyContext',program,block)" v-else-if="tab==='context'&&(composition.formatVersion||1)>=2" :run="run" :nodes="nodes" :context="context" :selected-path="selectedPath" :busy="busy" @close="tab='activity'" @select="emit('inspect',$event,true,true)" @activate="(path,itemId,active,skillName)=>emit('activate',path,itemId,active,skillName)"/>
    <ContextPanel v-else-if="tab==='context'" :context="context" :snapshot="!!run?.context" @close="tab='activity'"/>
    <div v-else-if="tab==='activity'" class="inspector-scroll">
      <div v-if="selectedPath" class="passage-selection"><button class="trace-node-link" @click="emit('inspect','',false)">Tous les nœuds</button><h3>{{selected?.label||selectedPath}}</h3><label v-if="passages.length">Passage<select :value="selected?.occurrenceId" aria-label="Passage du nœud" @change="selectPass(($event.target as HTMLSelectElement).value)"><option v-for="pass in passages" :key="pass.occurrenceId" :value="pass.occurrenceId">Étape {{pass.step}} · {{pass.status}} · {{pass.durationMs??'…'}} ms</option></select></label><p v-else class="muted">Ce nœud n’a pas encore été exécuté.</p></div>
      <ExecutionActivity v-if="selected" :activities="[selected]" :context-snapshots="run?.contextSnapshots" @select="emit('inspect',{nodePath:$event.path,occurrenceId:$event.occurrenceId},false)"/>
      <div v-else class="passage-index"><button v-for="pass in passages" :key="pass.occurrenceId" :data-node="pass.node" :data-status="pass.status" @click="selectPass(pass.occurrenceId)"><span>{{pass.step}}</span><strong>{{pass.label}}</strong><small>{{pass.status}}</small></button></div>
      <p v-if="!run?.activities?.length" class="muted">Les passages des nœuds apparaîtront ici pendant l’exécution.</p>
      <p v-if="run?.timelineApproximate||run?.activities?.some(activity=>activity.startedSeq===undefined)" class="timeline-history-note">L’ordre de cette ancienne session a été reconstitué à partir de l’historique disponible. Les passages sans origine enregistrée ne sont pas associés arbitrairement aux messages.</p>
    </div>
    <div v-else class="inspector-scroll"><div class="panel-title">État sauvegardé <button :disabled="stateLoading" @click="loadState">Actualiser</button></div><p v-if="stateLoading" class="detail-loading">Chargement de l’état…</p><p v-if="stateError" class="detail-error">{{stateError}}</p><p class="checkpoint-id">Checkpoint <code>{{stateDetail?.checkpoint||run?.checkpoint||'—'}}</code></p><div v-for="(value,key) in state" :key="key" class="state-row"><code>{{key}}</code><pre>{{typeof value==='string'?value:JSON.stringify(value,null,2)}}</pre></div><details class="event-log" @toggle="toggleEvents"><summary>Événements ADK · {{eventPage.events.length}}</summary><pre v-for="event in eventPage.events" :key="event.seq">{{event.seq}} {{JSON.stringify(event.event)}}</pre><p v-if="eventsLoading" class="detail-loading">Chargement des événements…</p><button v-if="eventPage.hasMore" :disabled="eventsLoading" @click="loadEvents">Charger la suite</button></details></div>
  </DialogContent></DialogRoot>
</template>

<style scoped>.runtime-active-hint{margin:0;padding:10px 16px;font-size:11px;color:#b9b6c4;background:#29272f}.runtime-inspector-graph{overflow:auto;flex:0 0 360px;border-bottom:1px solid #34343b}.expanded .runtime-inspector-graph{flex:1;min-height:350px}.runtime-inspector-graph :deep(.runtime-graph-preview){padding:10px}</style>
