<script lang="ts">
import type { ContextBlock as ContextDragBlock } from '@zedflow/sdk'

const BLOCK_MIME = 'application/x-zedflow-context-block'
let draggingBlock: { blocks: ContextDragBlock[]; id: string } | undefined
</script>
<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { ArrowUp, ArrowDown, ChevronDown, ChevronRight, Copy, Trash2, Folder, GitBranch, Text, Repeat2, GripVertical, MoreVertical, Pencil } from 'lucide-vue-next'
import { DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator } from 'reka-ui'
import ContextBlockMenu from './ContextBlockMenu.vue'
import ContextMessageProjection from './ContextMessageProjection.vue'
import ContextToolExchangeDialog from './ContextToolExchangeDialog.vue'
import ContextExpressionEditor from './ContextExpressionEditor.vue'
import ContextPredicateEditor from './ContextPredicateEditor.vue'
import { cloneContext, contextId, findContextBlock, newContextBlock } from '../../contextEngine'
import type { ContextBlock, ContextExpr, ContextTraceEntry, ContextType } from '@zedflow/sdk'
import { CONTEXT_SOURCE_MAX_DEPTH, contextSourceStyle, contextSourceTypeId, resolveSourceType, sourceAppearance } from '../../contextSources'
import { contextExpressionType } from './contextComposer'
import { textMessageValue, toolExchangeValue } from './contextMessageProjection'
const blocks = defineModel<ContextBlock[]>({ required: true })
const props = withDefaults(defineProps<{ resources: Record<string, ContextType>; types?: Record<string, ContextType>; variables?: string[]; variableTypes?: Record<string, ContextType>; trace?: ContextTraceEntry[]; selected?: string; depth?: number; label?: string; version?: number }>(), { depth: 0, label: 'Programme', version: 2, variables: () => [], variableTypes: () => ({}), trace: () => [] })
const emit = defineEmits<{ select: [id: string] }>()
const exchangeOpen = ref(false)
const collapsed = ref<Record<string, boolean>>({})
const details = ref<Record<string, boolean>>({})
const removed = ref<{ block: ContextBlock; index: number }>()
const dropIndex = ref<number>(), announcement = ref(''), renaming = ref('')
watch(blocks, (_next, previous) => {
  removed.value = undefined
  dropIndex.value = undefined
  if (draggingBlock?.blocks === previous) draggingBlock = undefined
})
function clearDrag() { dropIndex.value = undefined }
onMounted(() => document.addEventListener('dragend', clearDrag))
onUnmounted(() => document.removeEventListener('dragend', clearDrag))
watch(() => props.selected, id => { if (!id) return; for (const block of blocks.value) if (findContextBlock([block], id)) collapsed.value[block.id] = false })
async function focus(id: string) { await nextTick(); const element = document.getElementById(`context-block-${id}`); element?.querySelector<HTMLElement>(element.classList.contains('is-inline') ? '.ctx-block-grip' : '.ctx-block-title')?.focus() }
function add(kind: ContextBlock['kind']) { const block = newContextBlock(kind, Object.keys(props.resources)); blocks.value.push(block); emit('select', block.id); void focus(block.id) }
function move(index: number, delta: number) {
  const target = index + delta
  if (target < 0 || target >= blocks.value.length) return
  const [block] = blocks.value.splice(index, 1)
  blocks.value.splice(target, 0, block); emit('select', block.id); announcement.value = `Bloc déplacé à la position ${target + 1}.`; void focus(block.id)
}
function remove(index: number) {
  const [block] = blocks.value.splice(index, 1); removed.value = { block, index }
  const next = blocks.value[Math.min(index, blocks.value.length - 1)]
  if (next) { emit('select', next.id); void focus(next.id) }
}
function duplicate(index: number) {
  const copy = cloneContext(blocks.value[index])
  function ids(block: ContextBlock) { block.id = contextId(block.kind); if (block.kind === 'group' || block.kind === 'forEach') block.items.forEach(ids); if (block.kind === 'if') { block.then.forEach(ids); block.else.forEach(ids) } }
  ids(copy); blocks.value.splice(index + 1, 0, copy); emit('select', copy.id); void focus(copy.id)
}
function restore() {
  if (!removed.value) return
  if (findContextBlock(blocks.value, removed.value.block.id)) { removed.value = undefined; announcement.value = 'Ce bloc est déjà présent dans le programme.'; return }
  const { block, index } = removed.value; blocks.value.splice(Math.min(index, blocks.value.length), 0, block); removed.value = undefined; emit('select', block.id); void focus(block.id)
}
function sourceName(value: ContextExpr): string {
  if (value.kind === 'resource' || value.kind === 'variable') return value.name || 'Source à choisir'
  if (value.kind === 'literal') return 'Texte composé'
  if ('value' in value) return sourceName(value.value)
  if (value.kind === 'template') return 'Texte composé'
  if (value.kind === 'record') return 'Champs assemblés'
  if (value.kind === 'call') return value.name || 'Fonction'
  return 'Contenu'
}
function title(block: ContextBlock) {
  if (block.kind === 'group') return block.label || 'Groupe'
  if (block.kind === 'if') return 'Si'
  if (block.kind === 'forEach') return `Pour chaque ${sourceName(block.value)}`
  if (block.role === 'instruction') return 'Instructions'
  if (block.format === 'adkMessages') {
    const message = textMessageValue(block.value)
    return message ? message.role === 'user' ? 'Message utilisateur' : 'Réponse du modèle' : toolExchangeValue(block.value) ? 'Échange d’outil' : 'Échange de messages'
  }
  const name = sourceName(block.value), type = props.resources[name] || props.variableTypes[name]
  return type ? sourceAppearance(name, type).label : name
}
function roleLabel(block: Extract<ContextBlock, { kind: 'emit' }>) {
  const message = block.format === 'adkMessages' && textMessageValue(block.value)
  if (message) return message.role === 'user' ? 'Rôle : utilisateur' : 'Rôle : assistant'
  if (block.format === 'adkMessages' && toolExchangeValue(block.value)) return 'Échange d’outil'
  return block.role === 'instruction' ? 'Instruction' : block.format === 'adkMessages' ? 'Messages' : block.format === 'json' ? 'JSON' : block.format === 'media' ? 'Média' : 'Donnée'
}
function subtitle(block: ContextBlock) {
  if (block.kind === 'group') return `${block.items.length} ${block.items.length > 1 ? 'blocs' : 'bloc'}`
  if (block.kind === 'if') return 'Ajouter selon une condition'
  if (block.kind === 'forEach') return 'Parcourir la liste'
  return block.format === 'adkMessages' ? 'Ajouter un échange' : 'Ajouter au contexte'
}
function conditionOutcome(id: string) {
  const outcomes = props.trace.filter(entry => entry.blockId === id && typeof entry.outcome === 'boolean')
  if (!outcomes.length) return
  const yes = outcomes.filter(entry => entry.outcome).length, no = outcomes.length - yes
  return { label: yes && no ? 'Variable' : yes ? 'Vrai' : 'Faux', description: `Données d’essai : ${yes} condition${yes > 1 ? 's' : ''} vraie${yes > 1 ? 's' : ''}, ${no} fausse${no > 1 ? 's' : ''}.` }
}
function sourceStyle(block: ContextBlock) {
  if (block.kind !== 'emit') return {}
  const name = sourceName(block.value), type = props.resources[name] || props.variableTypes[name]
  return type ? contextSourceStyle(contextSourceTypeId(type)) : {}
}
function localTypes(block: Extract<ContextBlock, { kind: 'forEach' }>) {
  const type = contextExpressionType(block.value, props.resources, props.types, props.variableTypes)
  const resolved = type && resolveSourceType(type, props.types || {})
  return resolved?.kind === 'list' ? { ...props.variableTypes, [block.item]: resolved.item } : props.variableTypes
}
function localVariables(block: Extract<ContextBlock, { kind: 'forEach' }>) { return [...props.variables.filter(name => name !== block.item), block.item] }
function keys(event: KeyboardEvent, index: number) {
  if (event.altKey && ['ArrowUp', 'ArrowDown'].includes(event.key)) { event.preventDefault(); event.stopPropagation(); move(index, event.key === 'ArrowUp' ? -1 : 1) }
  if (event.key === 'Delete' && !event.altKey) { event.preventDefault(); event.stopPropagation(); remove(index) }
}
function dragStart(event: DragEvent, block: ContextBlock) {
  if (!event.dataTransfer) return
  draggingBlock = { blocks: blocks.value, id: block.id }
  event.dataTransfer.setData(BLOCK_MIME, block.id); event.dataTransfer.effectAllowed = 'move'
  const element = document.getElementById(`context-block-${block.id}`)
  if (element) event.dataTransfer.setDragImage(element, 20, 20)
  emit('select', block.id)
}
function containsList(block: ContextBlock, list: ContextBlock[]): boolean {
  if (block.kind === 'emit') return false
  const lists = block.kind === 'if' ? [block.then, block.else] : [block.items]
  return lists.some(children => children === list || children.some(child => containsList(child, list)))
}
function canDrop(event: DragEvent) {
  const dragged = draggingBlock?.blocks.find(block => block.id === draggingBlock?.id)
  return !!event.dataTransfer?.types.includes(BLOCK_MIME) && !!dragged && !containsList(dragged, blocks.value)
}
function dragOver(event: DragEvent, index: number) { if (!canDrop(event)) return; event.preventDefault(); event.stopPropagation(); dropIndex.value = index; if (event.dataTransfer) event.dataTransfer.dropEffect = 'move' }
function drop(event: DragEvent, index: number) {
  if (!canDrop(event) || !draggingBlock) return
  event.preventDefault(); event.stopPropagation()
  const from = draggingBlock.blocks, fromIndex = from.findIndex(block => block.id === draggingBlock?.id)
  if (fromIndex < 0) return
  const [block] = from.splice(fromIndex, 1)
  const destination = from === blocks.value && fromIndex < index ? index - 1 : index
  blocks.value.splice(destination, 0, block)
  draggingBlock = undefined; dropIndex.value = undefined; announcement.value = `Bloc déplacé à la position ${destination + 1}.`; emit('select', block.id); void focus(block.id)
}
function rename(block: Extract<ContextBlock, { kind: 'group' }>) { renaming.value = block.id }
async function closeMenuFocus(event: Event, block: ContextBlock) {
  if (renaming.value !== block.id) return
  // Renaming owns the next focus; the menu must not restore its trigger later.
  event.preventDefault()
  await nextTick()
  if (renaming.value === block.id) document.getElementById(`context-block-name-${block.id}`)?.focus()
}
</script>
<template>
  <div class="ctx-block-list ctx-composed-program" :aria-label="label" :data-depth="depth" @dragleave.self="dropIndex=undefined">
    <article v-for="(block,index) in blocks" :key="block.id" :id="`context-block-${block.id}`" :data-context-block="block.id" :data-block-kind="block.kind" :class="['ctx-block',block.kind,{selected:selected===block.id,'is-inline':depth>0&&block.kind==='emit'&&!collapsed[block.id],'is-collapsed':collapsed[block.id],'is-drop-before':dropIndex===index}]" @click.stop="emit('select',block.id)" @dragover="dragOver($event,index)" @drop="drop($event,index)">
      <div class="ctx-block-order"><span>{{ String(index+1).padStart(2,'0') }}</span><button type="button" class="ctx-block-grip" draggable="true" :aria-label="`Déplacer le bloc ${index+1}`" title="Déplacer · Alt + ↑ ou ↓" @dragstart.stop="dragStart($event,block)" @dragend="draggingBlock=undefined;dropIndex=undefined" @keydown="keys($event,index)"><GripVertical :size="14"/></button></div>
      <div class="ctx-block-content">
        <header class="ctx-block-header">
          <button class="ctx-collapse icon-button" :tabindex="depth>0&&block.kind==='emit'&&!collapsed[block.id]?-1:undefined" :aria-expanded="!collapsed[block.id]" :aria-label="`${collapsed[block.id]?'Déplier':'Replier'} le bloc ${index+1}`" @click.stop="collapsed[block.id]=!collapsed[block.id]"><ChevronRight v-if="collapsed[block.id]" :size="12"/><ChevronDown v-else :size="12"/></button>
          <Folder v-if="block.kind==='group'" class="ctx-block-symbol" :size="14"/><GitBranch v-else-if="block.kind==='if'" class="ctx-block-symbol" :size="14"/><Repeat2 v-else-if="block.kind==='forEach'" class="ctx-block-symbol" :size="14"/><span v-else class="ctx-block-source-dot" :style="sourceStyle(block)"/>
          <input v-if="block.kind==='group' && renaming===block.id" :id="`context-block-name-${block.id}`" v-model="block.label" class="ctx-block-name" aria-label="Nom du groupe" @blur="renaming=''" @keydown.enter="renaming=''" @keydown.esc="renaming=''"/>
          <button v-else class="ctx-block-title" :tabindex="depth>0&&block.kind==='emit'&&!collapsed[block.id]?-1:undefined" @focus="emit('select',block.id)" @keydown="keys($event,index)" @click.stop="collapsed[block.id]=!collapsed[block.id]"><span>{{title(block)}}</span></button>
          <small class="ctx-block-subtitle">{{subtitle(block)}}</small><span v-if="block.kind==='if' && conditionOutcome(block.id)" class="ctx-condition-outcome" :title="conditionOutcome(block.id)?.description" :aria-label="conditionOutcome(block.id)?.description">{{conditionOutcome(block.id)?.label}}</span>
          <DropdownMenuRoot><DropdownMenuTrigger class="ctx-block-more icon-button" :aria-label="`Actions du bloc ${index+1}`"><MoreVertical :size="14"/></DropdownMenuTrigger><DropdownMenuPortal><DropdownMenuContent class="compact-menu" align="end" :side-offset="4" @close-auto-focus="closeMenuFocus($event,block)"><DropdownMenuItem v-if="block.kind==='group'" @select="rename(block)"><Pencil :size="13"/>Renommer le groupe</DropdownMenuItem><DropdownMenuItem v-if="block.kind==='emit'" @select="details[block.id]=!details[block.id];collapsed[block.id]=false"><Text :size="13"/>Rôle et représentation</DropdownMenuItem><DropdownMenuItem :disabled="index===0" @select="move(index,-1)"><ArrowUp :size="13"/>Monter ce bloc <small>Alt ↑</small></DropdownMenuItem><DropdownMenuItem :disabled="index===blocks.length-1" @select="move(index,1)"><ArrowDown :size="13"/>Descendre ce bloc <small>Alt ↓</small></DropdownMenuItem><DropdownMenuItem @select="duplicate(index)"><Copy :size="13"/>Dupliquer ce bloc</DropdownMenuItem><DropdownMenuSeparator/><DropdownMenuItem @select="remove(index)"><Trash2 :size="13"/>Supprimer ce bloc</DropdownMenuItem></DropdownMenuContent></DropdownMenuPortal></DropdownMenuRoot>
        </header>
        <div v-show="!collapsed[block.id]" class="ctx-block-body">
          <template v-if="block.kind==='group'"><ContextBlockEditor v-if="depth<CONTEXT_SOURCE_MAX_DEPTH" v-model="block.items" :resources="resources" :types="types" :variables="variables" :variable-types="variableTypes" :selected="selected" :trace="trace" :version="version" :depth="depth+1" :label="`Contenu de ${block.label}`" @select="emit('select',$event)"/><p v-else class="ctx-depth-limit" role="alert">Ce programme dépasse 64 niveaux de blocs. Réduisez l’imbrication pour poursuivre l’édition visuelle.</p></template>
          <template v-else-if="block.kind==='emit'">
            <div class="ctx-fragment-line"><ContextExpressionEditor v-model="block.value" :resources="resources" :types="types" :variables="variables" :variable-types="variableTypes" :label="`Contenu du fragment · ${title(block)}`" :message-projection="block.format==='adkMessages'" compact :expected-kinds="block.format==='text'?['text']:block.format==='media'?['media']:block.format==='adkMessages'?['list']:undefined"/><button type="button" class="ctx-fragment-role" :aria-expanded="!!details[block.id]" aria-label="Configurer le rôle et la représentation" @click="details[block.id]=!details[block.id]">{{roleLabel(block)}}</button></div>
            <div v-if="details[block.id]" class="ctx-row ctx-fragment-options"><ContextMessageProjection v-if="version>=2" :model-value="block" @update:model-value="blocks[index]=$event" :resources="resources" :types="types" :variables="variableTypes"/><label>Rôle<select aria-label="Rôle" v-model="block.role"><option value="instruction">Instruction</option><option value="data">Donnée</option></select></label><label>Représentation<select aria-label="Représentation" v-model="block.format"><option value="text">Texte</option><option value="json">JSON structuré</option><option value="media">Référence média</option><option v-if="version>=2" value="adkMessages">Messages ADK structurés</option></select></label></div>
          </template>
          <template v-else-if="block.kind==='forEach'">
            <div class="ctx-loop-settings"><div class="ctx-loop-collection"><span>Collection</span><ContextExpressionEditor v-model="block.value" :resources="resources" :types="types" :variables="variables" :variable-types="variableTypes" :label="`Collection · ${title(block)}`" compact :expected-kinds="['list']"/></div><label>Élément courant<input v-model="block.item" aria-label="Nom de l’élément courant" placeholder="document"/></label></div>
            <ContextBlockEditor v-if="depth<CONTEXT_SOURCE_MAX_DEPTH" v-model="block.items" :resources="resources" :types="types" :variables="localVariables(block)" :variable-types="localTypes(block)" :selected="selected" :trace="trace" :version="version" :depth="depth+1" :label="`Pour chaque ${block.item}`" @select="emit('select',$event)"/><p v-else class="ctx-depth-limit" role="alert">Ce programme dépasse 64 niveaux de blocs. Réduisez l’imbrication pour poursuivre l’édition visuelle.</p>
          </template>
          <template v-else-if="block.kind==='if'"><ContextPredicateEditor v-model="block.condition" :resources="resources" :types="types" :variables="variables" :variable-types="variableTypes"/><div v-if="depth<CONTEXT_SOURCE_MAX_DEPTH" class="ctx-branches"><section><h4>Alors</h4><ContextBlockEditor v-model="block.then" :resources="resources" :types="types" :variables="variables" :variable-types="variableTypes" :selected="selected" :trace="trace" :version="version" :depth="depth+1" label="Branche Alors" @select="emit('select',$event)"/></section><section><h4>Sinon</h4><ContextBlockEditor v-model="block.else" :resources="resources" :types="types" :variables="variables" :variable-types="variableTypes" :selected="selected" :trace="trace" :version="version" :depth="depth+1" label="Branche Sinon" @select="emit('select',$event)"/></section></div><p v-else class="ctx-depth-limit" role="alert">Ce programme dépasse 64 niveaux de blocs. Réduisez l’imbrication pour poursuivre l’édition visuelle.</p></template>
        </div>
      </div>
    </article>
    <div class="ctx-program-end" :class="{'is-drop-before':dropIndex===blocks.length}" @dragover="dragOver($event,blocks.length)" @drop="drop($event,blocks.length)"><p v-if="!blocks.length && !depth" class="ctx-empty-blocks">Ajoutez un bloc, puis choisissez les champs qui composeront le contexte.</p><ContextBlockMenu :compact="depth>0" :label="!blocks.length && depth ? 'Ne rien ajouter · Ajouter un bloc' : 'Ajouter un bloc'" :version="version" @add="add" @exchange="exchangeOpen=true"/></div>
    <button v-if="removed" type="button" class="ctx-undo" @click="restore">Annuler la suppression du bloc</button>
    <ContextToolExchangeDialog v-model:open="exchangeOpen" :resources="resources" :types="types" @add="blocks.push($event);emit('select',$event.id)"/>
    <span class="ctx-program-announcement" aria-live="polite">{{announcement}}</span>
  </div>
</template>
<style scoped>
.ctx-composed-program{gap:8px;container-type:inline-size;container-name:context-program}.ctx-composed-program>.ctx-block{display:flex;border:1px solid #353539;border-radius:6px;background:#202022;min-width:0;overflow:visible}.ctx-composed-program>.ctx-block.selected{border-color:#6a6a71;box-shadow:none}.ctx-block-order{width:37px;flex:none;border-right:1px solid #353539;display:flex;flex-direction:column;align-items:center;padding-top:11px;gap:7px;color:#c0c0c7;font-size:11px;font-variant-numeric:tabular-nums}.ctx-block-grip{display:grid;place-items:center;width:26px;height:25px;padding:0;opacity:0;background:transparent;border:0;color:#898990;cursor:grab}.ctx-block:hover>.ctx-block-order .ctx-block-grip,.ctx-block:focus-within>.ctx-block-order .ctx-block-grip{opacity:1}.ctx-block-grip:active{cursor:grabbing}.ctx-block-content{flex:1;min-width:0}.ctx-block-content>.ctx-block-header{min-height:36px;padding:5px 7px 5px 4px;gap:7px;border:0;background:transparent;border-radius:0;flex-wrap:nowrap}.ctx-block-header>.ctx-collapse{width:14px;padding:0;color:#9a9aa2}.ctx-block-symbol{flex:none;color:#b2b2bb}.ctx-block.if>.ctx-block-content>.ctx-block-header>.ctx-block-symbol{color:#c6b1a1}.ctx-block-source-dot{width:8px;height:8px;flex:none;border-radius:50%;background:var(--ctx-source-color,#aab8ad)}.ctx-block-header>.ctx-block-title{flex:0 1 auto;gap:0;font-size:12px;font-weight:550;padding:2px 0;line-height:1.4;min-width:36px}.ctx-block-title span{flex:initial;white-space:nowrap;text-overflow:ellipsis;overflow:hidden}.ctx-block-header>.ctx-block-subtitle{font-size:10px;color:#a3a3ab;flex:1;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;margin-left:7px}.ctx-block-header>.ctx-block-more{margin-left:auto;padding:3px;flex:none;background:transparent;border:0;color:#a3a3ac}.ctx-block-content>.ctx-block-body{padding:0 10px 10px;gap:9px}.ctx-fragment-line{display:flex;align-items:flex-start;gap:5px;min-width:0}.ctx-fragment-role{flex:none;max-width:90px;border:1px solid #434349;border-radius:4px;background:#2b2b2f;padding:4px 5px;color:#b8b8c1;font-size:10px;margin-top:5px}.ctx-fragment-options{padding-top:5px}.ctx-fragment-options label{font-size:10px}.ctx-fragment-options select{font-size:11px}.ctx-composed-program .ctx-block-list{padding-left:0;border-left:0;gap:7px}.ctx-composed-program .ctx-block-list .ctx-block{background:#1e1e21;border-color:#38383d}.ctx-composed-program .ctx-block-list .ctx-block-order{width:28px;font-size:10px}.ctx-composed-program .ctx-block-list .ctx-block-subtitle{display:none}.ctx-composed-program .ctx-block-list .ctx-block-body{padding:0 7px 8px}.ctx-block-name{width:auto;min-width:0;flex:1;background:transparent;font-size:12px;padding:2px 4px}.ctx-branches{position:relative;border-left:1px solid #626268;margin-left:6px;padding-left:14px}.ctx-branches>section+section{border-top:0;margin-top:12px;padding-top:0}.ctx-branches h4{position:relative;text-transform:none;letter-spacing:0;color:#cacacf;font-size:11px;font-weight:500;margin:0 0 7px}.ctx-branches h4:before{content:'';position:absolute;width:5px;height:5px;left:-18px;top:4px;border:1px solid #93939a;border-radius:50%;background:#202022}.ctx-loop-settings{display:grid;grid-template-columns:minmax(0,1fr) minmax(90px,.5fr);gap:8px}.ctx-loop-settings>label,.ctx-loop-collection{min-width:0;font-size:10px;color:#9898a1;display:flex;flex-direction:column;gap:5px}.ctx-loop-settings input{min-height:34px;font-size:11px}.ctx-program-end{min-height:10px;padding-top:2px;display:flex;flex-direction:column;gap:9px}.ctx-program-end .ctx-empty-blocks{padding:13px;color:#ababaf;background:transparent;font-size:12px}.ctx-composed-program>.is-drop-before,.ctx-program-end.is-drop-before{box-shadow:0 -3px 0 #babac2}.ctx-program-announcement{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0}.is-collapsed>.ctx-block-order{padding-top:10px}.is-collapsed>.ctx-block-order>.ctx-block-grip{display:none}@container context-program (max-width:390px){.ctx-block-header>.ctx-block-subtitle{display:none}.ctx-block-order{width:29px}.ctx-fragment-line{flex-wrap:wrap}.ctx-fragment-line>.ctx-expression-shell{flex-basis:100%}.ctx-fragment-role{margin-top:0}.ctx-loop-settings{grid-template-columns:minmax(0,1fr)}}@media(max-width:900px){.ctx-block-grip{opacity:1}.ctx-block-order{width:29px}}
</style>
<style scoped>
/* Nested emissions are editable rows; the containing block carries their heading. */
.ctx-depth-limit{color:#c4a79d;font-size:11px;line-height:1.5}
.ctx-condition-outcome{flex:none;color:#b6c7ba;background:#29312d;border:1px solid #48574d;border-radius:4px;padding:2px 5px;font-size:10px;line-height:1.2}
.ctx-composed-program .ctx-block-list{gap:5px}
.ctx-composed-program>.ctx-block.is-inline{border:0;border-radius:4px;background:transparent}
.ctx-composed-program>.ctx-block.is-inline.selected{outline:1px solid #606067;outline-offset:1px}
.ctx-block.is-inline>.ctx-block-order{position:relative;width:24px;padding-top:12px;border-right:0;font-size:10px;color:#9e9ea7}
.ctx-block.is-inline>.ctx-block-order>.ctx-block-grip{position:absolute;inset:3px 0 auto;width:23px;height:32px;background:#202022;z-index:1}
.ctx-block.is-inline>.ctx-block-content{display:grid;grid-template-columns:minmax(0,1fr) 23px;align-items:start}
.ctx-block.is-inline>.ctx-block-content>.ctx-block-header{grid-column:2;grid-row:1;position:relative;min-height:37px;padding:5px 0;gap:0;justify-content:flex-end}
.ctx-block.is-inline>.ctx-block-content>.ctx-block-header>.ctx-block-title,
.ctx-block.is-inline>.ctx-block-content>.ctx-block-header>.ctx-collapse{position:absolute;width:1px;height:1px;padding:0;overflow:hidden;clip-path:inset(50%);white-space:nowrap}
.ctx-block.is-inline>.ctx-block-content>.ctx-block-header>.ctx-block-title:focus-visible{outline:0}
.ctx-block.is-inline>.ctx-block-content>.ctx-block-header>.ctx-block-symbol,
.ctx-block.is-inline>.ctx-block-content>.ctx-block-header>.ctx-block-source-dot,
.ctx-block.is-inline>.ctx-block-content>.ctx-block-header>.ctx-block-subtitle{display:none}
.ctx-block.is-inline>.ctx-block-content>.ctx-block-header>.ctx-block-more{margin:0;padding:4px 2px}
.ctx-block.is-inline>.ctx-block-content>.ctx-block-body{grid-column:1;grid-row:1;padding:2px 0;gap:8px}
.ctx-block.is-inline .ctx-fragment-role{max-width:130px;white-space:nowrap;font-size:10px}
.ctx-composed-program[data-depth]:not([data-depth="0"])>.ctx-program-end{padding-top:1px;min-height:21px}
@container context-program (max-width:390px){.ctx-block.is-inline .ctx-fragment-line{flex-wrap:nowrap}.ctx-block.is-inline .ctx-fragment-line>.ctx-expression-shell{flex-basis:auto}.ctx-block.is-inline .ctx-fragment-role{margin-top:5px;max-width:90px;font-size:9px}}
</style>
