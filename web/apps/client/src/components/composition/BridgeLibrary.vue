<script setup lang="ts">
import { ref } from 'vue'
import AppDialog from '../AppDialog.vue'
import CompositionOverview from './CompositionOverview.vue'
import { GitBranch, Plus, RefreshCw } from 'lucide-vue-next'
import type { Workspace } from '@zedflow/sdk'
import type { BridgeStudioController } from '../../compositionEngine'

defineProps<{studio:BridgeStudioController;workspaces:Workspace[]}>()
const overview=ref(false)
const emit=defineEmits<{workspace:[id:string]}>()
</script>
<template><div class="ctx-library"><label class="ctx-workspace">Workspace des bridges<select aria-label="Workspace des bridges" :value="studio.workspaceId" @change="emit('workspace',($event.target as HTMLSelectElement).value)"><option v-for="item in workspaces.filter(item=>item.open)" :key="item.id" :value="item.id">{{item.name}}</option></select></label><div class="ctx-library-actions"><button @click="studio.create()"><Plus :size="14"/>Créer un bridge</button><button aria-label="Actualiser les bridges" :disabled="studio.session.loading" @click="studio.refresh()"><RefreshCw :size="14"/></button></div><button @click="overview=true">Voir la composition</button><AppDialog v-model:open="overview" title="Composition des bridges" wide><CompositionOverview v-if="overview" :studio="studio" @edit="file=>{studio.open(file);overview=false}"/></AppDialog><p class="ctx-library-caption">.zedflow/bridges</p><button v-for="file in studio.session.files" :key="file.key" class="ctx-file" :class="{chosen:studio.session.selected===file.key}" :data-bridge-key="file.key" @click="studio.open(file)"><GitBranch :size="14"/><span>{{file.key}}<small>{{file.bridge?`${Object.keys(file.bridge.connections).length} connexions`:'Source invalide'}}</small></span></button><p class="ctx-library-caption">Brouillons</p><template v-for="(draft,key) in studio.session.drafts" :key="key"><button v-if="!draft.file&&(key===studio.session.selected||Object.keys(draft.bridge.imports).length||Object.keys(draft.bridge.connections).length)" class="ctx-file" :class="{chosen:studio.session.selected===key}" @click="studio.session.selected=key"><GitBranch :size="14"/><span>{{draft.key}}<small>Non enregistré</small></span></button></template><p v-if="studio.session.error" class="ctx-error" role="alert">{{studio.session.error}}</p></div></template>
