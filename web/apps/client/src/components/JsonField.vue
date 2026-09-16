<script setup lang="ts">
import { jsonValueSchema, type JsonValue } from '@zedflow/sdk'
import { ref, watch } from 'vue'
const props=defineProps<{modelValue:unknown;label:string;rows?:number}>()
const emit=defineEmits<{ 'update:modelValue':[JsonValue] }>()
const text=ref(''),error=ref('')
watch(()=>props.modelValue,v=>{const next=JSON.stringify(v??null,null,2);if(next!==text.value)text.value=next},{immediate:true})
function update(value:string){text.value=value;try{emit('update:modelValue',jsonValueSchema.parse(JSON.parse(value)));error.value=''}catch{error.value='JSON invalide · dernière valeur valide conservée'}}
</script>
<template><label>{{label}}<textarea :value="text" :rows="rows||4" @input="update(($event.target as HTMLTextAreaElement).value)" spellcheck="false"/><small v-if="error" class="field-error">{{error}}</small></label></template>
