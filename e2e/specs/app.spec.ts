import { test, expect } from '@playwright/test'
import { designTemplate, saveFlow, useDesign } from './helpers'
import { legacyTemplate as template } from './fixtures/templates'
test('compose, persist, generate Rust and resume an interactive ADK graph',async({page})=>{
 const errors:string[]=[];page.on('pageerror',e=>errors.push(e.message))
 await page.goto('/')
 await expect(page.getByText('Daemon connecté')).toBeVisible()
 await designTemplate(page,'interactive')
 await page.getByRole('textbox',{name:'Nom de composition'}).fill('Parcours navigateur')
 await page.getByRole('button',{name:'Enregistrer',exact:true}).click()
 await expect(page.getByRole('status')).toContainText('Flow enregistré')
 await page.reload()
 await page.getByRole('button',{name:'Conception',exact:true}).click()
 await expect(page.getByRole('textbox',{name:'Nom de composition'})).toHaveValue('Parcours navigateur')
 await page.screenshot({path:test.info().outputPath('01-conception.png')})
 await page.getByRole('button',{name:'Afficher le Rust',exact:true}).click()
 await expect(page.locator('.source-preview')).toContainText('graph.add_node')
 await page.getByRole('dialog',{name:'Rust du flow'}).getByRole('button',{name:'Fermer',exact:true}).click()
 await useDesign(page)
 await expect(page.locator('.composer-wait-prompt')).toBeVisible({timeout:30000})
 await expect(page.locator('.assistant-markdown').first()).toContainText('Mode démonstration')
 await page.screenshot({path:test.info().outputPath('02-interaction.png')})
 await page.locator('.composer textarea').fill('Examiner la suite du graphe')
 await page.locator('.composer textarea').press('Enter')
 await expect(page.locator('.is-user').last()).toContainText('Examiner la suite du graphe')
 await expect(page.locator('.composer-wait-prompt')).toBeVisible()
 await expect(page.locator('.session-row.chosen')).toContainText('Observer le workspace')
 await page.screenshot({path:test.info().outputPath('03-executions.png')})
 const sessionId=await page.locator('.session-row.chosen').getAttribute('data-session-id')
 await page.getByRole('button',{name:'Conception',exact:true}).click()
 await expect(page.locator('.workspace-sidebar .flow-library')).toBeVisible()
 await expect(page.locator('.workspace-sessions:visible')).toHaveCount(0)
 await expect(page.locator('.design-space .flow-library')).toHaveCount(0)
 await page.getByRole('button',{name:'Exécution',exact:true}).click()
 await expect(page.locator('.flow-library:visible')).toHaveCount(0)
 await expect(page.locator('.session-row.chosen')).toHaveAttribute('data-session-id',sessionId!)
 await expect(page.locator('.session-row.chosen')).toBeVisible()
 await expect(page.locator('.is-user').last()).toContainText('Examiner la suite du graphe')
 expect(errors).toEqual([])
})

test('edited output changes the autonomous graph result',async({page})=>{
 await page.goto('/')
 await expect(page.getByText('Daemon connecté')).toBeVisible()
 await designTemplate(page,'autonomous')
 await page.locator('.flow-card.output').click()
 await page.getByLabel('Contenu',{exact:true}).fill('Résultat personnalisé : {{output}}')
 await useDesign(page)
 await expect(page.locator('.session-status.completed')).toBeVisible()
 await expect(page.locator('.assistant-markdown')).toContainText('Résultat personnalisé : Mode démonstration')
 await expect(page.getByRole('region',{name:'Exécution autonome'})).toBeVisible()
 await expect(page.locator('.composer')).toHaveCount(0)
 await expect(page.getByRole('button',{name:'Nouvelle exécution',exact:true})).toBeVisible()
})

test('a late initial catalog cannot replace the flow the user has already started designing', async ({ page, request }) => {
 const earlier = template(false); earlier.name = 'A catalogue au démarrage'
 await saveFlow(request, earlier)
 let release!: () => void
 const pending = new Promise<void>(resolve => { release = resolve })
 let delayed = false
 await page.route('**/api/flows?*', async route => {
   if (delayed || route.request().method() !== 'GET') return route.continue()
   delayed = true
   const response = await route.fetch()
   await pending
   await route.fulfill({ response })
 })
 try {
   await page.goto('/')
   await expect(page.getByText('Daemon connecté')).toBeVisible()
   await designTemplate(page, 'interactive')
   await expect(page.getByRole('textbox', { name: 'Nom de composition' })).toHaveValue('Assistant de workspace')
   release()
   await expect(page.getByRole('button', { name: 'Utiliser', exact: true })).toBeEnabled()
   await expect(page.getByRole('textbox', { name: 'Nom de composition' })).toHaveValue('Assistant de workspace')
   await useDesign(page)
   await expect(page.locator('.composer-wait-prompt')).toBeVisible()
 } finally { release() }
})
