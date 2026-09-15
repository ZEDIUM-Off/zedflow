<script setup lang="ts">
import { computed, ref } from 'vue'
import { Plus, X, SlidersHorizontal } from 'lucide-vue-next'
import { DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem } from 'reka-ui'
import ContextTypeEditor from './ContextTypeEditor.vue'
import ContextValueEditor from './ContextValueEditor.vue'
import ContextPredicateEditor from './ContextPredicateEditor.vue'
import ContextExpressionSocket from './ContextExpressionSocket.vue'
import { defaultContextValue, defaultExpression, textExpression } from '../../contextEngine'
import type { ContextExpr, ContextType } from '@zedflow/sdk'
import { CONTEXT_SOURCE_MAX_DEPTH, resolveSourceType } from '../../contextSources'
import { contextExpressionType } from './contextComposer'
import { textMessage, textMessageValue, toolExchange, toolExchangeValue } from './contextMessageProjection'
const value = defineModel<ContextExpr>({ required: true })
const props = withDefaults(defineProps<{ resources: Record<string, ContextType>; types?: Record<string, ContextType>; variables?: string[]; variableTypes?: Record<string, ContextType>; label?: string; depth?: number; compact?: boolean; messageProjection?: boolean; expectedType?: ContextType; expectedKinds?: ContextType['kind'][] }>(), { label: 'Expression', depth: 0, variables: () => [], variableTypes: () => ({}), compact: false, messageProjection: false })
const name = ref(''), error = ref('')
const expanded = ref(false)
const projectedMessage = computed(() => props.messageProjection ? textMessageValue(value.value) : undefined)
const projectedExchange = computed(() => {
  const exchange = props.messageProjection && toolExchangeValue(value.value)
  if (!exchange) return
  const callType = contextExpressionType(exchange.call, props.resources, props.types, props.variableTypes)
  const resultType = contextExpressionType(exchange.result, props.resources, props.types, props.variableTypes)
  return callType && resultType ? { ...exchange, callType, resultType } : undefined
})
const socketValue = computed({
  get: () => projectedMessage.value?.value || value.value,
  set: next => { value.value = projectedMessage.value ? textMessage(next, projectedMessage.value.role) : next },
})
function replaceExchange(side: 'call' | 'result', next: ContextExpr) {
  const exchange = projectedExchange.value
  if (exchange) value.value = toolExchange(side === 'call' ? next : exchange.call, side === 'result' ? next : exchange.result)
}
const kinds: { label: string; items: { kind: ContextExpr['kind']; label: string }[] }[] = [
  { label: 'Sources', items: [{ kind: 'resource', label: 'Ressource' }, { kind: 'variable', label: 'Variable locale' }, { kind: 'literal', label: 'Valeur littérale' }] },
  { label: 'Projection', items: [{ kind: 'field', label: 'Lire un champ' }, { kind: 'project', label: 'Sélectionner des champs' }, { kind: 'record', label: 'Construire un objet' }, { kind: 'construct', label: 'Construire un type nommé' }, { kind: 'template', label: 'Composer un texte' }, { kind: 'truncate', label: 'Extraire le début du texte' }, { kind: 'toJson', label: 'Convertir en texte JSON' }] },
  { label: 'Collections', items: [{ kind: 'list', label: 'Assembler une liste' }, { kind: 'filter', label: 'Filtrer' }, { kind: 'sort', label: 'Trier' }, { kind: 'take', label: 'Prendre les premiers' }, { kind: 'map', label: 'Projeter chaque élément' }, { kind: 'groupBy', label: 'Regrouper par clé' }, { kind: 'dedup', label: 'Dédupliquer' }] },
  { label: 'Mesure et réutilisation', items: [{ kind: 'measure', label: 'Mesurer' }, { kind: 'call', label: 'Appeler un programme' }] },
]
const explanations: Record<ContextExpr['kind'], string> = {
  resource: 'Utiliser une donnée déclarée dans Sources. Sa valeur réelle sera fournie par le flow.',
  variable: 'Utiliser l’élément courant d’une collection, par exemple pendant un filtre ou une projection.',
  literal: 'Écrire une valeur fixe, enregistrée dans la stratégie et réutilisée à chaque passage.',
  list: 'Assembler des éléments du même type, dans leur ordre d’apparition. Chaque élément peut provenir d’un champ ou d’une expression.',
  field: 'Lire un seul champ de la donnée source, par exemple le titre d’un document.',
  project: 'Conserver uniquement les champs indiqués. Les autres champs ne seront pas envoyés.',
  record: 'Assembler un objet avec un nom et une expression pour chaque champ.',
  construct: 'Construire une donnée conforme à un type défini dans le catalogue.',
  template: 'Composer un texte avec des emplacements nommés, par exemple « Document : {{title}} ».',
  truncate: 'Conserver les premiers caractères du texte, sans couper un caractère Unicode.',
  toJson: 'Représenter un objet ou une liste sous forme de texte JSON.',
  filter: 'Conserver les éléments de la collection qui vérifient la condition.',
  sort: 'Ordonner une collection selon la valeur choisie pour chaque élément.',
  take: 'Conserver les premiers éléments d’une collection. Triez-la d’abord si l’ordre compte.',
  map: 'Transformer chaque élément de la collection avec la même expression.',
  groupBy: 'Rassembler les éléments qui possèdent la même clé.',
  dedup: 'Conserver un seul élément pour chaque valeur de clé.',
  measure: 'Mesurer la taille de cette donnée, par exemple pour la comparer à une limite.',
  call: 'Réutiliser une fonction déclarée dans la bibliothèque sélectionnée pour ce contexte.',
}
const localVariables = computed(() => { const expression = value.value; return 'item' in expression && typeof expression.item === 'string' ? [...props.variables.filter(name => name !== expression.item), expression.item] : props.variables })
const localVariableTypes = computed(() => {
  const expression = value.value
  if (!('item' in expression) || !('value' in expression)) return props.variableTypes
  const source = contextExpressionType(expression.value, props.resources, props.types, props.variableTypes)
  const type = source && resolveSourceType(source, props.types || {})
  return type?.kind === 'list' ? { ...props.variableTypes, [expression.item]: type.item } : props.variableTypes
})
const entries = computed(() => value.value.kind === 'record' ? value.value.fields : value.value.kind === 'template' ? value.value.values : value.value.kind === 'call' ? value.value.arguments : undefined)
function changeKind(kind: ContextExpr['kind']) { if (kind !== value.value.kind) value.value = defaultExpression(kind, Object.keys(props.resources)) }
function wrap(kind: ContextExpr['kind']) { value.value = defaultExpression(kind, Object.keys(props.resources), value.value) }
function addEntry() { const key = name.value.trim(); if (!entries.value || !key || Object.hasOwn(entries.value,key)) { error.value='Choisissez un nom unique.';return } entries.value[key]=textExpression();name.value='';error.value='' }
function literalType(type: ContextType) { if(value.value.kind==='literal'){value.value.dataType=type;value.value.value=defaultContextValue(type,props.types)} }
</script>
<template>
  <div class="ctx-expression-shell" :class="{ 'is-compact':compact }">
  <div v-if="compact && projectedExchange" class="ctx-tool-sockets">
    <ContextExpressionSocket :model-value="projectedExchange.call" @update:model-value="replaceExchange('call',$event)" :resources="resources" :types="types" :variables="variables" :variable-types="variableTypes" :label="`Appel enregistré · ${label}`" :expected-type="projectedExchange.callType" :editable="false" @edit="expanded=!expanded"/>
    <ContextExpressionSocket :model-value="projectedExchange.result" @update:model-value="replaceExchange('result',$event)" :resources="resources" :types="types" :variables="variables" :variable-types="variableTypes" :label="`Résultat enregistré · ${label}`" :expected-type="projectedExchange.resultType" :editable="false" @edit="expanded=!expanded"/>
    <button type="button" class="ctx-exchange-edit" :aria-label="`Configurer l’expression : ${label}`" title="Configurer l’expression" @click="expanded=!expanded"><SlidersHorizontal :size="12"/></button>
  </div>
  <ContextExpressionSocket v-else-if="compact" v-model="socketValue" :resources="resources" :types="types" :variables="variables" :variable-types="variableTypes" :label="label" :expected-type="projectedMessage?undefined:expectedType" :expected-kinds="projectedMessage?['text']:expectedKinds" @edit="expanded=!expanded"/>
  <div v-if="!compact || expanded" class="ctx-expression" :data-expression="value.kind">
    <button v-if="compact" type="button" class="ctx-expression-close" @click="expanded=false">Réduire les détails</button>
    <div class="ctx-expression-heading"><label>{{label}}<select :aria-label="label" :value="value.kind" @change="changeKind(($event.target as HTMLSelectElement).value as ContextExpr['kind'])"><optgroup v-for="group in kinds" :key="group.label" :label="group.label"><option v-for="kind in group.items" :key="kind.kind" :value="kind.kind">{{kind.label}}</option></optgroup></select></label><DropdownMenuRoot><DropdownMenuTrigger class="ctx-transform" aria-label="Transformer cette expression" title="Ajouter une transformation en conservant la source">Transformer ↳</DropdownMenuTrigger><DropdownMenuPortal><DropdownMenuContent class="compact-menu" align="end" :side-offset="4"><DropdownMenuItem @select="wrap('filter')">Filtrer ce résultat</DropdownMenuItem><DropdownMenuItem @select="wrap('field')">Lire un champ du résultat</DropdownMenuItem><DropdownMenuItem @select="wrap('take')">Limiter cette liste</DropdownMenuItem><DropdownMenuItem @select="wrap('truncate')">Extraire le début du texte</DropdownMenuItem><DropdownMenuItem @select="wrap('toJson')">Convertir ce résultat en texte</DropdownMenuItem><DropdownMenuItem @select="wrap('measure')">Mesurer ce résultat</DropdownMenuItem></DropdownMenuContent></DropdownMenuPortal></DropdownMenuRoot></div>
    <small v-if="depth===0" class="ctx-expression-help">{{explanations[value.kind]}}</small>
    <template v-if="value.kind==='resource'"><label>Ressource<select aria-label="Ressource" v-model="value.name"><option value="" disabled>Choisir une ressource</option><option v-for="(_,name) in resources" :key="name" :value="name">{{name}}</option><option v-if="value.name&&!Object.hasOwn(resources,value.name)" :value="value.name">{{value.name}} · non déclarée</option></select></label><small v-if="!Object.keys(resources).length">Déclarez une ressource dans le panneau Sources.</small></template>
    <label v-else-if="value.kind==='variable'">Variable locale<select aria-label="Variable locale" v-model="value.name"><option v-for="name in variables" :key="name" :value="name">{{name}}</option><option v-if="!variables.includes(value.name)" :value="value.name">{{value.name}} · hors portée</option></select></label>
    <template v-else-if="value.kind==='literal'"><ContextTypeEditor :model-value="value.dataType" @update:model-value="literalType" label="Type littéral" :names="Object.keys(types||{})"/><ContextValueEditor v-model="value.value" :type="value.dataType" :types="types" label="Valeur littérale"/></template>
    <template v-else-if="depth<CONTEXT_SOURCE_MAX_DEPTH">
      <ContextExpressionEditor v-if="'value' in value" v-model="value.value" :resources="resources" :types="types" :variables="variables" :variable-types="variableTypes" :depth="depth+1" label="Source" compact :expected-kinds="['filter','sort','take','map','groupBy','dedup'].includes(value.kind)?['list']:value.kind==='truncate'?['text']:undefined"/>
      <label v-if="value.kind==='construct'">Type construit<select aria-label="Type construit" v-model="value.name"><option value="" disabled>Choisir un type du catalogue</option><option v-for="name in Object.keys(types||{})" :key="name">{{name}}</option><option v-if="value.name&&!Object.hasOwn(types||{},value.name)" :value="value.name">{{value.name}} · non déclaré</option></select><small>Le contenu est validé contre la structure exacte du type, sans conversion implicite.</small></label><label v-if="value.kind==='field'">Champ<input v-model="value.field" placeholder="title"/></label>
      <label v-if="value.kind==='project'">Champs sélectionnés<input aria-label="Champs sélectionnés" :value="value.fields.join(', ')" @change="value.fields=($event.target as HTMLInputElement).value.split(',').map(field=>field.trim()).filter(Boolean)" placeholder="title, status"/><small>Noms exacts des champs, séparés par des virgules.</small></label>
      <label v-if="'item' in value">Nom de la variable locale<input v-model="value.item" placeholder="item"/></label>
      <ContextPredicateEditor v-if="value.kind==='filter'" v-model="value.condition" :resources="resources" :types="types" :variables="localVariables" :variable-types="localVariableTypes" :depth="depth+1"/>
      <ContextExpressionEditor v-if="'key' in value" v-model="value.key" :resources="resources" :types="types" :variables="localVariables" :variable-types="localVariableTypes" :depth="depth+1" label="Clé de l’élément" compact/>
      <ContextExpressionEditor v-if="value.kind==='map'" v-model="value.body" :resources="resources" :types="types" :variables="localVariables" :variable-types="localVariableTypes" :depth="depth+1" label="Résultat de chaque élément" compact/>
      <label v-if="value.kind==='sort'" class="ctx-checkbox"><input v-model="value.descending" type="checkbox"/> Ordre décroissant</label>
      <label v-if="value.kind==='take'">Nombre maximum<input v-model.number="value.count" type="number" min="0" step="1"/></label>
      <label v-if="value.kind==='truncate'">Nombre de caractères de l’extrait<input v-model.number="value.count" type="number" min="0" step="1"/></label>
      <label v-if="value.kind==='measure'">Unité<select aria-label="Unité" v-model="value.unit"><option value="bytes">Octets UTF-8</option><option value="items">Nombre d’éléments</option><option value="media">Nombre de médias</option></select></label>
      <template v-if="value.kind==='call'"><label>Catalogue<select aria-label="Catalogue" v-model="value.catalog"><option value="projection">Projections</option><option value="subprogram">Sous-programmes</option></select></label><label>Programme<input v-model="value.name" placeholder="documentSummary"/></label><small>Le programme doit être fourni par le catalogue explicite de la composition.</small></template>
      <label v-if="value.kind==='template'">Gabarit de texte<textarea v-model="value.template" rows="3" placeholder="Contexte : {{value}}"/><small>Chaque emplacement doit recevoir du texte. Utilisez la conversion JSON pour un objet.</small></label>
      <template v-if="value.kind==='list'"><ContextTypeEditor v-model="value.itemType" label="Type des éléments construits" :names="Object.keys(types||{})"/><div v-for="(_,index) in value.items" :key="index" class="ctx-field"><ContextExpressionEditor v-model="value.items[index]" :resources="resources" :types="types" :variables="variables" :variable-types="variableTypes" :depth="depth+1" :label="`Élément ${index+1}`" compact :expected-type="value.itemType"/><button type="button" class="icon-button" :aria-label="`Retirer l’élément ${index+1}`" @click="value.items.splice(index,1)"><X :size="13"/></button></div><button type="button" @click="value.items.push({kind:'literal',dataType:value.itemType,value:defaultContextValue(value.itemType,types)})"><Plus :size="13"/>Ajouter un élément</button></template>
      <template v-if="entries"><div v-for="(_,key) in entries" :key="key" class="ctx-field"><ContextExpressionEditor v-model="entries[key]" :resources="resources" :types="types" :variables="variables" :variable-types="variableTypes" :depth="depth+1" :label="key" compact :expected-kinds="value.kind==='template'?['text']:undefined"/><button type="button" class="icon-button" :aria-label="`Retirer ${key}`" @click="delete entries[key]"><X :size="13"/></button></div><div class="ctx-add-field"><input v-model="name" :aria-label="value.kind==='call'?'Nom du paramètre':'Nom du champ de projection'" placeholder="Nom" @keydown.enter.prevent="addEntry"/><button type="button" aria-label="Ajouter une expression nommée" @click="addEntry"><Plus :size="13"/></button></div><small v-if="error" role="alert">{{error}}</small></template>
    </template>
    <small v-else>Profondeur maximale de l’éditeur atteinte.</small>
  </div>
  </div>
</template>
<style scoped>
.ctx-tool-sockets{display:flex;align-items:flex-start;gap:5px;min-width:0}.ctx-tool-sockets>.ctx-socket-wrap{flex:1;min-width:0}.ctx-exchange-edit{flex:none;display:grid;place-items:center;width:20px;height:32px;padding:0;background:transparent;border:0;color:#a3a3aa}.ctx-exchange-edit:hover{color:#ededee}.ctx-tool-sockets :deep(.ctx-source-token){padding:4px 5px;font-size:10px}@container context-program (max-width:300px){.ctx-tool-sockets{flex-wrap:wrap}.ctx-tool-sockets>.ctx-socket-wrap{flex-basis:calc(100% - 25px)}}

.ctx-expression-shell{min-width:0;flex:1}.ctx-expression-shell.is-compact>.ctx-expression{margin-top:8px;background:#202023;border:1px solid #3a3a40}.ctx-expression-close{align-self:flex-end;border:0;padding:2px;background:transparent;color:#b1b1b8;text-decoration:underline;font-size:11px}.ctx-expression{background:#202023;border-color:#3a3a40;gap:10px}.ctx-expression-help{font-size:11px}.ctx-expression .ctx-field{align-items:center}.ctx-expression .ctx-field>.icon-button{margin-top:0}
</style>
