import { onUnmounted, type Ref } from 'vue'
const storageKey='zedflow.update-recovery.v1'
let initial:Record<string,unknown>={}
try{initial=JSON.parse(sessionStorage.getItem(storageKey)||'{}')}catch{}
export let managedReload=false
const captures=new Map<string,()=>unknown>()
export function useReloadPart<T>(key:string,capture:()=>T):T|undefined{
  captures.set(key,capture);onUnmounted(()=>captures.delete(key))
  const previous=initial[key] as T|undefined
  delete initial[key]
  try{sessionStorage.setItem(storageKey,JSON.stringify(initial))}catch{}
  return previous
}
export function useReloadRefs(key:string,refs:Record<string,Ref<any>>){
  const previous=useReloadPart(key,()=>Object.fromEntries(Object.entries(refs).map(([name,value])=>[name,value.value])))
  if(previous&&typeof previous==='object')for(const [name,value] of Object.entries(refs))if(Object.hasOwn(previous,name))value.value=previous[name]
}
/** One synchronous write: a quota/privacy failure must keep the current tab intact. */
export function preserveAndReload(){
  const snapshot={...initial}
  for(const [key,capture] of captures)snapshot[key]=capture()
  const encoded=JSON.stringify(snapshot)
  sessionStorage.setItem(storageKey,encoded)
  if(sessionStorage.getItem(storageKey)!==encoded)throw new Error('La sauvegarde des brouillons du navigateur a échoué ; rechargement annulé')
  // Components with additional unsaved state can cancel the reload rather than lose it.
  managedReload=true
  window.location.reload()
}

export function useReloadDrafts(key:string,states:Record<string,any>){
  const previous=useReloadPart(key,()=>states)
  if(previous&&typeof previous==='object'){
    for(const state of Object.values(previous)){
      state.loading=false
      for(const draft of Object.values(state.drafts||{}) as any[]){draft.pending=typeof draft.pending==='string'?'':false;draft.previewPending=false}
    }
    Object.assign(states,previous)
  }
}
