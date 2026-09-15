<script setup lang="ts">
import type { JsonValue } from '@zedflow/sdk'

import { computed } from 'vue'
import { FileText, Image, Braces, User, Sparkles, Wrench, ChevronDown } from 'lucide-vue-next'

import type { ContextItem, ContextTraceEntry, ContextType } from '@zedflow/sdk'
import { sourceAppearance } from '../../contextSources'

const props = withDefaults(defineProps<{ items: ContextItem[]; requirements: Record<string, ContextType>; trace?: ContextTraceEntry[]; selected?: string; occurrence?: string; source?: string; structure?: boolean; depth?: number }>(), { depth: 0 })
const emit = defineEmits<{ select: [id: string]; source: [name: string] }>()
const traceIndex = computed(() => new Map((props.trace || []).map(item => [item.id, item])))
const origin = (id: string) => traceIndex.value.get(id)?.blockId || id
function object(value: JsonValue | undefined): Record<string, JsonValue> { return value && typeof value === 'object' && !Array.isArray(value) ? value : {} }
function pretty(value: unknown) { return typeof value === 'string' ? value : JSON.stringify(value, null, 2) }
function argumentsText(value: JsonValue | undefined) { const compact=JSON.stringify(value); return compact && compact.length<=120 ? compact : pretty(value) }
function sources(item: ContextItem): string[] { return item.kind === 'fragment' ? item.sources : [...new Set(item.items.flatMap(sources))] }
function appearance(name: string) { return sourceAppearance(name, props.requirements[name] || { kind: 'text' }) }
function title(item: Extract<ContextItem, { kind: 'fragment' }>) {
  if (item.format === 'adkMessages') {
    const contents = messages(item.value)
    if (contents.some(message => parts(message).some(part => part.functionResponse || part.function_response || part.name && Object.hasOwn(part, 'args')))) return 'Échange d’outil'
    return props.depth > 0 && contents.length === 1 ? role(contents[0].role) : 'Conversation'
  }
  return item.role === 'instruction' ? 'Instructions' : item.format === 'media' ? 'Média' : item.sources.length === 1 ? appearance(item.sources[0]).label : 'Données composées'
}
function messages(value: JsonValue) { return (Array.isArray(value) ? value : [value]).map(object) }
function parts(message: Record<string, JsonValue>) { return Array.isArray(message.parts) ? message.parts.map(object) : [{ text: message.text || message.content || '' }] }
function toolResult(part: Record<string, JsonValue>) { return object(part.functionResponse || part.function_response) }
function response(part: Record<string, JsonValue>) { return object(toolResult(part).response) }
function responseMetadata(part: Record<string, JsonValue>) { return Object.fromEntries(Object.entries(response(part)).filter(([key]) => key !== 'content' && key !== 'status')) }
function role(value: JsonValue | undefined) { return ({ user: 'Utilisateur', model: 'Assistant', assistant: 'Assistant', function: 'Outil', tool: 'Outil', system: 'Instruction' } as Record<string, string>)[String(value)] || String(value || 'Message') }
function isError(value: JsonValue | undefined) { const data = object(value); return data.isError === true || data.is_error === true || !!data.error || ['error', 'erreur', 'failed', 'failure'].includes(String(data.status)) || data.statut === 'erreur' }
</script>

<template>
  <div class="ctx-result-list" :class="{ nested:depth>0 }">
    <section v-for="(item,index) in items" :key="item.id" class="ctx-result-item" :class="{ selected:occurrence?occurrence===item.id:selected===origin(item.id), related:source&&sources(item).includes(source), group:item.kind==='group' }" :data-fragment-id="item.id" :data-origin-block="origin(item.id)">
      <header class="ctx-result-heading"><span v-if="depth===0" class="ctx-result-number">{{ index+1 }}</span><button class="ctx-result-title" :aria-label="`Voir le bloc du fragment ${item.id}`" @click="emit('select',item.id)"><span class="ctx-source-dot" :style="{background:sources(item).length?appearance(sources(item)[0]).color:'#9999a4'}"/>{{ item.kind==='group'?item.label:title(item) }}</button><span class="ctx-result-sources"><button v-for="name in sources(item)" :key="name" class="ctx-result-chip" :style="{'--ctx-source-color':appearance(name).color}" :title="`Source ${name}`" @click="emit('source',name)"><FileText :size="12"/>{{ name }}</button></span></header>
      <template v-if="item.kind==='group'"><ContextPreviewDocument :items="item.items" :requirements="requirements" :trace="trace" :selected="selected" :occurrence="occurrence" :source="source" :structure="structure" :depth="depth+1" @select="emit('select',$event)" @source="emit('source',$event)"/></template>
      <template v-else>
        <div class="ctx-result-body ctx-preview-fragment" :class="{selected:occurrence?occurrence===item.id:selected===origin(item.id)}" role="button" tabindex="0" :aria-label="`Inspecter le contenu ${item.id}`" @click="emit('select',item.id)" @keydown.enter="emit('select',item.id)">
          <div v-if="structure" class="ctx-result-structure"><span>{{ item.role==='instruction'?'Instruction':'Donnée' }} · {{ item.format }}</span><code>{{ item.id }}</code><pre>{{ pretty(item.value) }}</pre></div>
          <template v-else-if="item.format==='adkMessages'">
            <div v-for="(message,mi) in messages(item.value)" :key="mi" class="ctx-result-message">
              <strong v-if="!(depth>0&&messages(item.value).length===1)&&parts(message).some(part=>typeof part.text==='string')"><User v-if="message.role==='user'" :size="13"/><Sparkles v-else :size="13"/>{{ role(message.role) }}</strong>
              <template v-for="(part,pi) in parts(message)" :key="pi">
                <pre v-if="typeof part.text==='string'" class="ctx-result-text">{{ part.text }}</pre>
                <details v-else-if="typeof part.thinking==='string'" @click.stop><summary>Raisonnement transmis</summary><pre>{{ part.thinking }}</pre></details>
                <div v-else-if="part.name&&Object.hasOwn(part,'args')" class="ctx-result-tool" :data-tool-call-id="String(part.id||'')"><strong><Wrench :size="13"/>Appel {{ part.name }}<span v-if="part.id"> · #{{ part.id }}</span></strong><pre>{{ argumentsText(part.args) }}</pre></div>
                <div v-else-if="part.functionResponse||part.function_response" class="ctx-result-tool" :class="{'tool-error':isError(toolResult(part).response)}" :data-tool-result-id="String(part.id||'')"><strong><FileText :size="13"/>Résultat {{ toolResult(part).name }}<span v-if="part.id"> · #{{ part.id }}</span><small v-if="response(part).status" class="ctx-tool-status">{{ response(part).status }}</small></strong><template v-if="typeof response(part).content==='string'"><pre class="ctx-result-text">{{ response(part).content }}</pre><details v-if="Object.keys(responseMetadata(part)).length" @click.stop><summary>Autres champs du résultat</summary><pre>{{ pretty(responseMetadata(part)) }}</pre></details></template><pre v-else>{{ pretty(toolResult(part).response) }}</pre></div>
                <div v-else-if="part.mime_type||part.file_uri" class="ctx-result-media"><Image :size="18"/><span>{{ part.mime_type || 'Média' }}</span><code>{{ part.file_uri || part.uri || 'Contenu binaire intégré' }}</code></div>
                <pre v-else>{{ pretty(part) }}</pre>
              </template>
            </div>
          </template>
          <div v-else-if="item.format==='media'" class="ctx-result-media"><Image :size="22"/><span>{{ object(item.value).mediaType }}</span><code>{{ object(item.value).contentRef }}</code><small>Référence transmise au modèle. Aucun téléchargement dans cet aperçu.</small></div>
          <pre v-else :class="{'ctx-result-text':item.format==='text'}">{{ pretty(item.value) }}</pre>
        </div>
        <footer v-if="structure||traceIndex.get(item.id)?.iterations.length"><button @click="emit('select',item.id)"><ChevronDown :size="12"/>Voir le bloc source</button><small v-if="traceIndex.get(item.id)?.iterations.length">{{ traceIndex.get(item.id)!.iterations.map(item=>`itération ${item.index+1}`).join(' · ') }}</small><span v-else-if="structure"><Braces :size="12"/>{{ sources(item).length }} source(s)</span></footer>
      </template>
    </section>
  </div>
</template>

<style scoped>
.ctx-result-list{display:flex;flex-direction:column;gap:10px;min-width:0}.ctx-result-item{border:1px solid #34373e;border-radius:6px;background:#1d1f23;min-width:0;overflow:hidden}.ctx-result-item.selected{border-color:#939ba9}.ctx-result-item.related{outline:1px solid #718a9e;outline-offset:1px}.ctx-result-heading{display:flex;align-items:center;gap:8px;padding:9px 10px;min-width:0;flex-wrap:wrap}.ctx-result-number{display:flex;width:23px;height:25px;align-items:center;justify-content:center;border-radius:4px;background:#2b2d32;font-size:12px;flex:none}.ctx-result-title{display:flex;align-items:center;gap:8px;min-width:0;flex:1;background:transparent;border:0;padding:0;text-align:left;font-size:12px;font-weight:550;color:#e5e7eb}.ctx-source-dot{width:8px;height:8px;border-radius:50%;flex:none}.ctx-result-sources{display:flex;gap:4px;flex-wrap:wrap;max-width:100%}.ctx-result-chip{display:inline-flex;align-items:center;gap:5px;border:1px solid color-mix(in srgb,var(--ctx-source-color) 42%,#31353b);color:var(--ctx-source-color);background:color-mix(in srgb,var(--ctx-source-color) 7%,#1d1f23);padding:4px 6px;border-radius:4px;max-width:100%;overflow:hidden;text-overflow:ellipsis;font-size:10px!important}.ctx-result-body{padding:0 12px 10px 40px;border:0;background:transparent!important;border-radius:0;cursor:pointer;display:block}.ctx-result-body:focus-visible{outline:2px solid #a3b4c8;outline-offset:-2px}.ctx-result-body pre{margin:0;white-space:pre-wrap;overflow-wrap:anywhere;font-size:11px;line-height:1.65;max-height:360px;overflow:auto;word-break:break-word}.ctx-result-body .ctx-result-text{font-family:inherit;font-size:12px}.ctx-result-body>.ctx-result-text{border:1px solid #34373e;padding:9px;border-radius:5px}.ctx-result-message{display:flex;flex-direction:column;gap:9px;padding:10px 0}.ctx-result-message+.ctx-result-message{border-top:1px solid #34373e}.ctx-result-message>strong,.ctx-result-tool>strong{font-size:11px;display:flex;align-items:center;gap:6px;font-weight:500}.ctx-result-tool{display:flex;flex-direction:column;gap:8px;padding:9px;border:1px solid #3b3e46;border-radius:4px}.ctx-result-tool.tool-error>pre{color:#dfa59a}.ctx-tool-status{font-size:10px;color:#a6afa8;margin-left:auto}.tool-error .ctx-tool-status{color:#dfa59a}.ctx-result-tool details{font-size:11px}.ctx-result-media{display:flex;flex-direction:column;gap:8px;font-size:12px}.ctx-result-media code{font-size:10px;overflow-wrap:anywhere}.ctx-result-structure{display:flex;flex-direction:column;gap:8px}.ctx-result-structure>span,.ctx-result-structure>code{font-size:10px;color:#b0b5bf}.ctx-result-item>footer{display:flex;justify-content:space-between;align-items:center;padding:0 12px 9px 40px;gap:8px}.ctx-result-item>footer button{display:flex;align-items:center;gap:5px;border:0;background:transparent;font-size:10px;color:#afb8c6;padding:0}.ctx-result-item>footer small,.ctx-result-item>footer span{font-size:10px;color:#9199a7}.ctx-result-list.nested{padding:0 10px 10px 32px;gap:6px}.nested>.ctx-result-item{background:transparent;border:0;border-top:1px solid #34373e;border-radius:0}.nested .ctx-result-heading{padding:9px 0}.nested .ctx-result-body,.nested .ctx-result-item>footer{padding-left:0}.nested .ctx-result-list.nested{padding-left:12px}@media(max-width:600px){.ctx-result-body{padding-left:12px}.ctx-result-item>footer{padding-left:12px}.ctx-result-heading{gap:6px}.ctx-result-sources{width:100%;padding-left:30px}}
</style>
