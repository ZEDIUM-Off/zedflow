import { test, expect } from '@playwright/test'
import { legacyTemplate as template } from './fixtures/templates'
import { openSession, saveFlow, waitRun } from './helpers'

test('autonomous typed launch uses results and its own history, while new sessions retain their composer',async({page,request})=>{
  const health=await(await request.get('/api/health')).json(),workspaceId=health.defaultWorkspaceId
  const flow=template(false);flow.formatVersion=3;flow.name=`Autonomie typée ${Date.now()}`
  flow.nodes[0]!.data.config.exports={contract:{entries:{main:{input:{kind:'record',fields:{question:{kind:'text'},count:{kind:'number'}}},output:{kind:'text'}}}},entries:{main:{node:'start',inputField:'input',outputField:'response'}},interactive:false}
  const action=flow.nodes.find(node=>node.data.kind==='agent')!;action.data.kind='set';action.data.config={field:'output',value:'Résultat autonome typé'}
  const file=await saveFlow(request,flow,workspaceId)
  await page.goto('/');await expect(page.getByText('Daemon connecté',{exact:true})).toBeVisible()
  await page.locator('.composer textarea').fill('Brouillon de ma session')
  await page.getByRole('button',{name:'Conception',exact:true}).click()
  await page.locator(`[data-flow-key="${file.key}"] .flow-file-open`).click()
  await page.getByRole('button',{name:'Utiliser',exact:true}).click()
  await expect(page.locator('.autonomous-launch')).toBeVisible();await expect(page.locator('.composer')).toHaveCount(0)
  await page.getByRole('button',{name:'Préparer cette exécution',exact:true}).focus();await page.keyboard.press('Enter')
  const dialog=page.getByRole('dialog',{name:'Composer une exécution'})
  await dialog.getByRole('button',{name:'Préparer le graphe résolu',exact:true}).click()
  await dialog.getByLabel('question',{exact:true}).fill('Analyse de données')
  await dialog.getByLabel('count',{exact:true}).fill('3')
  const launched=page.waitForResponse(response=>new URL(response.url()).pathname==='/api/runs'&&response.request().method()==='POST')
  await dialog.getByRole('button',{name:'Lancer avec cette donnée',exact:true}).focus();await page.keyboard.press('Enter')
  const response=await launched,ack=await response.json();expect(response.ok(),JSON.stringify(ack)).toBeTruthy()
  expect(response.request().postDataJSON().input.input).toEqual({question:'Analyse de données',count:3})
  const completed=await waitRun(request,ack.id,'completed',workspaceId)
  expect(completed.interactive).toBe(false);expect(completed.messages.some(message=>message.role==='user')).toBe(false)
  await expect(page.getByLabel('Résultats de l’exécution')).toContainText('Résultat autonome typé')
  await expect(page.locator(`[data-session-id="${ack.id}"]`)).toHaveCount(0)
  await expect(page.locator('.composer')).toHaveCount(0)
  await page.reload();await expect(page.getByLabel('Résultats de l’exécution')).toContainText('Résultat autonome typé')
  await page.getByRole('button',{name:'Conception',exact:true}).click()
  await expect(page.locator('.autonomous-history')).toHaveCount(0)
  await page.getByRole('button',{name:'Exécution',exact:true}).click()
  await page.locator('.autonomous-history summary').focus();await page.keyboard.press('Enter')
  const row=page.locator(`[data-execution-id="${ack.id}"]`);await expect(row).toBeVisible()
  await row.locator('button').first().focus();await page.keyboard.press('Enter')
  await expect(page.getByRole('region',{name:'Exécution autonome',exact:true})).toBeVisible()
  await page.getByRole('button',{name:'Nouvelle session',exact:true}).first().click()
  await expect(page.locator('.composer textarea')).toBeVisible()
  await expect(page.locator('.autonomous-execution')).toHaveCount(0)
})

test('legacy standalone autonomous errors remain accessible without a message composer',async({page,request})=>{
  const flow=template(false);flow.name=`Autonomie erreur ${Date.now()}`
  const node=flow.nodes.find(node=>node.data.kind==='agent')!;node.data.kind='tool';node.data.label='Outil';node.data.config={tool:'exec',arguments:{command:'exit 7'}}
  const file=await saveFlow(request,flow)
  const response=await request.post('/api/runs',{data:{flowKey:file.key,flowHash:file.hash,input:{input:'Erreur fixture autonome'}}})
  expect(response.ok(),await response.text()).toBeTruthy();const ack=await response.json()
  await expect.poll(async()=>{const run=await(await request.get(`/api/runs/${ack.id}`)).json();return run.status}).not.toBe('running')
  await page.goto('/');await openSession(page,ack.id)
  await expect(page.locator('.autonomous-execution')).toBeVisible();await expect(page.locator('.composer')).toHaveCount(0)
  await page.getByRole('button',{name:'Inspecter',exact:true}).click()
  await expect(page.getByRole('heading',{name:'Détails de l’exécution',exact:true})).toBeVisible()
  await expect(page.locator('.passage-index')).toContainText('Outil')
})
