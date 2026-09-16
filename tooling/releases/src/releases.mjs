import { resolve, join } from 'node:path'
import { readFile, mkdir, rm } from 'node:fs/promises'
import { spawn } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import { prepare, verifyRelease, readJSON, optionalJSON, atomicJSON } from './lib/releases.mjs'
const [command,...arguments_] = process.argv.slice(2)
function option(name,fallback){const index=arguments_.indexOf(name);return index<0?fallback:arguments_[index+1]}
const root=resolve(option('--root','.zedflow/app-updates'))
if(command==='prepare') {
  const release=await prepare(root);console.log(`Release prête : ${release.releaseId}\nClient : ${release.client.buildId}\nDaemon : ${release.daemon.buildId}\nActivation explicite depuis Versions et mises à jour.`)
} else if(command==='run') await supervise()
else throw new Error('Usage : node tooling/releases/src/releases.mjs prepare|run --root <dossier> [--workspace <dossier> --data <dossier> --listen 127.0.0.1:3142]')
async function supervise(){
  await mkdir(root,{recursive:true})
  const lock=join(root,'supervisor.lock'),token=randomUUID()
  try {await mkdir(lock)} catch(error){
    if(error.code!=='EEXIST')throw error
    const owner=await optionalJSON(join(lock,'owner.json'));if(!owner?.pid)throw new Error('Verrou de superviseur incomplet ; vérifier le processus avant de le retirer')
    try{process.kill(owner.pid,0);throw new Error('Un superviseur est déjà actif')}catch(e){if(e.code!=='ESRCH')throw e}
    await rm(lock,{recursive:true});await mkdir(lock)
  }
  await atomicJSON(join(lock,'owner.json'),{pid:process.pid,token})
  const args=arguments_.filter((_,index)=>arguments_[index]!=='--root'&&arguments_[index-1]!=='--root')
  if(args.includes('--web')||args.includes('--build-info'))throw new Error('Le superviseur choisit le client de la release')
  const listen=option('--listen','127.0.0.1:3142'),endpoint=`http://${listen}`
  let stopping=false,child=null,heartbeat=null
  const stop=()=>{stopping=true;child?.kill('SIGINT')}
  process.on('SIGINT',stop);process.on('SIGTERM',stop)
  let current=await optionalJSON(join(root,'current.json'))
  if(!current){const candidate=await readJSON(join(root,'candidate.json'));await verifyRelease(root,candidate.releaseId);current={releaseId:candidate.releaseId,previousReleaseId:null};await atomicJSON(join(root,'current.json'),current)}
  let active=current.releaseId,ready=false,heartbeatWrites=Promise.resolve()
  const status=async(value)=>atomicJSON(join(root,'status.json'),{...value,updatedAt:Date.now()})
  async function launch(id){
    const release=await verifyRelease(root,id)
    child=spawn(join(root,'releases',id,'daemon'),[...args,'--web',join(root,'releases',id,'web')],{stdio:'inherit',env:{...process.env,ZEDFLOW_RELEASE_ROOT:root,ZEDFLOW_RELEASE_ID:id}})
    let exited=false
    const done=new Promise(resolve=>{child.once('error',error=>{exited=true;resolve({error:String(error),code:-1})});child.once('exit',(code,signal)=>{exited=true;resolve({code,signal})})})
    const deadline=Date.now()+30_000;let healthy=false
    while(!exited&&!stopping&&Date.now()<deadline){
      try{const response=await fetch(`${endpoint}/api/version`,{signal:AbortSignal.timeout(1500),cache:'no-store'});const version=await response.json();if(response.ok&&version.releaseId===id&&version.daemon.buildId===release.daemon.buildId){healthy=true;break}}catch{}
      await new Promise(resolve=>setTimeout(resolve,250))
    }
    if(!healthy&&!exited){child.kill('SIGINT');await Promise.race([done,new Promise(resolve=>setTimeout(resolve,5000))]);if(!exited){child.kill('SIGKILL');await done}}
    return {healthy,done}
  }
  let launched=null
  try{
    const stale=await optionalJSON(join(root,'request.json'))
    if(stale){await status({phase:'recovered',error:'Activation interrompue : redémarrage de la dernière release confirmée',requestId:stale.id});await rm(join(root,'request.json'),{force:true})}
    const beat=()=>{heartbeatWrites=heartbeatWrites.then(()=>atomicJSON(join(root,'manager.json'),{pid:process.pid,token,releaseId:active,ready,updatedAt:Date.now()}));return heartbeatWrites}
    await beat();heartbeat=setInterval(()=>{void beat().catch(console.error)},2000)
    launched=await launch(active)
    if(stopping)return
    if(!launched.healthy)throw new Error('La release active ne démarre pas ; consulter les journaux du superviseur')
    ready=true;await beat()
    while(!stopping){
      const exit=await launched.done
      if(stopping)break
      if(exit.code!==75)throw new Error(`Daemon arrêté hors mise à jour (${exit.code})`)
      ready=false;await beat()
      let request
      try{
        request=await readJSON(join(root,'request.json'))
        if(request.source!==current.releaseId)throw new Error('La release active a changé depuis la demande')
        const previous=await verifyRelease(root,current.releaseId),next=await verifyRelease(root,request.target)
        if(request.expectedDaemonBuildId!==previous.daemon.buildId)throw new Error('Build source périmé')
        if(next.daemon.storageEpoch!==previous.daemon.storageEpoch)throw new Error('Migration de stockage hors ligne requise')
        await status({phase:'activating',requestId:request.id,target:request.target})
        active=request.target;await beat()
        launched=await launch(active)
        if(!launched.healthy)throw new Error('La nouvelle release ne répond pas ; retour à la précédente')
        const committed={releaseId:active,previousReleaseId:current.releaseId}
        await atomicJSON(join(root,'current.json'),committed);current=committed;ready=true;await beat()
        await status({phase:'complete',requestId:request.id,releaseId:active}).catch(console.error)
        await rm(join(root,'request.json'),{force:true}).catch(console.error)
      }catch(error){
        if(stopping)break
        // Any candidate process is stopped before reopening the previous data directory.
        if(launched?.healthy&&active!==current.releaseId){child.kill('SIGINT');await launched.done}
        active=current.releaseId;await beat()
        launched=await launch(active)
        if(!launched.healthy)throw new Error(`Échec du retour à la release précédente : ${error}`)
        ready=true;await beat()
        await status({phase:'failed',requestId:request?.id,error:String(error),restoredReleaseId:current.releaseId})
        await rm(join(root,'request.json'),{force:true})
      }
    }
  } finally {
    if(child&&child.exitCode===null){child.kill('SIGINT');await launched?.done}
    clearInterval(heartbeat);await heartbeatWrites.catch(()=>{});process.off('SIGINT',stop);process.off('SIGTERM',stop)
    await rm(join(root,'manager.json'),{force:true});await rm(lock,{recursive:true,force:true})
  }
}
