<script setup lang="ts">
import { nodeConfigSchema } from '@zedflow/sdk'
import { computed } from 'vue'
import type { NodeContract } from '@zedflow/sdk'
import { Handle, Position } from '@vue-flow/core'
import { Bot, Braces, CircleStop, GitBranch, MessageSquare, Play, CornerDownLeft, Wrench, BookOpen, Inbox, Signpost, FileText, ScrollText, Plus, Layers, Cpu } from 'lucide-vue-next'
import type { FlowNode, NodeActivity } from '@zedflow/sdk'
import { attachmentSlots, type AttachmentSlot, type AgentAttachments } from '../graph/attachments'
import { predicateLabel } from '../graph/predicate'
import '../graph.css'
const props = withDefaults(defineProps<{
  data: FlowNode['data'] & { active?: boolean; executionStatus?: NodeActivity['status']; occurrences?: number }
  contract?: NodeContract; selected?: boolean; formatVersion?: number; readonly?: boolean; historical?: boolean
}>(), {formatVersion: 1, readonly: false, historical: false})
const emit = defineEmits<{attachment:[slot:AttachmentSlot]}>()
const icons = { route: GitBranch, await_route: Inbox, context: Layers, model: Cpu, steering: Signpost, inbox: Inbox, tool: Wrench, subgraph: GitBranch, agent: Bot, set: Braces, end: CircleStop, condition: GitBranch, input: CornerDownLeft, output: MessageSquare, start: Play }
const pieceIcons = {instructions: ScrollText, skills: BookOpen, files: FileText, tools: Wrench}
const statuses = { running: 'En cours', completed: 'Terminé', waiting: 'Réponse attendue', error: 'Échec', interrupted: 'Interrompu', resumed: 'Repris' }
const pieces = computed(() => props.data.kind==='agent' && props.formatVersion >= 2 && !props.historical)
const parsedConfig=computed(()=>nodeConfigSchema.safeParse(props.data.config))
const attachments = computed(() => parsedConfig.value.success?parsedConfig.value.data.attachments:undefined)
const detail = computed(() => {
  const {kind} = props.data;const parsed=parsedConfig.value;if(!parsed.success)return 'Configuration non prise en charge · consulter la source';const config=parsed.data
  if (kind==='agent'||kind==='model') return config.modelBinding==='runtime'?'Modèle choisi à l’exécution':config.provider==='fixture'?'Fixture · une itération':config.model||'Modèle à configurer'
  if (kind==='condition') return config.predicate?predicateLabel(config.predicate):`${config.field} = ${JSON.stringify(config.equals)}`
  if (['input','inbox'].includes(kind)) return config.prompt
  if (kind==='route') return config.branch?`${config.branch} · ${config.invocation==='condition'?'continuer si aucune route':'route requise'}`:'Port public à sélectionner'
  if (kind==='await_route') return `Visite depuis ${config.inputField||'output'}`
  if (kind==='set') return `${config.field} ← valeur`
  if (kind==='context') {const key=typeof config.contextStrategy==='string'?config.contextStrategy:config.contextStrategy?.key||'';return props.formatVersion>=3 ? ({'harness-default':'Contexte du Harness','workspace-default':'Contexte du workspace','conversation-default':'Conversation et instructions','tools-default':'Conversation avec outils'} as Record<string,string>)[key]||key||(config.contextProgram?'Programme embarqué':'Stratégie à choisir') : 'Instructions · skills'}
  if (kind==='steering') return 'Consommer la prochaine consigne'
  return {start:'Entrée du flow',end:'Exécution terminée',output:'Publier dans la conversation',tool:config.tool,subgraph:'Flow embarqué'}[kind as 'start'|'end'|'output'|'tool'|'subgraph'] || kind
})
function count(slot: AttachmentSlot) { return attachments.value?.[slot]?.items.filter(item=>item.enabled!==false).length || 0 }
</script>
<template>
  <div class="flow-node" :class="[data.kind, {'with-pieces':pieces,selected,active:data.active}]" :data-node-kind="data.kind" :data-execution-status="data.executionStatus">
    <div v-if="pieces" class="agent-pieces before">
      <button v-for="slot in attachmentSlots.slice(0,2)" :key="slot.id" type="button" class="attachment-piece nodrag nopan" :class="{'is-empty':!count(slot.id)}" :data-slot="slot.id" :aria-label="`${readonly?'Voir':'Configurer'} ${slot.label.toLowerCase()}`" :title="`${slot.hint} · ${count(slot.id)} ressource(s)`" @click.stop="emit('attachment',slot.id)"><component :is="pieceIcons[slot.id]" :size="13"/><span>{{slot.label}}</span><small>{{count(slot.id)||'—'}}</small><Plus v-if="!readonly" :size="10"/></button>
    </div>
    <div class="flow-card" :class="[data.kind,{selected,active:data.active}]" :data-execution-status="data.executionStatus">
      <svg v-if="data.kind==='condition'" class="node-outline" viewBox="0 0 220 170" preserveAspectRatio="none" aria-hidden="true"><path d="M110 2 L218 85 L110 168 L2 85 Z"/><path class="port-stubs" d="M185 59.5 H220 M185 110.5 H220"/></svg>
      <svg v-else-if="['input','inbox','output'].includes(data.kind)" class="node-outline" viewBox="0 0 220 110" preserveAspectRatio="none" aria-hidden="true"><path :d="data.kind==='output'?'M2 2 H193 L218 55 L193 108 H2 Z':'M22 2 H218 V108 H22 L2 55 Z'"/></svg>
      <div class="node-content"><div class="node-heading"><component :is="Object.entries(icons).find(([kind])=>kind===data.kind)?.[1]" :size="16"/><span>{{data.label}}</span><span v-if="data.active" class="live-dot"/></div>
      <div v-if="formatVersion>=3&&['context','model'].includes(data.kind)" class="node-stage">{{data.kind==='context'?'1 · Préparer le contexte':'2 · Appeler le modèle'}}</div>
      <div v-if="historical" class="node-stage">Ancien format · à convertir</div>
      <div class="node-detail" :title="detail">{{detail}}</div>
      <div v-if="data.executionStatus" class="node-runtime"><span>{{statuses[data.executionStatus]}}</span><span v-if="data.occurrences&&data.occurrences>1">Passage {{data.occurrences}}</span></div></div>
    </div>
    <div v-if="pieces" class="agent-pieces after">
      <button v-for="slot in attachmentSlots.slice(2)" :key="slot.id" type="button" class="attachment-piece nodrag nopan" :class="{'is-empty':!count(slot.id)}" :data-slot="slot.id" :aria-label="`${readonly?'Voir':'Configurer'} ${slot.label.toLowerCase()}`" :title="`${slot.hint} · ${count(slot.id)} ressource(s)`" @click.stop="emit('attachment',slot.id)"><component :is="pieceIcons[slot.id]" :size="13"/><span>{{slot.label}}</span><small>{{count(slot.id)||'—'}}</small><Plus v-if="!readonly" :size="10"/></button>
    </div>
    <Handle v-if="data.kind!=='start'" :id="formatVersion>=4?contract?.inputs[0]?.id:undefined" type="target" :position="Position.Left" :title="contract?.inputs[0]?.label||'Entrée'"/><span v-if="formatVersion>=4&&contract?.inputs.length" class="named-input">{{contract.inputs[0]?.label}}</span>
    <template v-if="data.kind==='condition'"><Handle id="true" type="source" :position="Position.Right" style="top:35%"/><Handle id="false" type="source" :position="Position.Right" style="top:65%"/><span class="condition-port yes">Oui</span><span class="condition-port no">Non</span></template>
    <Handle v-else-if="data.kind!=='end'" :id="formatVersion>=4?contract?.outputs[0]?.id:undefined" type="source" :position="Position.Right" :title="contract?.outputs[0]?.label||'Sortie'"/><span v-if="formatVersion>=4&&data.kind!=='condition'&&contract?.outputs.length" class="named-output">{{contract.outputs[0]?.label}}</span>
  </div>
</template>

<style scoped>.named-input,.named-output{position:absolute;bottom:-17px;font-size:9px;color:#aeb8cb}.named-input{left:0}.named-output{right:0}</style>
