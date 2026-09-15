<script setup lang="ts">
import { computed } from 'vue'
import { textMessage, textMessageValue } from './contextMessageProjection'
import { contextExpressionType } from './contextComposer'
import { resolveSourceType } from '../../contextSources'
import type { ContextBlock, ContextType } from '@zedflow/sdk'

const block = defineModel<Extract<ContextBlock, { kind: 'emit' }>>({ required: true })
const props = defineProps<{ resources: Record<string, ContextType>; types?: Record<string, ContextType>; variables?: Record<string, ContextType> }>()
const message = computed(() => textMessageValue(block.value.value))
const canProject = computed(() => {
  if (message.value) return true
  const type = contextExpressionType(block.value.value, props.resources, props.types, props.variables)
  return type && resolveSourceType(type, props.types).kind === 'text'
})
function project(role: string) {
  if (!canProject.value) return
  if (role === 'user' || role === 'model') { block.value.value = textMessage(message.value?.value || block.value.value, role); block.value.format = 'adkMessages'; block.value.role = 'data' }
  else if (message.value) { block.value.value = message.value.value; block.value.format = 'text' }
}
</script>
<template>
  <label v-if="canProject" class="ctx-message-projection">Message à partir de ce texte<select aria-label="Projection du texte en message" :value="message?.role||'text'" @change="project(($event.target as HTMLSelectElement).value)"><option value="text">Texte du contexte</option><option value="user">Message utilisateur</option><option value="model">Réponse du modèle</option></select></label>
</template>
<style scoped>.ctx-message-projection{font-size:10px!important}.ctx-message-projection select{font-size:11px}</style>
