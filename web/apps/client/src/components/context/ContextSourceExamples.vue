<script setup lang="ts">
import type { JsonValue } from '@zedflow/sdk'

const client=useClient()
import { useClient } from '@zedflow/vue'

import { ref, shallowRef, watch, computed } from 'vue'
import { cloneContext } from '../../contextEngine'
import type { ContextType } from '@zedflow/sdk'
const props=defineProps<{workspaceId:string;type:ContextType;types:Record<string,ContextType>;value?:JsonValue}>()
const emit=defineEmits<{select:[value:JsonValue|undefined]}>()
interface Example { id:string;label:string;value:JsonValue }
const examples=shallowRef<Example[]>([]),selected=ref(''),baseline=ref(''),label=ref(''),error=ref(''),saving=ref(false)
const modified=computed(()=>!!selected.value&&JSON.stringify(props.value)!==baseline.value)
let intent=0
watch(()=>[props.workspaceId,props.type,props.types],async()=>{const current=++intent;examples.value=[];selected.value='';error.value='';try{const result=await client.context.queryExamples({workspaceId:props.workspaceId,dataType:props.type,types:props.types});if(current===intent)examples.value=result}catch(cause){if(current===intent)error.value=String(cause)}},{immediate:true,deep:true})
function select(id:string){selected.value=id;if(!id){baseline.value='';emit('select',undefined);return}const example=examples.value.find(item=>item.id===id);if(example){baseline.value=JSON.stringify(example.value);emit('select',cloneContext(example.value))}}
async function save(){saving.value=true;error.value='';const current=intent;try{const example=await client.context.saveExample({workspaceId:props.workspaceId,dataType:props.type,types:props.types,label:label.value,value:props.value!});if(current!==intent)return;examples.value=[...examples.value,example];selected.value=example.id;baseline.value=JSON.stringify(example.value);label.value=''}catch(cause){if(current===intent)error.value=String(cause)}finally{saving.value=false}}
</script>
<template><div class="source-examples"><label>Exemple du type<select :value="selected||(value!==undefined?'__custom':'')" @change="select(($event.target as HTMLSelectElement).value)"><option v-if="value!==undefined&&!selected" value="__custom" disabled>Valeur personnalisée</option><option value="">Absence de valeur</option><option v-for="example in examples" :key="example.id" :value="example.id">{{ example.label }}</option></select></label><small v-if="modified">Valeur modifiée depuis l’exemple.</small><small v-else-if="value!==undefined&&!selected">Valeur personnalisée.</small><details v-if="value!==undefined"><summary>Conserver comme exemple du type</summary><input v-model="label" aria-label="Nom du nouvel exemple" placeholder="Nom de l’exemple"/><button :disabled="saving||!label.trim()" @click="save">Enregistrer l’exemple</button></details><p v-if="error" role="alert">{{ error }}</p></div></template>
<style scoped>.source-examples{font-size:11px;margin:12px 0}.source-examples label{display:flex;flex-direction:column;gap:6px}.source-examples small{display:block;margin:6px 0;color:#a4a0b0}.source-examples select,.source-examples input{width:100%;min-width:0}</style>
