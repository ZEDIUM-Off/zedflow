<script setup lang="ts">
import { ref } from 'vue'
import { Plus, X } from 'lucide-vue-next'
import { contextType } from '../../contextEngine'
import type { ContextType } from '@zedflow/sdk'
const value = defineModel<ContextType>({ required: true })
withDefaults(defineProps<{ label?: string; names?: string[]; depth?: number }>(), { label: 'Type', depth: 0 })
const field = ref(''), error = ref('')
function add() {
  if (value.value.kind !== 'record') return
  const name = field.value.trim()
  if (!name || Object.hasOwn(value.value.fields, name)) { error.value = 'Choisissez un nom de champ unique.'; return }
  value.value.fields[name] = { kind: 'text' }; field.value = ''; error.value = ''
}
</script>
<template>
  <div class="ctx-type-editor">
    <label>{{label}}<select :aria-label="label" :value="value.kind" @change="value=contextType(($event.target as HTMLSelectElement).value as ContextType['kind'])"><option value="text">Texte</option><option value="number">Nombre</option><option value="boolean">Booléen</option><option value="list">Liste</option><option value="record">Objet à champs typés</option><option value="media">Référence média</option><option value="named">Type nommé</option></select></label>
    <label v-if="value.kind==='media'">Type média<input v-model="value.mediaType" placeholder="image/png"/></label>
    <label v-else-if="value.kind==='named'">Nom du type<input v-model="value.name" placeholder="Document"/><small v-if="names?.length">Disponibles : {{names.join(', ')}}</small></label>
    <ContextTypeEditor v-else-if="value.kind==='list'&&depth<20" v-model="value.item" label="Type des éléments" :names="names" :depth="depth+1"/>
    <template v-else-if="value.kind==='record'&&depth<20"><div v-for="(_,name) in value.fields" :key="name" class="ctx-field"><ContextTypeEditor v-model="value.fields[name]" :label="`Champ ${name}`" :names="names" :depth="depth+1"/><button type="button" class="icon-button" :aria-label="`Retirer le champ ${name}`" @click="delete value.fields[name]"><X :size="13"/></button></div><div class="ctx-add-field"><input v-model="field" aria-label="Nom du nouveau champ" placeholder="Nom du champ" @keydown.enter.prevent="add"/><button type="button" aria-label="Ajouter un champ typé" @click="add"><Plus :size="13"/></button></div><small v-if="error" role="alert">{{error}}</small></template>
  </div>
</template>
