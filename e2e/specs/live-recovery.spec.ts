import {test,expect} from '@playwright/test'
import { designTemplate, inspector, useDesign } from './helpers'

test('a WebRTC channel that opens without delivering snapshots cannot freeze Interagir',async({page})=>{
 await page.addInitScript(()=>{
   const Native=window.RTCPeerConnection
   window.RTCPeerConnection=class extends Native{
     override createDataChannel(label:string,options?:RTCDataChannelInit){
       const channel=super.createDataChannel(label,options)
       // Reproduce an established transport whose application events never arrive.
       channel.addEventListener('message',event=>event.stopImmediatePropagation())
       return channel
     }
   }
 })
 await page.goto('/')
 await expect(page.getByText('Daemon connecté')).toBeVisible()
 await designTemplate(page,'interactive')
 await useDesign(page)
 await expect(page.locator('.composer-wait-prompt')).toBeVisible({timeout:4000})
 await inspector(page,'État')
 await page.locator('.event-log>summary').click()
 await expect(page.locator('.event-log')).not.toContainText('Événements ADK · 0')
 await expect(page.locator('[data-transport=webrtc]')).toHaveCount(0)
 await page.locator('.composer textarea').fill('Reprendre sans changer de vue')
 await page.locator('.composer textarea').press('Enter')
 await expect(page.locator('.assistant-markdown').last()).toContainText('Reprendre sans changer de vue',{timeout:4000})
 await expect(page.locator('.composer-wait-prompt')).toBeVisible()
})

test('buffered SSE and silent WebRTC recover through HTTP snapshots without navigation',async({page})=>{
 await page.addInitScript(()=>{
   const Native=window.RTCPeerConnection
   window.RTCPeerConnection=class extends Native{
     override createDataChannel(label:string,options?:RTCDataChannelInit){
       const channel=super.createDataChannel(label,options)
       channel.addEventListener('message',event=>event.stopImmediatePropagation())
       return channel
     }
   }
   // A buffering proxy may establish SSE without forwarding a single event.
   window.EventSource=class extends EventTarget{
     onopen=null;onerror=null;onmessage=null;readyState=1
     close(){this.readyState=2}
   } as unknown as typeof EventSource
 })
 await page.goto('/');await expect(page.getByText('Daemon connecté')).toBeVisible()
 await designTemplate(page,'interactive')
 await useDesign(page)
 await expect(page.locator('.composer-wait-prompt')).toBeVisible({timeout:4000})
 await page.getByRole('button',{name:'Connexion au daemon',exact:true}).click()
 await expect(page.locator('[data-transport=http]')).toBeVisible()
 await page.keyboard.press('Escape')
 await inspector(page,'État')
 await page.locator('.event-log>summary').click()
 await expect(page.locator('.event-log')).not.toContainText('Événements ADK · 0')
 await page.locator('.composer textarea').fill('Réponse transmise via HTTP')
 await page.locator('.composer textarea').press('Enter')
 await expect(page.locator('.assistant-markdown').last()).toContainText('Réponse transmise via HTTP',{timeout:4000})
 await expect(page.locator('.composer-wait-prompt')).toBeVisible()
})

test('an old answer acknowledgment never overwrites a newer streamed state',async({page})=>{
 await page.goto('/');await expect(page.getByText('Daemon connecté')).toBeVisible()
 await designTemplate(page,'interactive')
 await useDesign(page)
 await expect(page.locator('.composer-wait-prompt')).toBeVisible()
 let release!:()=>void
 const delayed=new Promise<void>(resolve=>release=resolve)
 await page.route('**/api/runs/*/answer',async route=>{
   const response=await route.fetch()
   await delayed
   await route.fulfill({response})
 })
 await page.locator('.composer textarea').fill('La réponse avant son accusé de réception')
 await page.locator('.composer textarea').press('Enter')
 await expect(page.locator('.assistant-markdown').last()).toContainText('La réponse avant son accusé de réception')
 release()
 await expect(page.locator('.session-status.waiting')).toBeVisible()
 await expect(page.locator('.composer-wait-prompt')).toBeVisible()
})

test('losing WebRTC events after a successful connection still resumes the run',async({page})=>{
 await page.addInitScript(()=>{
   const Native=window.RTCPeerConnection
   window.RTCPeerConnection=class extends Native{
     override createDataChannel(label:string,options?:RTCDataChannelInit){
       const channel=super.createDataChannel(label,options)
       channel.addEventListener('message',event=>{if((window as any).__dropRtc)event.stopImmediatePropagation()})
       return channel
     }
   }
 })
 await page.goto('/');await expect(page.getByText('Daemon connecté')).toBeVisible()
 await designTemplate(page,'interactive')
 await useDesign(page)
 await page.getByRole('button',{name:'Connexion au daemon',exact:true}).click()
 await expect(page.locator('[data-transport=webrtc]')).toBeVisible()
 await page.keyboard.press('Escape')
 await expect(page.locator('.composer-wait-prompt')).toBeVisible()
 await page.evaluate(()=>(window as any).__dropRtc=true)
 await page.locator('.composer textarea').fill('Continuer malgré la perte du flux')
 await page.locator('.composer textarea').press('Enter')
 await expect(page.locator('.assistant-markdown').last()).toContainText('Continuer malgré la perte du flux',{timeout:18000})
 await expect(page.locator('.composer-wait-prompt')).toBeVisible()
})
