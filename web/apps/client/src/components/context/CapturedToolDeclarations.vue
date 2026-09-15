<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRunDetails } from '../../composables/runDetails'
const props = defineProps<{ invocation: string; revision: string }>()
const details = useRunDetails(), opened = ref(false)
const entry = computed(() => details.entry({...details.scope(),kind:'requests',id:props.invocation,revision:props.revision}))
const declarations = computed(() => entry.value?.value?.manifest?.request?.tools)
watch(() => [opened.value, props.invocation, props.revision], () => {
  if (opened.value) void details.load({...details.scope(),kind:'requests',id:props.invocation,revision:props.revision}).catch(() => {})
})
</script>
<template>
  <details @toggle="opened = ($event.target as HTMLDetailsElement).open">
    <summary>Déclarations effectivement transmises à ADK</summary>
    <p v-if="entry?.loading">Chargement des déclarations capturées…</p>
    <p v-else-if="entry?.error" role="alert">{{ entry.error }}</p>
    <template v-else-if="declarations">
      <details v-for="(declaration, name) in declarations" :key="name">
        <summary>{{ name }}</summary>
        <pre>{{ JSON.stringify(declaration, null, 2) }}</pre>
      </details>
      <p v-if="Object.keys(declarations).length === 0">Aucun outil déclaré pour cet appel.</p>
    </template>
    <p v-else-if="entry?.value">Déclarations historiques non capturées.</p>
  </details>
</template>
<style scoped>pre{white-space:pre-wrap;overflow-wrap:anywhere;font-size:11px}details{margin:12px 0}</style>
