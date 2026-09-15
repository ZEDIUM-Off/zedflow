<script setup lang="ts">
import { computed, useId } from 'vue'
import { BaseEdge, EdgeLabelRenderer, type EdgeProps } from '@vue-flow/core'
import type { RoutedEdge } from '../graph/routing'
const props = defineProps<EdgeProps & {route?: RoutedEdge}>()
const arrowId = `flow-direction-${useId()}`
// Vue Flow supplies an unresolved marker URL even when the flow has no marker.
// Own a direction marker for every routed control-flow edge in every graph view.
const arrowEnd = `url(#${arrowId})`
const branchLabel = computed(() => props.sourceHandleId === 'true' ? 'Oui' : props.sourceHandleId === 'false' ? 'Non' : props.label)
</script>
<template>
  <defs><marker :id="arrowId" viewBox="0 0 12 12" refX="11" refY="6" markerWidth="12" markerHeight="12" markerUnits="userSpaceOnUse" orient="auto"><path d="M1 1 L11 6 L1 11 Z" fill="#bdbdbd"/></marker></defs>
  <BaseEdge v-if="route" :id="id" :path="route.path" :marker-end="arrowEnd" :marker-start="markerStart" :interaction-width="22" :data-route-points="JSON.stringify(route.points)" :class="{'branch-true':sourceHandleId==='true','branch-false':sourceHandleId==='false'}"/>
  <EdgeLabelRenderer v-if="route&&branchLabel"><span class="graph-edge-label nodrag nopan" :data-edge-label="id" :style="{transform:`translate(-50%, -50%) translate(${route.label.x}px,${route.label.y}px)`}">{{branchLabel}}</span></EdgeLabelRenderer>
</template>
