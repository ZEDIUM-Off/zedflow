<script setup lang="ts">
const client=useClient()
import { useClient } from '@zedflow/vue'

import { ref, shallowRef, watch } from 'vue'
import { } from '../../contextEngine'
import type { ContextTypesFile } from '@zedflow/sdk'
const props=withDefaults(defineProps<{workspaceId:string;file?:ContextTypesFile;active?:boolean;revision?:number}>(),{active:true})
const emit=defineEmits<{select:[file:ContextTypesFile|undefined];edit:[]}>()
const files=shallowRef<ContextTypesFile[]>([]),error=ref(''),loading=ref(false);let intent=0
async function refresh(){const request=++intent;if(!props.workspaceId||props.active===false)return;loading.value=true;error.value='';try{const value=await client.context.listTypes({workspaceId:props.workspaceId});if(request===intent)files.value=Array.isArray(value)?value:[]}catch(cause){if(request===intent)error.value=cause instanceof Error?cause.message:String(cause)}finally{if(request===intent)loading.value=false}}
async function choose(key:string){if(!key){emit('select',undefined);return}const request=++intent;loading.value=true;error.value='';try{const file=await client.context.readTypes(key,{workspaceId:props.workspaceId});if(request===intent)emit('select',file)}catch(cause){if(request===intent)error.value=cause instanceof Error?cause.message:String(cause)}finally{if(request===intent)loading.value=false}}
watch(()=>[props.workspaceId,props.active,props.revision],()=>{files.value=[];void refresh()},{immediate:true})

</script>
<template><section class="ctx-types-selection"><label>Catalogue de types<select aria-label="Catalogue de types" :disabled="loading" :value="file?.key||''" @change="choose(($event.target as HTMLSelectElement).value)"><option value="">Types de la configuration</option><option v-for="item in files" :key="item.key" :value="item.key" :disabled="!item.types">{{item.key}}{{item.types?'':' · invalide'}}</option><option v-if="file&&!files.some(item=>item.key===file?.key)" :value="file.key">{{file.key}}</option></select></label><small v-if="file">{{file.hash.slice(0,12)}} · {{Object.keys(file.types||{}).join(', ')}}</small><div class="ctx-row"><button :disabled="loading" aria-label="Actualiser les catalogues de types" @click="refresh">Actualiser</button><button @click="emit('edit')">Éditer les types</button></div><p v-if="error" role="alert" class="field-error">{{error}}</p></section></template>
<style scoped>.ctx-types-selection{display:flex;flex-direction:column;gap:8px;margin:16px 0;padding-top:12px;border-top:1px solid #383840;font-size:11px}.ctx-types-selection label{display:flex;flex-direction:column;gap:7px}.ctx-types-selection small{color:#a7a4b4;overflow-wrap:anywhere}.ctx-types-selection button{font-size:11px}</style>
