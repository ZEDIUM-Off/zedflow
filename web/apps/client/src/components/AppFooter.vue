<script setup lang="ts">
import { PopoverRoot,PopoverTrigger,PopoverPortal,PopoverContent,PopoverClose } from 'reka-ui'
import { ref } from 'vue'
import AppVersions from './AppVersions.vue'
import TransportMetrics from './TransportMetrics.vue'
import { Monitor,PanelLeft } from 'lucide-vue-next'
import type { DaemonHealth } from '@zedflow/sdk'
import type { LiveStatus } from '@zedflow/sdk'
const metricsOpen=ref(false)
const props=defineProps<{mode:'execution'|'design';hostname:string;connected:boolean;health:DaemonHealth|undefined;live:LiveStatus;busy:string;path?:string}>()
const emit=defineEmits<{mode:[value:'execution'|'design'];navigation:[];retry:[];resync:[]}>()
</script>
<template>
  <footer class="statusbar app-footer">
    <button class="footer-navigation" aria-label="Afficher la navigation" @click="emit('navigation')"><PanelLeft :size="14"/></button>
    <nav aria-label="Espace de travail" class="footer-spaces"><button :class="{chosen:mode==='execution'}" :aria-pressed="mode==='execution'" @click="emit('mode','execution')">Exécution</button><button :class="{chosen:mode==='design'}" :aria-pressed="mode==='design'" @click="emit('mode','design')">Conception</button></nav>
    <span class="footer-workspace" :title="path">{{busy?`${busy}…`:path}}</span>
    <AppVersions/>
    <PopoverRoot><PopoverTrigger class="daemon-connection" :data-connected="connected" aria-label="Connexion au daemon"><Monitor :size="13"/><span class="daemon-host">{{hostname}}</span><span :class="['live-dot',{offline:!connected}]"/><span>{{connected?'Daemon connecté':health?'Daemon déconnecté':'Connexion au daemon…'}}</span></PopoverTrigger><PopoverPortal><PopoverContent class="daemon-popover composer-popover" side="top" align="end" :side-offset="8" :collision-padding="10"><strong>{{hostname}}</strong><small>{{health?.daemon?.version||health?.adk}}</small><p>{{connected?'Connexion au daemon active.':'Le daemon ne répond plus. Les données déjà chargées restent disponibles.'}}</p><PopoverClose v-if="live.transport!=='offline'" class="transport-badge" :data-transport="live.transport" title="Resynchroniser la session" @click="emit('resync')">{{live.transport==='webrtc'?'WebRTC connecté':live.transport==='sse'?'SSE · repli':live.transport==='http'?'HTTP · rattrapage':'Connexion à la session…'}}</PopoverClose><small>{{live.error instanceof Error ? live.error.message : live.error ? String(live.error) : live.transport === 'offline' ? 'Aucune session suivie' : 'Synchronisation de la session active'}}</small><button @click="emit('retry')">Vérifier la connexion</button><details v-if="live.metrics" @toggle="metricsOpen=($event.target as HTMLDetailsElement).open"><summary>Mesures de synchronisation</summary><TransportMetrics v-if="metricsOpen" :metrics="live.metrics"/></details></PopoverContent></PopoverPortal></PopoverRoot>
  </footer>
</template>
