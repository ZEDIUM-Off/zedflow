<script setup lang="ts">
import { onScopeDispose, computed, ref, watch } from 'vue'
import { useRunDetails } from '../composables/runDetails'
const props = defineProps<{ invocation: string; revision?: string }>()
const details = useRunDetails(), opened = ref(false), raw = ref(''), error = ref(''), loading = ref(false)
const entry = computed(() => details.entry({...details.scope(),kind:'requests',id:props.invocation,revision:props.revision}))
const boundary = computed(() => entry.value?.value?.capture?.boundary)
const labels: Record<string,string> = { codexHttpBody: 'Corps JSON Codex préparé pour le transport', adkRequest: 'Requête fournie à ADK — corps HTTP fournisseur non exposé', fixtureInput: 'Entrée du modèle fixture' }
let intent = 0
watch(() => [props.invocation, props.revision, opened.value, details.scope().runId, details.scope().workspaceId], async () => {
  const request = ++intent
  raw.value = ''; error.value = ''; loading.value = false
  if (!opened.value) return
  loading.value = true
  const invocation=props.invocation,revision=props.revision,scope=details.scope()
  try {
    const value = await details.load({...scope,kind:'requests',id:invocation,revision})
    if (!value?.capture) return
    const bytes = await details.requestRaw(scope,invocation)
    const text = new TextDecoder().decode(bytes)
    if (request === intent) raw.value = text
  } catch (cause) { if (request === intent) error.value = String(cause) }
  finally { if (request === intent) loading.value = false }
})

onScopeDispose(()=>{intent++})
</script>
<template><details class="inference-raw" @toggle="opened = ($event.target as HTMLDetailsElement).open"><summary>Raw de l’invocation</summary><p v-if="loading">Chargement…</p><p v-if="error" role="alert">{{ error }}</p><template v-if="entry?.value"><p v-if="boundary">{{ labels[boundary] || boundary }} · {{ entry.value.status === 'sent' ? 'requête envoyée' : 'requête préparée' }}</p><p v-else>Aucune capture brute conservée pour cette invocation. Le manifeste ADK reste consultable.</p><button v-if="boundary" @click="details.download('request',invocation).catch(cause=>error=String(cause))">Télécharger les octets originaux</button><pre v-if="raw">{{ raw }}</pre><details><summary>Manifeste de requête ADK</summary><pre>{{ JSON.stringify(entry.value.manifest, null, 2) }}</pre></details></template></details></template>
<style scoped>.inference-raw{margin:12px 0;min-width:0}.inference-raw p{font-size:12px;color:#a4a0b0}.inference-raw pre{white-space:pre-wrap;overflow-wrap:anywhere;max-height:500px;overflow:auto;font-size:11px}</style>
