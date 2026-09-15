<script setup lang="ts">
import { onMounted,ref,shallowRef } from 'vue'
import type { LiveMetrics, RunMetrics } from '@zedflow/sdk'
import { useRunDetails } from '../composables/runDetails'
defineProps<{metrics?:LiveMetrics}>()
const details=useRunDetails(),server=shallowRef<RunMetrics>(),error=ref(''),loading=ref(false)
const milliseconds=(value:number|undefined)=>`${(value||0).toFixed(2)} ms`
async function load(){loading.value=true;error.value='';try{server.value=await details.load({...details.scope(),kind:'metrics',revision:Date.now()})}catch(cause){error.value=cause instanceof Error?cause.message:String(cause)}finally{loading.value=false}}
onMounted(()=>void load())
</script>
<template>
  <section class="transport-metrics">
    <small>Depuis la connexion au flux · temps cumulés côté client</small>
    <dl><dt>Réceptions</dt><dd>{{metrics?.frames||0}}</dd><dt>Bootstrap / deltas</dt><dd>{{metrics?.bootstraps||0}} / {{metrics?.deltas||0}}</dd><dt>Heartbeats / doublons ignorés</dt><dd>{{metrics?.heartbeats||0}} / {{metrics?.duplicates||0}}</dd><dt>Caractères reçus sur SSE / RTC</dt><dd>{{(metrics?.receivedChars||0).toLocaleString()}}</dd><dt>Décodage JSON SSE / RTC</dt><dd>{{milliseconds(metrics?.decodeMs)}}</dd><dt>Application des changements</dt><dd>{{milliseconds(metrics?.applyMs)}}</dd><dt>Publication dans l’état Vue</dt><dd>{{milliseconds(metrics?.publicationMs)}}</dd></dl>
    <small>Le volume et le décodage mesurent le flux SSE / RTC. La publication ne mesure pas le rendu ou la peinture du navigateur.</small>
    <template v-if="server?.batches?.length"><strong>Dernier lot du daemon · {{server.batches.at(-1).events}} événements</strong><dl><dt>Attente du writer</dt><dd>{{milliseconds(server.batches.at(-1).writerWaitMs)}}</dd><dt>Encodage</dt><dd>{{milliseconds(server.batches.at(-1).encodeMs)}}</dd><dt>Commit SQLite</dt><dd>{{milliseconds(server.batches.at(-1).commitMs)}}</dd><dt>Publication</dt><dd>{{milliseconds(server.batches.at(-1).publishMs)}}</dd></dl></template>
    <p v-if="error" class="detail-error">{{error}}</p><button :disabled="loading" @click="load">Actualiser les mesures</button>
  </section>
</template>
