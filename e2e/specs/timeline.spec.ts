import { test, expect } from '@playwright/test'
import { legacyHarnessTemplate as harnessTemplate } from './fixtures/templates'
import { inspector, openSession, saveFlow, waitRun } from './helpers'

test('a real fixture displays tools before Markdown and preserves this order after reconnect', async ({ page, request }) => {
  const composition = harnessTemplate()
  composition.name = 'Réponse Markdown ordonnée'
  composition.nodes.find(node => node.id === 'model')!.data.config.fixtureSteps = [
    { tool: 'write', args: { path: 'markdown-proof.txt', content: 'Vérifié' } },
    { tool: 'read', args: { path: 'markdown-proof.txt' } },
    { tool: 'exec', args: { command: 'cat markdown-proof.txt' } },
    { text: '# Rapport\n\n**Vérifié** avec les outils.\n\n- Lecture\n- Commande\n\n| Outil | État |\n| --- | --- |\n| read | OK |\n\n```rust\nlet valid = true;\n```\n\n[Documentation](https://example.invalid/docs)\n\n<script>window.__zedflowMarkdownExecuted = true</script>' },
  ]
  const file = await saveFlow(request, composition)
  const response = await request.post('/api/runs', { data: { flowKey: file.key, flowHash: file.hash, input: { input: 'Vérifier le résultat' }, modelBindings: { model: { provider: 'fixture', model: 'fixture' } } } })
  expect(response.ok()).toBeTruthy()
  const run = await response.json()
  await waitRun(request, run.id, 'waiting')
  await page.goto('/')
  await openSession(page, run.id)
  const markdown = page.locator('.assistant-markdown')
  await expect(markdown.getByRole('heading', { name: 'Rapport' })).toBeVisible()
  await expect(markdown.locator('strong')).toHaveText('Vérifié')
  await expect(markdown.locator('li')).toHaveCount(2)
  await expect(markdown.locator('table')).toContainText('read')
  await expect(markdown.locator('pre')).toContainText('let valid = true;')
  await expect(markdown.getByRole('link', { name: 'Documentation' })).toHaveAttribute('href', 'https://example.invalid/docs')
  expect(await page.evaluate(() => (window as unknown as Record<string, unknown>).__zedflowMarkdownExecuted)).toBeUndefined()
  const order = () => page.locator('.conversation-content [data-timeline-id]').evaluateAll(entries => entries.map(entry => entry.getAttribute('data-timeline-id')))
  const before = await order()
  expect(before).toHaveLength(5)
  expect(before[0]).toMatch(/^message:/)
  expect(before.slice(1, 4).every(id => id?.startsWith('tool:'))).toBe(true)
  expect(before[4]).toMatch(/^message:/)
  await page.getByRole('button',{name:'Connexion au daemon',exact:true}).click()
  await page.locator('.transport-badge').click()
  await expect.poll(order).toEqual(before)
  await openSession(page, run.id)
  await expect.poll(order).toEqual(before)
  await page.locator('.tool-group-summary').click()
  const tools=page.locator('.tool-entry-row')
  await expect(tools).toHaveCount(3)
  for(const row of await tools.all()){
    const marker=await row.locator('.tool-entry-origin').boundingBox()
    const result=await row.locator('.workspace-tool > summary').boundingBox()
    expect(marker).not.toBeNull();expect(result).not.toBeNull()
    expect(Math.abs(marker!.y-result!.y)).toBeLessThan(10)
  }
  const lastMarker=tools.last().locator('.tool-entry-origin')
  const occurrence=await lastMarker.getAttribute('data-occurrence-id')
  await lastMarker.click()
  await expect(page.getByLabel('Passage du nœud',{exact:true})).toHaveValue(occurrence!)
  await page.getByRole('button',{name:'Fermer les détails',exact:true}).click()
  await page.locator('.conversation-content [data-tool="exec"] > summary').click()
  await expect(page.locator('.conversation-content [data-tool="exec"]')).toContainText('Vérifié')
})

test('execution opens on chat with a context popover and an optional accessible inspector', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByText('Daemon connecté')).toBeVisible()
  await expect(page.getByRole('heading', { name: 'Que souhaitez-vous faire ?' })).toBeVisible()
  await expect(page.locator('.session-inspector')).toHaveCount(0)
  await expect(page.locator('.design-space')).not.toBeVisible()
  await page.getByRole('button', { name: 'Contexte de la session', exact: true }).click()
  await expect(page.locator('.session-context-popover')).toContainText('Environnement')
  await page.getByRole('button', { name: 'Ouvrir les détails', exact: true }).click()
  await expect(page.locator('.session-inspector')).toBeVisible()
  await expect(page.locator('.session-context-popover')).toHaveCount(0)
  await page.getByRole('button', { name: 'Fermer les détails', exact: true }).click()
  await page.setViewportSize({ width: 900, height: 900 })
  await inspector(page, 'Contexte')
  await expect(page.getByRole('dialog', { name: 'Détails de la session', exact: true })).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(page.locator('.session-inspector')).toHaveCount(0)
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
})

test('a truncated command keeps its complete binary output available as an explicit download', async ({page,request})=>{
  const composition=harnessTemplate()
  composition.name='Sortie intégrale de commande'
  composition.nodes.find(node=>node.id==='model')!.data.config.fixtureSteps=[
    {tool:'exec',args:{command:"printf '\\000A\\n\\303\\251\\377'; head -c 65536 /dev/zero | tr '\\000' X"}},
    {text:'La sortie complète est conservée.'},
  ]
  const file=await saveFlow(request,composition)
  const response=await request.post('/api/runs',{data:{flowKey:file.key,flowHash:file.hash,input:{input:'Vérifier le téléchargement'},modelBindings:{model:{provider:'fixture',model:'fixture'}}}})
  expect(response.ok()).toBeTruthy()
  const run=await response.json()
  await waitRun(request,run.id,'waiting')
  await page.goto('/');await openSession(page,run.id)
  await page.locator('[data-tool="exec"]>summary').click()
  await expect(page.locator('[data-tool="exec"]')).toContainText('Aperçu tronqué')
  const output=page.getByRole('button',{name:'Télécharger la sortie complète',exact:true})
  await expect(output).toBeVisible()
  const downloading=page.waitForEvent('download')
  await output.click()
  const download=await downloading,stream=await download.createReadStream(),chunks:Buffer[]=[]
  for await(const chunk of stream!)chunks.push(chunk)
  expect(Buffer.concat(chunks)).toEqual(Buffer.concat([Buffer.from([0,65,10,195,169,255]),Buffer.alloc(65536,88)]))
})
