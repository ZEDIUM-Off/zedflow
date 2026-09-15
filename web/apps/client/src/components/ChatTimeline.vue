<script setup lang="ts">
import type { InspectionSelection } from '../composables/useExecutedDefinition'
import { computed, markRaw, ref } from 'vue'
import { PopoverRoot,PopoverTrigger,PopoverPortal,PopoverContent } from 'reka-ui'
import { Bot, Check, Circle, GitBranch, LoaderCircle, CornerDownRight } from 'lucide-vue-next'
import type { NodeActivity, Run, TimelineEntry } from '@zedflow/sdk'
import { Message, MessageContent, MessageResponse } from './ai-elements/message'
import ToolTimeline from './ToolTimeline.vue'
import MarkdownLink from './MarkdownLink.vue'
const nodeRenderers={link:markRaw(MarkdownLink)}
const props=defineProps<{run:Run|null;results?:boolean}>()
const emit=defineEmits<{inspect:[selection:InspectionSelection]}>()
const allEntries=computed<TimelineEntry[]>(()=>props.run?.timeline||props.run?.messages.map((message,index)=>({...message,id:message.id||`legacy:${index}`,seq:index,kind:'message' as const}))||[])
const entries=computed(()=>props.results?allEntries.value.filter(entry=>entry.kind==='tool'||entry.role==='assistant'):allEntries.value)
type Row={id:string;entries:TimelineEntry[];passes:NodeActivity[];origins:NodeActivity[];tools:Record<string,any>[]}
const openRail=ref<string>()
const cachedRows=new Map<string,Row>()
const same=(a:unknown[],b:unknown[])=>a.length===b.length&&a.every((item,index)=>item===b[index])
const rows=computed<Row[]>(()=>{
  const activities=props.run?.activities||[]
  const activityById=new Map(activities.map(activity=>[activity.occurrenceId,activity]))
  const knownOrigins=new Set(entries.value.flatMap(entry=>entry.origin?[entry.origin.occurrenceId]:[]))
  const unanchored=activities.filter(activity=>activity.startedSeq!==undefined&&!knownOrigins.has(activity.occurrenceId)).sort((a,b)=>a.startedSeq!-b.startedSeq!)
  const groups:TimelineEntry[][]=[]
  for(const entry of entries.value){const last=groups.at(-1);if(entry.kind==='tool'&&last?.[0]?.kind==='tool')last.push(entry);else groups.push([entry])}
  let cursor=0
  const result=groups.map(group=>{
    const passes:NodeActivity[]=[]
    const end=Math.max(...group.map(entry=>entry.seq))
    while(cursor<unanchored.length&&unanchored[cursor]!.startedSeq!<=end)passes.push(unanchored[cursor++]!)
    const ids=new Set(group.flatMap(entry=>entry.origin?[entry.origin.occurrenceId]:[]))
    const id=group[0]!.id,origins=[...ids].flatMap(id=>activityById.get(id)?[activityById.get(id)!]:[])
    const previous=cachedRows.get(id)
    if(previous&&same(previous.entries,group)&&same(previous.passes,passes)&&same(previous.origins,origins))return previous
    const tools=group.flatMap(entry=>entry.kind==='tool'?[{...entry.activity,origin:entry.origin,timelineId:entry.id,timelineSeq:entry.seq}]:[])
    return {id,entries:group,passes,origins,tools}
  })
  if(cursor<unanchored.length)result.push({id:'pending-passages',entries:[],passes:unanchored.slice(cursor),origins:[],tools:[]})
  cachedRows.clear();for(const row of result)cachedRows.set(row.id,row)
  return result
})
function select(activity:NodeActivity){openRail.value=undefined;emit('inspect',{nodePath:activity.path||activity.node,occurrenceId:activity.occurrenceId})}
function label(activity:NodeActivity){return `${activity.label} · passage ${activity.step} · ${activity.status==='running'?'en cours':activity.status==='waiting'?'en attente':activity.status==='error'?'échec':'terminé'}`}
</script>
<template>
  <div class="chat-timeline" :class="{'execution-results':results}" :aria-label="results?'Résultats de l’exécution':undefined">
    <div v-for="row in rows" :key="row.id" class="timeline-row" :class="{'passage-only':!row.entries.length}">
      <aside class="passage-rail" aria-label="Passages du graphe">
        <PopoverRoot v-if="row.passes.length" :open="openRail===row.id" @update:open="openRail=$event?row.id:undefined"><PopoverTrigger class="rail-passes-trigger" :aria-label="`${row.passes.length} étapes du graphe`" :title="row.passes.map(label).join('\n')"><GitBranch :size="13"/><small>{{row.passes.length}}</small></PopoverTrigger><PopoverPortal><PopoverContent class="rail-passage-list rail-passage-popover" side="right" align="start" :side-offset="8" :collision-padding="12"><button v-for="pass in row.passes" :key="pass.occurrenceId" :data-occurrence-id="pass.occurrenceId" :title="label(pass)" @click="select(pass)"><LoaderCircle v-if="pass.status==='running'" :size="12" class="spin"/><Check v-else :size="12"/><span>{{pass.label}}</span><small>{{pass.step}}</small></button></PopoverContent></PopoverPortal></PopoverRoot>
        <button v-for="pass in row.entries.length>1?[]:row.origins" :key="pass.occurrenceId" class="rail-origin" :class="pass.status" :title="label(pass)" :aria-label="`Inspecter ${label(pass)}`" :data-occurrence-id="pass.occurrenceId" @click="select(pass)"><LoaderCircle v-if="pass.status==='running'" :size="14" class="spin"/><Bot v-else-if="['agent','model'].includes(pass.kind)" :size="14"/><CornerDownRight v-else-if="pass.kind==='output'" :size="14"/><Circle v-else :size="11"/><small>{{pass.step}}</small></button>
      </aside>
      <div class="timeline-result">
        <template v-if="row.entries[0]?.kind==='message'"><article v-if="results" class="execution-result" :data-timeline-id="row.entries[0].id" :data-timeline-seq="row.entries[0].seq"><MessageResponse :content="row.entries[0].text" :node-renderers="nodeRenderers" mode="static" :enable-animate="false" :is-dark="true" class="assistant-markdown"/></article><Message v-else :from="row.entries[0].role" class="chat-message" :data-timeline-id="row.entries[0].id" :data-timeline-seq="row.entries[0].seq"><MessageContent><MessageResponse v-if="row.entries[0].role==='assistant'" :content="row.entries[0].text" :node-renderers="nodeRenderers" mode="static" :enable-animate="false" :is-dark="true" class="assistant-markdown"/><div v-else class="message-text">{{row.entries[0].text}}</div></MessageContent></Message></template>
        <ToolTimeline v-else-if="row.entries.length" :activities="row.tools" @select="emit('inspect',$event)"><template #rail="{entry}"><button v-if="row.entries.length>1&&entry.origin" class="rail-origin tool-entry-origin" :class="entry.status" :title="`${entry.nodePath||entry.origin.nodePath} · inspecter ce passage`" :aria-label="`Inspecter le passage de ${entry.name}`" :data-occurrence-id="entry.origin.occurrenceId" @click="emit('inspect',entry.origin)"><Circle :size="11"/><small>{{row.origins.find(pass=>pass.occurrenceId===entry.origin.occurrenceId)?.step}}</small></button></template></ToolTimeline>
        <span v-else-if="row.passes.some(pass=>pass.status==='running')" class="passage-running">{{row.passes.filter(pass=>pass.status==='running').at(-1)?.label}}…</span>
      </div>
    </div>
  </div>
</template>
