import { createHash, randomUUID } from 'node:crypto'
import { readFile, writeFile, readdir, mkdir, rename, lstat, open, rm, cp, chmod } from 'node:fs/promises'
import { resolve, join, relative } from 'node:path'
import { spawn } from 'node:child_process'
import { clientBuild, repository } from './build-info.mjs'
export const idValid = id => typeof id === 'string' && /^[a-f0-9]{64}$/.test(id)
export const hash = bytes => createHash('sha256').update(bytes).digest('hex')
export const readJSON = async path => JSON.parse(await readFile(path, 'utf8'))
export async function optionalJSON(path) { try { return await readJSON(path) } catch(error) { if(error.code === 'ENOENT') return null; throw error } }
export async function atomicJSON(path, value) {
  const temp = `${path}.${randomUUID()}.tmp`
  const file = await open(temp, 'wx', 0o600)
  try { await file.writeFile(JSON.stringify(value, null, 2)); await file.sync() } finally { await file.close() }
  await rename(temp, path)
  if(process.platform !== 'win32') { const dir=await open(resolve(path,'..'),'r');try{await dir.sync()}finally{await dir.close()} }
}
export async function inventory(directory) {
  const files = {}
  async function visit(path) {
    const stat = await lstat(path)
    if(stat.isSymbolicLink()) throw new Error(`Lien symbolique interdit dans une release : ${path}`)
    if(stat.isDirectory()) for(const name of (await readdir(path)).sort()) await visit(join(path,name))
    else if(stat.isFile()) files[relative(directory,path).replaceAll('\\','/')] = hash(await readFile(path))
    else throw new Error(`Fichier non ordinaire : ${path}`)
  }
  await visit(directory); return files
}
export function execute(command,args,options={}) {
  return new Promise((resolve,reject)=>{const child=spawn(command,args,{cwd:repository,stdio:'inherit',...options});child.on('error',reject);child.on('exit',(code,signal)=>code===0?resolve():reject(new Error(`${command}: ${signal || code}`)))})
}
export async function verifyRelease(root,id) {
  if(!idValid(id)) throw new Error('Identifiant de release invalide')
  const directory=join(root,'releases',id),manifest=await readJSON(join(directory,'manifest.json'))
  const {releaseId,createdAt,...body}=manifest
  if(manifest.format!==1 || releaseId!==id || hash(JSON.stringify(body))!==id) throw new Error('Identité de release altérée')
  const files=await inventory(directory);delete files['manifest.json']
  if(JSON.stringify(files)!==JSON.stringify(manifest.files)) throw new Error('Inventaire ou empreinte des fichiers de release altéré')
  if(manifest.platform!==process.platform || manifest.arch!==process.arch) throw new Error('Release destinée à une autre machine')
  if(manifest.daemon.protocol!==manifest.client.protocol) throw new Error('Protocole client/daemon incompatible')
  const client=await readJSON(join(directory,'web/client-version.json'))
  if(JSON.stringify(client)!==JSON.stringify(manifest.client)) throw new Error('Identité du client altérée')
  return manifest
}
export async function prepare(root) {
  root=resolve(root);await mkdir(join(root,'releases'),{recursive:true});await mkdir(join(root,'clients'),{recursive:true})
  async function daemonSources(){
    const values=[]
    for(const name of ['rust/crates','version.json','rust/Cargo.lock','rust/Cargo.toml','rust/rust-toolchain.toml']){
      const path=join(repository,name),stat=await lstat(path)
      values.push([name,stat.isDirectory()?await inventory(path):hash(await readFile(path))])
    }
    return hash(JSON.stringify(values))
  }
  const before=clientBuild(),daemonBefore=await daemonSources()
  await execute('pnpm',['build'],{cwd:join(repository,'web')})
  const target=resolve(process.env.CARGO_TARGET_DIR || '/tmp/zedflow-adk-target')
  await execute('cargo',['build','--locked','-p','zf-serve','--bin','zedflow-daemon'],{cwd:join(repository,'rust'),env:{...process.env,CARGO_TARGET_DIR:target}})
  const binary=join(target,'debug',process.platform==='win32'?'zedflow-daemon.exe':'zedflow-daemon')
  const daemon=await new Promise((resolve,reject)=>{let output='';const child=spawn(binary,['--build-info'],{stdio:['ignore','pipe','inherit']});child.stdout.on('data',chunk=>output+=chunk);child.on('error',reject);child.on('exit',code=>{try{if(code!==0)throw new Error('Identité du daemon indisponible');resolve(JSON.parse(output))}catch(e){reject(e)}})})
  if(await daemonSources()!==daemonBefore)throw new Error('Les sources du daemon ont changé pendant la préparation ; relancez-la')
  const client=await readJSON(join(repository,'web/apps/client/dist/client-version.json'))
  if(before.buildId!==client.buildId || clientBuild().buildId!==client.buildId) throw new Error('Les sources du client ont changé pendant la préparation ; relancez-la')
  const stage=join(root,`stage-${randomUUID()}`);await mkdir(stage)
  try {
    await cp(binary,join(stage,'daemon'));await chmod(join(stage,'daemon'),0o755)
    await cp(join(repository,'web/apps/client/dist'),join(stage,'web'),{recursive:true})
    const body={format:1,platform:process.platform,arch:process.arch,daemon,client,files:await inventory(stage)}
    const releaseId=hash(JSON.stringify(body)), manifest={releaseId,createdAt:new Date().toISOString(),...body}
    await atomicJSON(join(stage,'manifest.json'),manifest)
    if(!await optionalJSON(join(root,'releases',releaseId,'manifest.json')))await rename(stage,join(root,'releases',releaseId))
    await verifyRelease(root,releaseId)
    const clientDir=join(root,'clients',client.buildId)
    try{await lstat(clientDir)}catch(error){if(error.code!=='ENOENT')throw error;await cp(join(root,'releases',releaseId,'web'),clientDir,{recursive:true,errorOnExist:true,force:false})}
    if(JSON.stringify(await inventory(clientDir))!==JSON.stringify(await inventory(join(root,'releases',releaseId,'web'))))throw new Error('Collision de build client')
    await atomicJSON(join(root,'candidate.json'),{releaseId})
    return readJSON(join(root,'releases',releaseId,'manifest.json'))
  } finally {await rm(stage,{recursive:true,force:true})}
}
