import { test } from 'node:test'
import { execFileSync } from 'node:child_process'
import assert from 'node:assert/strict'
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, dirname } from 'node:path'
import { clientBuild, CLIENT_INPUTS } from '../src/lib/build-info.mjs'

test('client identity includes SDK, Vue, public assets and build inputs without generated caches',()=>{
  const root=mkdtempSync(join(tmpdir(),'zedflow-build-inputs-'))
  const paths=[]
  try {
    for(const name of CLIENT_INPUTS) {
      const path=join(root,/\/(src|public)$/.test(name)?`${name}/fixture`:name)
      mkdirSync(dirname(path),{recursive:true})
      writeFileSync(path,name==='version.json'?JSON.stringify({version:'fixture',protocol:1,storageEpoch:1}):'original')
      paths.push(path)
    }
    const initial=clientBuild(root)
    assert.equal(initial.component,'client');assert.equal(initial.version,'fixture')
    for(const path of paths.filter(path=>path!==join(root,'version.json'))) {
      const bytes=readFileSync(path)
      writeFileSync(path,'modified')
      assert.notEqual(clientBuild(root).buildId,initial.buildId,path)
      writeFileSync(path,bytes)
    }
    for(const path of ['web/apps/client/dist/cached','web/packages/sdk/dist/generated','web/node_modules/cache']) {
      mkdirSync(dirname(join(root,path)),{recursive:true});writeFileSync(join(root,path),'unrelated')
    }
    assert.equal(clientBuild(root).buildId,initial.buildId)
    mkdirSync(join(root,'web/apps/client/public/media'),{recursive:true})
    writeFileSync(join(root,'web/apps/client/public/media/new.bin'),Buffer.from([0,255,42]))
    assert.notEqual(clientBuild(root).buildId,initial.buildId)
    execFileSync('git',['init','--quiet',root])
    const commit=()=>execFileSync('git',['-c','user.name=Fixture','-c','user.email=fixture@example.invalid','-c','commit.gpgsign=false','-c','core.hooksPath=/dev/null','commit','--allow-empty','--quiet','-m','fixture'],{cwd:root})
    commit(); const firstRevision=clientBuild(root)
    commit(); const secondRevision=clientBuild(root)
    assert.notEqual(firstRevision.revision,secondRevision.revision)
    assert.notEqual(firstRevision.buildId,secondRevision.buildId,'embedded revision must change the identity even for a docs-only commit')
  } finally {rmSync(root,{recursive:true,force:true})}
})
