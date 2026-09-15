<script setup lang="ts">
import type { JsonValue } from '@zedflow/sdk'

const client=useClient()
import { useClient } from '@zedflow/vue'
import { computed, ref, shallowRef, watch } from 'vue'
import ContextSourceExamples from './ContextSourceExamples.vue'

import { contextSourceCategories, contextSourceStyle, contextSourceTypeId, normalizeContextSourceEntries, type ContextSourceCatalog, type ContextSourceEntry } from '../../contextSources'
const exampleValue=shallowRef<JsonValue>()
const props = defineProps<{ workspaceId: string }>()
defineEmits<{ copy: [entry: ContextSourceEntry] }>()
const entries = ref<ContextSourceEntry[]>([]), selected = ref(''), search = ref(''), category = ref('all'), error = ref(''), loading = ref(false)
const filtered = computed(() => entries.value.filter(entry => (category.value === 'all' || entry.category === category.value) && `${entry.label} ${entry.origin} ${entry.id}`.toLocaleLowerCase().includes(search.value.toLocaleLowerCase())))
const current = computed(() => entries.value.find(entry => entry.id === selected.value))
watch(selected,()=>{exampleValue.value=undefined})
let intent = 0
watch(() => props.workspaceId, async workspace => {
  const request = ++intent
  entries.value = []; selected.value = ''; error.value = ''; loading.value = true
  try {
    const value = await client.context.sourceTypes({workspaceId:workspace})
    if (request !== intent) return
    entries.value = normalizeContextSourceEntries(value.entries)
    error.value = value.diagnostics?.map(item => item.message).join(' · ') || ''
  } catch (cause) { if (request === intent) error.value = String(cause) }
  finally { if (request === intent) loading.value = false }
}, { immediate: true })
</script>
<template>
  <section class="type-explorer" aria-label="Tous les types connus">
    <header><h3>Types disponibles</h3><p>Types natifs et définitions des catalogues, stratégies, flows et lecteurs du workspace.</p></header>
    <div class="filters"><input v-model="search" aria-label="Rechercher un type" placeholder="Rechercher un type…"/><select v-model="category" aria-label="Famille de types"><option v-for="item in contextSourceCategories" :key="item.id" :value="item.id">{{ item.label }}</option></select></div>
    <p v-if="loading" role="status">Chargement des types…</p><p v-if="error" role="alert">{{ error }}</p>
    <div class="type-columns">
      <div class="type-list"><button v-for="entry in filtered" :key="entry.id" :style="contextSourceStyle(contextSourceTypeId(entry.type))" :aria-pressed="selected === entry.id" @click="selected = entry.id"><strong>{{ entry.label }}</strong><small>{{ entry.origin }}</small></button><p v-if="!loading && !filtered.length">Aucun type ne correspond à cette recherche.</p></div>
      <article v-if="current" class="type-detail"><h3>{{ current.label }}</h3><p>{{ current.origin }}</p><code>{{ current.id }}</code><h4>Schéma</h4><pre>{{ JSON.stringify(current.type.kind === 'named' ? current.types[current.type.name] : current.type, null, 2) }}</pre><details><summary>Définitions et dépendances</summary><pre>{{ JSON.stringify(current.types, null, 2) }}</pre></details><h4>Exemples du type</h4><ContextSourceExamples :key="current.id" :workspace-id="workspaceId" :type="current.type" :types="current.types" :value="exampleValue" @select="exampleValue=$event"/><pre v-if="exampleValue!==undefined">{{JSON.stringify(exampleValue,null,2)}}</pre><h4>Sources disponibles</h4><ul><li v-for="provider in current.providers" :key="provider">{{ provider }}</li></ul><button @click="$emit('copy', current)">Copier dans un nouveau catalogue</button><p v-if="current.id.startsWith('builtin:')">Type natif consultable. Une copie possède sa propre définition.</p></article>
      <p v-else class="type-detail">Sélectionnez un type pour consulter son schéma et sa provenance.</p>
    </div>
  </section>
</template>
<style scoped>
.type-explorer{padding:20px;min-height:0}.type-explorer p,.type-explorer small{color:var(--muted-foreground,#a4a0b0);font-size:12px;line-height:1.6}.filters{display:flex;gap:10px;margin:16px 0}.filters input{flex:1;min-width:0}.type-columns{display:grid;grid-template-columns:minmax(220px,1fr) minmax(260px,2fr);gap:20px}.type-list{display:flex;flex-direction:column;gap:6px}.type-list button{text-align:left;display:flex;flex-direction:column;gap:5px;padding:12px;border-left:3px solid var(--ctx-source-color,#99acd5)}.type-list button[aria-pressed=true]{background:#292c32}.type-detail{min-width:0;padding:16px;border:1px solid #34363d;border-radius:6px;align-self:start}.type-detail pre{white-space:pre-wrap;overflow-wrap:anywhere;font-size:12px}.type-detail code{overflow-wrap:anywhere}@media(max-width:800px){.type-columns{grid-template-columns:1fr}}
</style>
