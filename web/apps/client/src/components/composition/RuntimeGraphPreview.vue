<script setup lang="ts">
import type { RuntimeGraphSummary } from '@zedflow/sdk'

import { computed, shallowRef, ref, nextTick, onBeforeUnmount } from 'vue'
import { VueFlow, Handle, Position, type VueFlowStore } from '@vue-flow/core'
import { Background } from '@vue-flow/background'
import { Controls } from '@vue-flow/controls'

import { routeModes } from '../../compositionEngine'

import GraphRouting from '../GraphRouting.vue'
import OrthogonalEdge from '../OrthogonalEdge.vue'
import type { RoutedEdge } from '../../graph/routing'
import { rememberViewport, recalledViewport } from '../../graph/viewportMemory'
const props=defineProps<{overview:RuntimeGraphSummary;viewKey?:string}>()
const emit=defineEmits<{inspect:[path:string]}>()
const routes=shallowRef<Record<string,RoutedEdge>>({}),error=ref('')
const id=computed(()=>`runtime-preview-${props.viewKey||Object.keys(props.overview.instances).join('|')}`)
const viewport=shallowRef<VueFlowStore>()
function ready(store:VueFlowStore){viewport.value=store;const camera=recalledViewport(store.id);if(camera)void nextTick(()=>store.setViewport(camera,{duration:0}))}
onBeforeUnmount(()=>{if(viewport.value)rememberViewport(viewport.value.id,viewport.value.viewport.value)})
const nodes=computed(()=>Object.entries(props.overview.instances).map(([id,item],index)=>({id,type:'runtime',position:{x:(index%3)*340,y:Math.floor(index/3)*200},data:{...item,instance:id,root:id===props.overview.entry.instance}})))
const edges=computed(()=>Object.entries(props.overview.routes).map(([id,route])=>({id,source:route.from.instance,target:route.to.instance,label:id.split('/').at(-1)})))
</script>
<template><section class="runtime-graph-preview" aria-label="Graphe résolu"><div class="runtime-graph-canvas"><VueFlow :id="id" :key="id" :nodes="nodes" :edges="edges" :fit-view-on-init="!recalledViewport(id)" @init="ready" :min-zoom="0.15" :max-zoom="1" :nodes-draggable="false" :nodes-connectable="false" :delete-key-code="null"><Background :gap="20" pattern-color="#35353b"/><Controls :show-interactive="false"/><GraphRouting @routes="routes=$event" @error="error=$event"/><template #node-runtime="node"><div class="runtime-instance" :data-runtime-instance="node.id"><Handle type="target" :position="Position.Left"/><small>{{node.id}}{{node.data.root?' · entrée':''}}</small><strong>{{node.data.name}}</strong><span>{{node.data.interactive?'Peut attendre une réponse':'Autonome'}} · {{node.data.hash.slice(0,8)}}</span><Handle type="source" :position="Position.Right"/></div></template><template #edge-default="edge"><OrthogonalEdge v-bind="edge" :route="routes[edge.id]"/></template></VueFlow></div><p v-if="error" role="alert" class="ctx-error">{{error}}</p><div class="runtime-route-list"><h3>Routes présentes</h3><p v-if="!Object.keys(overview.routes).length" class="ctx-hint">Aucune route ajoutée par les bridges sélectionnés.</p><div v-for="(route,id) in overview.routes" :key="id" class="runtime-route-row" :data-runtime-route="id"><strong>{{id}}</strong><span>{{route.from.instance}} / {{route.from.port}} → {{route.to.instance}} / {{route.to.port}}</span><small>{{routeModes.find(mode=>mode.value===route.mode)?.label}} · {{route.condition?'Éligibilité conditionnelle':'Sans garde'}}<template v-if="route.toolName"> · outil {{route.toolName}}</template></small></div></div><details v-if="Object.keys(overview.dataBindings).length"><summary>Données partagées · {{Object.keys(overview.dataBindings).length}}</summary><p v-for="(binding,id) in overview.dataBindings" :key="id">{{id}} · {{binding.from.instance}} / {{binding.from.port}} → {{binding.to.instance}} / {{binding.to.port}} · {{binding.permissions.write?'Lecture et écriture':'Lecture'}}</p></details><div class="runtime-inference-list"><h3>Inférences</h3><button v-for="(item,path) in overview.inferences" :key="path" @click="emit('inspect',path)"><strong>{{item.label}}</strong><small>{{path}} · {{item.config.modelBinding==='runtime'?'Modèle à l’exécution':`${item.config.provider||'fixture'} / ${item.config.model||'fixture'}`}} · {{item.contextProgramHash?`Contexte ${item.contextProgramHash.slice(0,10)}`:'Pièces du nœud'}}</small></button></div></section></template>
<style scoped>.runtime-graph-canvas{height:320px;min-height:220px;border:1px solid #3b3b44;border-radius:7px;background:#1b1b20}.runtime-instance{background:#27272d;border:1px solid #60606b;border-radius:6px;padding:15px;width:230px;display:flex;flex-direction:column;gap:7px;font-size:13px}.runtime-instance small,.runtime-instance span{font-size:11px;color:#a2a2b0}.runtime-graph-preview h3{font-size:12px;margin:18px 0 10px}.runtime-route-row{display:flex;flex-direction:column;gap:5px;border-left:2px solid #626274;padding:8px 12px;margin-bottom:8px;font-size:12px}.runtime-route-row small,.runtime-inference-list small{color:#9898a7;font-size:11px}.runtime-inference-list>button{display:flex;flex-direction:column;align-items:flex-start;gap:5px;width:100%;padding:9px 10px;background:transparent;margin-bottom:5px;text-align:left}.runtime-inference-list strong{font-size:12px}.runtime-graph-preview details{font-size:12px;margin-top:20px}.runtime-graph-preview details summary{cursor:pointer}</style>
