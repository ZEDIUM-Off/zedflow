<script setup lang="ts">
import type { ContextItem } from '@zedflow/sdk'
defineProps<{ items: ContextItem[]; selected?: string; action?: 'definition'|'inspect' }>()
const emit=defineEmits<{select:[id:string]}>()
</script>
<template>
  <div class="ctx-preview-items"><template v-for="item in items" :key="item.id"><section v-if="item.kind==='group'" class="ctx-preview-group" :data-fragment-id="item.id"><button class="ctx-preview-group-title" :aria-label="action==='inspect'?`Inspecter le groupe ${item.id}`:undefined" @click="emit('select',item.id)">{{item.label}}</button><ContextPreviewItems :items="item.items" :selected="selected" :action="action" @select="emit('select',$event)"/></section><button v-else class="ctx-preview-fragment" :class="{selected:selected===item.id}" :data-fragment-id="item.id" :aria-label="action==='inspect'?`Inspecter le fragment ${item.id}`:`Voir le bloc du fragment ${item.id}`" @click="emit('select',item.id)"><span class="ctx-fragment-meta">{{item.role==='instruction'?'Instruction':'Donnée'}} <small>{{item.format}}</small></span><pre>{{typeof item.value==='string'?item.value:JSON.stringify(item.value,null,2)}}</pre><small>{{item.sources.length?`Sources : ${item.sources.join(', ')}`:'Valeur littérale'}} · {{action==='inspect'?'Inspecter ce fragment':'Voir le bloc ↗'}}</small></button></template></div>
</template>
