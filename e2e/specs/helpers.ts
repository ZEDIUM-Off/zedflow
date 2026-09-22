import { expect, type Page, type APIRequestContext } from '@playwright/test'
import { join } from 'node:path'
import type { Composition, FlowFile, Run } from '@zedflow/sdk'

export function fixturePath(...parts: string[]) {
  const root = process.env.ZEDFLOW_E2E_ROOT
  if (!root) throw new Error('Playwright fixture root is missing')
  return join(root, ...parts)
}

export async function designTemplate(page: Page, kind: 'interactive' | 'autonomous' | 'tools' | 'harness') {
  await page.getByRole('button', { name: 'Conception', exact: true }).click()
  const names = { interactive: 'Boucle interactive', autonomous: 'Exécution autonome', tools: 'Modèle et outils', harness: 'Harness de workspace' }
  await page.getByRole('button', { name: 'Créer un flow', exact: true }).click()
  await page.getByRole('menuitem', { name: new RegExp('^'+names[kind]) }).click()
}

export async function useDesign(page: Page, text = 'Observer le workspace') {
  await page.getByRole('button', { name: 'Utiliser', exact: true }).click()
  await expect(page.locator('.design-space')).not.toBeVisible()
  if(await page.locator('.autonomous-launch').isVisible()){await page.getByLabel('Donnée d’entrée',{exact:true}).fill(text);await page.getByRole('button',{name:'Lancer l’exécution',exact:true}).click()}else{await page.locator('.composer textarea').fill(text);await page.locator('.composer textarea').press('Enter')}
}

export async function inspector(page: Page, tab: 'Parcours' | 'Modèles' | 'Contexte' | 'État' = 'Parcours') {
  const toggle = page.getByRole('button', { name: 'Afficher les détails', exact: true })
  if (await toggle.getAttribute('aria-pressed') !== 'true') await toggle.click()
  await page.getByRole('tab', { name: tab, exact: true }).click()
}

export async function openSession(page: Page, id: string, workspaceId?: string) {
  await page.reload()
  await expect(page.getByText('Daemon connecté')).toBeVisible()
  const response=await page.request.get(`/api/runs/${id}${workspaceId?`?workspaceId=${workspaceId}`:''}`)
  expect(response.ok(),await response.text()).toBeTruthy()
  const run=await response.json()
  if(run.interactive===false){
    await page.getByRole('button',{name:'Conception',exact:true}).click()
    await page.getByRole('navigation',{name:'Espace de conception'}).getByRole('button',{name:'Flows',exact:true}).click()
    if(run.workspaceId)await page.getByLabel('Workspace de conception',{exact:true}).selectOption(run.workspaceId)
    await page.getByRole('button',{name:'Exécution',exact:true}).click()
    const history=page.locator('.autonomous-history');if((await history.getAttribute('open'))===null)await history.locator('summary').click()
    await page.getByLabel('Rechercher une exécution').fill(run.name)
    const row=page.locator(`[data-execution-id="${id}"]`);await expect(row).toBeVisible();await row.locator('button').first().click()
  }else{
    await page.getByLabel('Rechercher une session').fill(run.name)
    const row = page.locator(`[data-session-id="${id}"]`)
    await expect(row).toBeVisible()
    await row.locator('button').first().click()
    await page.getByLabel('Rechercher une session').fill('')
    await expect(page.locator('.footer-workspace')).not.toContainText('Ouverture')
  }
}

export async function saveFlow(request: APIRequestContext, composition: Composition, workspaceId?: string, scope: 'workspace' | 'global' = 'workspace'): Promise<FlowFile> {
  const response = await request.post('/api/flows', { data: { composition, workspaceId, scope } })
  expect(response.ok(), await response.text()).toBeTruthy()
  return response.json()
}

export async function waitRun(request: APIRequestContext, id: string, status: string, workspaceId?: string): Promise<Run> {
  let run: Run
  await expect.poll(async () => {
    const response = await request.get(`/api/runs/${id}${workspaceId?`?workspaceId=${workspaceId}`:''}`)
    expect(response.ok(),await response.text()).toBeTruthy()
    run = await response.json()
    if (run.status === 'error') throw new Error(run.error)
    return run.status
  }, { timeout: 20000 }).toBe(status)
  return run!
}
