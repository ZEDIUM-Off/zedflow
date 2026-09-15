import { nodeConfigSchema, type NodeConfig } from '@zedflow/sdk'
import { useClient } from '@zedflow/vue'
import type { NodePort, NodeContract, EdgeAnalysis, GraphAnalysis } from '@zedflow/sdk'
import { ref, watch, type Ref } from 'vue'
import type { Composition } from '@zedflow/sdk'

export function useGraphAnalysis(doc:Ref<Composition>){
  const client=useClient()
  const analysis=ref<GraphAnalysis>(),error=ref('');let intent=0
  watch(()=>JSON.stringify({id:doc.value.id,nodes:doc.value.nodes.map(({id,data})=>({id,data})),edges:doc.value.edges,channels:doc.value.channels}),(_,__,cleanup)=>{
    const request=++intent
    const timer=setTimeout(async()=>{try{const value=await client.flows.analyze(doc.value);if(request===intent){analysis.value=value;error.value=''}}catch(cause){if(request===intent)error.value=String(cause)}},180)
    cleanup(()=>clearTimeout(timer))
  },{immediate:true})
  return {analysis,error}
}

/** Validate the interpretation without replacing or stripping the saved raw config. */
export function isNodeConfig(value: unknown): value is NodeConfig {
  return nodeConfigSchema.safeParse(value).success
}
