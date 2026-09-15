<script setup lang="ts">
import { computed } from 'vue'
import { FileText, Braces, Variable } from 'lucide-vue-next'
import type { ContextExpr, ContextType } from '@zedflow/sdk'
import { CONTEXT_SOURCE_MAX_DEPTH, contextSourceStyle, contextSourceTypeId, sourceAppearance } from '../../contextSources'
import { contextExpressionDescription } from './contextComposer'
import { textMessageValue, toolExchangeValue } from './contextMessageProjection'
const props = withDefaults(defineProps<{ value: ContextExpr; resources: Record<string, ContextType>; variableTypes?: Record<string, ContextType> }>(), { variableTypes: () => ({}) })
const message = computed(() => textMessageValue(props.value))
const exchange = computed(() => toolExchangeValue(props.value))
const tokens = computed(() => {
  const result: { key: string; label: string; typeId: string; local: boolean }[] = []
  function visit(value: ContextExpr, path: string[] = [], depth = 0) {
    if (depth > CONTEXT_SOURCE_MAX_DEPTH || result.length >= 8) return
    if (value.kind === 'field') { visit(value.value, [value.field, ...path], depth + 1); return }
    if (value.kind === 'resource' || value.kind === 'variable') {
      const key = `${value.kind}:${value.name}:${path.join('.')}`
      if (result.some(token => token.key === key)) return
      const type = value.kind === 'resource' ? props.resources[value.name] : props.variableTypes[value.name]
      const peers = value.kind === 'resource' ? props.resources : props.variableTypes
      const repeatedType = type && Object.values(peers).filter(peer => contextSourceTypeId(peer) === contextSourceTypeId(type)).length > 1
      const display = type ? sourceAppearance(value.name, type).label : value.name
      const name = repeatedType && display !== value.name ? `${display} · ${value.name}` : display
      result.push({ key, label: [name, ...path].filter(Boolean).join(' · '), typeId: type ? contextSourceTypeId(type) : value.name, local: value.kind === 'variable' })
      return
    }
    if (value.kind === 'literal') return
    if ('value' in value) visit(value.value, [], depth + 1)
    if (value.kind === 'template') Object.values(value.values).forEach(child => visit(child, [], depth + 1))
    if (value.kind === 'record') Object.values(value.fields).forEach(child => visit(child, [], depth + 1))
    if (value.kind === 'list') value.items.forEach(child => visit(child, [], depth + 1))
    if (value.kind === 'call') Object.values(value.arguments).forEach(child => visit(child, [], depth + 1))
    if (value.kind === 'map') visit(value.body, [], depth + 1)
  }
  if (message.value) visit(message.value.value)
  else if (exchange.value) { visit(exchange.value.call); visit(exchange.value.result) }
  else visit(props.value)
  return result
})
const transformed = computed(() => !message.value && !exchange.value && !['resource', 'variable', 'field', 'literal'].includes(props.value.kind))
const description = computed(() => message.value ? contextExpressionDescription(message.value.value) : exchange.value ? 'Appel et résultat d’outil' : contextExpressionDescription(props.value))
</script>
<template>
  <span class="ctx-expression-summary">
    <span v-if="transformed && tokens.length" class="ctx-summary-operation">{{ description }}</span>
    <span v-for="token in tokens" :key="token.key" class="ctx-source-token" :style="contextSourceStyle(token.typeId)"><Variable v-if="token.local" :size="12"/><FileText v-else :size="12"/><span>{{ token.label }}</span></span>
    <span v-if="!tokens.length" class="ctx-summary-literal"><Braces v-if="value.kind==='literal' && typeof value.value==='object'" :size="12"/>{{ description }}</span>
  </span>
</template>
<style scoped>
.ctx-expression-summary{display:flex;align-items:center;flex-wrap:wrap;gap:5px;min-width:0;text-align:left}.ctx-source-token{display:inline-flex;align-items:center;gap:6px;padding:4px 7px;max-width:100%;border:1px solid color-mix(in srgb,var(--ctx-source-color) 40%,transparent);border-radius:5px;background:color-mix(in srgb,var(--ctx-source-color) 8%,transparent);color:color-mix(in srgb,var(--ctx-source-color) 65%,#efefef);font-size:11px;line-height:1.2}.ctx-source-token svg{flex:none}.ctx-source-token span{overflow:hidden;white-space:nowrap;text-overflow:ellipsis}.ctx-summary-operation{font-size:11px;color:#b5b5bb;width:100%;margin-bottom:1px}.ctx-summary-literal{display:flex;align-items:center;gap:5px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;max-width:100%;font-size:12px;color:#c7c7cc}
</style>
