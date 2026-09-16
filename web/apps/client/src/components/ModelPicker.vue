<script setup lang="ts">
import { modelSelectionSchema, type HistoricalModelSelection, type ModelEntry } from '@zedflow/sdk'

import { computed, ref, useId, watch } from 'vue'
import type { ModelSelection } from '../harness'
const props = defineProps<{ selection?: HistoricalModelSelection; models: ModelEntry[]; disabled?: boolean; label?: string }>()
const emit = defineEmits<{ change: [selection: ModelSelection] }>()
const id = useId()
const provider = ref(props.selection?.provider || '')
const model = ref(props.selection?.model || '')
const effort = ref(props.selection?.reasoningEffort || '')
const budget = ref<number | undefined>(props.selection?.thinkingBudget ?? undefined)
watch(() => JSON.stringify(props.selection), () => {
  const value = props.selection
  provider.value = value?.provider || ''
  model.value = value?.model || ''
  effort.value = value?.reasoningEffort || ''
  budget.value = value?.thinkingBudget ?? undefined
})
const providers = computed(() => [...new Set([...props.models.map(value => value.provider), 'codex', 'gemini', 'fixture'])])
const options = computed(() => props.models.filter(value => value.provider === provider.value))
const capability = computed(() => options.value.find(value => value.id === model.value))
const names: Record<string, string> = { codex: 'Codex', gemini: 'Gemini', fixture: 'Démonstration' }
const levels: Record<string, string> = { minimal: 'Minimale', low: 'Faible', medium: 'Moyenne', high: 'Élevée', xhigh: 'Très élevée', none: 'Aucune' }
const error = ref('')
function commit() {
  if (!provider.value || !model.value.trim()) return
  const parsed = modelSelectionSchema.safeParse({ provider: provider.value, model: model.value.trim(), ...(effort.value ? { reasoningEffort: effort.value } : {}) })
  error.value = parsed.success ? '' : parsed.error.issues.map(issue => issue.message).join('; ')
  if (parsed.success) emit('change', parsed.data)
}
function changeProvider() { model.value = options.value[0]?.id || ''; effort.value = ''; budget.value = undefined; commit() }
function changeModel() { effort.value = ''; budget.value = undefined; commit() }
</script>
<template>
  <div class="model-picker" :aria-label="label || 'Réglages du modèle'">
    <label>Fournisseur<select v-model="provider" :disabled="disabled" aria-label="Fournisseur du modèle" @change="changeProvider"><option value="" disabled>Choisir…</option><option v-for="name in providers" :key="name" :value="name">{{ names[name] || name }}</option></select></label>
    <label>Modèle<input v-model="model" :list="`${id}-models`" :disabled="disabled || !provider" aria-label="Modèle" placeholder="Choisir ou saisir un modèle" @change="changeModel"/><datalist :id="`${id}-models`"><option v-for="option in options" :key="option.id" :value="option.id">{{ option.label }}</option></datalist></label>
    <label>Réflexion<select v-model="effort" :disabled="disabled || !capability?.reasoningLevels?.length" aria-label="Réflexion" @change="commit"><option value="">Défaut du modèle</option><option v-for="level in capability?.reasoningLevels || []" :key="level" :value="level">{{ levels[level] || level }}</option></select></label>
    <p v-if="budget !== undefined" class="muted">Budget historique : {{budget}} tokens</p><p v-if="error" class="field-error">{{error}}</p>
  </div>
</template>
