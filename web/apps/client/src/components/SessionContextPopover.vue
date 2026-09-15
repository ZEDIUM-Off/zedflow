<script setup lang="ts">
import { computed } from 'vue'
import { PopoverRoot, PopoverTrigger, PopoverPortal, PopoverContent, PopoverClose } from 'reka-ui'
import { Activity, ArrowUpRight, Bot, Folder, GitBranch, Settings2 } from 'lucide-vue-next'
import type { Composition, ModelSelection, Run, Workspace } from '@zedflow/sdk'
import { fixedSelection, modelNodes } from '../harness'
import { statuses } from '../composables/useZedflow'
const props=defineProps<{workspace?:Workspace;composition:Composition;run:Run|null;bindings:Record<string,ModelSelection>}>()
const emit=defineEmits<{details:[tab?:'activity'|'models'|'context']}>()
const selections=computed(()=>modelNodes(props.composition).map(entry=>({path:entry.path,name:entry.node.data.label,selection:entry.runtime?props.bindings[entry.path]:fixedSelection(entry)})))
const active=computed(()=>props.run?.activities?.filter(activity=>['running','waiting'].includes(activity.status)).at(-1))
</script>
<template>
  <PopoverRoot><PopoverTrigger class="icon-button context-trigger" aria-label="Contexte de la session" title="Contexte de la session"><Settings2 :size="18"/></PopoverTrigger><PopoverPortal><PopoverContent class="session-context-popover" side="bottom" align="end" :side-offset="10" :collision-padding="12">
    <h2>Environnement</h2><div class="context-popover-row"><Folder :size="15"/><div><strong>{{workspace?.name||'Workspace'}}</strong><small>{{run?.workspacePath||workspace?.path}}</small></div></div>
    <div class="context-popover-row"><GitBranch :size="15"/><div><strong>{{composition.name}}</strong><small>{{run?'Version conservée pour cette session':'Flow sélectionné'}}</small></div></div>
    <h2>Modèles</h2><PopoverClose v-for="item in selections" :key="item.path" class="context-popover-row context-popover-button" @click="emit('details','models')"><Bot :size="15"/><div><strong>{{item.selection?.model||'À choisir'}}</strong><small>{{item.name}}<template v-if="item.selection?.reasoningEffort"> · {{item.selection.reasoningEffort}}</template><template v-if="item.selection?.thinkingBudget!==undefined"> · {{item.selection.thinkingBudget}} tokens</template></small></div></PopoverClose><p v-if="!selections.length" class="muted">Aucun appel modèle dans ce flow.</p>
    <h2>Session</h2><div class="context-popover-row"><Activity :size="15"/><div><strong>{{run?statuses[run.status]||run.status:'Prête à démarrer'}}</strong><small>{{active?`${active.label} · étape ${active.step}`:run?'Aucune étape active':'En attente de votre demande'}}</small></div></div>
    <PopoverClose class="context-popover-details" @click="emit('details','activity')">Ouvrir les détails <ArrowUpRight :size="15"/></PopoverClose>
  </PopoverContent></PopoverPortal></PopoverRoot>
</template>
