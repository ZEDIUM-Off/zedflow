import { computed, onUnmounted, ref, shallowRef, watch } from 'vue'
import type { ExecutedDefinition, Run, RunRevisions } from '@zedflow/sdk'
import { useClient } from '@zedflow/vue'

export function useExecutedDefinition(run:()=>Run|null, open:()=>boolean, nodePath:()=>string, occurrence:()=>string|undefined) {
  const client=useClient()
  const definition=shallowRef<ExecutedDefinition>(),revisions=shallowRef<RunRevisions>()
  const loading=ref(false),error=ref('')
  let intent=0,revisionIntent=0,timer:ReturnType<typeof setTimeout>|undefined
  const latest=computed(()=>occurrence()||(!nodePath()?run()?.activities?.at(-1)?.occurrenceId:run()?.activities?.filter(item=>item.path===nodePath()).at(-1)?.occurrenceId))
  async function load(){
    const current=run(),request=++intent
    if(!open()||!current)return
    const id=latest.value,path=nodePath()
    loading.value=true;error.value='';definition.value=client.definitions.entry({runId:current.id,workspaceId:current.workspaceId||'',query:{nodePath:path,...(id?{occurrenceId:id}:{})}})?.value
    try{
      const value=await client.definitions.load({runId:current.id,workspaceId:current.workspaceId||'',query:{nodePath:path,...(id?{occurrenceId:id}:{})}})
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
  watch(()=>[open(),run()?.workspaceId,run()?.id,nodePath(),latest.value],()=>{void load()},{immediate:true})
  watch(()=>[open(),run()?.workspaceId,run()?.id,run()?.revision],()=>{if(!open())return;if(timer)return;timer=setTimeout(()=>{timer=undefined;void refresh()},150)},{immediate:true})
  onUnmounted(()=>{intent++;revisionIntent++;clearTimeout(timer)})
  return {definition,revisions,loading,error,refresh}
}

/** UI selection may target a node before an occurrence exists. */
export interface InspectionSelection { nodePath:string; occurrenceId?:string }
