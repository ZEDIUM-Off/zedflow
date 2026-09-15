<script setup lang="ts">
const client=useClient()
import { useClient } from '@zedflow/vue'

import { computed, ref, watch } from 'vue'
import { ArrowUp, ChevronRight, Folder, Home, RefreshCw } from 'lucide-vue-next'
import type { DirectoryListing } from '@zedflow/sdk'
import AppDialog from './AppDialog.vue'
const open=defineModel<boolean>('open',{required:true})
const props=defineProps<{initialPath?:string;busy?:boolean}>()
const emit=defineEmits<{select:[path:string]}>()
const listing=ref<DirectoryListing|null>(null), path=ref(''), hidden=ref(false), loading=ref(false), error=ref('')
const crumbs=computed(()=>{const parts=(listing.value?.path||'').split('/').filter(Boolean);return [{name:'/',path:'/'},...parts.map((name,index)=>({name,path:'/'+parts.slice(0,index+1).join('/')}))]})
let version=0
// A pending navigation owns the path until its canonical result arrives.
// The input stays read-only during that interval, so hydration cannot interrupt
// a user's selection or append text to a freshly replaced value.
async function browse(target=''){
  const request=++version;loading.value=true;error.value=''
  try{const value=await client.workspaces.browse({path:target,showHidden:hidden.value});if(request===version){listing.value=value;path.value=value.path}}catch(cause){if(request===version)error.value=cause instanceof Error?cause.message:String(cause)}finally{if(request===version)loading.value=false}
}
watch(open,value=>{if(value)void browse(props.initialPath||'')})
watch(hidden,()=>void browse(listing.value?.path||path.value))

</script>
<template>
  <AppDialog v-model:open="open" title="Ouvrir un workspace" description="Choisissez un dossier sur la machine du daemon.">
    <form class="directory-path" @submit.prevent="browse(path)"><input v-model="path" :readonly="loading" :aria-busy="loading" aria-label="Chemin du dossier" placeholder="/home/…"/><button type="submit" :disabled="loading">Aller</button></form>
    <div class="directory-toolbar"><button class="icon-button" aria-label="Dossier parent" :disabled="!listing?.parent || loading" @click="browse(listing?.parent||'/')"><ArrowUp :size="16"/></button><button class="icon-button" aria-label="Dossier personnel du daemon" :disabled="loading" @click="browse(listing?.home||'')"><Home :size="16"/></button><button class="icon-button" aria-label="Actualiser les dossiers" :disabled="loading" @click="browse(listing?.path||'')"><RefreshCw :size="15"/></button><label class="check-field"><input v-model="hidden" type="checkbox"/> Dossiers cachés</label></div>
    <nav class="directory-crumbs" aria-label="Chemin courant"><template v-for="(crumb,index) in crumbs" :key="crumb.path"><ChevronRight v-if="index" :size="12"/><button @click="browse(crumb.path)">{{crumb.name}}</button></template></nav>
    <p v-if="error" role="alert" class="banner error">{{error}}</p>
    <div class="directory-list" :aria-busy="loading"><p v-if="loading" class="muted">Chargement des dossiers…</p><template v-else><button v-for="entry in listing?.entries||[]" :key="entry.path" @click="browse(entry.path)"><Folder :size="17"/><span>{{entry.name}}</span><ChevronRight :size="14"/></button><p v-if="!listing?.entries.length" class="empty">Aucun sous-dossier visible.</p></template></div>
    <p v-for="diagnostic in listing?.diagnostics||[]" :key="diagnostic" class="field-error">{{diagnostic}}</p>
    <footer><span :title="listing?.path" class="directory-selected">{{listing?.path}}</span><button class="primary" :disabled="loading||busy||!listing||!!error" @click="emit('select',listing!.path)">Ouvrir ce dossier</button></footer>
  </AppDialog>
</template>
