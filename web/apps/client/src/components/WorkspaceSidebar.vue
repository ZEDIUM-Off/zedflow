<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import { DropdownMenuRoot, DropdownMenuTrigger, DropdownMenuPortal, DropdownMenuContent, DropdownMenuItem } from 'reka-ui'
import { Bot, ChevronDown, ChevronRight, Download, Folder, FolderOpen, GitBranch, Import, MoreHorizontal, Plus, Search, Pencil, PanelLeftClose, X } from 'lucide-vue-next'
import type { RunSummary, Workspace } from '@zedflow/sdk'
import { statuses } from '../composables/useZedflow'
const props=defineProps<{mode:'execution'|'design';designLabel?:string;workspaces:Workspace[];runs:RunSummary[];workspaceId:string;currentId?:string;connected?:boolean}>()
const emit=defineEmits<{mode:[value:'execution'|'design'];open:[];newSession:[workspaceId?:string];session:[run:RunSummary];rename:[run:RunSummary];export:[run:RunSummary];import:[];close:[id:string];hide:[]}>()
const collapsed=ref<Record<string,boolean>>({}),expanded=ref<Record<string,boolean>>({}),query=ref('')
function initialWidth(){try {const value=Number(localStorage.getItem('zedflow.sidebar-width'));return value?Math.max(220,Math.min(440,value)):280}catch{return 280}}
const width=ref(initialWidth())
watch(width,value=>{try{localStorage.setItem('zedflow.sidebar-width',String(value))}catch{/* Storage may be disabled. */}})
const opened=computed(()=>props.workspaces.filter(item=>item.open))
const sessionGroups=computed(()=>{
  const grouped=new Map<string,RunSummary[]>(),term=query.value.toLocaleLowerCase()
  for(const run of props.runs){if(run.interactive===false)continue;if(!run.name.toLocaleLowerCase().includes(term))continue;const key=run.workspaceId||'';const items=grouped.get(key)||[];items.push(run);grouped.set(key,items)}
  for(const items of grouped.values())items.sort((a,b)=>(b.updatedAt||b.createdAt||0)-(a.updatedAt||a.createdAt||0))
  return grouped
})
function sessions(id:string){return sessionGroups.value.get(id)||[]}
function visibleSessions(id:string){const items=sessions(id);return query.value||expanded.value[id]?items:items.slice(0,5)}
watch(()=>props.currentId,id=>{const run=props.runs.find(run=>run.id===id);if(run?.workspaceId&&!visibleSessions(run.workspaceId).some(item=>item.id===id))expanded.value[run.workspaceId]=true})
let stopResize=()=>{}
function resize(event:PointerEvent){if(window.innerWidth<760)return;event.preventDefault();stopResize();const x=event.clientX,original=width.value;const move=(next:PointerEvent)=>{width.value=Math.max(220,Math.min(440,original+next.clientX-x))};const stop=()=>{window.removeEventListener('pointermove',move);window.removeEventListener('pointerup',stop);window.removeEventListener('pointercancel',stop)};stopResize=stop;window.addEventListener('pointermove',move);window.addEventListener('pointerup',stop,{once:true});window.addEventListener('pointercancel',stop,{once:true})}
function resizeKey(event:KeyboardEvent){if(!['ArrowLeft','ArrowRight','Home','End'].includes(event.key))return;event.preventDefault();width.value=event.key==='Home'?220:event.key==='End'?440:Math.max(220,Math.min(440,width.value+(event.key==='ArrowRight'?20:-20)))}
onUnmounted(()=>stopResize())
</script>
<template>
  <aside class="sidebar workspace-sidebar" :class="{'design-sidebar':mode==='design'}" :style="{'--sidebar-width':`${width}px`}">
    <div class="sidebar-utility"><span>{{mode==='design'?(designLabel||'Flows'):'Sessions'}}</span><button class="icon-button sidebar-toggle" aria-label="Masquer la navigation" @click="emit('hide')"><PanelLeftClose :size="16"/></button></div>
    <slot v-if="mode==='design'" name="design"/>
    <template v-else>
      <button class="new-session" @click="emit('newSession',workspaceId)"><Plus :size="16"/> Nouvelle session</button>
      <label class="session-search"><Search :size="14"/><input v-model="query" aria-label="Rechercher une session" placeholder="Rechercher une session…"/><button v-if="query" class="icon-button" aria-label="Effacer la recherche" @click="query=''"><X :size="12"/></button></label>
      <div class="sidebar-caption">Workspaces<button aria-label="Ouvrir un workspace" title="Ouvrir un workspace" @click="emit('open')"><FolderOpen :size="15"/></button></div>
      <div class="workspace-groups"><section v-for="item in opened" :key="item.id" class="workspace-group" :data-workspace-id="item.id">
        <div class="workspace-heading" :class="{active:workspaceId===item.id}"><button class="workspace-toggle" :aria-expanded="!collapsed[item.id]" :title="item.path" @click="collapsed[item.id]=!collapsed[item.id]"><ChevronRight v-if="collapsed[item.id]" :size="12"/><ChevronDown v-else :size="12"/><Folder :size="15"/><span>{{item.name}}</span></button>
          <DropdownMenuRoot><DropdownMenuTrigger class="icon-button row-menu" :aria-label="`Actions du workspace ${item.name}`"><MoreHorizontal :size="15"/></DropdownMenuTrigger><DropdownMenuPortal><DropdownMenuContent class="compact-menu" align="end" :side-offset="4"><DropdownMenuItem @select="emit('newSession',item.id)"><Plus :size="14"/> Nouvelle session</DropdownMenuItem><DropdownMenuItem @select="emit('close',item.id)"><X :size="14"/> Fermer le workspace</DropdownMenuItem></DropdownMenuContent></DropdownMenuPortal></DropdownMenuRoot>
        </div>
        <div v-if="!collapsed[item.id]||query" class="workspace-sessions">
          <div v-for="run in visibleSessions(item.id)" :key="run.id" class="session-row" :class="{chosen:currentId===run.id}" :data-session-id="run.id"><button :title="`${run.name} · ${statuses[run.status]||run.status}`" @click="emit('session',run)"><span class="session-name">{{run.name}}</span><span v-if="['running','waiting','error'].includes(run.status)" :class="['live-dot',{amber:run.status==='waiting',failed:run.status==='error'}]" :aria-label="statuses[run.status]"/></button>
            <DropdownMenuRoot><DropdownMenuTrigger class="icon-button row-menu" :aria-label="`Actions de ${run.name}`"><MoreHorizontal :size="15"/></DropdownMenuTrigger><DropdownMenuPortal><DropdownMenuContent class="compact-menu" align="end" :side-offset="4"><DropdownMenuItem @select="emit('rename',run)"><Pencil :size="14"/> Renommer la session</DropdownMenuItem><DropdownMenuItem :disabled="run.status==='running'||run.runtimeActive" @select="emit('export',run)"><Download :size="14"/> Exporter la session</DropdownMenuItem></DropdownMenuContent></DropdownMenuPortal></DropdownMenuRoot>
          </div>
          <button v-if="!query&&sessions(item.id).length>5" class="show-more-sessions" @click="expanded[item.id]=!expanded[item.id]">{{expanded[item.id]?'Voir moins':`Voir plus · ${sessions(item.id).length-5}`}}</button>
          <button v-if="!sessions(item.id).length&&!query" class="workspace-empty" @click="emit('newSession',item.id)">Commencer une session</button>
        </div>
      </section><p v-if="query&&!opened.some(item=>sessions(item.id).length)" class="workspace-empty">Aucune session correspondante.</p><button v-if="!opened.length" class="workspace-empty" @click="emit('open')">Ouvrir votre premier workspace</button></div>
    </template>
    <slot v-if="mode==='execution'" name="executions"/>
    <div class="sidebar-footer-actions"><button class="open-workspace" @click="emit('open')"><FolderOpen :size="15"/> Ouvrir un workspace</button><button class="icon-button" aria-label="Importer une session" title="Importer une session" @click="emit('import')"><Import :size="15"/></button></div>

    <div role="separator" tabindex="0" aria-label="Redimensionner la navigation" aria-orientation="vertical" :aria-valuenow="width" :aria-valuemin="220" :aria-valuemax="440" class="sidebar-resizer" @pointerdown="resize" @keydown="resizeKey"/>
  </aside>
</template>
