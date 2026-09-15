<script setup lang="ts">
import { computed } from 'vue'
import { NodeList, useContext, useSanitizers, type LinkNodeRendererProps } from 'vue-stream-markdown'

const props=defineProps<LinkNodeRendererProps>()
const {hardenOptions}=useContext()
const {transformedUrl,isHardenUrl}=useSanitizers({url:()=>props.node.url,hardenOptions})
const href=computed(()=>!isHardenUrl.value&&transformedUrl.value?transformedUrl.value:undefined)
</script>
<template>
  <!-- Native href preserves keyboard activation, copying, and browser menus.
       Keep the Markdown renderer's URL sanitization for unsafe destinations. -->
  <a v-if="href" :href="href" :title="node.title||undefined" target="_blank" rel="noopener noreferrer" data-stream-markdown="link"><NodeList v-bind="props" :parent-node="node" :nodes="node.children" :deep="deep+1"/></a>
  <span v-else><NodeList v-bind="props" :parent-node="node" :nodes="node.children" :deep="deep+1"/></span>
</template>
