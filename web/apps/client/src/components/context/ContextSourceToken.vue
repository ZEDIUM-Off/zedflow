<script setup lang="ts">
import { FileText, GripVertical } from 'lucide-vue-next'
import { contextSourceStyle, writeContextSourceDrag, type ContextSourceField } from '../../contextSources'
withDefaults(defineProps<{ field: ContextSourceField; interactive?: boolean; draggable?: boolean }>(), { interactive: false, draggable: false })
const emit = defineEmits<{ insert: [field: ContextSourceField]; select: [source: string] }>()
</script>
<template>
  <component :is="interactive ? 'button' : 'span'" :type="interactive ? 'button' : undefined" class="ctx-source-token" :style="contextSourceStyle(field.typeId)" :draggable="draggable" :title="`${field.label} · ${field.type.kind}`" :aria-label="interactive ? `Insérer ${field.label}` : undefined" @dragstart="writeContextSourceDrag($event, field)" @click="interactive && emit('insert', field); emit('select', field.source)">
    <GripVertical v-if="draggable" :size="11" class="ctx-token-grip"/><FileText :size="12"/><span>{{ field.label }}</span>
  </component>
</template>
<style scoped>
.ctx-source-token{display:inline-flex;align-items:center;gap:5px;max-width:100%;min-height:26px;padding:3px 7px;border:1px solid color-mix(in srgb,var(--ctx-source-color) 43%,transparent);border-radius:5px;color:var(--ctx-source-color);background:color-mix(in srgb,var(--ctx-source-color) 8%,var(--surface,#1c1c21));font-size:11px;line-height:1.4;font-weight:500;vertical-align:middle}
.ctx-source-token span{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.ctx-source-token svg{flex-shrink:0}.ctx-source-token[draggable=true]{cursor:grab}.ctx-source-token[draggable=true]:active{cursor:grabbing}button.ctx-source-token:hover{background:color-mix(in srgb,var(--ctx-source-color) 15%,var(--surface,#1c1c21))}.ctx-source-token:focus-visible{outline:2px solid var(--ctx-source-color);outline-offset:2px}.ctx-token-grip{opacity:.7}
</style>
