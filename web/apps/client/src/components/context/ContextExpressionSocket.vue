<script setup lang="ts">
import { computed, inject, onMounted, onUnmounted, ref, useId } from 'vue'
import { ChevronDown, SlidersHorizontal } from 'lucide-vue-next'
import { DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem, DropdownMenuLabel, DropdownMenuSeparator } from 'reka-ui'
import { cloneContext, typeLabel } from '../../contextEngine'
import type { ContextExpr, ContextType } from '@zedflow/sdk'
import { CONTEXT_SOURCE_MIME, contextSourceFields, contextSourceStyle, contextTypesCompatible, readContextSourceDrag, resolveSourceType, type ContextSourceField } from '../../contextSources'
import ContextExpressionSummary from './ContextExpressionSummary.vue'
import { CONTEXT_SOCKET_SELECTION, contextExpressionType, type ContextSocketTarget } from './contextComposer'

const value = defineModel<ContextExpr>({ required: true })
const props = withDefaults(defineProps<{
  resources: Record<string, ContextType>; types?: Record<string, ContextType>; variables?: string[]; variableTypes?: Record<string, ContextType>
  label?: string; expectedType?: ContextType; expectedKinds?: ContextType['kind'][]; editable?: boolean
}>(), { types: () => ({}), variables: () => [], variableTypes: () => ({}), label: 'Contenu', editable: true })
const emit = defineEmits<{ edit: [] }>()
const id = `context-socket-${useId()}`
const selection = inject(CONTEXT_SOCKET_SELECTION, undefined)
const error = ref(''), dragging = ref(false)
const fields = computed(() => Object.entries(props.resources).flatMap(([source, type]) => contextSourceFields(source, type, props.types)))
const knownVariables = computed(() => [...new Set([...props.variables, ...Object.keys(props.variableTypes)])])
const localFields = computed(() => Object.entries(props.variableTypes).flatMap(([name, type]) => contextSourceFields(name, type, props.types)))
const scalarLiteral = computed(() => value.value.kind === 'literal' && ['text', 'number', 'boolean'].includes(value.value.dataType.kind))
const typeError = computed(() => {
  const type = contextExpressionType(value.value, props.resources, props.types, props.variableTypes)
  return type && !acceptsType(type) ? `Type actuel : ${typeLabel(type)}. ${props.expectedType ? `Type attendu : ${typeLabel(props.expectedType)}.` : 'Choisissez un champ compatible avec cet emplacement.'}` : ''
})
function acceptsType(type: ContextType) {
  if (props.expectedType && !contextTypesCompatible(type, props.expectedType, props.types)) return false
  return !props.expectedKinds || props.expectedKinds.includes(resolveSourceType(type, props.types).kind)
}
function resolveField(field: ContextSourceField) {
  return fields.value.find(candidate => candidate.source === field.source && candidate.path.join('\u0000') === field.path.join('\u0000'))
}
function accepts(field: ContextSourceField) { const resolved = resolveField(field); return !!resolved && acceptsType(resolved.type) }
function insert(field: ContextSourceField) {
  const resolved = resolveField(field)
  if (!resolved || !acceptsType(resolved.type)) {
    error.value = `Ce champ ne convient pas à ${props.label.toLocaleLowerCase()}.${props.expectedType ? ` Type attendu : ${typeLabel(props.expectedType)}.` : ''}`
    return false
  }
  value.value = cloneContext(resolved.expression)
  error.value = ''
  return true
}
const target: ContextSocketTarget = { id, get label() { return props.label }, accepts, insert }
function select() { selection?.select(target) }
onMounted(() => selection?.register(target))
onUnmounted(() => selection?.clear(id))
function drop(event: DragEvent) {
  dragging.value = false
  if (!event.dataTransfer?.types.includes(CONTEXT_SOURCE_MIME)) return
  event.preventDefault(); event.stopPropagation()
  const field = readContextSourceDrag(event)
  if (field) { select(); insert(field) }
}
function dragover(event: DragEvent) {
  if (!event.dataTransfer?.types.includes(CONTEXT_SOURCE_MIME)) return
  event.preventDefault(); event.stopPropagation(); dragging.value = true
  event.dataTransfer.dropEffect = 'copy'
}
function localExpression(expression: ContextExpr): ContextExpr {
  if (expression.kind === 'resource') return { kind: 'variable', name: expression.name }
  if (expression.kind === 'field') return { ...expression, value: localExpression(expression.value) }
  return cloneContext(expression)
}
function insertLocal(field: ContextSourceField) { if (!acceptsType(field.type)) return; value.value = localExpression(field.expression); error.value = '' }
function setLiteral(event: Event) {
  if (value.value.kind !== 'literal') return
  const input = event.target as HTMLInputElement
  if (value.value.dataType.kind === 'number' && !Number.isFinite(input.valueAsNumber)) { error.value = 'Saisissez un nombre valide.'; return }
  value.value.value = value.value.dataType.kind === 'number' ? input.valueAsNumber : input.value
  error.value = ''
}
</script>
<template>
  <div class="ctx-socket-wrap">
    <div :id="id" class="ctx-expression-socket" :class="{ 'is-selected': selection?.selectedId.value===id, 'is-dragging': dragging, 'has-error': error || typeError }" :data-socket-label="label" @focusin="select" @dragover="dragover" @dragleave="dragging=false" @drop="drop">
      <template v-if="scalarLiteral && value.kind==='literal'">
        <select v-if="value.dataType.kind==='boolean'" class="ctx-socket-literal" :aria-label="label" :value="String(value.value)" @change="value.value=($event.target as HTMLSelectElement).value==='true'"><option value="true">vrai</option><option value="false">faux</option></select>
        <input v-else class="ctx-socket-literal" :type="value.dataType.kind==='number'?'number':'text'" :aria-label="label" :value="value.value" placeholder="Saisir une valeur ou choisir un champ" @input="setLiteral"/>
      </template>
      <DropdownMenuRoot>
        <DropdownMenuTrigger :class="['ctx-socket-choice', { 'is-icon':scalarLiteral }]" :aria-label="`Choisir un champ pour ${label}`" @click="select">
          <ContextExpressionSummary v-if="!scalarLiteral" :value="value" :resources="resources" :variable-types="variableTypes"/>
          <ChevronDown :size="12"/>
        </DropdownMenuTrigger>
        <DropdownMenuPortal><DropdownMenuContent class="compact-menu ctx-socket-menu" align="start" :side-offset="5">
          <DropdownMenuLabel class="ctx-socket-menu-label">{{ expectedType ? `Champs compatibles · ${typeLabel(expectedType)}` : 'Choisir une source ou un champ' }}</DropdownMenuLabel>
          <DropdownMenuItem v-for="field in fields" :key="`${field.source}.${field.path.join('.')}`" :disabled="!acceptsType(field.type)" @select="insert(field)"><span class="ctx-field-dot" :style="contextSourceStyle(field.typeId)"/><span>{{ field.label }}</span><small>{{ typeLabel(field.type) }}</small></DropdownMenuItem>
          <DropdownMenuLabel v-if="!fields.length" class="ctx-socket-menu-label">Déclarez une source pour choisir ses champs.</DropdownMenuLabel>
          <template v-if="knownVariables.length"><DropdownMenuSeparator/><DropdownMenuLabel class="ctx-socket-menu-label">Éléments dans cette portée</DropdownMenuLabel><DropdownMenuItem v-for="field in localFields" :key="`local:${field.source}.${field.path.join('.')}`" :disabled="!acceptsType(field.type)" @select="insertLocal(field)"><span>{{ field.label }}</span><small>{{ typeLabel(field.type) }}</small></DropdownMenuItem><DropdownMenuItem v-for="name in knownVariables.filter(name=>!variableTypes[name])" :key="name" @select="value={kind:'variable',name}">{{ name }}</DropdownMenuItem></template>
          <DropdownMenuSeparator/><DropdownMenuItem @select="emit('edit')"><SlidersHorizontal :size="13"/>Composer une expression…</DropdownMenuItem>
        </DropdownMenuContent></DropdownMenuPortal>
      </DropdownMenuRoot>
      <button v-if="editable" type="button" class="ctx-socket-edit" :aria-label="`Configurer l’expression : ${label}`" title="Configurer l’expression" @click="emit('edit')"><SlidersHorizontal :size="12"/></button>
      <slot/>
    </div>
    <small v-if="error || typeError" class="ctx-socket-error" role="alert">{{ error || typeError }}</small>
  </div>
</template>
<style scoped>
.ctx-socket-wrap{min-width:0;flex:1}.ctx-expression-socket{display:flex;align-items:center;min-width:0;min-height:34px;padding:3px 5px;background:#1c1c1f;border:1px solid #38383d;border-radius:6px;gap:4px}.ctx-expression-socket.is-selected{border-color:#78787f}.ctx-expression-socket.is-dragging{border:1px dashed #ccccd2;background:#29292d}.ctx-expression-socket.has-error{border-color:#b58b81}.ctx-socket-choice{display:flex;align-items:center;justify-content:space-between;gap:8px;min-width:0;flex:1;min-height:25px;border:0;padding:1px;background:transparent;text-align:left}.ctx-socket-choice>:first-child{min-width:0;flex:1}.ctx-socket-choice>svg{flex:none;color:#8e8e95}.ctx-socket-choice.is-icon{flex:none;width:20px;justify-content:center}.ctx-socket-edit{flex:none;width:22px;height:24px;display:grid;place-items:center;padding:0;color:#a3a3aa;background:transparent;border:0;border-radius:4px}.ctx-socket-edit:hover{background:#333338;color:#eee}.context-studio .ctx-socket-literal{flex:1;min-width:0;background:transparent;border:0;padding:3px 2px;font-size:12px}.ctx-socket-error{display:block;color:#d9afa4;margin-top:4px}.ctx-socket-menu{max-height:360px;max-width:min(460px,90vw);overflow:auto}.ctx-socket-menu [role=menuitem]{display:flex;align-items:center;gap:7px}.ctx-socket-menu [role=menuitem]>span:not(.ctx-field-dot){flex:1}.ctx-socket-menu [role=menuitem]>small{font-size:10px;color:#9c9ca4;max-width:130px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.ctx-socket-menu [role=menuitem][data-disabled]{opacity:.4}.ctx-socket-menu-label{padding:7px;font-size:11px;color:#a7a7ae}.ctx-field-dot{display:inline-block;width:7px;height:7px;flex:none;border-radius:50%;background:var(--ctx-source-color)}
</style>
