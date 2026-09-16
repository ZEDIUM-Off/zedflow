import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtemp, mkdir, writeFile, chmod, cp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { spawn } from 'node:child_process'
import { fileURLToPath } from 'node:url'
const supervisorScript=fileURLToPath(new URL('../src/releases.mjs',import.meta.url))
import { createServer } from 'node:net'
import { hash, inventory, atomicJSON, readJSON, verifyRelease } from '../src/lib/releases.mjs'
const binary=`#!/usr/bin/env node
const fs=require('node:fs'),http=require('node:http'),path=require('node:path');
const root=process.env.ZEDFLOW_RELEASE_ROOT,id=process.env.ZEDFLOW_RELEASE_ID;
const manifest=JSON.parse(fs.readFileSync(path.join(root,'releases',id,'manifest.json')));
if(manifest.fail)process.exit(1);
const port=process.argv[process.argv.indexOf('--listen')+1].split(':').pop();
const server=http.createServer((req,res)=>{res.setHeader('content-type','application/json');
if(req.url==='/api/version')return res.end(JSON.stringify({releaseId:id,daemon:manifest.daemon}));
if(req.url==='/update'){let raw='';req.on('data',s=>raw+=s);req.on('end',()=>{const value=JSON.parse(raw);fs.writeFileSync(path.join(root,'request.json'),JSON.stringify({id:'test-'+Date.now(),source:id,target:value.target,expectedDaemonBuildId:manifest.daemon.buildId}));res.end('{}');setTimeout(()=>server.close(()=>process.exit(75)),20)});return;}
res.end('{}')});server.listen(Number(port),'127.0.0.1');process.on('SIGINT',()=>server.close(()=>process.exit(0)));
`
async function release(root,label,fail=false){
  const staging=join(root,'stage-'+label);await mkdir(join(staging,'web'),{recursive:true});await writeFile(join(staging,'daemon'),binary);await chmod(join(staging,'daemon'),0o755)
  const client={component:'client',buildId:hash('client-'+label),version:label,protocol:1,storageEpoch:1};const daemon={...client,component:'daemon',buildId:hash('daemon-'+label)}
  await atomicJSON(join(staging,'web/client-version.json'),client);await writeFile(join(staging,'web/index.html'),label)
  const body={format:1,platform:process.platform,arch:process.arch,daemon,client,fail,files:await inventory(staging)}
  const releaseId=hash(JSON.stringify(body));const manifest={releaseId,createdAt:new Date().toISOString(),...body};await atomicJSON(join(staging,'manifest.json'),manifest)
  await cp(staging,join(root,'releases',releaseId),{recursive:true});await rm(staging,{recursive:true});return manifest
}
async function until(action,predicate,timeout=15000){const end=Date.now()+timeout;let value;while(Date.now()<end){try{value=await action();if(predicate(value))return value}catch{}await new Promise(r=>setTimeout(r,75))}throw new Error('Timeout: '+JSON.stringify(value))}
async function port(){const server=createServer();await new Promise(r=>server.listen(0,'127.0.0.1',r));const port=server.address().port;await new Promise(r=>server.close(r));return port}
test('supervisor activates repeatedly, rolls back failed startup, and rejects damaged releases',async()=>{
  const root=await mkdtemp(join(tmpdir(),'zedflow-releases-'));await mkdir(join(root,'releases'))
  const first=await release(root,'first'),second=await release(root,'second'),broken=await release(root,'broken',true),damaged=await release(root,'damaged')
  await verifyRelease(root,first.releaseId);await atomicJSON(join(root,'candidate.json'),{releaseId:first.releaseId})
  const listen=await port(),url=`http://127.0.0.1:${listen}`;let logs=''
  const supervisor=spawn(process.execPath,[supervisorScript,'run','--root',root,'--listen',`127.0.0.1:${listen}`],{stdio:['ignore','pipe','pipe']})
  supervisor.stdout.on('data',s=>logs+=s);supervisor.stderr.on('data',s=>logs+=s)
  const exited=new Promise(r=>supervisor.on('exit',r))
  const version=async()=>fetch(url+'/api/version').then(r=>r.json())
  const apply=async target=>{await fetch(url+'/update',{method:'POST',body:JSON.stringify({target})})}
  try{
    await until(version,v=>v.releaseId===first.releaseId)
    await until(()=>readJSON(join(root,'manager.json')),v=>v.ready===true)
    await apply(second.releaseId);await until(()=>readJSON(join(root,'current.json')),v=>v.releaseId===second.releaseId)
    await until(()=>readJSON(join(root,'status.json')),v=>v.phase==='complete')
    await apply(first.releaseId);await until(()=>readJSON(join(root,'current.json')),v=>v.releaseId===first.releaseId)
    await until(()=>readJSON(join(root,'status.json')),v=>v.phase==='complete'&&v.releaseId===first.releaseId)
    await apply(broken.releaseId);await until(()=>readJSON(join(root,'status.json')),v=>v.phase==='failed')
    assert.equal((await version()).releaseId,first.releaseId)
    await writeFile(join(root,'releases',damaged.releaseId,'web/index.html'),'corrupt')
    await assert.rejects(verifyRelease(root,damaged.releaseId),/altéré/)
    await apply(damaged.releaseId);await until(()=>readJSON(join(root,'status.json')),v=>v.phase==='failed'&&v.error.includes('altéré'))
    assert.equal((await version()).releaseId,first.releaseId)
  }catch(error){error.message+='\n'+logs;throw error}
  finally{supervisor.kill('SIGINT');await exited;await rm(root,{recursive:true,force:true})}
})
test('an interrupted activation restarts the last confirmed release and leaves no pending request',async()=>{
  const root=await mkdtemp(join(tmpdir(),'zedflow-recovery-'));await mkdir(join(root,'releases'))
  const first=await release(root,'confirmed'),second=await release(root,'unconfirmed')
  await atomicJSON(join(root,'current.json'),{releaseId:first.releaseId,previousReleaseId:null})
  await atomicJSON(join(root,'request.json'),{id:'interrupted',source:first.releaseId,target:second.releaseId,expectedDaemonBuildId:first.daemon.buildId})
  const listen=await port();let logs=''
  const supervisor=spawn(process.execPath,[supervisorScript,'run','--root',root,'--listen',`127.0.0.1:${listen}`],{stdio:['ignore','pipe','pipe']})
  supervisor.stderr.on('data',s=>logs+=s);supervisor.stdout.on('data',s=>logs+=s)
  const exited=new Promise(r=>supervisor.on('exit',r))
  try{
    await until(()=>readJSON(join(root,'manager.json')),v=>v.ready===true)
    const version=await fetch(`http://127.0.0.1:${listen}/api/version`).then(r=>r.json())
    assert.equal(version.releaseId,first.releaseId)
    assert.equal((await readJSON(join(root,'status.json'))).phase,'recovered')
    await assert.rejects(readFile(join(root,'request.json')),error=>error.code==='ENOENT')
  }catch(error){error.message+='\n'+logs;throw error}
  finally{supervisor.kill('SIGINT');await exited;await rm(root,{recursive:true,force:true})}
})
