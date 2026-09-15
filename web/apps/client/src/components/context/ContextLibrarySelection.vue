<script setup lang="ts">
const client=useClient()
import { useClient } from '@zedflow/vue'

import { ref, shallowRef, watch } from 'vue'
import { cloneContext, } from '../../contextEngine'
import type { ContextLibraryFile, ContextLibrary } from '@zedflow/sdk'
const props=withDefaults(defineProps<{workspaceId:string;revision?:number;file?:ContextLibraryFile;active?:boolean}>(),{active:true})
const emit=defineEmits<{select:[file:ContextLibraryFile|undefined];edit:[]}>()
const files=shallowRef<ContextLibraryFile[]>([]),error=ref(''),loading=ref(false)
let intent=0
async function load(){const version=++intent;loading.value=true;error.value='';if(!props.workspaceId){files.value=[];loading.value=false;return}try{const value=await client.context.listLibraries({workspaceId:props.workspaceId});if(version===intent)files.value=Array.isArray(value)?value:[]}catch(cause){if(version===intent)error.value=String(cause instanceof Error?cause.message:cause)}finally{if(version===intent)loading.value=false}}
async function choose(key:string){if(!key){emit('select',undefined);return}const workspace=props.workspaceId,version=++intent;loading.value=true;error.value='';try{const file=await client.context.readLibrary(key,{workspaceId:workspace});if(version===intent&&file.library)emit('select',cloneContext(file))}catch(cause){if(version===intent)error.value=String(cause instanceof Error?cause.message:cause)}finally{if(version===intent)loading.value=false}}
watch(()=>[props.workspaceId,props.revision,props.active],()=>{intent++;if(props.active)void load()},{immediate:true})

</script>
<template><div class="ctx-library-selection"><label>Bibliothèque de fonctions<select aria-label="Bibliothèque de fonctions" :value="file?.key||''" :disabled="loading" @change="choose(($event.target as HTMLSelectElement).value)"><option value="">Aucune bibliothèque</option><option v-for="item in files" :key="item.key" :value="item.key" :disabled="!item.library">{{item.key}}{{item.library?'':' · source invalide'}}</option><option v-if="file&&!files.some(item=>item.key===file?.key)" :value="file.key">{{file.key}} · version sélectionnée</option></select></label><small v-if="file">{{file.hash.slice(0,10)}} · Catalogue fourni explicitement à cet aperçu.</small><p v-if="file&&files.some(item=>item.key===file?.key&&item.hash!==file?.hash)" class="ctx-error">Le fichier a changé. Sélectionnez-le à nouveau pour adopter sa version.</p><p v-if="error" role="alert" class="ctx-error">{{error}}</p><button type="button" @click="emit('edit')">Éditer les bibliothèques</button></div></template>
