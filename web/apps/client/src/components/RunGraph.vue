<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, shallowRef, watch } from 'vue'
import { VueFlow, type VueFlowStore } from '@vue-flow/core'
import { Background } from '@vue-flow/background'
import { Controls } from '@vue-flow/controls'
import type { Composition, NodeActivity, Run } from '@zedflow/sdk'
import { passagesByNode } from '../runIndexes'
import FlowNode from './FlowNode.vue'
import GraphRouting from './GraphRouting.vue'
import OrthogonalEdge from './OrthogonalEdge.vue'
import AppDialog from './AppDialog.vue'
import { useRunDetails } from '../composables/runDetails'
import { useGraphAnalysis } from '../graph/contracts'

import type { RoutedEdge } from '../graph/routing'
import { rememberViewport, recalledViewport } from '../graph/viewportMemory'
import '../graph.css'

const props = defineProps<{ composition: Composition; run?: Run | null; selectedPath?: string; selectedOccurrence?:string; focusRevision?: number; expanded?: boolean; pathPrefix?:string }>()
const emit = defineEmits<{ inspect: [selection: string|{nodePath:string;occurrenceId:string}] }>()
const scope = ref('')
const routes = shallowRef<Record<string,RoutedEdge>>({}), routingError = ref('')
const viewport = shallowRef<VueFlowStore>()
let pendingReveal = false
watch(() => props.composition.id, () => { scope.value = '' })
watch(() => [props.selectedPath, props.focusRevision, props.pathPrefix], async ([, revision], previous) => {
  const previousRevision=previous?.[1]
  if (!props.selectedPath) return
  const local=props.pathPrefix&&props.selectedPath.startsWith(`${props.pathPrefix}/`)?props.selectedPath.slice(props.pathPrefix.length+1):props.selectedPath
  const destination = local.split('/').slice(0, -1).join('/')
  pendingReveal = pendingReveal || destination !== scope.value || revision !== previousRevision
  scope.value = destination
  if (!pendingReveal) return
  await nextTick()
  await revealSelected()
},{immediate:true})
const scopes = computed(() => {
  const result = [{ path: '', label: props.composition.name, composition: props.composition }]
  let parent = props.composition, path = ''
  for (const id of scope.value.split('/').filter(Boolean)) {
    const node = parent.nodes.find(value => value.id === id)
    if (node?.data.kind !== 'subgraph' || !node.data.config.composition) break
    path = path ? `${path}/${id}` : id; parent = node.data.config.composition
    result.push({ path, label: node.data.label, composition: parent })
  }
  return result
})
const currentScope = computed(() => scopes.value.at(-1)!)
const {analysis:contracts}=useGraphAnalysis(computed(()=>currentScope.value.composition))
const details=useRunDetails(),edgeOpen=ref(false),selectedEdge=ref(''),boundaryOccurrence=ref('')
const inspectedEdge=computed(()=>currentScope.value.composition.edges.find(edge=>edge.id===selectedEdge.value))
const targetPath=computed(()=>inspectedEdge.value?fullPath(inspectedEdge.value.target):'')
const edgePassages=computed(()=>passagesByNode(props.run?.activities).get(targetPath.value)||[])
const edgeContract=computed(()=>contracts.value?.edges?.find(edge=>edge.edgeId===selectedEdge.value))
const boundary=computed(()=>details.entry({...details.scope(),kind:'boundaries',id:boundaryOccurrence.value}))
watch(()=>[props.run?.id,currentScope.value.path],()=>{edgeOpen.value=false;selectedEdge.value='';boundaryOccurrence.value=''})
function selectBoundary(id:string){boundaryOccurrence.value=id;const pass=edgePassages.value.find(pass=>pass.occurrenceId===id);if(pass)emit('inspect',{nodePath:pass.path||pass.node,occurrenceId:id})}
function inspectEdge(id:string){selectedEdge.value=id;edgeOpen.value=true;const selected=edgePassages.value.find(pass=>pass.occurrenceId===props.selectedOccurrence)||edgePassages.value.at(-1);if(selected)selectBoundary(selected.occurrenceId);else boundaryOccurrence.value=''}
watch(()=>[edgeOpen.value,boundaryOccurrence.value],()=>{if(edgeOpen.value&&boundaryOccurrence.value)void details.load({...details.scope(),kind:'boundaries',id:boundaryOccurrence.value}).catch(()=>{})})
const graphId = computed(() => `run-graph-${props.run?.id || props.composition.id}-${props.pathPrefix||''}-${currentScope.value.path}`)
watch(graphId,()=>{if(viewport.value)rememberViewport(`${viewport.value.id}:${!!props.expanded}`,viewport.value.viewport.value);routes.value={};routingError.value=''})
let cameraFrame=0, sizeFrame=0
watch(()=>props.expanded,async(value,previous)=>{
  const store=viewport.value
  if(!store)return
  const id=graphId.value
  rememberViewport(`${id}:${!!previous}`,store.viewport.value)
  cancelAnimationFrame(cameraFrame);cancelAnimationFrame(sizeFrame)
  await nextTick()
  // Vue Flow measures its new container through ResizeObserver before fitting.
  sizeFrame=requestAnimationFrame(()=>{cameraFrame=requestAnimationFrame(()=>{
    if(viewport.value?.id!==id||props.expanded!==value)return
    const camera=recalledViewport(`${id}:${!!value}`)
    if(camera)void store.setViewport(camera,{duration:0})
    else void store.fitView({padding:0.2,maxZoom:1,duration:0})
  })})
})
onBeforeUnmount(()=>{if(viewport.value)rememberViewport(`${viewport.value.id}:${!!props.expanded}`,viewport.value.viewport.value);cancelAnimationFrame(cameraFrame);cancelAnimationFrame(sizeFrame)})
async function revealSelected() {
  const store = viewport.value, id = props.selectedPath?.split('/').at(-1)
  if (!pendingReveal || !id || store?.id !== graphId.value) return
  const node = store.findNode(id)
  if (!node?.dimensions.width || !node.dimensions.height) return
  pendingReveal = false
  await store.fitView({ nodes: [id], padding: 0.3, maxZoom: 1, duration: 0 })
}
function ready(store: VueFlowStore) { viewport.value = store;const camera=recalledViewport(`${store.id}:${!!props.expanded}`);if(camera)void nextTick(()=>store.setViewport(camera,{duration:0}));void revealSelected() }
function fullPath(id: string) { return [props.pathPrefix,currentScope.value.path,id].filter(Boolean).join('/') }
function inspect(id: string) { emit('inspect', fullPath(id)) }
function enter(id: string) { if (currentScope.value.composition.nodes.find(node => node.id === id)?.data.kind === 'subgraph') scope.value = [currentScope.value.path,id].filter(Boolean).join('/') }
const nodes = computed(() => currentScope.value.composition.nodes.map(node => {
  const path = fullPath(node.id)
  const occurrences = passagesByNode(props.run?.activities).get(path)||[]
  const latest = occurrences.at(-1)
  // Several occurrences or descendants can be active at once. Keep the owning
  // subgraph visible while a child executes, without inventing child nodes.
  const isInside = (candidate: string) => candidate === path || candidate.startsWith(`${path}/`)
  const running = (props.run?.activeNodes || []).some(isInside)
  const waiting = props.run?.wait && isInside(props.run.wait.nodePath || props.run.wait.node)
  const status: NodeActivity['status'] | undefined = running ? 'running'
    : waiting ? 'waiting'
    : latest?.status
  return {
    ...node,
    selected: props.selectedPath === path,
    data: {
      ...node.data,
      active: status === 'running',
      executionStatus: status,
      occurrences: occurrences.length,
    },
  }
}))
</script>

<template>
  <div class="run-graph-view"><div v-if="scopes.length>1" class="graph-breadcrumbs"><button v-for="entry in scopes" :key="entry.path" @click="scope=entry.path">{{entry.label}}</button></div><VueFlow
    :id="graphId"
    :key="graphId"
    :nodes="nodes"
    :edges="currentScope.composition.edges"
    :fit-view-on-init="!recalledViewport(`${graphId}:${!!expanded}`)"
    :min-zoom="0.2"
    :max-zoom="2"
    :nodes-draggable="false"
    :nodes-connectable="false"
    :delete-key-code="null"
    @node-click="inspect($event.node.id)"
    @edge-click="inspectEdge($event.edge.id)"
    @node-double-click="enter($event.node.id)"
    @init="ready"
    @nodes-initialized="revealSelected"
  >
    <GraphRouting @routes="routes=$event" @error="routingError=$event"/>
    <Controls :show-interactive="false" />
    <Background pattern-color="#353535" :gap="22" />
    <template #node-flow="nodeProps"><FlowNode :contract="contracts?.nodes?.find(node=>node.nodeId===nodeProps.id)" v-bind="nodeProps" :format-version="currentScope.composition.formatVersion" readonly @attachment="inspect(nodeProps.id)"/></template>
    <template #edge-default="edgeProps"><OrthogonalEdge v-bind="edgeProps" :route="routes[edgeProps.id]"/></template>
  </VueFlow><p v-if="routingError" class="graph-routing-error" role="alert">{{routingError}}</p>
  <AppDialog v-model:open="edgeOpen" title="Données à l’arrivée du lien" wide>
    <p>{{inspectedEdge?.source}} → {{inspectedEdge?.target}}</p>
    <p v-if="edgeContract?.junction">Jonction : l’état du passage réunit les apports des branches exécutées.</p>
    <p>Canaux déclarés par la source : {{edgeContract?.produces.join(', ')||'selon sa configuration'}}.</p>
    <p>Canaux consommés par la cible : {{edgeContract?.consumes.join(', ')||'selon sa configuration'}}.</p>
    <label v-if="edgePassages.length">Passage de la cible<select :value="boundaryOccurrence" aria-label="Passage à l’arrivée du lien" @change="selectBoundary(($event.target as HTMLSelectElement).value)"><option v-for="pass in edgePassages" :key="pass.occurrenceId" :value="pass.occurrenceId">{{pass.step}} · {{pass.occurrenceId}} · {{pass.status}}</option></select></label>
    <p v-else>Aucun passage enregistré pour cette cible.</p>
    <p v-if="boundary?.loading">Chargement de l’état capturé…</p><p v-if="boundary?.error" role="alert">{{boundary.error}}</p>
    <template v-if="boundary?.value"><p>État exact au démarrage du passage {{boundaryOccurrence}}. La capture décrit la cible ; elle n’attribue pas chaque canal à ce lien.</p><pre v-if="boundary.value.state!==null" class="source-preview">{{JSON.stringify(boundary.value.state,null,2)}}</pre><p v-else>Cette ancienne occurrence ne conserve pas l’état exact de son NodeContext.</p></template>
  </AppDialog></div>
</template>
