<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import AppDialog from '../AppDialog.vue'
import { contextId } from '../../contextEngine'
import type { ContextBlock, ContextType } from '@zedflow/sdk'
import { resolveSourceType } from '../../contextSources'
import { toolExchange } from './contextMessageProjection'

const open = defineModel<boolean>('open', { required: true })
const props = defineProps<{ resources: Record<string, ContextType>; types?: Record<string, ContextType> }>()
const emit = defineEmits<{ add: [block: ContextBlock] }>()
const call = ref(''), result = ref('')
function candidates(fields: Record<string, ContextType['kind']>) {
  return Object.entries(props.resources).filter(([, type]) => {
    const source = resolveSourceType(type, props.types)
    return source.kind === 'record' && Object.entries(fields).every(([key, kind]) => source.fields[key] && resolveSourceType(source.fields[key], props.types).kind === kind)
  }).map(([name]) => name)
}
const calls = computed(() => candidates({ id: 'text', name: 'text', arguments: 'record' }))
const results = computed(() => candidates({ callId: 'text', status: 'text', content: 'text' }))
watch(open, value => { if (value) { call.value = calls.value[0] || ''; result.value = results.value[0] || '' } })
function add() {
  if (!calls.value.includes(call.value) || !results.value.includes(result.value)) return
  emit('add', { kind: 'emit', id: contextId('tool-exchange'), role: 'data', format: 'adkMessages', value: toolExchange({ kind: 'resource', name: call.value }, { kind: 'resource', name: result.value }) }); open.value = false
}
</script>
<template>
  <AppDialog v-model:open="open" title="Composer un échange d’outil" description="Ajouter un appel enregistré et son résultat au contexte. Cette opération n’exécute aucun outil.">
    <div class="ctx-tool-exchange-form"><label>Appel enregistré<select v-model="call" aria-label="Source de l’appel enregistré"><option value="" disabled>Choisir une source</option><option v-for="name in calls" :key="name">{{ name }}</option></select></label><label>Résultat enregistré<select v-model="result" aria-label="Source du résultat enregistré"><option value="" disabled>Choisir une source</option><option v-for="name in results" :key="name">{{ name }}</option></select></label><p v-if="!calls.length||!results.length">Déclarez les types Appel d’outil et Résultat d’outil dans Sources. Les champs id, name, arguments et callId, status, content définissent leur association.</p><small>Les identifiants de l’appel et du résultat sont conservés. Vérifiez qu’ils désignent le même appel dans vos données d’essai.</small></div>
    <footer><button :disabled="!call||!result" @click="add">Ajouter l’échange au contexte</button></footer>
  </AppDialog>
</template>
<style scoped>.ctx-tool-exchange-form{display:flex;flex-direction:column;gap:16px;font-size:12px}.ctx-tool-exchange-form label{display:flex;flex-direction:column;gap:7px}.ctx-tool-exchange-form small{color:#a4a7b1;line-height:1.6}</style>
