<script setup lang="ts">
const client=useClient()
import { useClient } from '@zedflow/vue'
import { computed, ref, watch } from 'vue'
import { Braces, ChevronDown, ChevronRight, Copy, FileText, Image, MessageSquare, Plus, Sparkles, Wrench, X } from 'lucide-vue-next'
import ContextTypeEditor from './ContextTypeEditor.vue'
import ContextValueEditor from './ContextValueEditor.vue'
import ContextSourceExamples from './ContextSourceExamples.vue'
import ContextSourceGallery from './ContextSourceGallery.vue'
import ContextSourceTree from './ContextSourceTree.vue'
import { cloneContext, defaultContextValue, typeLabel, type ContextDraft } from '../../contextEngine'
import type { ContextType } from '@zedflow/sdk'
import { contextSourceOrder, contextSourceStyle, contextSourceTypeId, contextTypeDependencies, createContextSourceField, normalizeContextSourceEntries, resolveSourceType, sameContextType, sourceAppearance, sourceUseCount, writeContextSourceDrag, type ContextSourceCatalog, type ContextSourceEntry, type ContextSourceField } from '../../contextSources'
const props = withDefaults(defineProps<{ draft: ContextDraft; workspaceId?: string; selectedSource?: string }>(), { workspaceId: '', selectedSource: '' })
const emit = defineEmits<{ project: [name: string]; insert: [field: ContextSourceField]; select: [source: string]; explore: [] }>()
const galleryOpen = ref(false), catalog = ref<ContextSourceEntry[]>([]), loading = ref(false), catalogError = ref(''), error = ref(''), notice = ref('')
const expanded = ref<Record<string, boolean>>({}), aliases = ref<Record<string, string>>({}), pendingRemoval = ref(''), typeName = ref(''), capabilityName = ref('')
let catalogIntent = 0
const requirements = computed(() => Object.entries(props.draft.strategy.requirements).sort(([, first], [, second]) => contextSourceOrder(first) - contextSourceOrder(second)))
const entries = computed(() => {
  const values = [...catalog.value]
  for (const name of Object.keys(props.draft.types)) {
    values.push({ id: `draft:${name}`, label: name, category: 'custom', type: { kind: 'named', name }, types: props.draft.types, origin: 'Types du brouillon', providers: [] })
  }
  return normalizeContextSourceEntries(values)
})
function metadata(name: string, type: ContextType) {
  const appearance = sourceAppearance(name, type)
  const entry = entries.value.find(item => sameContextType(item.type, type))
  return { ...appearance, label: appearance.builtin ? appearance.label : entry?.label || appearance.label }
}
function icon(type: ContextType) { const kind = sourceAppearance('', type).icon; return kind === 'file' ? FileText : kind === 'message' ? MessageSquare : kind === 'sparkles' ? Sparkles : kind === 'tool' ? Wrench : kind === 'media' ? Image : Braces }
function fieldCount(type: ContextType) {
  const resolved = resolveSourceType(type, props.draft.types)
  const fields = (type: Extract<ContextType, { kind: 'record' }>) => { const count = Object.keys(type.fields).length; return `${count} champ${count === 1 ? '' : 's'}` }
  if (resolved.kind === 'record') return fields(resolved)
  if (resolved.kind === 'list') { const item = resolveSourceType(resolved.item, props.draft.types); return item.kind === 'record' ? `Liste · ${fields(item)} / élément` : typeLabel(resolved) }
  return typeLabel(resolved)
}
function uses(name: string) { return sourceUseCount(props.draft.strategy.program, name) }
function rootField(name: string, type: ContextType) { return createContextSourceField(name, [], type, type) }
function insert(field: ContextSourceField) { emit('select', field.source); emit('insert', field) }
async function refresh() {
  const intent = ++catalogIntent
  loading.value = true; catalogError.value = ''
  try {
    const result = await client.context.sourceTypes({workspaceId:props.workspaceId})
    if (intent !== catalogIntent) return
    catalog.value = Array.isArray(result.entries) ? result.entries : []
    if (result.diagnostics?.length) catalogError.value = result.diagnostics.map(item => item.message).join(' · ')
  } catch (cause) { if (intent === catalogIntent) catalogError.value = cause instanceof Error ? cause.message : String(cause) }
  finally { if (intent === catalogIntent) loading.value = false }
}
function uniqueAlias(value: string) {
  const base = value.replace(/^Zedflow[.:/]/, '').replace(/([a-z])([A-Z])/g, '$1_$2').normalize('NFD').replace(/[\u0300-\u036f]/g, '').replace(/[^a-zA-Z0-9_]/g, '_').replace(/^_+|_+$/g, '').toLowerCase() || 'source'
  let name = base, suffix = 2
  while (Object.hasOwn(props.draft.strategy.requirements, name)) name = `${base}_${suffix++}`
  return name
}
function addEntries(selected: ContextSourceEntry[]) {
  const registry = { ...props.draft.types }, additions: { name: string; type: ContextType }[] = []
  const occupied = new Set(Object.keys(props.draft.strategy.requirements))
  for (const entry of selected) {
    for (const [name, type] of Object.entries(contextTypeDependencies(entry.type, entry.types))) {
      if (registry[name] && !sameContextType(registry[name], type)) { error.value = `Le type ${name} possède déjà un autre schéma dans ce brouillon.`; return }
      registry[name] = cloneContext(type)
    }
    const base = uniqueAlias(entry.alias || (entry.type.kind === 'named' ? entry.type.name : entry.label))
    let name = base, suffix = 2
    while (occupied.has(name)) name = `${base}_${suffix++}`
    occupied.add(name)
    additions.push({ name, type: cloneContext(entry.type) })
  }
  if (props.draft.typesFile && Object.keys(registry).some(name => !props.draft.typesFile?.types?.[name] || !sameContextType(registry[name], props.draft.typesFile.types[name]))) {
    props.draft.typesFile = undefined
    notice.value = 'Les types du catalogue sélectionné sont conservés dans ce brouillon avec les nouveaux types.'
  }
  props.draft.types = registry
  for (const { name, type } of additions) { props.draft.strategy.requirements[name] = type; expanded.value[name] = false }
  error.value = ''
}
function addCustom(name: string, type: ContextType) {
  if (Object.hasOwn(props.draft.types, name)) { error.value = `Le type ${name} existe déjà.`; return }
  addEntries([{ id: `draft:${name}`, label: name, category: 'custom', type: { kind: 'named', name }, types: { ...props.draft.types, [name]: type }, origin: 'Types du brouillon', providers: [] }])
}
function duplicate(name: string, type: ContextType) { const alias = uniqueAlias(name); props.draft.strategy.requirements[alias] = cloneContext(type); expanded.value[alias] = true; aliases.value[alias] = alias; notice.value = `Source ${alias} déclarée avec le même type. Configurez sa valeur séparément.` }
function rename(name: string) {
  const alias = (aliases.value[name] || '').trim()
  if (alias === name) return
  if (!alias || Object.hasOwn(props.draft.strategy.requirements, alias)) { error.value = 'Chaque source doit avoir un alias non vide et unique.'; return }
  const replace = (value: unknown) => {
    if (!value || typeof value !== 'object') return
    if (Array.isArray(value)) { value.forEach(replace); return }
    const item = value as Record<string, unknown>
    if (item.kind === 'resource' && item.name === name) item.name = alias
    if (item.kind !== 'literal') Object.values(item).forEach(replace)
  }
  props.draft.strategy.requirements = Object.fromEntries(Object.entries(props.draft.strategy.requirements).map(([key, type]) => [key === name ? alias : key, type]))
  if (Object.hasOwn(props.draft.resources, name)) { props.draft.resources[alias] = props.draft.resources[name]; delete props.draft.resources[name] }
  for (const fixture of props.draft.fixtures || []) if (Object.hasOwn(fixture.resources, name)) { fixture.resources[alias] = fixture.resources[name]; delete fixture.resources[name] }
  if (props.draft.bindings && Object.hasOwn(props.draft.bindings, name)) { props.draft.bindings[alias] = props.draft.bindings[name]; delete props.draft.bindings[name] }
  replace(props.draft.strategy.program)
  expanded.value[alias] = true; aliases.value[alias] = alias
  delete expanded.value[name]; delete aliases.value[name]
  error.value = ''; notice.value = `Alias renommé en ${alias}. Les références du programme ont été mises à jour.`; emit('select', alias)
}
function remove(name: string) {
  if (uses(name) && pendingRemoval.value !== name) { pendingRemoval.value = name; return }
  delete props.draft.strategy.requirements[name]; delete props.draft.resources[name]
  for (const fixture of props.draft.fixtures || []) delete fixture.resources[name]
  if (props.draft.bindings) delete props.draft.bindings[name]
  pendingRemoval.value = ''; emit('select', '')
}
function hasResource(name: string) { return Object.keys(props.draft.resources).includes(name) }
function supplied(name: string, value: boolean) { if (value) props.draft.resources[name] = defaultContextValue(props.draft.strategy.requirements[name], props.draft.types); else delete props.draft.resources[name] }
function addType() { const name = typeName.value.trim(); if (!name || Object.hasOwn(props.draft.types, name)) { error.value = 'Le nom de type doit être unique.'; return } props.draft.types[name] = { kind: 'record', fields: {} }; typeName.value = ''; error.value = '' }
function addCapability() { const id = capabilityName.value.trim(); if (!id || props.draft.strategy.capabilities.some(item => item.id === id)) { error.value = 'Le nom de capacité doit être unique.'; return } props.draft.strategy.capabilities.push({ id, input: { kind: 'record', fields: {} }, output: { kind: 'text' } }); capabilityName.value = ''; error.value = '' }
watch(galleryOpen, open => { if (open) void refresh() })
watch(() => props.workspaceId, () => { catalogIntent++; catalog.value = []; catalogError.value = ''; if (galleryOpen.value) void refresh() })
</script>
<template>
  <div class="ctx-resources ctx-structured-sources">
    <header class="ctx-sources-header"><div><h2>Sources</h2><p>{{ requirements.length ? `${requirements.length} source${requirements.length > 1 ? 's' : ''} attendue${requirements.length > 1 ? 's' : ''}` : 'Déclarez les types de sources attendus par cette stratégie.' }}</p></div><button v-if="requirements.length" type="button" class="ctx-add-types" @click="galleryOpen = true"><Plus :size="13"/>Ajouter des types</button></header>
    <div v-if="!requirements.length" class="ctx-sources-empty"><FileText :size="28"/><strong>Aucun type déclaré</strong><p>Ajoutez des types de sources pour commencer.</p><button type="button" @click="galleryOpen = true"><Plus :size="14"/>Ajouter des types</button><small>Le programme décide ensuite de leur utilisation.</small></div>
    <div v-else class="ctx-source-drawer" aria-label="Sources attendues">
      <section v-for="[name, type] in requirements" :key="name" class="ctx-declared-source" :class="{ expanded: expanded[name], selected: selectedSource === name }" :style="contextSourceStyle(contextSourceTypeId(type))" :data-resource="name">
        <div class="ctx-declared-heading">
          <button type="button" class="ctx-source-disclosure" draggable="true" :aria-label="`${expanded[name] ? 'Replier' : 'Déplier'} la source ${name}`" :aria-expanded="!!expanded[name]" @dragstart="writeContextSourceDrag($event, rootField(name, type)); emit('select', name)" @click="expanded[name] = !expanded[name]; emit('select', name)"><component :is="icon(type)" :size="19"/><span class="ctx-declared-title"><strong><i/>{{ metadata(name, type).label }}</strong><small>{{ name }} · {{ fieldCount(type) }}</small></span><span class="ctx-source-uses">{{ uses(name) ? `${uses(name)} util.` : 'Non utilisé' }}</span><ChevronDown v-if="expanded[name]" :size="13"/><ChevronRight v-else :size="13"/></button>
        </div>
        <div v-if="expanded[name]" class="ctx-declared-content">
          <ContextSourceTree :source="name" :type="type" :root-type="type" :types="draft.types" @insert="insert" @select="emit('select', $event)"/>
          <button type="button" class="ctx-project-source" :aria-label="`Ajouter ${name} au programme`" @click="insert(rootField(name, type))"><Plus :size="12"/>Insérer la source entière</button>
          <details class="ctx-source-options"><summary>Alias et configuration</summary><label>Alias de la source<div class="ctx-source-alias"><input :value="aliases[name] ?? name" :aria-label="`Alias de ${name}`" @input="aliases[name] = ($event.target as HTMLInputElement).value" @keydown.enter.prevent="rename(name)"/><button type="button" :disabled="!aliases[name] || aliases[name] === name" @click="rename(name)">Appliquer</button></div></label><ContextTypeEditor v-model="draft.strategy.requirements[name]" :label="`Type de ${name}`" :names="Object.keys(draft.types)"/><div class="ctx-source-option-actions"><button type="button" :aria-label="`Déclarer une autre source de type ${metadata(name, type).label}`" @click="duplicate(name, type)"><Copy :size="12"/>Autre source du même type</button><button type="button" :aria-label="`Retirer la ressource ${name}`" @click="remove(name)"><X :size="12"/>Retirer</button></div><div v-if="pendingRemoval === name" class="ctx-source-remove-confirm"><p>{{ uses(name) }} utilisation(s) resteront à corriger dans le programme.</p><button type="button" @click="pendingRemoval = ''">Conserver</button><button type="button" @click="remove(name)">Retirer la déclaration</button></div></details>
          <ContextSourceExamples :workspace-id="workspaceId" :type="type" :types="draft.types" :value="draft.resources[name]" @select="value => { if (value === undefined) delete draft.resources[name]; else draft.resources[name] = value }"/><details class="ctx-source-options"><summary>Valeur manuelle d’aperçu <span>{{ hasResource(name) ? 'Fournie' : 'Non fournie' }}</span></summary><label class="ctx-checkbox"><input type="checkbox" :checked="hasResource(name)" @change="supplied(name, ($event.target as HTMLInputElement).checked)"/>Fournir {{ name }} pour l’aperçu</label><ContextValueEditor v-if="hasResource(name)" v-model="draft.resources[name]" :type="type" :types="draft.types" :label="`Valeur de ${name}`"/><small v-else>Absente de cet aperçu. Une condition « Est présent » peut traiter son absence.</small><small>Valeur d’essai uniquement. Le flow fournit les données à l’exécution.</small></details>
        </div>
      </section>
    </div>
    <p v-if="requirements.length" class="ctx-sources-help">Glissez une source ou un champ dans le programme, ou sélectionnez un champ avec Entrée.</p>
    <details class="ctx-resource-section ctx-sources-advanced"><summary>Types du brouillon <small>{{ Object.keys(draft.types).length }}</small></summary><p>Ces définitions valident les sources déclarées. Un type présent ici n’est pas ajouté automatiquement au tiroir.</p><p v-if="draft.typesFile">Catalogue {{ draft.typesFile.key }} sélectionné. Son schéma s’édite dans Catalogues de types.</p><fieldset :disabled="!!draft.typesFile"><div v-for="(_, name) in draft.types" :key="name" class="ctx-resource"><div class="ctx-resource-heading"><strong>{{ name }}</strong><button type="button" class="icon-button" :aria-label="`Retirer le type ${name}`" @click="delete draft.types[name]"><X :size="13"/></button></div><ContextTypeEditor v-model="draft.types[name]" :label="`Définition de ${name}`" :names="Object.keys(draft.types)"/></div><div class="ctx-add-field"><input v-model="typeName" aria-label="Nom du type" placeholder="Nouveau type" @keydown.enter.prevent="addType"/><button type="button" aria-label="Ajouter un type nommé" @click="addType"><Plus :size="14"/></button></div></fieldset></details>
    <details class="ctx-resource-section ctx-sources-advanced"><summary>Capacités demandées <small>{{ draft.strategy.capabilities.length }}</small></summary><p>Capacités accordées explicitement par le flow ou un bridge.</p><div v-for="(item, index) in draft.strategy.capabilities" :key="item.id" class="ctx-resource"><div class="ctx-resource-heading"><strong>{{ item.id }}</strong><button type="button" class="icon-button" :aria-label="`Retirer la capacité ${item.id}`" @click="draft.strategy.capabilities.splice(index, 1); draft.grants = draft.grants.filter(id => id !== item.id)"><X :size="13"/></button></div><ContextTypeEditor v-model="item.input" label="Type d’entrée" :names="Object.keys(draft.types)"/><ContextTypeEditor v-model="item.output" label="Type de sortie" :names="Object.keys(draft.types)"/><small>La liaison au flow vérifie cette capacité.</small></div><div class="ctx-add-field"><input v-model="capabilityName" aria-label="Nom de la capacité" placeholder="read" @keydown.enter.prevent="addCapability"/><button type="button" aria-label="Ajouter une capacité" @click="addCapability"><Plus :size="14"/></button></div></details>
    <p v-if="error" role="alert" class="ctx-error">{{ error }}</p><p v-if="notice" role="status" class="ctx-sources-help">{{ notice }}</p>
    <ContextSourceGallery v-model:open="galleryOpen" :entries="entries" :requirements="draft.strategy.requirements" :types="draft.types" :loading="loading" :error="catalogError" @add="addEntries" @custom="addCustom" @retry="refresh" @explore="emit('explore')"/>
  </div>
</template>
<style scoped>
.ctx-structured-sources{display:flex;flex-direction:column;gap:10px;min-width:0}.ctx-sources-header{display:flex;align-items:flex-start;gap:8px;justify-content:space-between;margin-bottom:3px}.ctx-sources-header h2{font-size:13px;font-weight:600;margin:0 0 5px}.ctx-sources-header p{font-size:10px;line-height:1.6;margin:0;color:var(--muted,#a0a0a0)}.ctx-add-types{display:inline-flex;align-items:center;gap:4px;white-space:nowrap;padding:5px 7px;font-size:10px;min-height:28px}.ctx-sources-empty{display:flex;flex-direction:column;align-items:center;justify-content:center;gap:13px;min-height:350px;padding:28px 20px;border:1px dashed var(--border,#35353a);border-radius:7px;text-align:center;color:var(--muted,#a0a0a0)}.ctx-sources-empty>svg{opacity:.55}.ctx-sources-empty strong{font-size:12px;font-weight:500}.ctx-sources-empty p{font-size:11px;line-height:1.6;margin:0!important;max-width:185px}.ctx-sources-empty button{display:flex;align-items:center;gap:6px;margin:6px 0;padding:8px 11px;font-size:11px}.ctx-sources-empty small{font-size:10px;line-height:1.6;max-width:180px}.ctx-source-drawer{display:grid;gap:7px}.ctx-declared-source{min-width:0;border:1px solid var(--border,#35353a);border-left:2px solid var(--ctx-source-color);border-radius:6px;background:var(--surface,#1c1c21);overflow:hidden}.ctx-declared-source.selected,.ctx-declared-source.expanded{border-color:color-mix(in srgb,var(--ctx-source-color) 60%,var(--border,#333))}.ctx-source-disclosure{display:flex;align-items:center;width:100%;gap:9px;text-align:left;min-height:59px;border:0;border-radius:0;background:none;padding:10px 9px;cursor:pointer}.ctx-source-disclosure:hover{background:color-mix(in srgb,var(--ctx-source-color) 5%,transparent)}.ctx-source-disclosure>svg:first-child{color:var(--ctx-source-color);flex-shrink:0}.ctx-source-disclosure>svg:last-child{color:var(--muted,#a0a0a0);flex-shrink:0}.ctx-declared-title{display:grid;gap:5px;min-width:0;flex:1}.ctx-declared-title strong{display:flex;align-items:center;gap:7px;font-size:11px;font-weight:550;line-height:1.25}.ctx-declared-title i{display:block;width:7px;height:7px;border-radius:50%;background:var(--ctx-source-color);flex-shrink:0}.ctx-declared-title small{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;color:var(--muted,#a0a0a0);font-size:9px;line-height:1.3}.ctx-source-uses{font-size:9px;color:var(--muted,#a0a0a0);white-space:nowrap}.ctx-declared-content{display:grid;gap:9px;padding:0 9px 10px}.ctx-project-source{display:flex;align-items:center;justify-content:center;gap:5px;width:100%;font-size:10px;padding:5px;border:1px dashed var(--border,#3a3a3f);background:none;color:var(--muted,#aaa)}.ctx-project-source:hover{color:var(--ctx-source-color);border-color:var(--ctx-source-color)}.ctx-source-options{border-top:1px solid var(--border,#333);padding-top:7px;font-size:10px;color:var(--muted,#a0a0a0)}.ctx-source-options summary{cursor:pointer;line-height:1.6}.ctx-source-options summary span{float:right;font-size:9px}.ctx-source-options>label,.ctx-source-options>small{display:grid;gap:5px;margin-top:9px}.ctx-source-options small{font-size:10px;line-height:1.6}.ctx-source-alias{display:flex;gap:4px;min-width:0}.ctx-source-alias input{width:100%;min-width:0;padding:5px 6px;font-size:11px}.ctx-source-alias button{font-size:10px;padding:4px 6px}.ctx-source-option-actions{display:flex;gap:5px;flex-wrap:wrap;margin-top:8px}.ctx-source-option-actions button{display:inline-flex;align-items:center;gap:4px;padding:5px;font-size:9px}.ctx-source-remove-confirm p{font-size:10px;color:var(--danger,#e5a19a)}.ctx-source-remove-confirm button{font-size:9px;margin-right:5px}.ctx-sources-help{font-size:10px!important;line-height:1.7!important;color:var(--muted,#a0a0a0);margin:0!important}.ctx-sources-advanced{margin:0!important;border-top:1px solid var(--border,#303035);padding-top:10px;font-size:10px}.ctx-sources-advanced summary{font-size:10px!important}.ctx-sources-advanced>p{font-size:10px!important;line-height:1.6}.ctx-source-options :deep(label){font-size:10px}.ctx-source-options :deep(input),.ctx-source-options :deep(select),.ctx-source-options :deep(textarea){font-size:11px;min-width:0}.ctx-source-options :deep(fieldset){min-width:0}.ctx-source-options :deep(.ctx-checkbox){display:flex}
</style>
