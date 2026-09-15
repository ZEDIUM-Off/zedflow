<script setup lang="ts">
const client=useClient()
import { useClient } from '@zedflow/vue'

import { computed, onMounted, onUnmounted, ref } from 'vue'
import { PopoverRoot, PopoverTrigger, PopoverPortal, PopoverContent } from 'reka-ui'
import { CLIENT_BUILD, type BuildInfo } from '../buildInfo'
import { preserveAndReload } from '../composables/reloadState'
import { HttpError, type DaemonVersion, type Release } from '@zedflow/sdk'
const development=import.meta.env.DEV
const version=ref<DaemonVersion>(),error=ref(''),pending=ref(false),checking=ref(false),unavailable=ref(false),offline=ref(false)
const short=(build?:BuildInfo|null)=>build?`${build.version} · ${build.buildId.slice(0,12)}`:'Version non publiée'
const clientUpdate=computed(()=>!development&&!!version.value?.client&&version.value.client.buildId!==CLIENT_BUILD.buildId)
const releaseUpdate=computed(()=>!!version.value?.candidate&&version.value.candidate.releaseId!==version.value.releaseId)
const compatible=computed(()=>!version.value||version.value.daemon.protocol===CLIENT_BUILD.protocol)
const title=computed(()=>offline.value||version.value?.catalogError?'Versions indisponibles':unavailable.value?'Daemon sans gestion des versions':!compatible.value?'Client incompatible':clientUpdate.value?'Client à actualiser':releaseUpdate.value?'Mise à jour disponible':version.value?(version.value.client?(version.value.managed?'À jour · canal local':'Client à jour · daemon non supervisé'):'Version du client disponible inconnue'):'Vérification des versions…')
let controller:AbortController|undefined,timer:ReturnType<typeof setInterval>
async function check(){
  if(checking.value)return;checking.value=true;controller=new AbortController()
  const timeout=setTimeout(()=>controller?.abort(),4000)
  try{version.value=await client.daemon.version(controller.signal);unavailable.value=false;offline.value=false}
  catch(cause){if(cause instanceof HttpError&&cause.status===404){unavailable.value=true;version.value=undefined;offline.value=false}else offline.value=true}
  finally{clearTimeout(timeout);checking.value=false}
}
async function apply(release:Release){
  if(pending.value||!version.value)return;pending.value=true;error.value=''
  try{await client.daemon.applyUpdate({releaseId:release.releaseId,expectedDaemonBuildId:version.value.daemon.buildId})
    version.value.maintenance=true;await check()
  }catch(cause){error.value=String(cause instanceof Error?cause.message:cause)}finally{pending.value=false}
}
function reload(){error.value='';try{preserveAndReload()}catch(cause){error.value=String(cause)}}
const wake=()=>{if(document.visibilityState==='visible')void check()}
onMounted(()=>{void check();timer=setInterval(wake,15000);window.addEventListener('focus',wake);window.addEventListener('online',wake)})
onUnmounted(()=>{clearInterval(timer);controller?.abort();window.removeEventListener('focus',wake);window.removeEventListener('online',wake)})
</script>
<template><PopoverRoot><PopoverTrigger class="app-version-trigger" aria-label="Versions et mises à jour" :data-update="clientUpdate||releaseUpdate||!compatible"><span>{{CLIENT_BUILD.version}}</span><span> · {{title}}</span></PopoverTrigger><PopoverPortal><PopoverContent class="app-versions composer-popover" side="top" align="end" :side-offset="8" :collision-padding="12"><strong>Versions et mises à jour</strong>
  <dl><dt>Client chargé dans cet onglet</dt><dd :title="CLIENT_BUILD.buildId">{{short(CLIENT_BUILD)}}</dd><dt>Client disponible sur le daemon</dt><dd :title="version?.client?.buildId">{{short(version?.client)}}</dd><dt>Daemon en cours</dt><dd :title="version?.daemon.buildId">{{short(version?.daemon)}}</dd></dl>
  <p>{{title}}</p><p v-if="development">Client de développement : les changements sont servis par le serveur de développement.</p>
  <p v-if="!compatible" role="alert">Protocoles incompatibles : client {{CLIENT_BUILD.protocol}}, daemon {{version?.daemon.protocol}}. Les commandes de modification sont refusées.</p>
  <p v-if="version?.maintenance">Activation en cours. La connexion se rétablira après le démarrage de la release.</p>
  <p v-if="version?.catalogError" role="alert">{{version.catalogError}}</p>
  <p v-if="version?.operation?.error" role="alert">{{version.operation.error}}</p><p v-if="error" role="alert">{{error}}</p>
  <button v-if="clientUpdate||!compatible" @click="reload">Actualiser le client et conserver les brouillons</button>
  <template v-if="releaseUpdate&&version?.candidate"><hr/><strong>Release préparée</strong><small>Préparée le {{new Date(version.candidate.createdAt).toLocaleString()}}</small><small>Daemon {{short(version.candidate.daemon)}}</small><small>Client {{short(version.candidate.client)}}</small><button :disabled="!version.managed||pending||version.maintenance||offline||(version.activeExecutions??0)!==0" @click="apply(version!.candidate!)">Activer la mise à jour daemon + client</button></template>
  <details v-if="version?.previous&&version.previous.releaseId!==version.releaseId"><summary>Release précédente</summary><p>{{short(version.previous.daemon)}}</p><button :disabled="!version.managed||pending||version.maintenance||offline||(version.activeExecutions??0)!==0" @click="apply(version!.previous!)">Revenir à la release précédente</button></details>
  <p v-if="version&&!version.managed">Le daemon est lancé sans superviseur disponible. Son redémarrage doit être effectué sur la machine qui l’héberge.</p>
  <small>Canal local : releases préparées sur ce daemon. Aucun registre de mises à jour distant n’est configuré.</small>
  <p v-if="(version?.activeExecutions??0)>0">{{version?.activeExecutions}} exécution(s) encore active(s).</p>
  <p>Une activation attend l’absence d’exécution active. Elle ne reprend ni n’arrête une session à votre place.</p>
  <button :disabled="checking" @click="check">Vérifier les mises à jour</button>
</PopoverContent></PopoverPortal></PopoverRoot></template>
<style scoped>.app-version-trigger{font-size:11px;white-space:nowrap;max-width:32vw;overflow:hidden;text-overflow:ellipsis}.app-version-trigger[data-update=true]{color:#e6c68c}.app-versions{width:min(440px,calc(100vw - 24px));max-height:80vh;overflow:auto;padding:16px;font-size:12px;display:flex;flex-direction:column;gap:9px}.app-versions dl{margin:4px 0;display:grid;gap:5px}.app-versions dt{color:#999eaa;font-size:11px}.app-versions dd{margin:0 0 7px;font-family:monospace;overflow-wrap:anywhere}.app-versions p{margin:3px 0;line-height:1.5}.app-versions small{color:#a8abb3;line-height:1.4}.app-versions [role=alert]{color:#e3ac91}@media(max-width:760px){.app-version-trigger span:last-child{display:none}}</style>
