<script setup lang="ts">
import InferenceRaw from './InferenceRaw.vue'
import { useContextDetail } from '../composables/runDetails'
const props=withDefaults(defineProps<{snapshot:Record<string,any>;active?:boolean}>(),{active:true})
const {snapshot,entry,load}=useContextDetail(()=>props.snapshot,()=>props.active)
</script>
<template>
  <div class="captured-context"><InferenceRaw v-if="snapshot.invocationId" :invocation="snapshot.invocationId" :revision="[snapshot.requestRef,snapshot.rawRef,snapshot.requestStatus].join('/')"/>
    <p v-if="entry?.loading" class="detail-loading">Chargement du contexte…</p>
    <p v-if="entry?.error" class="detail-error">{{entry.error}} <button @click="load">Réessayer</button></p>
    <details v-for="resource in snapshot.resources||[]" :key="`${resource.id}:${resource.path}`" class="captured-resource"><summary>{{resource.name||resource.path||resource.id}}<span v-if="resource.truncated"> · partiel</span></summary><small>{{resource.kind}} · {{resource.hash}}</small><pre>{{resource.content}}</pre></details>
    <p v-if="!snapshot.resources?.length&&!entry?.loading" class="muted">Aucun contenu de ressource injecté lors de cet appel.</p>
    <h3>Catalogue présenté · {{snapshot.skillCatalog?.length||0}}</h3><div v-for="skill in snapshot.skillCatalog||[]" :key="skill.activationKey" class="captured-skill"><strong>{{skill.name}}</strong><small>{{skill.path}}</small></div>
  </div>
</template>
