<script setup lang="ts">
import type { JsonValue, ModelEntry } from '@zedflow/sdk'

import { computed, ref, shallowRef, watch } from 'vue'
import type { Composition, ModelSelection } from '@zedflow/sdk'
import { flowExports } from '../compositionEngine'

import { defaultContextValue } from '../contextEngine'

import { modelNodes, fixedSelection } from '../harness'
import ContextValueEditor from './context/ContextValueEditor.vue'
import ModelPicker from './ModelPicker.vue'
import AppDialog from './AppDialog.vue'
const open=defineModel<boolean>('open',{required:true})
const props=defineProps<{composition:Composition;models:ModelEntry[];busy:boolean;launch:(input:Record<string,JsonValue>,bindings:Record<string,ModelSelection>)=>Promise<void>}>()
const value=shallowRef<JsonValue>(''),bindings=ref<Record<string,ModelSelection>>({}),error=ref(''),pending=ref(false)
const ports=computed(()=>flowExports(props.composition)),entry=computed(()=>Object.keys(ports.value?.entries||{})[0]),contract=computed(()=>entry.value?ports.value?.contract.entries[entry.value]:undefined)
const type=computed(()=>contract.value?.input||{kind:'text'} as const),inputField=computed(()=>entry.value?ports.value?.entries[entry.value]?.inputField||'input':'input')
const nodes=computed(()=>modelNodes(props.composition))
watch(()=>[open.value,props.composition.id],()=>{if(open.value){value.value=defaultContextValue(type.value,ports.value?.types);bindings.value={};error.value=''}})
async function launch(){pending.value=true;error.value='';try{await props.launch({[inputField.value]:value.value},bindings.value);open.value=false}catch(cause){error.value=cause instanceof Error?cause.message:String(cause)}finally{pending.value=false}}
</script>
<template><AppDialog v-model:open="open" title="Tester dans un workspace temporaire" wide><section class="draft-run"><p>Le daemon exécute une copie de « {{composition.name}} » sans enregistrer ce brouillon. Les chemins relatifs des outils partent du workspace temporaire ; les modèles et outils configurés restent ceux du flow.</p><ContextValueEditor v-model="value" :type="type" :types="ports?.types" label="Entrée du test"/><details v-if="nodes.length"><summary>Modèles du test · {{nodes.length}}</summary><div v-for="node in nodes" :key="node.path" class="draft-run-model"><strong>{{node.node.data.label}}</strong><small>{{node.path}}{{node.runtime?' · choix à l’exécution':' · fixé dans le flow'}}</small><ModelPicker :selection="node.runtime?bindings[node.path]:fixedSelection(node)" :models="models" :disabled="!node.runtime||pending" @change="bindings[node.path]=$event"/></div></details><p v-if="error" role="alert" class="field-error">{{error}}</p><footer><button :disabled="pending||busy" @click="launch">Lancer le test du brouillon</button><small>Le run conserve ses résultats et ses versions. La consultation ne remplace pas votre brouillon.</small></footer></section></AppDialog></template>
<style scoped>.draft-run{display:flex;flex-direction:column;gap:16px;max-height:76vh;overflow:auto;font-size:12px}.draft-run p,.draft-run small{color:#aaa5b2;font-size:11px;line-height:1.6}.draft-run summary{cursor:pointer}.draft-run-model{display:flex;flex-direction:column;gap:8px;border-top:1px solid #37333e;padding:12px 0}.draft-run footer{display:flex;flex-direction:column;gap:10px;align-items:flex-start}</style>
