<script setup lang="ts">
import type { InspectionSelection } from '../composables/useExecutedDefinition'
import { computed, ref, watch } from 'vue'
import { Terminal, FileText, FilePenLine, LoaderCircle, ChevronRight } from 'lucide-vue-next'
import type {  } from '@zedflow/sdk'
import { detailVersion,useRunDetails } from '../composables/runDetails'
import ToolResult from './ToolResult.vue'
const props=defineProps<{activities:Record<string,any>[]}>()
const emit=defineEmits<{select:[selection:InspectionSelection]}>()
const expanded=ref(false)
const details=useRunDetails(),opened=ref(new Set<string>())
const bases=computed<Record<string,any>[]>(()=>props.activities.map(activity=>({...activity,name:activity.name||activity.tool||'Outil',args:activity.args||activity.arguments||activity.argumentsPreview||{},status:activity.status||'running'})))
const entries=computed(()=>bases.value.map(base=>{const loaded=details.entry({...details.scope(),kind:'tools',id:base.callId,revision:detailVersion(base)})?.value;return {...base,...loaded,args:loaded?.args||loaded?.arguments||base.args}}))
function requestDetail(base:Record<string,any>){if(base.inputRef||base.argumentsRef||base.outputRef||base.resultRef||base.detailRef)void details.load({...details.scope(),kind:'tools',id:base.callId,revision:detailVersion(base)}).catch(()=>{})}
function toggle(entry:Record<string,any>,event:Event){if((event.target as HTMLDetailsElement).open){opened.value.add(entry.callId);requestDetail(bases.value.find(base=>base.callId===entry.callId)||entry)}else opened.value.delete(entry.callId)}
watch(()=>bases.value,values=>{for(const base of values)if(opened.value.has(base.callId))requestDetail(base)})
function detailState(entry:Record<string,any>){const base=bases.value.find(base=>base.callId===entry.callId)||entry;return details.entry({...details.scope(),kind:'tools',id:entry.callId,revision:detailVersion(base)})}
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
const downloadError=ref('')
</script>
<template><p v-if="downloadError" class="field-error">{{downloadError}}</p>
  <section v-if="entries.length" class="tool-timeline minimal-tools" aria-label="Outils du workspace">
    <button v-if="entries.length>1" class="tool-group-summary" :aria-expanded="expanded||hasActive||hasError" @click="expanded=!expanded"><LoaderCircle v-if="hasActive" :size="14" class="spin"/><FileText v-else :size="14"/><span>{{groupLabel}}</span><small>{{entries.length}}</small><span v-if="hasError" class="tool-error-label">À vérifier</span><ChevronRight :size="12" :class="{rotated:expanded}"/></button>
    <div v-show="entries.length===1||expanded||hasActive||hasError" class="tool-group-items">
      <div v-for="(entry,index) in entries" :key="entry.callId||index" class="tool-entry-row"><slot name="rail" :entry="entry"/><details class="workspace-tool" @toggle="toggle(entry,$event)" :data-tool="entry.name" :data-timeline-id="entry.timelineId" :data-timeline-seq="entry.timelineSeq">
        <summary><Terminal v-if="entry.name==='exec'" :size="14"/><FilePenLine v-else-if="['write','edit'].includes(entry.name)" :size="14"/><FileText v-else :size="14"/><strong>{{verbs[entry.name]||entry.name}}</strong><code :title="entry.args.path||entry.args.file_path||entry.args.command||''">{{entry.args.path||entry.args.file_path||entry.args.command||''}}</code><LoaderCircle v-if="entry.status==='running'" :size="13" class="spin"/><span v-else-if="entry.status!=='completed'" class="tool-error-label">{{['error','failed'].includes(entry.status)?'Échec':entry.status==='interrupted'?'Interrompu':entry.status}}</span><ChevronRight :size="12" class="tool-disclosure"/></summary>
        <div class="tool-detail-body"><p v-if="detailState(entry)?.loading" class="detail-loading">Chargement du résultat…</p><p v-if="detailState(entry)?.error" class="detail-error">{{detailState(entry)?.error}}</p><details class="tool-arguments"><summary>Arguments</summary><pre>{{JSON.stringify(entry.args,null,2)}}</pre></details><pre v-if="entry.output||entry.text||entry.chunks" class="command-output">{{entry.output||entry.text||(Array.isArray(entry.chunks)?entry.chunks.join(''):entry.chunks)}}</pre><ToolResult v-if="entry.result!=null" :value="entry.result" :tool="entry.name"/><button v-if="(entry.fullOutputRef||entry.result?.fullOutputRef)" @click="details.download('tool',entry.callId).catch(cause=>downloadError=String(cause))" class="tool-full-output" download="tool-output.bin">Télécharger la sortie complète</button><p v-if="entry.error" class="field-error">{{entry.error}}</p><p v-if="entry.status==='completed'" class="tool-completion">Terminé<span v-if="entry.durationMs!==undefined"> · {{entry.durationMs}} ms</span></p><button v-if="entry.nodePath" class="trace-node-link" @click="emit('select',entry.origin||{nodePath:entry.nodePath})">Inspecter le passage · {{entry.nodePath}}</button></div>
      </details></div>
    </div>
  </section>
</template>
