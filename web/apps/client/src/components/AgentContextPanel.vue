<script setup lang="ts">
import { requireNodeConfig } from '@zedflow/sdk'
import type { RunContext } from '@zedflow/sdk'

import { computed, ref, watch } from 'vue'
import { BookOpen, FileText, Wrench, X } from 'lucide-vue-next'
import type { Run } from '@zedflow/sdk'
import type { ModelNode } from '../harness'
import CapturedContext from './CapturedContext.vue'
import ProjectedContext from './context/ProjectedContext.vue'
import ContextWindowPanel from './context/ContextWindowPanel.vue'
import type { ContextProgram } from '@zedflow/sdk'
import type { AgentAttachments } from '../graph/attachments'
import { useContextDetail, useRunDetails, detailVersion } from '../composables/runDetails'
const props=defineProps<{run:Run|null;nodes:ModelNode[];context?:RunContext|null;selectedPath:string;selectedOccurrence?:string;busy:boolean}>()
const emit=defineEmits<{copy:[program:ContextProgram,blockId:string];close:[];select:[path:string];activate:[nodePath:string,itemId:string,active:boolean,skillName?:string]}>()
const path=ref(props.nodes.find(node=>node.path===props.selectedPath||node.contextPath===props.selectedPath)?.path||props.nodes[0]?.path||'')
watch(()=>props.selectedPath,value=>{const target=props.nodes.find(node=>node.path===value||node.contextPath===value);if(target)path.value=target.path})
watch(()=>props.nodes.map(node=>node.path).join(','),()=>{if(!props.nodes.some(node=>node.path===path.value))path.value=props.nodes[0]?.path||''})
const entry=computed(()=>props.nodes.find(node=>node.path===path.value))
const contextConfig=computed(()=>requireNodeConfig((entry.value?.context||entry.value?.node)?.data.config ?? {}))
const attachments=computed<AgentAttachments>(()=>contextConfig.value.attachments||{})
const activated=computed(()=>props.run?.capabilityActivations?.[path.value]||[])
const details=useRunDetails()
const selectedActivity=computed(()=>props.run?.activities?.find(item=>item.occurrenceId===props.selectedOccurrence&&[path.value,entry.value?.contextPath].includes(item.path)))
watch(selectedActivity,activity=>{if(activity?.outputRef)void details.load({...details.scope(),kind:'activities',id:activity.occurrenceId,revision:detailVersion(activity)}).catch(()=>{})},{immediate:true})
const sourceSnapshot=computed(()=>{
  const occurrences=props.run?.contextSnapshots?.filter(item=>(item.agentPath||item.nodePath||item.origin?.nodePath)===path.value)||[]
  const index=selectedActivity.value
  const activity=index?(details.entry({...details.scope(),kind:'activities',id:index.occurrenceId,revision:detailVersion(index)})?.value||index):undefined
  if(props.selectedOccurrence&&!activity)return undefined
  const snapshotId=activity?.output?.modelResponse?.contextSnapshotId||activity?.output?.modelResponse?.invocationId
  const recorded=activity?occurrences.find(item=>(snapshotId&&item.invocationId===snapshotId)||item.origin?.occurrenceId===activity.occurrenceId):occurrences.at(-1)
  if(props.selectedOccurrence)return recorded
  return recorded||props.run?.activities?.filter(activity=>activity.path===path.value&&activity.output?.modelResponse?.effectiveContext).at(-1)?.output?.modelResponse?.effectiveContext
})
const {snapshot}=useContextDetail(()=>sourceSnapshot.value)
const resources=computed(()=>{
  const list:{id:string;key:string;name:string;kind:string;source:string;always:boolean;enabled:boolean;skillName?:string}[]=[]
  for(const item of attachments.value.instructions?.items||[])list.push({id:item.id,key:item.id,name:item.source.kind==='workspace'?'Instructions du workspace':item.source.kind==='text'?'Instructions du flow':item.source.path,kind:'Instructions',source:item.source.kind==='text'?item.source.text:item.source.kind==='workspace'?'AGENTS.md et instructions découvertes':item.source.path,always:item.activation!=='explicit',enabled:item.enabled!==false})
  for(const item of attachments.value.files?.items||[])list.push({id:item.id,key:item.id,name:item.path,kind:'Fichier',source:`${item.path}${item.startLine?` · lignes ${item.startLine}–${item.endLine||'fin'}`:''}`,always:item.activation!=='explicit',enabled:item.enabled!==false})
  for(const item of attachments.value.skills?.items||[]){
    const recorded=snapshot.value?.skillCatalog?.filter((skill:any)=>skill.itemId===item.id)
    const skills=recorded?.length?recorded:item.source.kind==='workspace'?(props.context?.skills||[]).filter(skill=>!item.name||skill.name===item.name):[{name:item.name||'',path:item.source.path}]
    for(const skill of skills)list.push({id:item.id,key:skill.name?`${item.id}::${skill.name}`:item.id,name:skill.name?`/skill:${skill.name}`:skill.path,kind:'Skill',source:skill.path,always:item.activation==='always',enabled:item.enabled!==false,...(skill.name?{skillName:skill.name}:{})})
  }
  return list
})
function active(resource:typeof resources.value[number]){return resource.enabled&&(resource.always||(activated.value.includes(resource.key)||(!resource.skillName&&resource.kind==='Skill'&&activated.value.some(key=>key.startsWith(`${resource.id}::`)))))}
</script>
<template>
  <section class="harness-panel agent-context-panel" aria-label="Capacités et contexte de l’agent"><header><div><strong>Contexte par agent</strong><small>Sources, stratégie et fenêtre effectivement transmise.</small></div><button aria-label="Fermer le contexte" @click="emit('close')"><X :size="16"/></button></header>
    <label class="context-agent-select">Agent<select v-model="path" aria-label="Agent à inspecter" @change="emit('select',path)"><option v-for="node in nodes" :key="node.path" :value="node.path">{{node.node.data.label}} · {{node.path}}</option></select></label>
    <ProjectedContext v-if="snapshot?.prepared?.program" :snapshot="snapshot"/>
    <template v-else>
    <p v-if="entry?.context" class="muted">Préparation : {{entry.context.data.label}} → {{entry.node.data.label}}</p>
    <p v-if="!entry" class="muted">Ce flow ne contient pas d’appel modèle.</p>
    <p v-else-if="!contextConfig.contextStrategy&&!resources.length&&!attachments.tools?.items.length" class="muted">Aucune pièce attachée. Ce modèle ne reçoit aucun contexte implicite.</p>
    <div v-for="resource in resources" :key="resource.key" class="agent-resource"><label><input type="checkbox" :checked="active(resource)" :disabled="busy||!run||resource.always||!resource.enabled||!!run.import?.resumeBlocked.length" :aria-label="`Activer ${resource.name} pour ${entry?.node.data.label}`" @change="emit('activate',path,resource.id,($event.target as HTMLInputElement).checked,resource.skillName)"/><span>{{resource.name}}</span><small>{{!resource.enabled?'Désactivé':resource.always?'Permanent':active(resource)?'Activé':'À la demande'}}</small></label><details><summary>{{resource.kind}} · Source</summary><pre>{{resource.source}}</pre></details></div>
    <p v-if="run&&resources.some(resource=>!resource.always)" class="context-activation-hint">Les changements s’appliquent au prochain appel de cet agent.</p>
    <h3><Wrench :size="14"/> Outils accordés</h3><div class="agent-tool-grants"><code v-for="tool in attachments.tools?.items?.filter(item=>item.enabled!==false)||[]" :key="tool.id">{{tool.name}}</code><span v-if="!attachments.tools?.items?.some(item=>item.enabled!==false)" class="muted">Aucun outil</span></div>
    <template v-if="snapshot"><h3><FileText :size="14"/> Contexte du passage sélectionné</h3><CapturedContext :snapshot="snapshot"/></template>
    <p v-else-if="run" class="muted">Le contenu effectivement chargé apparaîtra lors du premier appel de cet agent.</p>
    </template>
    <ContextWindowPanel v-if="run&&path&&contextConfig.contextStrategy" :run="run" :node-path="path" :captured="snapshot?.prepared?.window" @copy="(program,block)=>emit('copy',program,block)"/>
  </section>
</template>
