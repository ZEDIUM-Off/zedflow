import { test, expect } from '@playwright/test'
const daemonUrl=`http://127.0.0.1:${process.env.ZEDFLOW_E2E_DAEMON_PORT||3157}`
test('a stale production client announces its update and restores chat and strategy drafts',async({page,request})=>{
  const response=await request.get('/api/version');expect(response.ok()).toBeTruthy();const real=await response.json()
  expect(real.client?.buildId).toBeTruthy()
  let stale=true
  await page.route('**/api/version',route=>route.fulfill({json:{...real,client:{...real.client,buildId:stale?'f'.repeat(64):real.client.buildId}}}))
  await page.goto(daemonUrl)
  await expect(page.getByText('Daemon connecté',{exact:true})).toBeVisible()
  const composer=page.locator('.composer textarea');await expect(composer).toBeVisible();await composer.fill('Brouillon conservé é🦀')
  await page.getByRole('button',{name:'Conception',exact:true}).click()
  await page.getByRole('navigation',{name:'Espace de conception',exact:true}).getByRole('button',{name:'Contexte',exact:true}).click()
  await page.getByRole('button',{name:'Créer une stratégie',exact:true}).click()
  await page.getByLabel('Nom de la stratégie',{exact:true}).fill('Stratégie encore non enregistrée')
  await page.getByRole('button',{name:'Versions et mises à jour',exact:true}).click()
  await expect(page.getByText('Client à actualiser',{exact:true})).toBeVisible()
  stale=false
  await page.getByRole('button',{name:'Actualiser le client et conserver les brouillons',exact:true}).click()
  await expect(page.getByLabel('Nom de la stratégie',{exact:true})).toHaveValue('Stratégie encore non enregistrée')
  await page.getByRole('button',{name:'Exécution',exact:true}).click()
  await expect(page.locator('.composer textarea')).toHaveValue('Brouillon conservé é🦀')
  await page.getByRole('button',{name:'Versions et mises à jour',exact:true}).click()
  await expect(page.getByText('Client à jour · daemon non supervisé',{exact:true})).toBeVisible()
  await page.keyboard.press('Escape')
  await page.setViewportSize({width:390,height:844})
  await page.getByRole('button',{name:'Versions et mises à jour',exact:true}).click()
  await expect(page.getByRole('button',{name:'Vérifier les mises à jour',exact:true})).toBeInViewport()
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBeTruthy()
})
test('unknown legacy and offline daemons are never reported as up to date',async({page})=>{
  let offline=false
  await page.route('**/api/version',route=>offline?route.abort():route.fulfill({status:404,json:{error:'not found'}}))
  await page.goto(daemonUrl)
  await page.getByRole('button',{name:'Versions et mises à jour',exact:true}).click()
  await expect(page.getByText('Daemon sans gestion des versions',{exact:true})).toBeVisible()
  offline=true;await page.getByRole('button',{name:'Vérifier les mises à jour',exact:true}).click()
  await expect(page.getByText('Versions indisponibles',{exact:true})).toBeVisible()
})
