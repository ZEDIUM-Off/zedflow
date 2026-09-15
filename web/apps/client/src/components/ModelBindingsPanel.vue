<script setup lang="ts">
import type { ModelEntry } from '@zedflow/sdk'

import { computed } from 'vue'
import { X, LocateFixed, LockKeyhole } from 'lucide-vue-next'
import ModelPicker from './ModelPicker.vue'
import { fixedSelection, type ModelNode, type ModelSelection } from '../harness'
const props = defineProps<{ nodes: ModelNode[]; models: ModelEntry[]; bindings: Record<string, ModelSelection>; selectedPath?: string; busy?: boolean; running?: boolean }>()
const emit = defineEmits<{ close: []; select: [path: string]; change: [path: string, selection: ModelSelection] }>()
const groups = computed(() => [...new Set(props.nodes.map(entry => entry.group))].map(group => ({ label: group || 'Flow principal', entries: props.nodes.filter(entry => entry.group === group) })))
</script>
<template>
  <section class="harness-panel models-panel" aria-label="Modèles du flow">
    <header><div><strong>Modèles du flow</strong><small>{{ running ? 'Les changements s’appliquent au prochain appel.' : 'Configurez maintenant, ou au passage dans le nœud.' }}</small></div><button aria-label="Fermer les modèles" @click="emit('close')"><X :size="16"/></button></header>
    <section v-for="group in groups" :key="group.label" class="model-group"><h3>{{ group.label }}</h3><div v-for="entry in group.entries" :key="entry.path" class="model-binding-row" :class="{selected: selectedPath === entry.path}" :data-model-path="entry.path">
      <button class="model-node-heading" @click="emit('select', entry.path)"><LocateFixed :size="14"/><strong>{{ entry.node.data.label }}</strong><span>{{ entry.runtime ? bindings[entry.path] ? 'Configuré' : 'À choisir' : 'Fixé dans le flow' }}</span><LockKeyhole v-if="!entry.runtime" :size="12"/></button><small>{{ entry.path }}</small>
      <ModelPicker :selection="entry.runtime ? bindings[entry.path] : fixedSelection(entry)" :models="models" :disabled="!entry.runtime || busy" @change="emit('change', entry.path, $event)"/>
    </div></section>
  </section>
</template>
