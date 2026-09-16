<script setup lang="ts">
import { computed } from 'vue'
import { Plus, X } from 'lucide-vue-next'
import JsonField from './JsonField.vue'
import { comparisonOperators, defaultPredicate } from '../graph/predicate'
import type { Predicate } from '@zedflow/sdk'
const value = defineModel<Predicate>({required:true})
const valueType = computed(() => value.value.kind !== 'compare' || value.value.value === null ? 'null' : Array.isArray(value.value.value) ? 'array' : typeof value.value.value)
function kind(next: string) {
  if (next === 'compare') value.value = value.value.kind==='compare'?value.value:defaultPredicate()
  else value.value = {kind: next as 'all'|'any', items:value.value.kind==='compare'?[value.value]:value.value.items}
}
function operator(next: string) {
  if (value.value.kind !== 'compare') return
  value.value.operator = next as typeof value.value.operator
  if (next === 'exists') delete value.value.value
  else if (['gt','gte','lt','lte'].includes(next) && typeof value.value.value !== 'number') value.value.value = 0
  else if (next === 'in' && !Array.isArray(value.value.value)) value.value.value = []
  else if (value.value.value === undefined) value.value.value = ''
}
function type(next: string) { const defaults:Record<string,import('@zedflow/sdk').JsonValue>={string:'',number:0,boolean:true,null:null,array:[],object:{}};if(value.value.kind==='compare')value.value.value=defaults[next] }
</script>
<template>
  <div class="predicate-editor">
    <label>Combinaison<select :value="value.kind" @change="kind(($event.target as HTMLSelectElement).value)"><option value="compare">Un critère</option><option value="all">Tous les critères · ET</option><option value="any">Au moins un critère · OU</option></select></label>
    <template v-if="value.kind==='compare'">
      <label>Champ d’état<input v-model="value.field" placeholder="input ou /result/status"/></label>
      <label>Comparaison<select :value="value.operator" @change="operator(($event.target as HTMLSelectElement).value)"><option v-for="operator in comparisonOperators" :key="operator.value" :value="operator.value">{{operator.label}}</option></select></label>
      <template v-if="value.operator!=='exists'"><label>Type de valeur<select :value="valueType" @change="type(($event.target as HTMLSelectElement).value)"><option value="string">Texte</option><option value="number">Nombre</option><option value="boolean">Booléen</option><option value="null">Null</option><option value="array">Liste JSON</option><option value="object">Objet JSON</option></select></label>
        <label v-if="valueType==='string'">Valeur<input v-model="value.value"/></label>
        <label v-else-if="valueType==='number'">Valeur numérique<input type="number" :value="value.value" @change="value.value=Number(($event.target as HTMLInputElement).value)"/></label>
        <label v-else-if="valueType==='boolean'">Valeur booléenne<select :value="String(value.value)" @change="value.value=($event.target as HTMLSelectElement).value==='true'"><option value="true">Vrai</option><option value="false">Faux</option></select></label>
        <JsonField v-else-if="['array','object'].includes(valueType)" v-model="value.value" label="Valeur JSON" :rows="3"/>
      </template>
    </template>
    <template v-else><div v-for="(item,index) in value.items" :key="index" class="predicate-child"><button class="icon-button predicate-remove" :disabled="value.items.length===1" aria-label="Retirer ce critère" @click="value.items.splice(index,1)"><X :size="12"/></button><PredicateEditor v-model="value.items[index]"/></div><button type="button" class="predicate-add" @click="value.items.push(defaultPredicate())"><Plus :size="12"/>Ajouter un critère</button></template>
  </div>
</template>
