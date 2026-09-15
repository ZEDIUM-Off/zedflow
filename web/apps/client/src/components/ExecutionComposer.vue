<script setup lang="ts">
import type { WorkspaceContext, ModelEntry } from '@zedflow/sdk'

import { computed, ref, watch } from 'vue'
import { PopoverRoot, PopoverTrigger, PopoverPortal, PopoverContent } from 'reka-ui'
import { Check, ChevronDown, Code2, Folder, GitBranch, Plus, SlidersHorizontal, X } from 'lucide-vue-next'
import type { Composition, FlowFile, ModelSelection, Run, Workspace } from '@zedflow/sdk'
import type { AgentAttachments } from '../graph/attachments'
import { fixedSelection, type ModelNode } from '../harness'
import ModelPicker from './ModelPicker.vue'
import SkillSuggestions from './SkillSuggestions.vue'
import { useContextDetail } from '../composables/runDetails'
import { PromptInput, PromptInputTextarea, PromptInputSubmit, PromptInputFooter } from './ai-elements/prompt-input'
const draft=defineModel<string>('draft',{default:''})
const draftWait=defineModel<string|undefined>('draftWait')
const props=defineProps<{run:Run|null;workspace?:Workspace;composition:Composition;flows:FlowFile[];models:ModelEntry[];nodes:ModelNode[];bindings:Record<string,ModelSelection>;context:WorkspaceContext|null|undefined;busy:boolean;composed?:boolean;initializing?:boolean;submit:(text:string,kind:'steering'|'followup',waitId?:string,nodePath?:string)=>Promise<void>}>()
const emit=defineEmits<{flow:[file:FlowFile];definition:[];composition:[];details:[tab:'models'|'context'];change:[path:string,selection:ModelSelection];answer:[value:unknown,waitId:string];remove:[id:string];command:[action:'abort'|'resume'];newSession:[]}>()
const flowOpen=ref(false),flowQuery=ref(''),modelOpen=ref(false),messageKind=ref<'steering'|'followup'>('steering'),waitingSelection=ref<ModelSelection>()
const skillAgent=ref(props.nodes[0]?.path||'')
const skillTarget=computed(()=>props.nodes.find(node=>node.path===skillAgent.value)||props.nodes[0])
const {snapshot:skillSnapshot,entry:skillDetail,load:loadSkills}=useContextDetail(
  ()=>props.run?.contextSnapshots?.filter(snapshot=>(snapshot.agentPath||snapshot.nodePath||snapshot.origin?.nodePath)===skillTarget.value?.path).at(-1),
  ()=>(props.composition.formatVersion||1)>=2&&/^\/skill:[^\s]*$/.test(draft.value),
)
function sourcePath(path:string){const absolute=path.startsWith('/')?path:`${props.run?.workspacePath||props.workspace?.path||''}/${path}`;const parts:string[]=[];for(const part of absolute.split('/')){if(part==='..')parts.pop();else if(part&&part!=='.')parts.push(part)}return '/'+parts.join('/')}
const availableSkills=computed(()=>{
  if((props.composition.formatVersion||1)<2)return props.context?.skills||[]
  const pieces:AgentAttachments=(skillTarget.value?.context||skillTarget.value?.node)?.data.config.attachments||{}
  const discovered=(props.context?.skills||[]).filter(skill=>pieces.skills?.items.some(item=>item.enabled!==false&&(!item.name||item.name===skill.name)&&(item.source.kind==='workspace'||sourcePath(item.source.path)===sourcePath(skill.path))))
  const recorded=skillSnapshot.value?.skillCatalog||[]
  return [...new Map([...discovered,...recorded].map(skill=>[skill.name,skill])).values()]
})
const staleDraft=computed(()=>!!draft.value.trim()&&!!draftWait.value&&draftWait.value!==props.run?.wait?.id)
function reuseDraft(){draftWait.value=props.run?.status==='waiting'&&props.run.wait?.kind!=='model_selection'?props.run.wait?.id:undefined}
const modelWait=computed(()=>props.run?.wait?.kind==='model_selection')
const confirmation=computed(()=>props.run?.status==='waiting'&&!modelWait.value&&props.run.wait?.config.responseType==='confirmation')
const waitPath=computed(()=>props.run?.wait?.nodePath||props.run?.wait?.node||'')
const queueMode=computed(()=>props.run?.status==='running'||modelWait.value)
const pending=computed(()=>props.run?.queue?.filter(message=>message.status==='pending')||[])
const canWrite=computed(()=>!props.run||['running','stopped','interrupted'].includes(props.run.status)||props.run.status==='waiting'&&!confirmation.value)
const blocked=computed(()=>props.run?.import?.resumeBlocked||[])
const choices=computed(()=>props.flows.filter(file=>file.composition&&file.name.toLocaleLowerCase().includes(flowQuery.value.toLocaleLowerCase())))
const primary=computed(()=>props.nodes[0])
const selection=computed(()=>primary.value?(primary.value.runtime?props.bindings[primary.value.path]:fixedSelection(primary.value)):undefined)
const effortLabels:Record<string,string>={minimal:'Minimale',low:'Faible',medium:'Moyenne',high:'Élevée',xhigh:'Très élevée',none:'Aucune'}
const modelLabel=computed(()=>props.nodes.length>1?`${props.nodes.length} modèles`:selection.value?.model||'Choisir un modèle')
const waitingPrompt=computed(()=>props.run?.status==='waiting'?(props.run.wait?.config.prompt||(modelWait.value?'Choisissez le modèle de ce nœud':confirmation.value?'Confirmer cette étape ?':'Sur quoi continuer ?')):'')
watch(()=>props.run?.wait?.id,()=>{waitingSelection.value=props.bindings[waitPath.value]},{immediate:true})
watch(()=>JSON.stringify(props.bindings[waitPath.value]),()=>{waitingSelection.value=props.bindings[waitPath.value]})
function choose(file:FlowFile){flowOpen.value=false;flowQuery.value='';emit('flow',file)}
function answer(value:unknown){const id=props.run?.wait?.id;if(id)emit('answer',value,id)}
async function submit(message:{text:string}){await props.submit(message.text,messageKind.value,draftWait.value||props.run?.wait?.id,message.text.startsWith('/skill:')?skillTarget.value?.path:undefined)}
</script>
<template>
  <div class="composer-wrap">
    <div v-if="pending.length" class="pending-messages" aria-label="Messages en attente"><div v-for="message in pending" :key="message.id"><span class="queue-kind">{{message.kind==='steering'?'Réorientation':'En file'}}</span><p>{{message.text}}</p><button :disabled="busy" :aria-label="`Retirer ${message.text}`" @click="emit('remove',message.id)"><X :size="14"/></button></div></div>
    <div class="composer-context-line"><span :title="run?.workspacePath||workspace?.path"><Folder :size="13"/>{{workspace?.name||'Workspace'}}</span><PopoverRoot v-model:open="flowOpen"><PopoverTrigger class="flow-picker" aria-label="Choisir un flow"><GitBranch :size="13"/><span>{{composition.name}}</span><ChevronDown :size="11"/></PopoverTrigger><PopoverPortal><PopoverContent class="flow-menu composer-popover" side="top" align="start" :side-offset="10" :collision-padding="12"><input v-model="flowQuery" placeholder="Rechercher un flow…" aria-label="Rechercher un flow"/><button v-for="file in choices" :key="file.key" @click="choose(file)"><GitBranch :size="14"/><span>{{file.name}}<small>{{file.scope==='global'?'Global':workspace?.name}} · {{file.path.includes('/.agents/')?'.agents/flows':'.zedflow/flows'}}</small></span><Check v-if="file.composition?.id===composition.id" :size="13"/></button><small v-if="!choices.length">Aucun flow trouvé. Créez un flow dans Conception.</small><small v-if="run">Choisir un flow prépare une nouvelle session.</small><button @click="flowOpen=false;emit('definition')"><Code2 :size="14"/> Ouvrir la définition</button></PopoverContent></PopoverPortal></PopoverRoot><button v-if="!run||composed" class="composer-context-button" :disabled="busy" @click="emit('composition')"><GitBranch :size="13"/><span>{{composed?'Graphe résolu':'Composer'}}</span></button><button class="composer-context-button" title="Contexte chargé" @click="emit('details','context')"><SlidersHorizontal :size="13"/><span>Contexte</span></button></div>
    <div v-if="blocked.length" class="composer-import-blocked"><strong>Reprise indisponible</strong><p v-for="reason in blocked" :key="reason">{{reason}}</p></div>
    <div v-if="confirmation" class="confirmation-composer"><p class="composer-wait-prompt">{{waitingPrompt}}</p><div class="actions"><button :disabled="busy||!!blocked.length" @click="answer(false)">Refuser</button><button class="primary" :disabled="busy||!!blocked.length" @click="answer(true)">Confirmer</button></div></div>
    <template v-else-if="canWrite">
      <div v-if="modelWait" class="model-wait-composer"><p class="composer-wait-prompt">{{waitingPrompt}}</p><ModelPicker :key="run?.wait?.id" :selection="waitingSelection" :models="models" :disabled="busy||!!blocked.length" @change="waitingSelection=$event"/><button class="primary" :disabled="!waitingSelection||busy||!!blocked.length" @click="answer(waitingSelection)">Choisir et reprendre</button></div>
      <div v-if="staleDraft" class="stale-draft-notice" role="status"><span>Ce brouillon répondait à une attente précédente.</span><button type="button" @click="reuseDraft">Réutiliser le brouillon pour cette étape</button></div><PromptInput v-model="draft" class="composer" @submit="submit"><SkillSuggestions :skills="availableSkills" :loading="skillDetail?.loading" :error="skillDetail?.error" @retry="loadSkills"/><label v-if="draft.startsWith('/skill:')&&nodes.length>1" class="skill-agent-target">Agent destinataire<select v-model="skillAgent" aria-label="Agent destinataire du skill"><option v-for="node in nodes" :key="node.path" :value="node.path">{{node.node.data.label}}</option></select></label><p v-if="waitingPrompt&&!modelWait" class="composer-wait-prompt">{{waitingPrompt}}</p><PromptInputTextarea :readonly="initializing||(!run&&busy)" :disabled="!!blocked.length" :placeholder="queueMode?(messageKind==='steering'?'Réorienter le travail…':'Ajouter une demande à la file…'):run?.status==='stopped'||run?.status==='interrupted'?'Reprendre avec une instruction…':waitingPrompt?'Votre réponse…':'Que souhaitez-vous faire ?'"/><PromptInputFooter><div class="composer-actions"><select v-if="queueMode" v-model="messageKind" aria-label="Mode d’envoi"><option value="steering">Réorienter</option><option value="followup">Mettre en file</option></select><span v-else class="composer-hint">{{run?'Continuer la session':'Nouvelle session'}}</span><button v-if="run?.status==='running'" type="button" class="stop-button" :disabled="busy" @click="emit('command','abort')">■ Arrêter</button></div><div class="composer-send-controls"><PopoverRoot v-if="nodes.length" v-model:open="modelOpen"><PopoverTrigger type="button" class="model-picker-trigger" aria-label="Réglages des modèles"><span>{{modelLabel}}</span><small v-if="nodes.length===1&&selection?.reasoningEffort">{{effortLabels[selection.reasoningEffort]||selection.reasoningEffort}}</small><ChevronDown :size="12"/></PopoverTrigger><PopoverPortal><PopoverContent class="model-menu composer-popover" side="top" align="end" :side-offset="14" :collision-padding="12"><div v-for="node in nodes" :key="node.path" class="model-menu-entry"><h3>{{node.node.data.label}}</h3><ModelPicker :selection="node.runtime?bindings[node.path]:fixedSelection(node)" :models="models" :disabled="busy||!node.runtime||!!blocked.length" @change="emit('change',node.path,$event)"/><small v-if="!node.runtime">Modèle fixé dans le flow.</small></div><button class="model-menu-details" @click="modelOpen=false;emit('details','models')">Tous les détails des modèles</button></PopoverContent></PopoverPortal></PopoverRoot><PromptInputSubmit :disabled="initializing||busy||!workspace||!!blocked.length||staleDraft"/></div></PromptInputFooter></PromptInput>
    </template>
    <div v-else class="composer-finished"><span>{{run?.status==='completed'?'Session terminée.':'Cette session ne peut plus recevoir de message.'}}</span><button @click="emit('newSession')"><Plus :size="15"/> Nouvelle session</button></div>
  </div>
</template>
