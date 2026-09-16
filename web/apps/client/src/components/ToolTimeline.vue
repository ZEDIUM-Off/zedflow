<script setup lang="ts">
import type { InspectionSelection } from '../composables/useExecutedDefinition'
import { computed, ref, watch } from 'vue'
import { Terminal, FileText, FilePenLine, LoaderCircle, ChevronRight } from 'lucide-vue-next'
import { jsonObjectSchema, type ToolActivity, type JsonValue } from '@zedflow/sdk'
import { detailVersion,useRunDetails } from '../composables/runDetails'
import ToolResult from './ToolResult.vue'
type TimelineTool=ToolActivity & {timelineId?:string;timelineSeq?:number}
const props=defineProps<{activities:TimelineTool[]}>()
const emit=defineEmits<{select:[selection:InspectionSelection]}>()
const expanded=ref(false)
const details=useRunDetails(),opened=ref(new Set<string>())
const bases=computed(()=>props.activities)
const entries=computed(()=>bases.value.map(base=>{const loaded=details.entry({...details.scope(),kind:'tools',id:base.callId,revision:detailVersion(base)})?.value;const merged={...base,...loaded};return {...merged,name:merged.name||'Outil',status:merged.status||'running',args:merged.args??merged.arguments??merged.argumentsPreview??{},displayOutput:merged.output??(typeof merged.text==='string'?merged.text:Array.isArray(merged.chunks)?merged.chunks.filter(chunk=>typeof chunk==='string').join(''):''),downloadable:!!merged.fullOutputRef||hasFullOutput(merged.result)}}))
function requestDetail(base:ToolActivity){if(base.inputRef||base.argumentsRef||base.outputRef||base.resultRef||base.detailRef)void details.load({...details.scope(),kind:'tools',id:base.callId,revision:detailVersion(base)}).catch(()=>{})}
function toggle(entry:ToolActivity,event:Event){if((event.target as HTMLDetailsElement).open){opened.value.add(entry.callId);requestDetail(bases.value.find(base=>base.callId===entry.callId)||entry)}else opened.value.delete(entry.callId)}
watch(()=>bases.value,values=>{for(const base of values)if(opened.value.has(base.callId))requestDetail(base)})
function detailState(entry:ToolActivity){const base=bases.value.find(base=>base.callId===entry.callId)||entry;return details.entry({...details.scope(),kind:'tools',id:entry.callId,revision:detailVersion(base)})}
const hasActive=computed(()=>entries.value.some(entry=>entry.status==='running'))
const hasError=computed(()=>entries.value.some(entry=>['failed','error','interrupted'].includes(entry.status)))
const groupLabel=computed(()=>{
  const names=new Set(entries.value.map(entry=>entry.name));const labels=[]
  if(names.has('read'))labels.push('Lecture de fichiers')
  if(names.has('write')||names.has('edit'))labels.push('Modifications')
  if(names.has('exec'))labels.push('Commandes')
  if([...names].some(name=>!['read','write','edit','exec'].includes(name)))labels.push('Outils')
  return labels.join(' · ')
})
const verbs:Record<string,string>={read:'Lecture',write:'Écriture',edit:'Modification',exec:'Commande'}
function hasFullOutput(value:JsonValue|undefined){const result=jsonObjectSchema.safeParse(value);return result.success&&typeof result.data.fullOutputRef==='string'}
function argumentLabel(value:JsonValue){const parsed=jsonObjectSchema.safeParse(value);if(!parsed.success)return '';const label=parsed.data.path??parsed.data.file_path??parsed.data.command;return typeof label==='string'?label:''}
const downloadError=ref('')
</script>
<template><p v-if="downloadError" class="field-error">{{downloadError}}</p>
  <section v-if="entries.length" class="tool-timeline minimal-tools" aria-label="Outils du workspace">
    <button v-if="entries.length>1" class="tool-group-summary" :aria-expanded="expanded||hasActive||hasError" @click="expanded=!expanded"><LoaderCircle v-if="hasActive" :size="14" class="spin"/><FileText v-else :size="14"/><span>{{groupLabel}}</span><small>{{entries.length}}</small><span v-if="hasError" class="tool-error-label">À vérifier</span><ChevronRight :size="12" :class="{rotated:expanded}"/></button>
    <div v-show="entries.length===1||expanded||hasActive||hasError" class="tool-group-items">
      <div v-for="(entry,index) in entries" :key="entry.callId||index" class="tool-entry-row"><slot name="rail" :entry="entry"/><details class="workspace-tool" @toggle="toggle(entry,$event)" :data-tool="entry.name" :data-timeline-id="entry.timelineId" :data-timeline-seq="entry.timelineSeq" >
        <summary><Terminal v-if="entry.name==='exec'" :size="14"/><FilePenLine v-else-if="['write','edit'].includes(entry.name)" :size="14"/><FileText v-else :size="14"/><strong>{{verbs[entry.name]||entry.name}}</strong><code :title="argumentLabel(entry.args)">{{argumentLabel(entry.args)}}</code><LoaderCircle v-if="entry.status==='running'" :size="13" class="spin"/><span v-else-if="entry.status!=='completed'" class="tool-error-label">{{['error','failed'].includes(entry.status)?'Échec':entry.status==='interrupted'?'Interrompu':entry.status}}</span><ChevronRight :size="12" class="tool-disclosure"/></summary>
        <div class="tool-detail-body"><p v-if="detailState(entry)?.loading" class="detail-loading">Chargement du résultat…</p><p v-if="detailState(entry)?.error" class="detail-error">{{detailState(entry)?.error}}</p><details class="tool-arguments"><summary>Arguments</summary><pre>{{JSON.stringify(entry.args,null,2)}}</pre></details><pre v-if="entry.displayOutput" class="command-output">{{entry.displayOutput}}</pre><ToolResult v-if="entry.result!=null" :value="entry.result" :tool="entry.name"/><button v-if="entry.downloadable" @click="details.download('tool',entry.callId).catch(cause=>downloadError=String(cause))" class="tool-full-output" download="tool-output.bin">Télécharger la sortie complète</button><p v-if="entry.error" class="field-error">{{entry.error}}</p><p v-if="entry.status==='completed'" class="tool-completion">Terminé<span v-if="entry.durationMs!==undefined"> · {{entry.durationMs}} ms</span></p><button v-if="entry.nodePath" class="trace-node-link" @click="emit('select',entry.origin||{nodePath:entry.nodePath})">Inspecter le passage · {{entry.nodePath}}</button></div>
      </details></div>
    </div>
  </section>
</template>
