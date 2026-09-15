<script setup lang="ts">
import { computed, onBeforeUnmount, watch } from 'vue'
import { useVueFlow } from '@vue-flow/core'
import { loadRouter, ObstacleRouter, type RoutedEdge, type RoutingNode } from '../graph/routing'
const emit = defineEmits<{routes: [value: Record<string, RoutedEdge>]; error: [message: string]}>()
const {getNodes, getEdges} = useVueFlow()
let router: ObstacleRouter | undefined, frame = 0, disposed = false
const geometry = computed(() => ({
  nodes: getNodes.value.filter(node => !node.hidden && node.dimensions.width && node.dimensions.height).map(node => ({
    id: node.id, x: node.computedPosition.x, y: node.computedPosition.y, ...node.dimensions,
    ports: (['source', 'target'] as const).flatMap(type => (node.handleBounds[type] || []).map(handle => ({
      id: `${type}:${handle.id || ''}`, x: handle.x + handle.width / 2, y: handle.y + handle.height / 2, side: handle.position,
    }))),
  })) as RoutingNode[],
  edges: getEdges.value.filter(edge => !edge.hidden).map(edge => ({id: edge.id, source: edge.source, target: edge.target, sourcePort: `source:${edge.sourceHandle || ''}`, targetPort: `target:${edge.targetHandle || ''}`})),
}))
function schedule() {
  cancelAnimationFrame(frame)
  frame = requestAnimationFrame(() => {
    if (!router || disposed) return
    try { emit('routes', router.update(geometry.value.nodes, geometry.value.edges)); emit('error', '') }
    catch (error) { emit('error', `Le tracé des connexions n’a pas pu être calculé : ${String(error)}`) }
  })
}
watch(geometry, schedule, {deep: true})
void loadRouter().then(avoid => { if (!disposed) { router = new ObstacleRouter(avoid); schedule() } }).catch(error => emit('error', `Chargement du routeur impossible : ${String(error)}`))
onBeforeUnmount(() => { disposed = true; cancelAnimationFrame(frame); router?.destroy() })
</script>
<template><span class="graph-routing-status" aria-hidden="true"/></template>
