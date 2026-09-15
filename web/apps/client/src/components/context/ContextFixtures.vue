<script setup lang="ts">
import type { JsonValue } from '@zedflow/sdk'

import { computed, ref } from 'vue'
import { FlaskConical, Plus } from 'lucide-vue-next'
import AppDialog from '../AppDialog.vue'
import ContextValueEditor from './ContextValueEditor.vue'
import { cloneContext, contextId, defaultContextValue, type ContextDraft } from '../../contextEngine'
import type { ContextType } from '@zedflow/sdk'
import { resolveSourceType } from '../../contextSources'

const props = defineProps<{ draft: ContextDraft; compact?: boolean }>()
const open = ref(false), name = ref(''), raw = ref(''), rawError = ref(''), rawMode = ref(false)
const preset = computed(() => props.draft.fixtureName || 'custom')
const presets = [{ id: 'custom', name: 'Valeurs personnalisées' }, { id: 'empty', name: 'Sans données' }, { id: 'success', name: 'Lecture réussie' }, { id: 'failure', name: 'Échec de lecture' }]

function isConversationMessage(type: ContextType): boolean {
  if (type.kind !== 'named' || type.name !== 'ConversationMessage') return false
  const message = resolveSourceType(type, props.draft.types)
  if (message.kind !== 'record' || Object.keys(message.fields).length !== 2) return false
  const role = message.fields.role, parts = message.fields.parts
  if (!role || !parts || resolveSourceType(role, props.draft.types).kind !== 'text') return false
  const list = resolveSourceType(parts, props.draft.types)
  if (list.kind !== 'list') return false
  const part = resolveSourceType(list.item, props.draft.types)
  return part.kind === 'record' && !Object.keys(part.fields).length
}
function conversationSample(failure: boolean): JsonValue[] {
  return [
    { role: 'user', parts: [{ text: 'Peux-tu lire le fichier de termes ?' }] },
    { role: 'model', parts: [{ text: 'Je vais consulter le document demandé.' }] },
    { role: 'model', parts: [{ id: '17', name: 'read', args: { path: 'docs/terms.md' } }] },
    { role: 'function', parts: [{ id: '17', functionResponse: { name: 'read', response: {
      status: failure ? 'erreur' : 'succès',
      content: failure ? 'Fichier indisponible : docs/terms.md' : 'Trois définitions sont disponibles dans docs/terms.md.',
    } } }] },
  ]
}
function sample(type: ContextType, path: string, failure: boolean, depth = 0): JsonValue {
  if (depth > 16) return null
  const resolved = resolveSourceType(type, props.draft.types)
  const key = path.toLowerCase()
  if (isConversationMessage(type)) return conversationSample(failure)[0]
  if (resolved.kind === 'record') {
    if (!Object.keys(resolved.fields).length && /\.(arguments|args)$/.test(key)) return { path: 'docs/terms.md' }
    return Object.fromEntries(Object.entries(resolved.fields).map(([field, value]) => [field, sample(value, `${path}.${field}`, failure, depth + 1)]))
  }
  if (resolved.kind === 'list') return isConversationMessage(resolved.item) ? conversationSample(failure) : [sample(resolved.item, path, failure, depth + 1)]
  if (resolved.kind === 'boolean') return !failure
  if (resolved.kind === 'number') return /id$/.test(key) ? 17 : 1
  if (resolved.kind === 'media') return { contentRef: 'fixture:media-example', mediaType: resolved.mediaType }
  if (resolved.kind !== 'text') return defaultContextValue(type, props.draft.types)
  if (/status|statut/.test(key)) return failure ? 'erreur' : 'succès'
  if (/callid|appelid|\.id$/.test(key)) return '17'
  if (/path|chemin|uri/.test(key)) return 'docs/terms.md'
  if (/title|titre/.test(key)) return 'Working System · Documentation'
  if (/instruction/.test(key)) return 'Préserver les sources et les incertitudes.'
  if (/skill/.test(key)) return 'Skills d’essai : consulter les sources et citer les fichiers utilisés.'
  if (/toolcall|appel/.test(key) && /name|nom/.test(key)) return 'read'
  if (/toolresult|resultat|result/.test(key)) return failure ? 'Fichier indisponible : docs/terms.md' : 'Trois définitions sont disponibles dans docs/terms.md.'
  if (/user|question|input|demande/.test(key)) return 'Peux-tu lire le fichier de termes ?'
  if (/model|assistant|sortie/.test(key)) return 'Je vais consulter le document demandé.'
  if (/document|file/.test(key)) return 'Ce document présente une vue d’ensemble du projet, son architecture et les principes de composition.'
  return 'Exemple de valeur'
}
function choose(value: string) {
  if (value === 'custom') { props.draft.fixtureName = value; return }
  // A selected dataset replaces fixtures only. No source, binding or permission changes.
  const resources = value === 'empty' ? {} : value === 'success' || value === 'failure'
    ? Object.fromEntries(Object.entries(props.draft.strategy.requirements).map(([key, type]) => [key, sample(type, key, value === 'failure')]))
    : cloneContext(props.draft.fixtures?.find(item => item.id === value)?.resources || {})
  props.draft.resources = resources
  props.draft.fixtureName = value
}
function supplied(key: string, checked: boolean) {
  if (checked) props.draft.resources[key] = defaultContextValue(props.draft.strategy.requirements[key], props.draft.types)
  else delete props.draft.resources[key]
  props.draft.fixtureName = 'custom'
}
function remember() {
  if (!name.value.trim()) return
  const id = contextId('fixture')
  ;(props.draft.fixtures ||= []).push({ id, name: name.value.trim(), resources: cloneContext(props.draft.resources) })
  props.draft.fixtureName = id; name.value = ''
}
function show() { raw.value = JSON.stringify(props.draft.resources, null, 2); rawError.value = ''; open.value = true }
function applyRaw() {
  try {
    const value = JSON.parse(raw.value)
    if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Un objet associe chaque source à sa valeur.')
    const extra = Object.keys(value).filter(key => !Object.hasOwn(props.draft.strategy.requirements, key))
    if (extra.length) throw new Error(`Sources non déclarées : ${extra.join(', ')}`)
    props.draft.resources = value; props.draft.fixtureName = 'custom'; rawError.value = ''; rawMode.value = false
  } catch (error) { rawError.value = error instanceof Error ? error.message : String(error) }
}
</script>

<template>
  <div class="ctx-fixtures" :class="{ compact }">
    <strong v-if="!compact"><FlaskConical :size="13"/> Données d’essai</strong>
    <div class="ctx-fixture-controls"><select :aria-label="compact ? 'Jeu de données de l’aperçu' : 'Jeu de données d’essai'" :value="preset" @change="choose(($event.target as HTMLSelectElement).value)"><option v-for="item in presets" :key="item.id" :value="item.id">{{ item.name }}</option><option v-for="item in draft.fixtures || []" :key="item.id" :value="item.id">{{ item.name }}</option></select><button v-if="!compact" @click="show">Voir les valeurs</button></div>
    <AppDialog v-model:open="open" title="Données d’essai du contexte" description="Ces valeurs restent dans le brouillon. Aucun fichier, outil ou modèle n’est exécuté pour cet aperçu." wide>
      <div class="ctx-fixture-dialog">
        <nav aria-label="Édition des données d’essai"><button :aria-pressed="!rawMode" @click="rawMode=false">Champs</button><button :aria-pressed="rawMode" @click="raw=JSON.stringify(draft.resources,null,2);rawMode=true">JSON</button></nav>
        <template v-if="rawMode"><textarea v-model="raw" aria-label="Valeurs JSON des sources" rows="16" spellcheck="false"/><p v-if="rawError" role="alert">{{ rawError }}</p><button @click="applyRaw">Appliquer les valeurs JSON</button></template>
        <template v-else><details v-for="(type,key) in draft.strategy.requirements" :key="key" open><summary>{{ key }} <small>{{ Object.keys(draft.resources).includes(key) ? draft.resources[key]===null?'null':'Fourni':'Non fourni' }}</small></summary><label class="ctx-checkbox"><input type="checkbox" :checked="Object.keys(draft.resources).includes(key)" @change="supplied(key,($event.target as HTMLInputElement).checked)"/>Fournir {{ key }} pour l’aperçu</label><ContextValueEditor v-if="Object.keys(draft.resources).includes(key)" v-model="draft.resources[key]" :type="type" :types="draft.types" :label="`Valeur de ${key}`" @update:model-value="draft.fixtureName='custom'"/></details><p v-if="!Object.keys(draft.strategy.requirements).length">Déclarez un type de source pour lui associer des valeurs d’essai.</p></template>
        <div class="ctx-fixture-save"><input v-model="name" aria-label="Nom du jeu de données" placeholder="Nommer ce jeu de données…" @keydown.enter.prevent="remember"/><button :disabled="!name.trim()" @click="remember"><Plus :size="14"/>Conserver dans le brouillon</button></div>
      </div>
    </AppDialog>
  </div>
</template>

<style scoped>
.ctx-fixtures{margin-top:20px;font-size:12px}.ctx-fixtures>strong{display:flex;align-items:center;gap:6px;font-weight:550;margin-bottom:9px}.ctx-fixture-controls{display:flex;gap:8px;align-items:center}.ctx-fixture-controls select{flex:1;min-width:0;max-width:240px}.ctx-fixture-controls button{flex:none;background:transparent;border:0;padding:4px 0;color:#a6bdd5;font-size:11px}.ctx-fixtures.compact{margin:0;max-width:230px}.ctx-fixtures.compact select{font-size:11px;padding:6px}.ctx-fixture-dialog{display:flex;flex-direction:column;gap:16px;overflow:auto;max-height:70vh}.ctx-fixture-dialog nav{display:flex;gap:8px}.ctx-fixture-dialog nav [aria-pressed=true]{background:#38383e}.ctx-fixture-dialog details{border-bottom:1px solid #3a3a40;padding:8px 0 16px}.ctx-fixture-dialog summary{cursor:pointer;margin-bottom:12px}.ctx-fixture-dialog summary small{float:right;color:#a7a7b1}.ctx-fixture-dialog label{display:flex;gap:8px}.ctx-fixture-dialog .ctx-checkbox{margin-bottom:12px}.ctx-fixture-dialog .ctx-checkbox input{width:auto}.ctx-fixture-dialog textarea{width:100%;font-family:monospace;background:#202024;color:#e8e8ee;padding:12px;border:1px solid #44444c}.ctx-fixture-save{display:flex;gap:8px}.ctx-fixture-save>input{flex:1;min-width:0}.ctx-fixture-save button{display:flex;align-items:center;gap:6px}@media(max-width:650px){.ctx-fixture-save{flex-wrap:wrap}}
</style>
