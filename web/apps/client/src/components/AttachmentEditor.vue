<script setup lang="ts">
import { computed } from 'vue'
import { Plus, Trash2, ArrowUp, ArrowDown } from 'lucide-vue-next'
import type { RunContext } from '@zedflow/sdk'
import { attachmentSlots, primitiveTools, type AgentAttachments, type AttachmentSlot, type InstructionItem, type SkillItem, type FileItem, type ToolItem } from '../graph/attachments'
const value = defineModel<AgentAttachments>({default: () => ({})})
const slot = defineModel<AttachmentSlot>('slot', {default: 'instructions'})
const props = defineProps<{context?: RunContext | null}>()
const items = computed(() => value.value[slot.value]?.items || [])
function add() {
  const id = crypto.randomUUID()
  const item = slot.value === 'instructions' ? {id, source: {kind: 'text', text: ''}, activation: 'always', mode: 'literal'}
    : slot.value === 'skills' ? {id, source: {kind: 'workspace'}, activation: 'explicit'}
    : slot.value === 'files' ? {id, path: '', maxChars: 32000, activation: 'always'} : {id, name: 'read'}
  value.value = {...value.value, [slot.value]: {items: [...items.value, item]}}
}
function remove(index: number) { value.value = {...value.value, [slot.value]: {items: items.value.filter((_, i) => i !== index)}} }
function move(index: number, offset: number) {
  const next = [...items.value]; [next[index], next[index + offset]] = [next[index + offset], next[index]]
  value.value = {...value.value, [slot.value]: {items: next}}
}
function optionalNumber(item: FileItem, key: 'startLine'|'endLine'|'maxChars', event: Event) {
  const input = (event.target as HTMLInputElement).value
  if (!input) delete item[key]
  else item[key] = Number(input)
}
function navigate(event: KeyboardEvent) {
  const index = attachmentSlots.findIndex(entry=>entry.id===slot.value)
  const next = event.key==='ArrowRight'?(index+1)%4:event.key==='ArrowLeft'?(index+3)%4:event.key==='Home'?0:event.key==='End'?3:-1
  if(next<0)return
  event.preventDefault();slot.value=attachmentSlots[next].id
  ;(event.currentTarget as HTMLElement).querySelectorAll<HTMLButtonElement>('[role=tab]')[next]?.focus()
}
function instructionSource(item: InstructionItem, kind: string) {
  item.source = kind === 'text' ? {kind, text: ''} : kind === 'file' ? {kind, path: 'AGENTS.md'} : {kind: 'workspace'}
}
function skillSource(item: SkillItem, path: string) {
  item.source = path === '@workspace' ? {kind: 'workspace'} : {kind: 'file', path: path === '@file' ? '' : path}
  const discovered = props.context?.skills.find(skill => skill.path === path)
  if (discovered) item.name = discovered.name
}
</script>
<template>
  <section class="attachment-editor" aria-label="Ressources de l’agent">
    <div class="attachment-tabs" role="tablist" aria-label="Pièces de l’agent" @keydown="navigate"><button v-for="entry in attachmentSlots" :key="entry.id" :id="`attachment-tab-${entry.id}`" role="tab" :tabindex="slot===entry.id?0:-1" :aria-selected="slot===entry.id" :class="{chosen:slot===entry.id}" @click="slot=entry.id">{{entry.label}}<small>{{value[entry.id]?.items.length||0}}</small></button></div>
    <div role="tabpanel" :aria-labelledby="`attachment-tab-${slot}`">
      <p class="attachment-description">{{attachmentSlots.find(entry=>entry.id===slot)?.hint}}</p>
      <p v-if="!items.length" class="attachment-empty">Aucune ressource attachée.</p>
      <article v-for="(item,index) in items" :key="item.id" class="attachment-item" :data-attachment-id="item.id">
        <header><label class="check-field"><input type="checkbox" :checked="item.enabled!==false" @change="item.enabled=($event.target as HTMLInputElement).checked"/>Ressource {{index+1}}</label><div><button type="button" class="icon-button" :disabled="index===0" aria-label="Monter la ressource" @click="move(index,-1)"><ArrowUp :size="12"/></button><button type="button" class="icon-button" :disabled="index===items.length-1" aria-label="Descendre la ressource" @click="move(index,1)"><ArrowDown :size="12"/></button><button type="button" class="icon-button" aria-label="Retirer la ressource" @click="remove(index)"><Trash2 :size="12"/></button></div></header>
        <template v-if="slot==='instructions'">
          <label>Source d’instructions<select :value="(item as InstructionItem).source.kind" @change="instructionSource(item as InstructionItem,($event.target as HTMLSelectElement).value)"><option value="text">Texte</option><option value="file">Fichier</option><option value="workspace">AGENTS.md du workspace</option></select></label>
          <label v-if="(item as InstructionItem).source.kind==='text'">Instructions<textarea v-model="((item as InstructionItem).source as {kind:'text';text:string}).text" rows="4"/></label>
          <label v-else-if="(item as InstructionItem).source.kind==='file'">Chemin d’instructions<input v-model="((item as InstructionItem).source as {kind:'file';path:string}).path" placeholder="AGENTS.md"/></label>
          <label>Interprétation<select :value="(item as InstructionItem).mode||'literal'" @change="(item as InstructionItem).mode=($event.target as HTMLSelectElement).value as 'literal'|'template'"><option value="literal">Texte littéral</option><option value="template">Template avec état du graphe</option></select></label>
        </template>
        <template v-else-if="slot==='skills'">
          <label>Source du skill<select :value="(item as SkillItem).source.kind==='workspace'?'@workspace':(item as SkillItem).source.kind==='file'&&context?.skills.some(skill=>skill.path===((item as SkillItem).source as {path:string}).path)?((item as SkillItem).source as {path:string}).path:'@file'" @change="skillSource(item as SkillItem,($event.target as HTMLSelectElement).value)"><option value="@workspace">Catalogue du workspace</option><option v-for="skill in context?.skills||[]" :key="skill.path" :value="skill.path">{{skill.name}}</option><option value="@file">Fichier SKILL.md</option></select></label>
          <template v-if="(item as SkillItem).source.kind==='file'"><label>Chemin du skill<input v-model="((item as SkillItem).source as {kind:'file';path:string}).path" placeholder=".agents/skills/nom/SKILL.md"/></label><label>Nom affiché<input v-model="(item as SkillItem).name" placeholder="Facultatif"/></label></template>
          <p v-else class="attachment-note">Les skills découverts seront proposés à cet agent. Leur contenu suit le mode d’activation choisi.</p>
        </template>
        <template v-else-if="slot==='files'">
          <label>Chemin du fichier<input v-model="(item as FileItem).path" placeholder="src/main.rs"/></label>
          <div class="field-pair"><label>Première ligne<input :value="(item as FileItem).startLine" @input="optionalNumber(item as FileItem,'startLine',$event)" type="number" min="1" placeholder="1"/></label><label>Dernière ligne<input :value="(item as FileItem).endLine" @input="optionalNumber(item as FileItem,'endLine',$event)" type="number" min="1" placeholder="Fin"/></label></div>
          <label>Limite de caractères<input :value="(item as FileItem).maxChars" @input="optionalNumber(item as FileItem,'maxChars',$event)" type="number" min="1" placeholder="32000"/></label>
        </template>
        <label v-else>Primitive<select v-model="(item as ToolItem).name"><option v-for="name in primitiveTools" :key="name">{{name}}</option></select></label>
        <label v-if="slot!=='tools'">Activation<select :value="(item as InstructionItem|SkillItem).activation||(slot==='skills'?'explicit':'always')" @change="(item as InstructionItem|SkillItem).activation=($event.target as HTMLSelectElement).value as 'always'|'explicit'"><option value="always">À chaque appel</option><option value="explicit">Sur activation explicite</option></select></label>
      </article>
      <button type="button" class="attachment-add" @click="add"><Plus :size="13"/>Ajouter {{slot==='instructions'?'des instructions':slot==='skills'?'un skill':slot==='files'?'un fichier':'un outil'}}</button>
    </div>
  </section>
</template>
