<script setup lang="ts">
import { computed, ref, onUnmounted, watch } from 'vue'
import { CircleCheck, LoaderCircle, CircleAlert, CornerDownLeft, GitBranch } from 'lucide-vue-next'
import type { NodeActivity } from '@zedflow/sdk'
import { Tool, ToolHeader, ToolContent } from './ai-elements/tool'
import { Task, TaskTrigger, TaskContent } from './ai-elements/task'
import { ChainOfThoughtStep } from './ai-elements/chain-of-thought'
import CapturedContext from './CapturedContext.vue'
import { detailVersion,useRunDetails } from '../composables/runDetails'
import ToolResult from './ToolResult.vue'
const props=defineProps<{activities:NodeActivity[];contextSnapshots?:Record<string,any>[]}>()
const emit=defineEmits<{select:[NodeActivity]}>()
const now=ref(Date.now());const timer=setInterval(()=>now.value=Date.now(),500);onUnmounted(()=>clearInterval(timer))
const details=useRunDetails(),contextOpen=ref(new Set<string>())
const entries=computed(()=>props.activities.map(activity=>details.entry({...details.scope(),kind:'activities',id:activity.occurrenceId,revision:detailVersion(activity)})?.value||activity))
watch(()=>props.activities,activities=>{for(const activity of activities)if(activity.inputRef||activity.outputRef)void details.load({...details.scope(),kind:'activities',id:activity.occurrenceId,revision:detailVersion(activity)}).catch(()=>{})},{immediate:true})
const active=computed(()=>props.activities.filter(a=>a.status==='running').length)
const status:Record<string,string>={running:'En cours',completed:'Terminé',waiting:'Attend une réponse',error:'Échec',interrupted:'Interrompu',resumed:'Réponse reçue · repris'}
function duration(a:NodeActivity){const ms=a.durationMs??Math.max(0,now.value-a.startedAt);return ms<1000?`${ms} ms`:`${(ms/1000).toFixed(1)} s`}
function effectiveContext(a:NodeActivity){return a.output?.modelResponse?.effectiveContext||props.contextSnapshots?.find(snapshot=>(!!a.output?.modelResponse?.contextSnapshotId&&snapshot.invocationId===a.output.modelResponse.contextSnapshotId)||snapshot.origin?.occurrenceId===a.occurrenceId)}
function toolState(a:NodeActivity){return ['error','interrupted'].includes(a.status)?'output-error':a.status==='completed'?'output-available':'input-available'}
function result(a:NodeActivity){if(!a.output)return undefined;if(a.output.toolResults?.length===1){const result=a.output.toolResults[0].result;return a.ui?.renderer==='table' && Array.isArray(result?.rows)?result.rows:result}if(a.output.toolResults?.length)return a.output.toolResults;const entries=Object.entries(a.output).filter(([k])=>!['messages','toolCalls','hasToolCalls'].includes(k));return entries.length===1?entries[0]![1]:a.output}
</script>
<template><Task v-if="activities.length" :default-open="true"><div class="execution-activity" data-testid="execution-activity"><TaskTrigger :title="`Parcours d’exécution · ${activities.length} passages${active?` · ${active} en cours`:''}`"/><TaskContent><div class="activity-list">
  <div v-for="a in entries" :key="a.occurrenceId" :data-node="a.node" :data-status="a.status" class="activity-entry">
    <p v-if="details.entry({...details.scope(),kind:'activities',id:a.occurrenceId,revision:detailVersion(activities.find(item=>item.occurrenceId===a.occurrenceId)||a)})?.loading" class="detail-loading">Chargement du passage…</p><ChainOfThoughtStep :label="a.label" :description="`${status[a.status]} · étape ${a.step} · ${duration(a)}`" :status="a.status==='running'?'active':a.status==='waiting'?'pending':'complete'">
      <template #icon><LoaderCircle v-if="a.status==='running'" :size="14" class="spin"/><CircleAlert v-else-if="['error','interrupted'].includes(a.status)" :size="14"/><CornerDownLeft v-else-if="a.status==='waiting'" :size="14"/><CircleCheck v-else :size="14"/></template>
      <p v-if="a.gapBeforeMs!==undefined" class="passage-gap">Avant ce passage · {{a.gapBeforeMs<1000?`${a.gapBeforeMs} ms`:`${(a.gapBeforeMs/1000).toFixed(1)} s`}}</p><button class="trace-node-link" @click="emit('select',a)"><GitBranch :size="11"/>{{a.path || a.node}}</button>
      <Tool v-if="a.kind==='tool'" :default-open="true" class="tool-activity"><ToolHeader type="dynamic-tool" :tool-name="a.tool||a.label" :title="a.ui?.title||a.label" :state="toolState(a)"/><ToolContent><div class="tool-body"><details><summary>Arguments</summary><pre>{{JSON.stringify(a.input,null,2)}}</pre></details><p v-if="a.status==='running'" class="muted">L’outil s’exécute sur le daemon…</p><p v-else-if="a.error" class="field-error">{{a.error}}</p><ToolResult v-else :value="result(a)" :tool="a.output?.toolResults?.[0]?.name || a.tool" :renderer="a.ui?.renderer" :language="a.ui?.language"/><details v-if="a.output"><summary>Sorties brutes</summary><pre>{{JSON.stringify(a.output,null,2)}}</pre></details></div></ToolContent></Tool>
      <template v-else>
      <div v-if="['agent','model'].includes(a.kind) && a.output?.modelResponse" class="model-meta"><span>{{a.output.modelResponse.provider}}</span><span>{{a.output.modelResponse.model}}</span><span v-if="a.output.modelResponse.runtimeSelection?.reasoningEffort">Réflexion {{a.output.modelResponse.runtimeSelection.reasoningEffort}}</span><span v-if="a.output.modelResponse.runtimeSelection?.thinkingBudget !== undefined">Réflexion {{a.output.modelResponse.runtimeSelection.thinkingBudget}} tokens</span><span v-if="a.output.modelResponse.usage">{{a.output.modelResponse.usage.total_token_count ?? a.output.modelResponse.usage.total_tokens ?? '—'}} tokens</span></div>
      <details v-if="effectiveContext(a)" class="activity-data" @toggle="($event.target as HTMLDetailsElement).open?contextOpen.add(a.occurrenceId):contextOpen.delete(a.occurrenceId)"><summary>Contexte réellement chargé</summary><CapturedContext :snapshot="effectiveContext(a)" :active="contextOpen.has(a.occurrenceId)"/></details>
      <details v-if="a.output?.modelResponse?.reasoningSummary" class="activity-data"><summary>Résumé fourni par le modèle</summary><p>{{a.output.modelResponse.reasoningSummary}}</p></details>
      <Tool v-for="(call,i) in a.output?.toolCalls||[]" :key="call.id||i" class="tool-activity"><ToolHeader type="dynamic-tool" :tool-name="call.name" state="input-streaming" :title="call.name+' · appel demandé'"/><ToolContent><div class="tool-body"><pre>{{JSON.stringify(call.args,null,2)}}</pre><small>{{call.id}} · exécuté par le nœud Outil du graphe</small></div></ToolContent></Tool>
      <details class="activity-data"><summary>{{['agent','model'].includes(a.kind)?'Contexte, réponse et appels demandés':'Entrées et sorties'}}</summary><span>Entrée</span><pre>{{JSON.stringify(a.input,null,2)}}</pre><span v-if="a.output">Sortie</span><pre v-if="a.output">{{JSON.stringify(a.output,null,2)}}</pre><p v-if="a.error" class="field-error">{{a.error}}</p></details></template>
    </ChainOfThoughtStep>
  </div>
</div></TaskContent></div></Task></template>
