import { isNodeConfig } from './graph/contracts'
import { jsonObjectSchema, flowExportsReadSchema } from '@zedflow/sdk'

import type { RuntimeGraphSummary } from '@zedflow/sdk'

import { HttpError } from '@zedflow/sdk'
import type { PortContract, InvocationKind, DataPermission, FlowDefinition, FlowExports, Endpoint, RouteMode, BridgeConnection, BridgeDefinition, BridgeFile, RuntimeSelection, RuntimeGraph, PreparedRuntime } from '@zedflow/sdk'

import { useClient } from '@zedflow/vue'

import { useReloadDrafts, managedReload } from './composables/reloadState'
import type { Composition, FlowFile } from '@zedflow/sdk'
import type { ContextType, ContextDiagnostic } from '@zedflow/sdk'

import type { Predicate } from '@zedflow/sdk'

export function flowExports(composition?: Composition): FlowExports | undefined {
  const config=composition?.nodes.find(node=>node.data.kind==='start')?.data.config
  if(isNodeConfig(config))return config.exports
  const object=jsonObjectSchema.safeParse(config)
  const parsed=flowExportsReadSchema.safeParse(object.success?object.data.exports:undefined)
  return parsed.success?parsed.data:undefined
}
export function exportedFlows(flows: FlowFile[]) { return flows.filter(file => file.composition && flowExports(file.composition)) }
export function emptyFlowExports(): FlowExports { return { contract: { entries: {}, branches: {}, data: {}, requires: {}, inferenceNodes: {} }, types: {}, entries: {}, branches: {}, data: {}, requires: {}, interactive: false } }
export function emptyBridge(): BridgeDefinition { return { requires: [], imports: {}, connections: {}, bindings: {} } }
export const builtInChannels = ['input', 'output', 'response', 'messages', 'toolCalls', 'toolResults', 'hasToolCalls', 'modelResponse', 'hasSteering', 'hasFollowUp']
export const routeModes: { value: RouteMode; label: string; detail: string }[] = [{ value: 'callAwait', label: 'Appeler et attendre', detail: 'Le flow appelant reprend avec le résultat.' }, { value: 'launch', label: 'Lancer', detail: 'Le flow appelant obtient une identité de visite.' }, { value: 'handoff', label: 'Transférer', detail: 'Le flow destinataire poursuit le travail.' }]

import { computed, onUnmounted, reactive, watch, type Ref } from 'vue'
import { cloneContext, contextId } from './contextEngine'

export interface BridgeDraft { key:string;bridge:BridgeDefinition;file?:BridgeFile;saved:string;pending:boolean;error:string;notice:string;rootFlow:string }
interface BridgeSession { files:BridgeFile[];drafts:Record<string,BridgeDraft>;selected:string;error:string;loading:boolean }
export function useBridgeStudio(workspaceId:Ref<string>,active:Ref<boolean>){
  const client=useClient()
  const sessions=reactive<Record<string,BridgeSession>>({})
  useReloadDrafts('bridge-drafts',sessions)
  function blank():BridgeDraft{return{key:contextId('bridge'),bridge:emptyBridge(),saved:'',pending:false,error:'',notice:'',rootFlow:''}}
  function state(id=workspaceId.value){return sessions[id]||={files:[],drafts:{new:blank()},selected:'new',error:'',loading:false}}
  const session=computed(()=>state()),current=computed(()=>session.value.drafts[session.value.selected])
  async function refresh(){const id=workspaceId.value,s=state(id);if(!id||s.loading)return;s.loading=true;s.error='';try{const files=await client.composition.listBridges({workspaceId:id});s.files=Array.isArray(files)?files:[]}catch(cause){s.error=String(cause instanceof Error?cause.message:cause)}finally{s.loading=false}}
  async function open(file:BridgeFile,reload=false){const s=state(),id=workspaceId.value;if(s.drafts[file.key]&&!reload){s.selected=file.key;return}s.selected=file.key;const draft=s.drafts[file.key]||=blank();draft.pending=true;try{const value=await client.composition.readBridge(file.key,{workspaceId:id});draft.file=value;draft.key=value.key;draft.bridge=cloneContext(value.bridge||emptyBridge());draft.saved=JSON.stringify(draft.bridge);draft.error=value.diagnostics.map(item=>item.message).join('; ')}catch(cause){draft.error=String(cause instanceof Error?cause.message:cause)}finally{draft.pending=false}}
  function create(copy=false){const draft=blank();if(copy){draft.bridge=cloneContext(current.value.bridge);draft.rootFlow=current.value.rootFlow}const key=`draft:${draft.key}`;session.value.drafts[key]=draft;session.value.selected=key}
  async function save(){const draft=current.value,s=state(),id=workspaceId.value,slot=s.selected;if(draft.pending)return;draft.error='';draft.notice='';draft.pending=true;const bridge=cloneContext(draft.bridge),key=draft.key;try{const file=await client.composition.saveBridge({workspaceId:id,key,bridge,...(draft.file?{expectedHash:draft.file.hash}:{})});draft.file=file;draft.saved=JSON.stringify(bridge);draft.notice='Bridge enregistré en Rust';s.files=[...s.files.filter(item=>item.key!==file.key),file];if(slot!==file.key){s.drafts[file.key]=draft;delete s.drafts[slot];if(s.selected===slot)s.selected=file.key}}catch(cause){draft.error=String(cause instanceof Error?cause.message:cause);if(cause instanceof HttpError&&cause.status===409)draft.error+=' · Le brouillon est conservé. Rechargez ou dupliquez le bridge.'}finally{draft.pending=false}}
  watch([workspaceId,active],()=>{if(active.value)void refresh()},{immediate:true})
  const onFocus=()=>{if(active.value)void refresh()};window.addEventListener('focus',onFocus);onUnmounted(()=>window.removeEventListener('focus',onFocus))
  const beforeUnload=(event:BeforeUnloadEvent)=>{if(!managedReload&&Object.values(sessions).some(s=>Object.values(s.drafts).some(d=>JSON.stringify(d.bridge)!==d.saved&&(Object.keys(d.bridge.imports).length||Object.keys(d.bridge.connections).length))))event.preventDefault()};window.addEventListener('beforeunload',beforeUnload);onUnmounted(()=>window.removeEventListener('beforeunload',beforeUnload))
  return reactive({session,current,workspaceId,refresh,open,create,save})
}
export type BridgeStudioController=ReturnType<typeof useBridgeStudio>

import type { ModelNode } from './harness'
import type { ModelSelection } from '@zedflow/sdk'

export interface RuntimePreparation { workspaceId:string;selection:RuntimeSelection;overview:RuntimeGraphSummary;models:ModelNode[];bindings:Record<string,ModelSelection>;inputField:string;inputType:ContextType }
export function runtimeModelNodes(overview:RuntimeGraphSummary):ModelNode[]{return Object.entries(overview.inferences).map(([path,item])=>({path,group:overview.instances[item.instance]?.name||item.instance,runtime:item.config.modelBinding==='runtime',contextPath:item.contextPath,context:item.context?{id:item.context.node,type:'flow',position:{x:0,y:0},data:{kind:'context',label:item.context.label,config:item.context.config}}:undefined,node:{id:item.node,type:'flow',position:{x:0,y:0},data:{kind:item.config.contextNode?'model':'agent',label:item.label,config:item.config}}}))}
/** Explicit public contracts govern interaction; legacy flows expose input/inbox. */
export function flowIsInteractive(doc?:Composition):boolean{
  if(!doc)return true
  const declared=flowExports(doc)
  if(declared)return declared.interactive===true
  return doc.nodes.some(node=>['input','inbox'].includes(node.data.kind)||node.data.kind==='subgraph'&&isNodeConfig(node.data.config)&&flowIsInteractive(node.data.config.composition))
}
