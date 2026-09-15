<script setup lang="ts">
import { computed } from 'vue'
import { Plus, X } from 'lucide-vue-next'
import ContextExpressionEditor from './ContextExpressionEditor.vue'
import { defaultContextValue, defaultPredicate, textExpression } from '../../contextEngine'
import type { ContextExpr, ContextPredicate, ContextType } from '@zedflow/sdk'
import { contextExpressionType } from './contextComposer'
import { CONTEXT_SOURCE_MAX_DEPTH, resolveSourceType } from '../../contextSources'
const value = defineModel<ContextPredicate>({ required: true })
const props = withDefaults(defineProps<{ resources: Record<string, ContextType>; types?: Record<string, ContextType>; variables?: string[]; variableTypes?: Record<string, ContextType>; depth?: number }>(), { depth: 0, variables: () => [], variableTypes: () => ({}) })
const operator = computed(() => value.value.kind === 'compare' ? value.value.operator : value.value.kind)
const left = computed<ContextExpr | undefined>({
  get: () => 'left' in value.value ? value.value.left : 'value' in value.value ? value.value.value : undefined,
  set: expression => {
    if (!expression) return
    if ('left' in value.value) value.value.left = expression
    else if ('value' in value.value) value.value.value = expression
    const type = contextExpressionType(expression, props.resources, props.types, props.variableTypes)
    const resolved = type && resolveSourceType(type, props.types || {})
    if (type && resolved && ['boolean', 'number', 'text'].includes(resolved.kind) && right.value?.kind === 'literal' && right.value.value === '') right.value = { kind: 'literal', dataType: type, value: defaultContextValue(type, props.types) }
  },
})
const right = computed<ContextExpr | undefined>({
  get: () => 'right' in value.value ? value.value.right : value.value.kind === 'contains' ? value.value.item : undefined,
  set: expression => { if (!expression) return; if ('right' in value.value) value.value.right = expression; else if (value.value.kind === 'contains') value.value.item = expression },
})
const rightType = computed(() => {
  if (!left.value) return undefined
  const type = contextExpressionType(left.value, props.resources, props.types, props.variableTypes)
  const resolved = type && resolveSourceType(type, props.types || {})
  return value.value.kind === 'contains' && resolved?.kind === 'list' ? resolved.item : type
})
function changeOperator(event: Event) {
  const kind = (event.target as HTMLSelectElement).value
  const source = left.value || { kind: 'resource', name: Object.keys(props.resources)[0] || '' } as ContextExpr
  const other = right.value || textExpression()
  if (['lt', 'lte', 'gt', 'gte', 'ne'].includes(kind)) value.value = { kind: 'compare', left: source, operator: kind as 'lt' | 'lte' | 'gt' | 'gte' | 'ne', right: other }
  else if (kind === 'eq') value.value = { kind, left: source, right: other }
  else if (kind === 'present') value.value = { kind, value: source }
  else if (kind === 'contains') value.value = { kind, value: source, item: other }
  else if (kind === 'not') value.value = { kind, item: value.value }
  else if (kind === 'and' || kind === 'or') value.value = { kind, items: [value.value] }
}
</script>
<template>
  <div class="ctx-predicate" :class="{ 'is-compound':!left }">
    <div class="ctx-condition-fields">
      <div v-if="left" class="ctx-condition-field ctx-condition-source"><span>Champ</span><ContextExpressionEditor v-model="left" :resources="resources" :types="types" :variables="variables" :variable-types="variableTypes" :depth="depth+1" label="Champ de la condition" compact/></div>
      <label class="ctx-condition-operator"><span>{{ left ? 'Opérateur' : 'Condition' }}</span><select aria-label="Opérateur de la condition" :value="operator" @change="changeOperator"><option value="present">est présent</option><option value="eq">est égal à</option><option value="ne">est différent de</option><option value="lt">est inférieur à</option><option value="lte">est inférieur ou égal à</option><option value="gt">est supérieur à</option><option value="gte">est supérieur ou égal à</option><option value="contains">contient</option><optgroup label="Combiner des conditions"><option value="and">Toutes · ET</option><option value="or">Au moins une · OU</option><option value="not">Négation · NON</option></optgroup></select></label>
      <div v-if="right" class="ctx-condition-field ctx-condition-value"><span>Valeur</span><ContextExpressionEditor v-model="right" :resources="resources" :types="types" :variables="variables" :variable-types="variableTypes" :depth="depth+1" label="Valeur de la condition" compact :expected-type="rightType"/></div>
    </div>
    <template v-if="depth<CONTEXT_SOURCE_MAX_DEPTH">
      <ContextPredicateEditor v-if="value.kind==='not'" v-model="value.item" :resources="resources" :types="types" :variables="variables" :variable-types="variableTypes" :depth="depth+1"/>
      <template v-if="value.kind==='and'||value.kind==='or'"><div v-for="(_,index) in value.items" :key="index" class="ctx-condition-combined"><span class="ctx-condition-join">{{ index ? value.kind==='and'?'ET':'OU' : 'Si' }}</span><ContextPredicateEditor v-model="value.items[index]" :resources="resources" :types="types" :variables="variables" :variable-types="variableTypes" :depth="depth+1"/><button type="button" class="icon-button" :disabled="value.items.length<=1" aria-label="Retirer cette condition" @click="value.items.splice(index,1)"><X :size="13"/></button></div><button type="button" class="ctx-condition-add" @click="value.items.push(defaultPredicate('present',Object.keys(resources)))"><Plus :size="13"/>Ajouter une condition</button></template>
    </template>
    <small v-else>Profondeur maximale de l’éditeur atteinte.</small>
  </div>
</template>
<style scoped>
.ctx-predicate{border:0;background:transparent;padding:0;gap:10px;min-width:0}.ctx-condition-fields{display:grid;grid-template-columns:minmax(0,1.3fr) minmax(100px,.8fr) minmax(0,1fr);gap:7px;align-items:start}.ctx-condition-fields:has(>.ctx-condition-source):not(:has(>.ctx-condition-value)){grid-template-columns:minmax(0,1fr) minmax(120px,.7fr)}.ctx-condition-fields:not(:has(>.ctx-condition-source)){display:flex}.ctx-condition-fields>label,.ctx-condition-field{min-width:0;display:flex;flex-direction:column;gap:5px}.ctx-condition-fields>label>span,.ctx-condition-field>span{font-size:10px;color:#96969f}.ctx-condition-operator select{height:34px;padding:6px;font-size:11px;background:#1c1c1f;border-color:#38383d}.ctx-condition-combined{display:flex;align-items:flex-start;gap:7px}.ctx-condition-combined>.ctx-predicate{flex:1;min-width:0}.ctx-condition-join{color:#aaaab2;font-size:10px;padding-top:23px;flex:none;min-width:17px}.ctx-condition-combined>.icon-button{margin-top:18px}.ctx-condition-add{display:flex;align-items:center;align-self:flex-start;gap:5px;font-size:11px;background:transparent;border:0;padding:3px;color:#b5b5bc}.is-compound>.ctx-predicate{border-left:1px solid #424248;padding-left:10px}.ctx-condition-combined .ctx-condition-fields{grid-template-columns:minmax(0,1fr)}@container context-program (max-width:420px){.ctx-condition-fields{grid-template-columns:minmax(0,1fr) minmax(100px,1fr)}.ctx-condition-source{grid-column:1/-1}}@media(max-width:600px){.ctx-condition-fields{grid-template-columns:minmax(0,1fr) minmax(100px,1fr)}.ctx-condition-source{grid-column:1/-1}}
</style>
