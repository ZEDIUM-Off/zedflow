<script setup lang="ts">
import { useClient } from '@zedflow/vue'
const client=useClient()
import { useGraphAnalysis } from '../graph/contracts'

import { computed, nextTick, onBeforeUnmount, ref, shallowRef, watch } from 'vue'
import { VueFlow, type Connection, type NodeDragEvent, type VueFlowStore } from '@vue-flow/core'
import { Background } from '@vue-flow/background'
import { Controls } from '@vue-flow/controls'
import { MiniMap } from '@vue-flow/minimap'
import { Plus, Settings2, X, ArrowRight, Layers } from 'lucide-vue-next'
import type { Composition, Kind, WorkspaceContext } from '@zedflow/sdk'
import FlowNode from './FlowNode.vue'
import NodeSettings from './NodeSettings.vue'
import { catalog, separateContextNodes } from '../templates'
import GraphRouting from './GraphRouting.vue'
import OrthogonalEdge from './OrthogonalEdge.vue'
import AttachmentEditor from './AttachmentEditor.vue'
import PredicateEditor from './PredicateEditor.vue'
import JsonField from './JsonField.vue'
import type { RoutedEdge } from '../graph/routing'
import type { AttachmentSlot } from '../graph/attachments'
import type { Predicate } from '@zedflow/sdk'
import '../graph.css'
const doc = defineModel<Composition>({required:true})
const propertiesOpen=defineModel<boolean>('propertiesOpen',{default:true})
const props=withDefaults(defineProps<{saved:Composition[];paletteOpen:boolean;context?:WorkspaceContext|null;workspaceId?:string;active?:boolean}>(),{active:true})
const emit=defineEmits<{context:[key?:string];convert:[composition:Composition]}>()
const viewport=shallowRef<VueFlowStore>()
let cameraReady=false,cameraFrame=0,sizeFrame=0
function fitInitialView(){
  if(cameraReady||!props.active)return
  cancelAnimationFrame(cameraFrame);cancelAnimationFrame(sizeFrame)
  sizeFrame=requestAnimationFrame(()=>{cameraFrame=requestAnimationFrame(async()=>{
    const store=viewport.value
    if(cameraReady||!props.active||!store?.dimensions.value.width||!store.getNodes.value.every(node=>node.dimensions.width&&node.dimensions.height))return
    cameraReady=await store.fitView({padding:0.2,maxZoom:1,duration:0})
  })})
}
function graphReady(store:VueFlowStore){viewport.value=store;fitInitialView()}
watch(()=>props.active,fitInitialView)
onBeforeUnmount(()=>{cancelAnimationFrame(cameraFrame);cancelAnimationFrame(sizeFrame)})
const selected = ref('model')
const propertyPanel=ref<HTMLElement>()
const attachmentSlot=ref<AttachmentSlot>('instructions')
function storedSnap(){try{return localStorage.getItem('zedflow:graph-snap')}catch{return null}}
const snapToGrid=ref(storedSnap()===null?(doc.value.formatVersion||1)>=2:storedSnap()==='true')
function setSnap(event:Event){snapToGrid.value=(event.target as HTMLInputElement).checked;try{localStorage.setItem('zedflow:graph-snap',String(snapToGrid.value))}catch{}}
const routes=shallowRef<Record<string,RoutedEdge>>({}), routingError=ref('')
const entries=computed(()=>catalog.filter(entry=>{const version=doc.value.formatVersion||1;return entry.kind==='context'?version!==2:['model','route','await_route'].includes(entry.kind)?version>=3:true}))
const connectionError=ref('')
const pairedNode=computed(()=>{const node=selectedNode.value;return node?doc.value.nodes.find(candidate=>candidate.id===(node.data.kind==='context'?node.data.config.modelNode:node.data.config.contextNode)):undefined})
const canSeparate=computed(()=>doc.value.nodes.some(node=>node.data.kind==='agent'))
function convert(){try{emit('convert',separateContextNodes(doc.value));connectionError.value=''}catch(cause){connectionError.value=cause instanceof Error?cause.message:String(cause)}}
function inspectPair(){if(pairedNode.value)selected.value=pairedNode.value.id}
const selectedNode = computed(()=>doc.value.nodes.find(node=>node.id===selected.value))
const predicate=computed<Predicate>({get:()=>selectedNode.value?.data.config.predicate||{kind:'compare',field:selectedNode.value?.data.config.field||'input',operator:'eq',value:selectedNode.value?.data.config.equals},set:value=>{if(selectedNode.value){selectedNode.value.data.config.predicate=value;delete selectedNode.value.data.config.equals;delete selectedNode.value.data.config.field}}})
watch(()=>doc.value.id,()=>{routes.value={};if(storedSnap()===null)snapToGrid.value=(doc.value.formatVersion||1)>=2;selected.value=doc.value.nodes.find(node=>['context','model','agent'].includes(node.data.kind))?.id||doc.value.nodes[0]?.id||''})
async function attachment(id:string,slot:AttachmentSlot){selected.value=id;attachmentSlot.value=slot;propertiesOpen.value=true;await nextTick();propertyPanel.value?.querySelector('.attachment-editor')?.scrollIntoView({block:'nearest'})}
function freePosition(point:{x:number;y:number},width=220){
  const candidate={...point}
  const overlaps=()=>doc.value.nodes.some(node=>{const nodeWidth=node.data.kind==='agent'&&(doc.value.formatVersion||1)>=2?320:['start','end'].includes(node.data.kind)?140:220;const nodeHeight=['agent','condition'].includes(node.data.kind)?176:112;return candidate.x<node.position.x+nodeWidth+24&&candidate.x+width+24>node.position.x&&candidate.y<node.position.y+nodeHeight+24&&candidate.y+136>node.position.y})
  while(overlaps())candidate.y+=176
  return candidate
}
function add(kind:Kind){
  const entry=catalog.find(node=>node.kind===kind)
  if(!entry)return
  const id=crypto.randomUUID(),config=structuredClone(entry.config)
  if(kind==='context'&&(doc.value.formatVersion||1)<3)for(const key of Object.keys(config))delete config[key]
  if(kind==='condition'&&(doc.value.formatVersion||1)<2){delete config.predicate;config.field='input';config.equals='oui'}
  const anchor=selectedNode.value
  const hasFreeContext=kind==='model'&&anchor?.data.kind==='context'&&!doc.value.edges.some(edge=>edge.source===anchor.id)
  const position=freePosition(anchor?{x:anchor.position.x+300,y:anchor.position.y}:{x:280,y:160},kind==='model'&&!hasFreeContext?520:220)
  const node={id,type:'flow' as const,position,data:{kind,label:entry.label,config}}
  doc.value.nodes.push(node)
  if(kind==='model'){
    let context=hasFreeContext?anchor:undefined
    if(!context){const contextId=crypto.randomUUID(),definition=catalog.find(item=>item.kind==='context')!;context={id:contextId,type:'flow',position:{...position},data:{kind:'context',label:definition.label,config:structuredClone(definition.config)}};doc.value.nodes.push(context);node.position={x:position.x+300,y:position.y}}
    context.data.config.modelNode=id;config.contextNode=context.id
    doc.value.edges.push({id:crypto.randomUUID(),source:context.id,target:id})
  }
  selected.value=id;propertiesOpen.value=true;connectionError.value=''
}
// Vue Flow's global snap flag also rounds untouched nodes when their bounds update.
// Restrict alignment to the nodes explicitly moved by this gesture.
function snapDragged(event:NodeDragEvent){if(snapToGrid.value)for(const moved of event.nodes){const node=doc.value.nodes.find(value=>value.id===moved.id);if(node)node.position={x:Math.round(moved.position.x/16)*16,y:Math.round(moved.position.y/16)*16}}}
const {analysis:contractAnalysis}=useGraphAnalysis(doc)
const selectedEdge=ref('')
function copyWithPorts(){
  const copy=JSON.parse(JSON.stringify(doc.value)) as Composition
  copy.id=crypto.randomUUID();copy.name+=' · ports v4';copy.revision=0
  function convertPorts(graph:Composition):boolean{
    if(graph.nodes.some(node=>node.data.kind==='agent'))return false
    graph.formatVersion=4
    for(const node of graph.nodes)if(node.data.kind==='subgraph'&&node.data.config.composition&&!convertPorts(node.data.config.composition))return false
    for(const edge of graph.edges){const source=graph.nodes.find(node=>node.id===edge.source);const target=graph.nodes.find(node=>node.id===edge.target);edge.sourceHandle=source?.data.kind==='condition'?edge.sourceHandle:source?.data.kind==='context'?'context':'state';edge.targetHandle=target?.data.kind==='model'?'context':'state'}
    return true
  }
  if(!convertPorts(copy)){connectionError.value='Séparez les nœuds Agent historiques en Contexte et Modèle avant la conversion des ports.';return}
  emit('convert',copy)
}
const edgeAnalysis=computed(()=>contractAnalysis.value?.edges?.find(edge=>edge.edgeId===selectedEdge.value))

async function connect(connection:Connection){
  if(!connection.source||!connection.target)return
  const connectingDoc=doc.value
  const source=doc.value.nodes.find(node=>node.id===connection.source)!,target=doc.value.nodes.find(node=>node.id===connection.target)!
  connectionError.value=''
  if((doc.value.formatVersion||1)>=4){
    try {
      const id=crypto.randomUUID()
      const analysis=await client.flows.analyze({...doc.value,edges:[...doc.value.edges,{id,source:connection.source,target:connection.target,sourceHandle:connection.sourceHandle,targetHandle:connection.targetHandle}]})
      if(doc.value!==connectingDoc)return
      const errors=analysis.diagnostics.filter(item=>item.path===`edges.${id}`)
      if(errors.length){connectionError.value=errors.map(item=>item.message).join('; ');return}
    }catch(cause){connectionError.value=String(cause);return}
  }
  if((doc.value.formatVersion||1)>=3){
    if(target.data.kind==='model'&&source.data.kind!=='context'){connectionError.value='Revenez au nœud Contexte : chaque appel modèle consomme une nouvelle préparation.';return}
    if(source.data.kind==='context'){
      if(target.data.kind!=='model'){connectionError.value='La sortie du contexte se relie directement à son modèle.';return}
      if(doc.value.edges.some(edge=>edge.source===source.id||edge.target===target.id)){connectionError.value='Cette paire est déjà reliée. Retirez sa connexion avant de choisir un autre modèle.';return}
      source.data.config.modelNode=target.id;target.data.config.contextNode=source.id
    }
  }
  if(doc.value.edges.some(edge=>edge.source===connection.source&&edge.target===connection.target&&edge.sourceHandle===connection.sourceHandle))return
  doc.value.edges.push({id:crypto.randomUUID(),source:connection.source,target:connection.target,sourceHandle:connection.sourceHandle,...((doc.value.formatVersion||1)>=4?{targetHandle:connection.targetHandle}:{})})
}
function remove(){doc.value.nodes=doc.value.nodes.filter(node=>node.id!==selected.value);doc.value.edges=doc.value.edges.filter(edge=>edge.source!==selected.value&&edge.target!==selected.value);selected.value=''}
function updateConfig(key:string,value:unknown){if(selectedNode.value)selectedNode.value.data.config[key]=value}
function attachChild(id:string){const child=props.saved.find(flow=>flow.id===id);if(selectedNode.value&&child)selectedNode.value.data.config.composition=JSON.parse(JSON.stringify(child))}
</script>
<template>
      <div class="designer">
        <aside v-if="paletteOpen" class="palette"><div class="panel-title">Nœuds du flow</div><p class="muted">Ajouter une étape</p><p v-if="(doc.formatVersion||1)>=3" class="graph-pair-note"><span class="graph-pair-sequence">Contexte <ArrowRight :size="11"/> Modèle</span>Préparez les ressources, puis appelez le modèle. Chaque boucle repasse par le contexte.</p><button v-for="entry in entries" :key="entry.kind" @click="add(entry.kind)" :class="['palette-item',entry.kind]"><span class="type-dot"/><span>{{ entry.label }}<small>{{ entry.description }}</small></span><Plus :size="13"/></button><div class="palette-note">La boucle appartient au graphe.<br/>Un appel modèle représente une itération.</div></aside>
        <div class="canvas"><button v-if="doc.formatVersion===3" class="ports-conversion" @click="copyWithPorts">Créer une copie avec ports typés</button><div v-if="canSeparate" class="graph-legacy-banner"><span>Cette définition utilise encore un Agent historique. Convertissez-la pour préparer le contexte puis appeler le modèle.</span><button @click="convert">Séparer contexte et modèle</button></div><VueFlow :id="`design-${doc.id}`" :key="doc.id" v-model:nodes="doc.nodes" v-model:edges="doc.edges" :snap-to-grid="false" :snap-grid="[16,16]" @init="graphReady" @nodes-initialized="fitInitialView" :min-zoom="0.2" :max-zoom="2" @connect="connect" @edge-click="selectedEdge=$event.edge.id" @node-drag-stop="snapDragged" @selection-drag-stop="snapDragged" @node-click="selected=$event.node.id;propertiesOpen=true"><GraphRouting @routes="routes=$event" @error="routingError=$event"/><Background pattern-color="#373737" :gap="16"/><Controls/><MiniMap :pannable="true" :zoomable="true"/><template #node-flow="nodeProps"><FlowNode :contract="contractAnalysis?.nodes?.find(item=>item.nodeId===nodeProps.id)" v-bind="nodeProps" :format-version="doc.formatVersion" :historical="nodeProps.data.kind==='agent'" @attachment="attachment(nodeProps.id,$event)"/></template><template #edge-default="edgeProps"><OrthogonalEdge v-bind="edgeProps" :route="routes[edgeProps.id]"/></template></VueFlow><label class="graph-grid-toggle" title="Aligner les nœuds déplacés au relâchement"><input :checked="snapToGrid" type="checkbox" @change="setSnap"/>Aligner sur la grille</label><aside v-if="edgeAnalysis" class="edge-data-inspector" aria-label="Données de la connexion"><button @click="selectedEdge=''">Fermer</button><strong>État disponible à l’arrivée</strong><p v-if="edgeAnalysis.junction">Jonction : ces canaux peuvent venir de plusieurs prédécesseurs.</p><p>Garantis : {{edgeAnalysis.guaranteed.join(', ')||'aucun établi'}}</p><p>Conditionnels : {{edgeAnalysis.conditional.join(', ')||'aucun établi'}}</p><p>Consommés : {{edgeAnalysis.consumes.join(', ')||'selon la configuration'}}</p><p>Produits par la source : {{edgeAnalysis.produces.join(', ')||'aucun établi'}}</p><small v-if="edgeAnalysis.unknown">Les valeurs fournies à l’exécution restent inconnues dans cette vue.</small></aside><div class="canvas-caption">Reliez les poignées · Suppr pour retirer une connexion</div><p v-if="routingError||connectionError" class="graph-routing-error" role="alert">{{routingError||connectionError}}<button v-if="connectionError" class="icon-button" aria-label="Fermer le diagnostic du graphe" @click="connectionError=''"><X :size="13"/></button></p></div>
        <aside v-if="propertiesOpen" ref="propertyPanel" class="inspector graph-properties"><div class="panel-title"><Settings2 :size="15"/> Propriétés<button class="icon-button properties-close" aria-label="Fermer les propriétés" @click="propertiesOpen=false"><X :size="14"/></button></div><template v-if="selectedNode"><div :class="['node-tag',selectedNode.data.kind]">{{ selectedNode.data.kind }}</div><template v-if="selectedNode.data.kind==='agent'"><p class="graph-pair-note">Cet ancien nœud réunit ressources et inférence. Sa configuration est conservée pour la lecture. La conversion crée une nouvelle définition avec Contexte → Modèle.</p><button @click="convert">Créer la définition séparée</button><details class="raw"><summary>Configuration historique</summary><pre>{{ JSON.stringify(selectedNode.data.config,null,2) }}</pre></details></template><template v-else><label>Nom du nœud<input v-model="selectedNode.data.label"/></label><label v-if="!['start','end','output','condition','context'].includes(selectedNode.data.kind)">Champ de sortie<input :value="selectedNode.data.config.field" @input="updateConfig('field',($event.target as HTMLInputElement).value)"/></label>
        <template v-if="selectedNode.data.kind==='model'"><div v-if="selectedNode.data.kind==='model'" class="graph-context-link"><Layers :size="14"/><span>{{pairedNode?.data.label||'Contexte manquant'}}</span><button v-if="pairedNode" @click="inspectPair">Configurer</button></div><label>Choix du modèle<select :value="selectedNode.data.config.modelBinding || 'fixed'" @change="updateConfig('modelBinding',($event.target as HTMLSelectElement).value)"><option value="fixed">Fixé dans le flow</option><option value="runtime">À choisir à l’exécution</option></select></label><template v-if="selectedNode.data.config.modelBinding!=='runtime'"><label>Fournisseur<select v-model="selectedNode.data.config.provider"><option value="fixture">Démonstration · sans réseau</option><option value="gemini">Gemini · GOOGLE_API_KEY</option><option value="codex">Codex · abonnement ChatGPT</option></select></label><label v-if="selectedNode.data.config.provider!=='fixture'">Modèle<input v-model="selectedNode.data.config.model" placeholder="Identifiant de modèle"/></label></template><p v-else class="muted">Le modèle et la réflexion sont choisis dans Exécution, avant le lancement ou lorsque ce nœud est atteint.</p><label>Champ d’entrée<input v-model="selectedNode.data.config.inputField"/></label></template>
        <template v-if="selectedNode.data.kind==='context'&&(doc.formatVersion||1)>=3"><p class="graph-pair-note">Ce passage sélectionne les ressources, applique la stratégie et fige la fenêtre utilisée par le modèle suivant.</p><div class="graph-context-link"><ArrowRight :size="14"/><span>{{pairedNode?.data.label||'Modèle à relier'}}</span><button v-if="pairedNode" @click="inspectPair">Configurer</button><button v-else @click="add('model')">Ajouter le modèle</button></div></template>
        <template v-if="selectedNode.data.kind==='subgraph'"><label>Composition enregistrée<select :value="selectedNode.data.config.composition?.id || ''" @change="attachChild(($event.target as HTMLSelectElement).value)"><option value="" disabled>Choisir une composition</option><option v-for="child in saved.filter(c=>c.id!==doc.id)" :key="child.id" :value="child.id">{{child.name}} · r{{child.revision}}</option></select></label><p class="muted">Entrée input → input. Sortie response → output. Version embarquée et isolée.</p></template><label v-if="selectedNode.data.kind==='set'">Valeur / template<textarea v-model="selectedNode.data.config.value" rows="5"/></label>
        <label v-if="selectedNode.data.kind==='output'">Contenu<textarea v-model="selectedNode.data.config.text" rows="5"/></label>
        <template v-if="selectedNode.data.kind==='condition'"><template v-if="(doc.formatVersion||1)>=2"><PredicateEditor :key="selectedNode.id" v-model="predicate"/><p class="muted">Les valeurs sont comparées sans conversion de type. Utilisez une clé ou un JSON Pointer pour les champs imbriqués.</p></template><template v-else><label>Champ à comparer<input v-model="selectedNode.data.config.field"/></label><JsonField v-model="selectedNode.data.config.equals" label="Valeur attendue JSON" :rows="2"/></template></template>
        <template v-if="['input','inbox'].includes(selectedNode.data.kind)"><label>Question<textarea v-model="selectedNode.data.config.prompt" rows="4"/></label><label>Type de réponse<select v-model="selectedNode.data.config.responseType"><option value="text">Texte libre</option><option value="confirmation">Confirmation</option></select></label></template>
        <NodeSettings :key="selectedNode.id" :node="selectedNode" :composition="doc" :format-version="doc.formatVersion" :workspace-id="workspaceId" @context="emit('context',$event)"/><details v-if="selectedNode.data.kind==='context'&&(doc.formatVersion||1)>=3" class="context-sources"><summary>Sources et capacités accordées</summary><p class="graph-pair-note">Ces ressources sont disponibles pour la stratégie. Son programme décide de leur présence dans la fenêtre.</p><AttachmentEditor :key="selectedNode.id" v-model="selectedNode.data.config.attachments" v-model:slot="attachmentSlot" :context="context"/></details><details class="raw"><summary>Configuration JSON</summary><pre>{{ JSON.stringify(selectedNode.data.config,null,2) }}</pre></details><button class="danger" @click="remove">Supprimer le nœud</button></template></template><p v-else class="muted">Sélectionnez un nœud pour modifier ses propriétés.</p></aside>
      </div>
</template>

<style scoped>.ports-conversion{position:absolute;top:12px;left:16px;z-index:10}.edge-data-inspector{position:absolute;bottom:36px;left:20px;max-width:420px;background:#202126;border:1px solid #454750;border-radius:8px;padding:14px;z-index:10;font-size:12px}.edge-data-inspector strong{display:block}.edge-data-inspector button{float:right}.edge-data-inspector p,.edge-data-inspector small{color:#b6b6bf}</style>
