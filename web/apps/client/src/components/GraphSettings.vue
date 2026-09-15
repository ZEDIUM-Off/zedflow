<script setup lang="ts">
const client=useClient()
import { useClient } from '@zedflow/vue'

import { onMounted, ref } from 'vue'
import type { Composition } from '@zedflow/sdk'
import JsonField from './JsonField.vue'
const props=defineProps<{doc:Composition}>()
props.doc.settings ||= {recursionLimit:100,strictChannels:false}
props.doc.channels ||= []
const capabilities=ref<any>(null)
onMounted(async()=>{try{capabilities.value=await client.daemon.capabilities()}catch{/* Settings remain editable offline. */}})

</script>
<template><div class="graph-settings">
  <p class="muted">Ces paramètres sont appliqués par ADK et inclus dans le Rust généré.</p>
  <div class="settings-columns"><section><h3>Exécution du graphe</h3>
    <label>Dossier de travail du flow<input aria-label="Dossier de travail du flow" :value="doc.settings!.workingDirectory" @input="($event.target as HTMLInputElement).value.trim()?doc.settings!.workingDirectory=($event.target as HTMLInputElement).value:delete doc.settings!.workingDirectory" placeholder="Dossier du workspace"/><small>Chemin sur la machine du daemon. Un chemin relatif part du workspace. Les ressources et outils de cette instance utilisent ce dossier.</small></label>
    <label>Limite d’étapes<input type="number" v-model.number="doc.settings!.recursionLimit" min="1"/></label>
    <label>Concurrence maximale<input type="number" :value="doc.settings!.maxConcurrency" min="1" placeholder="Automatique" @input="doc.settings!.maxConcurrency=($event.target as HTMLInputElement).value?Number(($event.target as HTMLInputElement).value):null"/></label>
    <label>Délai maximal par nœud · ms<input type="number" :value="doc.settings!.timeoutMs" min="1" placeholder="Non défini" @input="doc.settings!.timeoutMs=($event.target as HTMLInputElement).value?Number(($event.target as HTMLInputElement).value):null"/></label>
    <label>Délai sans progression · ms<input type="number" :value="doc.settings!.idleTimeoutMs" min="1" placeholder="Non défini" @input="doc.settings!.idleTimeoutMs=($event.target as HTMLInputElement).value?Number(($event.target as HTMLInputElement).value):null"/></label>
    <label class="check-field"><input type="checkbox" v-model="doc.settings!.strictChannels"/>Canaux stricts</label>
    <p class="muted">Les écritures doivent cibler un canal déclaré lorsque le mode strict est activé.</p>
    <details class="settings-group"><summary>Politique de reprise sur erreur</summary><label class="check-field"><input type="checkbox" :checked="!!doc.settings!.retry" @change="doc.settings!.retry=($event.target as HTMLInputElement).checked?{maxAttempts:2,initialDelayMs:1000,maxDelayMs:60000,backoffFactor:2,jitter:0,retryOn:'any'}:null"/>Activer</label><template v-if="doc.settings!.retry"><label>Tentatives<input type="number" min="1" v-model.number="doc.settings!.retry.maxAttempts"/></label><label>Délai initial · ms<input type="number" min="0" v-model.number="doc.settings!.retry.initialDelayMs"/></label><label>Délai maximal · ms<input type="number" min="0" v-model.number="doc.settings!.retry.maxDelayMs"/></label><label>Facteur<input type="number" min="1" step="0.1" v-model.number="doc.settings!.retry.backoffFactor"/></label><label>Jitter<input type="number" min="0" max="1" step="0.1" v-model.number="doc.settings!.retry.jitter"/></label><label>Déclencheur<select v-model="doc.settings!.retry.retryOn"><option value="any">Toute erreur</option><option value="timeout">Délai dépassé</option></select></label></template></details>
  </section><section><h3>État et reducers</h3><p class="muted">Les canaux rassemblent les écritures des nœuds, notamment des branches parallèles.</p>
    <div v-for="(channel,i) in doc.channels" :key="i" class="channel-card"><label>Nom<input v-model="channel.name"/></label><label>Reducer<select v-model="channel.reducer"><option value="overwrite">Remplacer</option><option value="append">Ajouter à une liste</option><option value="sum">Sommer</option></select></label><JsonField v-model="channel.default" label="Valeur initiale JSON" :rows="2"/><button class="danger" @click="doc.channels!.splice(i,1)">Retirer le canal</button></div>
    <button @click="doc.channels!.push({name:'',reducer:'overwrite',default:null})">Ajouter un canal</button>
  </section></div>
  <details v-if="capabilities" class="settings-group"><summary>Capacités exposées par ce daemon · ADK 2.2.0</summary><p class="muted">Inventaire d’intégration. Les capacités non prises en charge restent explicites.</p><div class="capabilities-grid"><div v-for="item in capabilities.nodes" :key="item.kind" class="capability-card"><strong>{{item.label}}</strong><small>{{item.description}}</small><p>{{item.fields.join(' · ')||'Aucun paramètre'}}</p></div></div><h3 class="capability-heading">À intégrer</h3><div v-for="item in capabilities.unsupported" :key="item.id" class="capability-gap"><code>{{item.id}}</code><p>{{item.reason}}</p></div></details>
</div></template>
