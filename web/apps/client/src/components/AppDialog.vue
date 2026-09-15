<script setup lang="ts">
import { DialogRoot, DialogPortal, DialogOverlay, DialogContent, DialogTitle, DialogDescription, DialogClose } from 'reka-ui'
import { useId } from 'vue'
import { X } from 'lucide-vue-next'
const descriptionId=useId()
const open=defineModel<boolean>('open',{required:true})
defineProps<{title:string;description?:string;wide?:boolean}>()
</script>
<template>
  <DialogRoot v-model:open="open"><DialogPortal><DialogOverlay class="dialog-overlay"/><DialogContent :class="['app-dialog',{wide}]" :aria-describedby="description ? descriptionId : undefined">
    <header><div><DialogTitle>{{title}}</DialogTitle><DialogDescription v-if="description" :id="descriptionId">{{description}}</DialogDescription></div><DialogClose aria-label="Fermer" class="icon-button"><X :size="18"/></DialogClose></header>
    <slot/>
  </DialogContent></DialogPortal></DialogRoot>
</template>
