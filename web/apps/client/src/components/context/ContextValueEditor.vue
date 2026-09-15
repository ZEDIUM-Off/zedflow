<script setup lang="ts">
import type { JsonValue } from '@zedflow/sdk'

import { computed } from 'vue'
import { Plus, X } from 'lucide-vue-next'
import { defaultContextValue } from '../../contextEngine'
import type { ContextType } from '@zedflow/sdk'
const value = defineModel<JsonValue>({ required: true })
const props = withDefaults(defineProps<{ type: ContextType; label?: string; types?: Record<string, ContextType>; depth?: number }>(), { label: 'Valeur', depth: 0 })
const resolved = computed(() => { let result=props.type; const seen=new Set<string>(); while(result.kind==='named'&&props.types?.[result.name]&&!seen.has(result.name)){seen.add(result.name);result=props.types[result.name]} return result })
const fields = computed(() => resolved.value.kind === 'record' ? resolved.value.fields : {})
function field(name: string, item: JsonValue) { value.value = { ...(typeof value.value === 'object' && !Array.isArray(value.value) && value.value || {}), [name]: item } }
function member(name: string, type: ContextType) { return value.value && typeof value.value === 'object' && !Array.isArray(value.value) ? value.value[name] ?? defaultContextValue(type, props.types) : defaultContextValue(type, props.types) }
function listValue(index: number, item: JsonValue) { const items = Array.isArray(value.value) ? [...value.value] : []; items[index] = item; value.value = items }
function number(event: Event) { const input=event.target as HTMLInputElement;if(input.value!==''&&Number.isFinite(input.valueAsNumber))value.value=input.valueAsNumber }
</script>
<template>
  <label v-if="resolved.kind==='text'">{{label}}<textarea :aria-label="label" :value="typeof value==='string'?value:''" rows="3" @input="value=($event.target as HTMLTextAreaElement).value"/></label>
  <label v-else-if="resolved.kind==='number'">{{label}}<input :aria-label="label" type="number" :value="value" step="any" @input="number"/></label>
  <label v-else-if="resolved.kind==='boolean'">{{label}}<select :aria-label="label" :value="String(value)" @change="value=($event.target as HTMLSelectElement).value==='true'"><option value="false">Faux</option><option value="true">Vrai</option></select></label>
  <fieldset v-else-if="resolved.kind==='media'" class="ctx-value-fields"><legend>{{label}}</legend><label>Référence du contenu<input :value="member('contentRef',{kind:'text'})" @input="field('contentRef',($event.target as HTMLInputElement).value)" placeholder="sha256:…"/></label><label>Type média<input :value="member('mediaType',{kind:'text'})" @input="field('mediaType',($event.target as HTMLInputElement).value)" :placeholder="resolved.mediaType"/></label></fieldset>
  <fieldset v-else-if="resolved.kind==='record'&&depth<20" class="ctx-value-fields"><legend>{{label}}</legend><ContextValueEditor v-for="(type,name) in fields" :key="name" :model-value="member(name,type)" @update:model-value="field(name,$event)" :label="name" :type="type" :types="types" :depth="depth+1"/><small v-if="!Object.keys(fields).length">Cet objet n’a aucun champ déclaré.</small></fieldset>
  <fieldset v-else-if="resolved.kind==='list'&&depth<20" class="ctx-value-fields"><legend>{{label}}</legend><div v-for="(item,index) in Array.isArray(value)?value:[]" :key="index" class="ctx-field"><ContextValueEditor :model-value="item" @update:model-value="listValue(index,$event)" :type="resolved.item" :types="types" :label="`Élément ${index+1}`" :depth="depth+1"/><button type="button" class="icon-button" :aria-label="`Retirer l’élément ${index+1}`" @click="Array.isArray(value)&&value.splice(index,1)"><X :size="13"/></button></div><button type="button" @click="value=[...(Array.isArray(value)?value:[]),defaultContextValue(resolved.item,types)]"><Plus :size="13"/> Ajouter un élément</button></fieldset>
  <p v-else class="ctx-hint">{{label}} : renseignez la définition du type nommé avant de fournir sa valeur.</p>
</template>
