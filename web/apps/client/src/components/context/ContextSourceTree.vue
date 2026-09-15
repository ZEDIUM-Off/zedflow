<script setup lang="ts">
import { computed, ref } from 'vue'
import { Braces, ChevronDown, ChevronRight, GripVertical, Hash, List, Plus, Type } from 'lucide-vue-next'
import { typeLabel } from '../../contextEngine'
import type { ContextType } from '@zedflow/sdk'
import { CONTEXT_SOURCE_MAX_DEPTH, createContextSourceField, resolveSourceType, writeContextSourceDrag, type ContextSourceField } from '../../contextSources'
const props = withDefaults(defineProps<{ source: string; type: ContextType; rootType: ContextType; types: Record<string, ContextType>; path?: string[]; depth?: number }>(), { path: () => [], depth: 0 })
const emit = defineEmits<{ insert: [field: ContextSourceField]; select: [source: string] }>()
const expanded = ref<Record<string, boolean>>({})
const resolved = computed(() => resolveSourceType(props.type, props.types))
const depthLimited = computed(() => props.depth >= CONTEXT_SOURCE_MAX_DEPTH && resolved.value.kind === 'record' && Object.keys(resolved.value.fields).length > 0)
const fields = computed(() => resolved.value.kind === 'record' && !depthLimited.value ? Object.entries(resolved.value.fields) : [])
function field(name: string, type: ContextType) { return createContextSourceField(props.source, [...props.path, name], type, props.rootType) }
function expandable(type: ContextType) { return resolveSourceType(type, props.types).kind === 'record' && props.depth < CONTEXT_SOURCE_MAX_DEPTH }
function icon(type: ContextType) { const kind = resolveSourceType(type, props.types).kind; return kind === 'record' ? Braces : kind === 'list' ? List : kind === 'text' ? Type : Hash }
</script>
<template>
  <div class="ctx-source-tree" :class="{ 'ctx-source-tree-deep': depth > 6 }">
    <div v-for="[name, type] in fields" :key="name" class="ctx-source-tree-item">
      <div class="ctx-source-field-row">
        <button v-if="expandable(type)" type="button" class="ctx-field-disclosure" :aria-label="`${expanded[name] ? 'Replier' : 'Déplier'} ${name}`" :aria-expanded="!!expanded[name]" @click="expanded[name] = !expanded[name]"><ChevronDown v-if="expanded[name]" :size="12"/><ChevronRight v-else :size="12"/></button>
        <button type="button" class="ctx-source-field" draggable="true" :aria-label="`Insérer ${[source, ...path, name].join(' · ')}`" :title="`Glisser ou cliquer pour insérer ${[source, ...path, name].join('.')}`" @dragstart="writeContextSourceDrag($event, field(name, type)); emit('select', source)" @click="emit('insert', field(name, type))">
          <component :is="icon(type)" :size="12"/><span class="ctx-source-field-name">{{ name }}</span><small>{{ typeLabel(type) }}</small><GripVertical :size="12" class="ctx-field-grip"/>
        </button>
      </div>
      <ContextSourceTree v-if="expanded[name] && expandable(type)" :source="source" :type="type" :root-type="rootType" :types="types" :path="[...path, name]" :depth="depth + 1" @insert="emit('insert', $event)" @select="emit('select', $event)"/>
    </div>
    <p v-if="depthLimited" class="ctx-source-tree-note" role="status">Profondeur maximale de {{ CONTEXT_SOURCE_MAX_DEPTH }} niveaux atteinte. Insérez l’objet parent pour conserver ses champs.</p>
    <p v-else-if="resolved.kind === 'list'" class="ctx-source-tree-note"><List :size="12"/> Liste de {{ typeLabel(resolved.item).toLocaleLowerCase() }}. Ses champs sont accessibles dans « Pour chaque ».</p>
    <p v-else-if="resolved.kind === 'named'" class="ctx-source-tree-note">Schéma {{ resolved.name }} à définir dans les types du brouillon.</p>
    <p v-else-if="resolved.kind === 'record' && !fields.length" class="ctx-source-tree-note">Aucun champ déclaré. Insérez l’objet entier ou précisez son schéma.</p>
    <button v-else-if="!path.length && resolved.kind !== 'record'" type="button" class="ctx-source-value-button" draggable="true" @dragstart="writeContextSourceDrag($event, createContextSourceField(source, [], type, rootType))" @click="emit('insert', createContextSourceField(source, [], type, rootType))"><Plus :size="12"/>Insérer la valeur <small>{{ typeLabel(type) }}</small></button>
  </div>
</template>
<style scoped>
.ctx-source-tree .ctx-source-tree.ctx-source-tree-deep{margin-left:0;padding-left:0;border-left:0}
.ctx-source-tree{display:grid;gap:5px}.ctx-source-tree .ctx-source-tree{margin:5px 0 1px 15px;padding-left:8px;border-left:1px solid var(--border,#333)}.ctx-source-field-row{display:flex;align-items:center;min-width:0;gap:2px}.ctx-source-field{display:flex;align-items:center;gap:7px;min-width:0;width:100%;min-height:29px;text-align:left;border:1px solid var(--border,#333);background:var(--surface,#1c1c21);border-radius:5px;padding:4px 6px;cursor:grab;font-size:11px}.ctx-source-field:active{cursor:grabbing}.ctx-source-field:hover,.ctx-source-field:focus-visible{border-color:var(--ctx-source-color);background:color-mix(in srgb,var(--ctx-source-color) 9%,var(--surface,#1c1c21))}.ctx-source-field svg{flex-shrink:0;color:var(--ctx-source-color)}.ctx-source-field-name{min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.ctx-source-field small{margin-left:auto;color:var(--muted,#a0a0a0);font-size:10px;white-space:nowrap;padding:1px 4px;background:var(--surface-raised,#29292e);border-radius:4px}.ctx-source-field .ctx-field-grip{color:var(--muted,#a0a0a0);opacity:.55}.ctx-field-disclosure{display:grid;place-items:center;flex:0 0 15px;min-height:26px;border:0;background:none;padding:0;color:var(--muted,#a0a0a0)}.ctx-source-tree-note{display:flex;align-items:flex-start;gap:5px;margin:3px 0!important;font-size:10px!important;line-height:1.6;color:var(--muted,#a0a0a0)}.ctx-source-tree-note svg{flex-shrink:0;margin-top:2px}.ctx-source-value-button{display:flex;gap:5px;align-items:center;width:100%;font-size:11px;padding:5px 6px}.ctx-source-value-button small{margin-left:auto;color:var(--muted,#a0a0a0)}
</style>
