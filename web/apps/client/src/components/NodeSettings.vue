<script setup lang="ts">
const client=useClient()
import { useClient } from '@zedflow/vue'

import { ref, watch } from 'vue'
import type { FlowNode, Composition } from '@zedflow/sdk'
import JsonField from './JsonField.vue'
import ContextBindingEditor from './context/ContextBindingEditor.vue'
import FlowExportsEditor from './composition/FlowExportsEditor.vue'
import { flowExports } from '../compositionEngine'

const props=defineProps<{node:FlowNode;formatVersion?:number;workspaceId?:string;composition?:Composition}>()
const emit=defineEmits<{context:[key?:string]}>()
const auth=ref<any>(null),authError=ref('')
async function checkAuth(){try{auth.value=await client.models.codexStatus();authError.value=''}catch(e){authError.value=String(e)}}
watch(()=>props.node.data.config.provider,v=>{if(v==='codex')void checkAuth()},{immediate:true})
function number(key:string,e:Event){const value=(e.target as HTMLInputElement).value;props.node.data.config[key]=value===''?null:Number(value)}
function tools(name:string,enabled:boolean){const c=props.node.data.config;c.tools=enabled?[...new Set([...(c.tools||[]),name])]:(c.tools||[]).filter((t:string)=>t!==name)}
const supportedTools=['read','write','edit','exec','inspect_json','format_text','delay']

</script>
<template>
  <FlowExportsEditor v-if="node.data.kind==='start'&&composition" :composition="composition"/>
  <template v-if="node.data.kind==='route'"><label>Point de branchement<select aria-label="Point de branchement" v-model="node.data.config.branch"><option value="" disabled>Choisir un port public</option><option v-for="(branch,name) in flowExports(composition)?.contract.branches||{}" :key="name" :value="name" :disabled="branch.invocations.includes('tool')">{{name}}</option><option v-if="node.data.config.branch&&!flowExports(composition)?.contract.branches[node.data.config.branch]" :value="node.data.config.branch">{{node.data.config.branch}} · non exposé</option></select></label><label>Déclenchement de la route<select aria-label="Déclenchement de la route" v-model="node.data.config.invocation"><option value="condition">Continuer si aucune route ne s’applique</option><option value="node">Exiger une route</option></select></label><label>Canal d’entrée de la route<input v-model="node.data.config.inputField"/></label><label>Route résolue · facultative<input :value="node.data.config.routeId" @input="($event.target as HTMLInputElement).value?node.data.config.routeId=($event.target as HTMLInputElement).value:delete node.data.config.routeId" placeholder="bridge/connexion"/></label><JsonField v-if="node.data.config.invocation==='condition'" v-model="node.data.config.fallback" label="Résultat si aucune route ne s’applique" :rows="2"/><p class="muted">Les bridges apportent les routes de ce point. Une route éligible est empruntée ; plusieurs routes éligibles produisent un diagnostic.</p><p v-if="node.data.config.invocation==='condition'" class="muted">Sans route éligible, le flow poursuit sa connexion et écrit la valeur de remplacement dans son champ de sortie. Le nœud Contexte suivant décide comment utiliser le résultat.</p></template>
  <label v-if="node.data.kind==='await_route'">Canal de la visite à attendre<input v-model="node.data.config.inputField"/><small>Référence retournée par une route en mode Lancer.</small></label>
  <ContextBindingEditor v-if="node.data.kind==='agent'||node.data.kind==='context'&&(formatVersion||1)>=3" :composition="composition" :node="node" :workspace-id="workspaceId" @edit="emit('context',$event)"/>
  <template v-if="['agent','model'].includes(node.data.kind)">
    <div v-if="node.data.config.provider==='codex'" class="auth-card">
      <span :class="['live-dot',{offline:!auth?.authenticated}]"/> {{auth?.authenticated?'Abonnement ChatGPT connecté':'Connexion Codex'}}
      <p class="muted">{{auth?.message || authError || 'Lecture du statut sur le daemon…'}}</p>
      <p class="muted">Authentification sur la machine du daemon : <code>{{auth?.loginCommand||'codex login'}}</code>. Les identifiants restent sur cette machine.</p>
      <button @click="checkAuth">Vérifier la connexion</button>
      <small>Transport compatible Pi · endpoint Codex susceptible d’évoluer.</small>
    </div>
    <details class="settings-group" open><summary>Génération</summary>
      <template v-if="node.data.config.provider!=='codex'">
        <div class="field-pair"><label>Température<input type="number" min="0" max="2" step="0.1" :value="node.data.config.temperature" placeholder="Défaut modèle" @input="number('temperature',$event)"/></label><label>Top P<input type="number" min="0" max="1" step="0.05" :value="node.data.config.topP" placeholder="Défaut" @input="number('topP',$event)"/></label></div>
        <div class="field-pair"><label>Top K<input type="number" min="1" :value="node.data.config.topK" @input="number('topK',$event)"/></label><label>Tokens maximum<input type="number" min="1" :value="node.data.config.maxOutputTokens" @input="number('maxOutputTokens',$event)"/></label></div>
        <label>Séquences d’arrêt · une par ligne<textarea :value="(node.data.config.stopSequences||[]).join('\n')" rows="2" @change="node.data.config.stopSequences=($event.target as HTMLTextAreaElement).value.split('\n').filter(Boolean)"/></label>
      </template>
      <template v-else>
        <label>Effort de raisonnement<select v-model="node.data.config.reasoningEffort"><option :value="undefined">Défaut modèle</option><option v-for="v in ['minimal','low','medium','high','xhigh']" :key="v">{{v}}</option></select></label>
        <label>Résumé du raisonnement<select v-model="node.data.config.reasoningSummary"><option :value="undefined">Défaut</option><option>auto</option><option>concise</option><option>detailed</option></select></label>
        <label>Verbosité<select v-model="node.data.config.textVerbosity"><option :value="undefined">Défaut</option><option>low</option><option>medium</option><option>high</option></select></label>
      </template>
      <label>Format de réponse<select v-model="node.data.config.responseFormat"><option value="text">Texte</option><option value="json">JSON structuré</option></select></label>
      <JsonField v-if="node.data.config.responseFormat==='json'" v-model="node.data.config.responseSchema" label="Schéma de réponse JSON"/>
    </details>
    <details class="settings-group"><summary>{{(formatVersion||1)>=2?'Champs et description':'Contexte et outils'}}</summary>
      <label>Description<textarea v-model="node.data.config.description" rows="2"/></label>
      <label v-if="(formatVersion||1)<2">Instructions globales<textarea v-model="node.data.config.globalInstructions" rows="4"/></label>
      <label>Champ d’historique<input v-model="node.data.config.historyField" placeholder="messages"/></label>
      <label>Champ des appels d’outils<input v-model="node.data.config.toolCallsField" placeholder="toolCalls"/></label>
      <template v-if="(formatVersion||1)<2"><p class="muted">Outils proposés au modèle. Un nœud Outil doit les exécuter, puis revenir vers le modèle.</p>
      <label v-for="name in supportedTools" :key="name" class="check-field"><input type="checkbox" :checked="node.data.config.tools?.includes(name)" @change="tools(name,($event.target as HTMLInputElement).checked)"/>{{name}}</label></template>
    </details>
  </template>
  <template v-if="node.data.kind==='tool'">
    <label>Outil ADK<select v-model="node.data.config.tool"><option value="execute_next_call">Exécuter le prochain appel · checkpoint par outil</option><option value="execute_calls">Exécuter les appels du modèle</option><option value="read">Lire un fichier</option><option value="write">Écrire un fichier</option><option value="edit">Modifier un fichier</option><option value="exec">Exécuter une commande</option><option value="inspect_json">Inspecter du JSON</option><option value="format_text">Formater du texte</option><option value="delay">Attente asynchrone</option></select></label>
    <template v-if="['execute_calls','execute_next_call'].includes(node.data.config.tool)"><label>Champ des appels<input v-model="node.data.config.toolCallsField" placeholder="toolCalls"/></label><label>Champ d’historique<input v-model="node.data.config.historyField" placeholder="messages"/></label></template>
    <template v-else><label>Arguments depuis l’état · facultatif<input v-model="node.data.config.inputField" placeholder="Sinon : arguments ci-dessous"/></label><JsonField v-model="node.data.config.arguments" label="Arguments JSON"/></template>
    <details class="settings-group" open><summary>Présentation dans la conversation</summary>
      <label>Rendu<select :value="node.data.config.ui?.renderer||'json'" @change="node.data.config.ui={...node.data.config.ui,renderer:($event.target as HTMLSelectElement).value}"><option value="json">JSON</option><option value="table">Tableau</option><option value="code">Code</option><option value="markdown">Texte mis en forme</option></select></label>
      <label>Titre<input :value="node.data.config.ui?.title" @input="node.data.config.ui={...node.data.config.ui,title:($event.target as HTMLInputElement).value}"/></label>
      <label v-if="node.data.config.ui?.renderer==='code'">Langage<select :value="node.data.config.ui?.language||'json'" @change="node.data.config.ui={...node.data.config.ui,language:($event.target as HTMLSelectElement).value}"><option v-for="language in ['json','typescript','javascript','rust','python','bash','text']" :key="language">{{language}}</option></select></label>
      <p class="muted">Rendu attaché au nœud. Les entrées et sorties brutes restent consultables.</p>
    </details>
  </template>
  <details v-if="!['start','end'].includes(node.data.kind)" class="settings-group"><summary>Arrivées et reprise sur erreur</summary>
    <label>Convergence des connexions<select :value="node.data.config.fanIn||'all'" @change="node.data.config.fanIn=($event.target as HTMLSelectElement).value"><option value="all">Attendre les branches · ADK</option><option value="any">Une arrivée suffit · boucle / alternatives</option></select></label><p class="muted">Une boucle modèle peut reprendre depuis un outil ou une réponse humaine. Une jonction parallèle attend ses branches.</p>
    <label class="check-field"><input type="checkbox" :checked="!!node.data.config.retry" @change="node.data.config.retry=($event.target as HTMLInputElement).checked?{maxAttempts:2,initialDelayMs:1000,maxDelayMs:60000,backoffFactor:2,jitter:0,retryOn:'any'}:null"/>Surcharger la politique du graphe</label>
    <template v-if="node.data.config.retry"><label>Tentatives maximum<input type="number" v-model.number="node.data.config.retry.maxAttempts" min="1"/></label><label>Délai initial · ms<input type="number" v-model.number="node.data.config.retry.initialDelayMs" min="0"/></label><label>Délai maximal · ms<input type="number" v-model.number="node.data.config.retry.maxDelayMs" min="0"/></label><div class="field-pair"><label>Facteur<input type="number" v-model.number="node.data.config.retry.backoffFactor" min="1" step="0.1"/></label><label>Jitter<input type="number" v-model.number="node.data.config.retry.jitter" min="0" max="1" step="0.1"/></label></div><label>Déclencheur<select v-model="node.data.config.retry.retryOn"><option value="any">Toute erreur</option><option value="timeout">Délai dépassé</option></select></label><p class="muted">Un outil avec effet externe peut être exécuté plusieurs fois.</p></template>
  </details>
</template>
