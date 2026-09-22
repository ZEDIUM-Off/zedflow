<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, provide, reactive, ref, shallowRef, watch } from 'vue'
import { Code2, Copy, Play, Save, RefreshCw, PanelLeft, PanelRight } from 'lucide-vue-next'
import AppDialog from '../AppDialog.vue'
import ContextBlockEditor from './ContextBlockEditor.vue'
import ContextResources from './ContextResources.vue'
import ContextGuide from './ContextGuide.vue'
import ContextFixtures from './ContextFixtures.vue'
import ContextStudioPreview from './ContextStudioPreview.vue'
import ContextBlockMenu from './ContextBlockMenu.vue'
import ContextToolExchangeDialog from './ContextToolExchangeDialog.vue'
import { CONTEXT_SOCKET_SELECTION, type ContextSocketTarget } from './contextComposer'
import { useContextPanels } from './useContextPanels'
import { type ContextSourceField, expressionResources, resolveSourceType, CONTEXT_SOURCE_MIME, readContextSourceDrag, contextSourceFields, sameContextType } from '../../contextSources'
import ContextLibrarySelection from './ContextLibrarySelection.vue'
import ContextFunctionLibrary from './ContextFunctionLibrary.vue'
import ContextTypesLibrary from './ContextTypesLibrary.vue'
import ContextTypesSelection from './ContextTypesSelection.vue'
import { contextId, findContextBlock, newContextBlock, type ContextStudioController } from '../../contextEngine'
import type { ContextBlock, ContextTypesFile } from '@zedflow/sdk'
import './context-studio.css'
const props=defineProps<{studio:ContextStudioController;active:boolean}>()
const emit=defineEmits<{use:[selection:{key:string;hash:string}];navigate:[id:string]}>()
const draft=computed(()=>props.studio.current)
const conversionOpen=ref(false),conversionFormats=ref<Record<string,'json'|'adkMessages'>>({})
const sourceOpen=ref(false),reloadOpen=ref(false),sourcesOpen=ref(true),previewOpen=ref(true)
const libraryMode=ref(false),libraryRevision=ref(0),typesMode=ref(false),typesRevision=ref(0)
const guideOpen=ref(false)
const mobilePanel=ref<'sources'|'program'|'preview'>('program')
const programRoot=ref<HTMLElement>()
const layout=ref<HTMLElement>()
const panels=useContextPanels(layout)
const selectedSource=ref(''), insertionOpen=ref(false), insertionField=shallowRef<ContextSourceField>()
const sourceOverProgram=ref(false),exchangeOpen=ref(false)
const sockets=reactive(new Map<string,ContextSocketTarget>()), selectedSocket=shallowRef<ContextSocketTarget>()
provide(CONTEXT_SOCKET_SELECTION,{selectedId:computed(()=>selectedSocket.value?.id||null),register:target=>{sockets.set(target.id,target)},select:target=>{selectedSocket.value=target},clear:id=>{sockets.delete(id);if(selectedSocket.value?.id===id)selectedSocket.value=undefined}})
const compatibleSockets=computed(()=>insertionField.value?[...sockets.values()].filter(target=>target.accepts(insertionField.value!)):[])
const invalid=computed(()=>!!draft.value.file&&!draft.value.file.strategy)
const sourceStale=computed(()=>draft.value.sourceSignature!==JSON.stringify(draft.value.strategy))
const jsonBlocks=computed(()=>{function visit(blocks:ContextBlock[]):Extract<ContextBlock,{kind:'emit'}>[] {return blocks.flatMap(block=>block.kind==='emit'?(block.format==='json'?[block]:[]):block.kind==='group'||block.kind==='forEach'?visit(block.items):[...visit(block.then),...visit(block.else)])}return visit(draft.value.strategy.program)})
function conversionChoice(id:string,format:string){if(format)conversionFormats.value[id]=format as 'json'|'adkMessages';else delete conversionFormats.value[id]}
async function convert(){if(await props.studio.convert(conversionFormats.value))conversionOpen.value=false}
async function selectBlock(id:string){
  const occurrence=id
  id=draft.value.preview?.evaluation.trace?.find(entry=>entry.id===id)?.blockId||id
  if(!findContextBlock(draft.value.strategy.program,id)){draft.value.notice='Ce bloc appartient à un ancien aperçu. Recalculez l’aperçu pour retrouver la définition courante.';return}
  draft.value.selectedBlock=id;draft.value.selectedPreviewItem=occurrence;mobilePanel.value='program';emit('navigate',id)
  await nextTick();document.getElementById(`context-block-${id}`)?.scrollIntoView({behavior:'smooth',block:'center'})
}
async function selectSource(name:string){selectedSource.value=name;mobilePanel.value='sources';sourcesOpen.value=true;await nextTick();layout.value?.querySelector(`[data-resource="${CSS.escape(name)}"]`)?.scrollIntoView({block:'nearest',behavior:'smooth'})}
function insertField(field:ContextSourceField){selectedSource.value=field.source;if(selectedSocket.value?.accepts(field)&&selectedSocket.value.insert(field)){mobilePanel.value='program';return}insertionField.value=field;insertionOpen.value=true}
function insertAt(target:ContextSocketTarget){if(insertionField.value&&target.insert(insertionField.value)){insertionOpen.value=false;mobilePanel.value='program'}}
async function emitField(field=insertionField.value){if(!field)return;const type=resolveSourceType(field.type,draft.value.types);const block:ContextBlock={kind:'emit',id:contextId('fragment'),role:'data',format:type.kind==='text'?'text':type.kind==='media'?'media':'json',value:field.expression};draft.value.strategy.program.push(block);insertionOpen.value=false;await selectBlock(block.id)}
function dragSource(event:DragEvent){if(!event.dataTransfer?.types.includes(CONTEXT_SOURCE_MIME)||draft.value.pending)return;event.preventDefault();event.dataTransfer.dropEffect='copy';sourceOverProgram.value=true}
function finishSourceDrag(){sourceOverProgram.value=false}
onMounted(()=>{document.addEventListener('drop',finishSourceDrag,true);document.addEventListener('dragend',finishSourceDrag,true)})
onUnmounted(()=>{document.removeEventListener('drop',finishSourceDrag,true);document.removeEventListener('dragend',finishSourceDrag,true)})
function dropSource(event:DragEvent){sourceOverProgram.value=false;const field=readContextSourceDrag(event);if(!field||draft.value.pending)return;event.preventDefault();event.stopPropagation();const type=draft.value.strategy.requirements[field.source];const live=type&&contextSourceFields(field.source,type,draft.value.types).find(item=>JSON.stringify(item.path)===JSON.stringify(field.path));if(!live){draft.value.notice='Ce champ n’est plus disponible dans les sources de la stratégie.';return}void emitField(live)}
function selectTypes(file?:ContextTypesFile){
  if(!file){draft.value.typesFile=undefined;return}
  const entries=Object.entries(file.types||{}),conflicts=entries.filter(([name,type])=>draft.value.types[name]&&!sameContextType(draft.value.types[name],type))
  if(conflicts.length){draft.value.error=`Ce catalogue définit différemment des types déjà utilisés : ${conflicts.map(([name])=>name).join(', ')}. Les définitions de la stratégie sont conservées.`;return}
  draft.value.types={...draft.value.types,...JSON.parse(JSON.stringify(file.types||{}))};draft.value.typesFile=file
}
function addBlock(kind:ContextBlock['kind']){const block=newContextBlock(kind,Object.keys(draft.value.strategy.requirements));draft.value.strategy.program.push(block);void selectBlock(block.id)}
async function projectResource(name:string){
  let type=draft.value.strategy.requirements[name]
  const seen=new Set<string>()
  while(type?.kind==='named'&&!seen.has(type.name)){seen.add(type.name);type=draft.value.types[type.name]}
  const block:ContextBlock={kind:'emit',id:contextId('fragment'),role:'data',format:type?.kind==='text'?'text':type?.kind==='media'?'media':'json',value:{kind:'resource',name}}
  draft.value.strategy.program.push(block)
  await selectBlock(block.id)
}
async function showSource(){sourceOpen.value=true;if(!invalid.value&&(!draft.value.source||sourceStale.value))await props.studio.source()}
async function reload(){if(draft.value.file)await props.studio.open(draft.value.file,true);reloadOpen.value=false}
function downloadSource(){const blob=new Blob([draft.value.source],{type:'text/rust;charset=utf-8'}),url=URL.createObjectURL(blob),anchor=document.createElement('a');anchor.href=url;anchor.download=`${draft.value.strategy.id}.rs`;anchor.click();URL.revokeObjectURL(url)}
async function preview(){previewOpen.value=true;mobilePanel.value='preview';await props.studio.preview()}
function key(event:KeyboardEvent){if(!props.active||libraryMode.value||typesMode.value)return;if((event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==='s'){event.preventDefault();void props.studio.save()}if((event.ctrlKey||event.metaKey)&&event.key==='Enter'){event.preventDefault();void preview()}}
watch(()=>props.active,active=>{if(!active)sourceOpen.value=false})
watch(()=>props.studio.session.selected,()=>{selectedSource.value='';selectedSocket.value=undefined;insertionOpen.value=false})
watch(()=>draft.value.selectedBlock,id=>{const block=findContextBlock(draft.value.strategy.program,id);const sources=expressionResources(block);const consulted=sources.length?sources:draft.value.preview?.evaluation.trace?.find(entry=>entry.blockId===id)?.sources||[];selectedSource.value=consulted.length===1?consulted[0]:''})
watch(()=>props.studio.navigationIntent,()=>{libraryMode.value=false;typesMode.value=false})
</script>
<template>
  <section class="context-studio" aria-label="Studio de contexte" @keydown="key" :data-context-workspace="studio.workspaceId">
    <nav class="ctx-editor-tabs" aria-label="Éditeur de contexte"><button :aria-pressed="!libraryMode&&!typesMode" @click="libraryMode=false;typesMode=false">Stratégie</button><button :aria-pressed="libraryMode" @click="libraryMode=true;typesMode=false">Bibliothèques de fonctions</button><button :aria-pressed="typesMode" @click="typesMode=true;libraryMode=false">Catalogues de types</button></nav>
    <ContextFunctionLibrary v-show="libraryMode" :workspace-id="studio.workspaceId" :active="active&&libraryMode" :types="draft.types" @saved="libraryRevision++"/>
    <ContextTypesLibrary v-show="typesMode" :workspace-id="studio.workspaceId" :active="active&&typesMode" @saved="typesRevision++" @use="selectTypes($event);typesMode=false"/>
    <div v-show="!libraryMode&&!typesMode" class="ctx-strategy-workspace" @focusin="studio.beginEditing()">
    <header class="ctx-toolbar"><div class="ctx-strategy-title"><input v-model="draft.strategy.name" aria-label="Nom de la stratégie" :readonly="invalid||!!draft.pending"/><small :title="draft.file?.path">{{invalid?'Fichier hors format':studio.dirty?'Brouillon non enregistré':'Version enregistrée'}}<template v-if="draft.file"> · {{draft.file.hash.slice(0,10)}}</template></small></div><div class="ctx-toolbar-actions"><button v-if="draft.strategy.version===1" :disabled="!!draft.pending||invalid" @click="conversionFormats={};conversionOpen=true">Créer une copie v2</button><button class="icon-button ctx-desktop-toggle" :aria-pressed="sourcesOpen" aria-label="Afficher les sources du contexte" @click="sourcesOpen=!sourcesOpen"><PanelLeft :size="15"/></button><button class="icon-button ctx-desktop-toggle" :aria-pressed="previewOpen" aria-label="Afficher l’aperçu du contexte" @click="previewOpen=!previewOpen"><PanelRight :size="15"/></button><button :disabled="!!draft.pending" aria-label="Afficher le Rust de la stratégie" title="Source Rust exacte" @click="showSource"><Code2 :size="15"/></button><button :disabled="!!draft.pending||invalid" aria-label="Dupliquer la stratégie" @click="studio.create(true)"><Copy :size="15"/></button><button v-if="draft.file" :disabled="!!draft.pending" aria-label="Recharger la stratégie depuis le disque" @click="reloadOpen=true"><RefreshCw :size="15"/></button><button :disabled="!!draft.pending||invalid||!studio.workspaceId" @click="studio.save()"><Save :size="14"/>Enregistrer la stratégie</button><button class="primary" :disabled="!!draft.pending||invalid||!studio.workspaceId" @click="preview"><Play :size="14"/>Prévisualiser</button></div></header>
    <div v-if="draft.error" role="alert" class="ctx-alert ctx-error">{{draft.error}}<span v-if="draft.conflict"> Votre brouillon est conservé. Rechargez le fichier ou dupliquez la stratégie pour enregistrer une copie.</span></div><div v-if="draft.notice" role="status" class="ctx-alert">{{draft.notice}}</div><div v-if="draft.pending" role="status" class="ctx-progress">{{draft.pending}}…</div>
    <div v-if="invalid" class="ctx-invalid"><h2>Ce fichier Rust ne peut pas être édité visuellement</h2><p>{{draft.file?.path}}</p><p v-for="(item,index) in draft.diagnostics" :key="index">{{item.path}} · {{item.message}}</p><button @click="showSource">Lire la source Rust</button></div>
    <template v-else>
      <ContextGuide v-model:open="guideOpen" :disabled="!!draft.pending" @example="studio.createExample($event);guideOpen=false;mobilePanel='program'"/>
      <nav class="ctx-mobile-tabs" aria-label="Panneaux du studio"><button v-for="tab in (['sources','program','preview'] as const)" :key="tab" :aria-pressed="mobilePanel===tab" @click="mobilePanel=tab">{{{sources:'Sources',program:'Programme',preview:'Aperçu'}[tab]}}</button></nav>
      <div ref="layout" class="ctx-layout ctx-structured-layout" :style="panels.style.value" :class="{'hide-sources':!sourcesOpen,'hide-preview':!previewOpen}" :data-mobile-panel="mobilePanel" :data-selected-source="selectedSource">
        <aside class="ctx-source-panel" :class="{'desktop-hidden':!sourcesOpen}">
          <fieldset :disabled="!!draft.pending">
            <ContextResources :key="`${studio.workspaceId}/${studio.session.selected}`" :draft="draft" :workspace-id="studio.workspaceId" :selected-source="selectedSource" @project="projectResource" @insert="insertField" @select="selectedSource=$event" @explore="typesMode=true;libraryMode=false"/>
            <ContextFixtures :draft="draft"/>
            <details class="ctx-studio-dependencies"><summary>Bibliothèques et catalogues</summary>
              <ContextTypesSelection :active="active&&!libraryMode&&!typesMode" :workspace-id="studio.workspaceId" :revision="typesRevision" :file="draft.typesFile" @select="selectTypes($event)" @edit="typesMode=true;libraryMode=false"/>
              <ContextLibrarySelection :active="active&&!libraryMode&&!typesMode" :workspace-id="studio.workspaceId" :revision="libraryRevision" :file="draft.libraryFile" @select="draft.libraryFile=$event;draft.library=$event?.library||{projections:{},subprograms:{}}" @edit="libraryMode=true"/>
            </details>
          </fieldset>
        </aside>
        <div v-if="sourcesOpen" class="ctx-panel-resizer source-resizer" role="separator" tabindex="0" aria-label="Largeur du panneau Sources" aria-orientation="vertical" :aria-valuenow="Math.round(panels.sizes.value.source)" :aria-valuemin="230" :aria-valuemax="440" @pointerdown="panels.pointer($event,'source')" @keydown="panels.keyboard($event,'source')"/>
        <div ref="programRoot" class="ctx-program-panel" :class="{'source-drag-over':sourceOverProgram}" @dragover="dragSource" @drop="dropSource" @dragleave.self="sourceOverProgram=false" @dragend="sourceOverProgram=false">
          <header class="ctx-panel-heading"><div><h2>Programme</h2><p>Construisez la logique de traitement en ajoutant des blocs séquentiels.</p></div><ContextBlockMenu :version="draft.strategy.version" @add="addBlock" @exchange="exchangeOpen=true"/></header>
          <fieldset :disabled="!!draft.pending"><ContextBlockEditor :key="`${studio.workspaceId}/${studio.session.selected}`" v-model="draft.strategy.program" :version="draft.strategy.version" :resources="draft.strategy.requirements" :types="draft.types" :selected="draft.selectedBlock" :trace="studio.previewStale?[]:draft.preview?.evaluation.trace" @select="draft.selectedBlock=$event;draft.selectedPreviewItem=undefined"/></fieldset>
          <details class="ctx-identity ctx-program-identity"><summary>Identité et raccourcis</summary><label>Identifiant de la stratégie<input v-model="draft.strategy.id" :readonly="!!draft.file" pattern="[a-zA-Z0-9_-]+"/></label><small>Alt + ↑ / ↓ sur le titre d’un bloc pour le déplacer. Suppr pour le retirer. Ctrl + S pour enregistrer.</small></details>
        </div>
        <div v-if="previewOpen" class="ctx-panel-resizer preview-resizer" role="separator" tabindex="0" aria-label="Largeur du panneau Aperçu" aria-orientation="vertical" :aria-valuenow="Math.round(panels.sizes.value.preview)" :aria-valuemin="280" :aria-valuemax="600" @pointerdown="panels.pointer($event,'preview')" @keydown="panels.keyboard($event,'preview')"/>
        <aside class="ctx-preview-panel" :class="{'desktop-hidden':!previewOpen}"><ContextStudioPreview :draft="draft" :stale="studio.previewStale" :selected-source="selectedSource" @select="selectBlock" @source="selectSource" @preview="preview"/></aside>
      </div>
    </template>
    </div>
    <ContextToolExchangeDialog v-model:open="exchangeOpen" :resources="draft.strategy.requirements" :types="draft.types" @add="draft.strategy.program.push($event);selectBlock($event.id)"/>
    <AppDialog v-model:open="insertionOpen" title="Insérer un champ dans le programme" :description="insertionField?.label">
      <p>Choisissez un emplacement compatible, ou ajoutez un fragment au contexte.</p>
      <div class="ctx-insertion-targets"><button v-for="target in compatibleSockets" :key="target.id" @click="insertAt(target)">{{ target.label }}</button></div>
      <p v-if="!compatibleSockets.length" class="ctx-hint">Aucun emplacement compatible n’est ouvert. Dépliez les paramètres du bloc à modifier, puis sélectionnez son emplacement.</p>
      <footer><button @click="emitField()">Ajouter ce champ au contexte</button></footer>
    </AppDialog>
    <AppDialog v-model:open="conversionOpen" title="Créer une copie de contexte v2"><p>La v2 distingue les données JSON des messages ADK. Le fichier v1 et ses occurrences restent conservés.</p><label v-for="block in jsonBlocks" :key="block.id" class="ctx-conversion-choice">{{block.id}}<select :aria-label="`Représentation v2 de ${block.id}`" :value="conversionFormats[block.id]||''" @change="conversionChoice(block.id,($event.target as HTMLSelectElement).value)"><option value="">Selon les liaisons connues</option><option value="json">Conserver comme donnée JSON</option><option value="adkMessages">Messages ADK structurés</option></select></label><p v-if="draft.error" role="alert" class="field-error">{{draft.error}}</p><p v-for="diagnostic in draft.diagnostics" :key="diagnostic.path">{{diagnostic.path}} · {{diagnostic.message}}</p><footer><button :disabled="!!draft.pending" @click="convert">Créer le brouillon v2</button></footer></AppDialog>
    <AppDialog v-model:open="sourceOpen" title="Rust de la stratégie" wide><p v-if="sourceStale&&!invalid" class="ctx-hint">{{draft.pending?'Génération de la source du brouillon…':'Cette source correspond à la dernière version validée.'}}</p><p v-if="draft.error" role="alert" class="ctx-error">{{draft.error}}</p><pre class="source-preview ctx-rust-source">{{draft.source||'Aucune source valide disponible.'}}</pre><footer><small>Le daemon parse les formes Rust structurées sans exécuter le fichier.</small><button :disabled="!draft.source||!!draft.pending" @click="downloadSource">Télécharger le fichier Rust</button></footer></AppDialog>
    <AppDialog v-model:open="reloadOpen" title="Recharger la stratégie"><p>Remplacer ce brouillon par la version présente sur le disque ? Les autres brouillons et la session ouverte sont conservés.</p><footer><button @click="reloadOpen=false">Conserver le brouillon</button><button class="primary" @click="reload">Recharger depuis le disque</button></footer></AppDialog>
  </section>
</template>
