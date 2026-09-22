import { computed, onUnmounted, ref, shallowRef, watch } from 'vue'
import type { DefinitionQuery, DefinitionRequest, ExecutedDefinition, Run, RunRevisions, RunSummary } from '@zedflow/sdk'
import { useClient } from '@zedflow/vue'

export function executedSourceQuery(nodePath: string, occurrenceId?: string): Pick<DefinitionQuery, 'nodePath' | 'occurrenceId'> {
  if (!nodePath) {
    if (occurrenceId) throw new Error('A passage selection requires its node path');
    return {};
  }
  return { nodePath, ...(occurrenceId ? { occurrenceId } : {}) };
}

/** Cache only a captured admission identity, not a mutable latest read or a bare run ID. */
export function executedDefinitionRequest(run: RunSummary, nodePath = '', occurrenceId?: string): DefinitionRequest {
  const query = executedSourceQuery(nodePath, occurrenceId)
  const graph = typeof run.runtimeGraphRef === 'string' ? run.runtimeGraphRef : undefined
  const source = typeof run.flowSourceRef === 'string' ? run.flowSourceRef : undefined
  const composition = typeof run.compositionRef === 'string' ? run.compositionRef : undefined
  const revision = !nodePath && (graph || source && composition)
    ? JSON.stringify(['initial', graph, source, composition, run.flowPackageRef, run.createdAt, run.import])
    : undefined
  return {runId:run.id,workspaceId:run.workspaceId||'',query,...(revision ? {revision} : {})}
}

export function useExecutedDefinition(run:()=>Run|null, open:()=>boolean, nodePath:()=>string, occurrence:()=>string|undefined) {
  const client=useClient()
  const definition=shallowRef<ExecutedDefinition>(),revisions=shallowRef<RunRevisions>()
  const loading=ref(false),error=ref('')
  let intent=0,revisionIntent=0,timer:ReturnType<typeof setTimeout>|undefined
  const latest=computed(()=>nodePath()?(occurrence()||run()?.activities?.filter(item=>item.path===nodePath()).at(-1)?.occurrenceId):undefined)
  async function load(){
    const current=run(),request=++intent
    if(!open()||!current)return
    const selection=executedDefinitionRequest(current,nodePath(),latest.value)
    loading.value=true;error.value='';definition.value=client.definitions.entry(selection)?.value
    try{
      const value=await client.definitions.load(selection)
      if(request!==intent)return
      if(typeof value?.exact==='boolean'){definition.value=value}
    }catch(cause){if(request===intent)error.value=cause instanceof Error?cause.message:String(cause)}
    finally{if(request===intent)loading.value=false}
  }
  async function refresh(){
    const current=run(),request=++revisionIntent
    if(!open()||!current)return
    try{const value=await client.runs.revisions(current.id,{workspaceId:current.workspaceId||''});if(request===revisionIntent&&Array.isArray(value?.instances))revisions.value=value}catch(cause){if(request===revisionIntent)error.value=cause instanceof Error?cause.message:String(cause)}
  }
  watch(()=>[run()?.id,run()?.workspaceId],()=>{intent++;revisionIntent++;definition.value=undefined;revisions.value=undefined;error.value=''})
  watch(()=>[open(),run()?.workspaceId,run()?.id,nodePath(),latest.value,run()&&executedDefinitionRequest(run()!).revision],()=>{void load()},{immediate:true})
  watch(()=>[open(),run()?.workspaceId,run()?.id,run()?.revision],()=>{if(!open())return;if(timer)return;timer=setTimeout(()=>{timer=undefined;void refresh()},150)},{immediate:true})
  onUnmounted(()=>{intent++;revisionIntent++;clearTimeout(timer)})
  return {definition,revisions,loading,error,refresh}
}

/** UI selection may target a node before an occurrence exists. */
export interface InspectionSelection { nodePath:string; occurrenceId?:string }
