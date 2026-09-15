import type { Composition, FlowNode, Kind } from '@zedflow/sdk'
import { instructionAttachments } from './graph/attachments'
import type { FlowExports } from '@zedflow/sdk'
export const catalog: { kind: Kind; label: string; description: string; config: Record<string, any> }[] = [
  { kind: 'start', label: 'Début', description: 'Entrée du graphe', config: {} },
  { kind: 'context', label: 'Contexte', description: 'Préparer la fenêtre avant le modèle', config: { fanIn: 'any', modelNode: '', contextStrategy: 'conversation-default', contextBindings: defaultContextBindings(), attachments: instructionAttachments('Réponds de manière concise en français.') } },
  { kind: 'model', label: 'Modèle', description: 'Prédire à partir du contexte préparé', config: { provider: 'fixture', contextNode: '', inputField: 'input', historyField: 'messages', field: 'output', toolCallsField: 'toolCalls' } },
  { kind: 'steering', label: 'Réorientation', description: 'Consommer la prochaine réorientation', config: { fanIn: 'any', field: 'input', historyField: 'messages' } },
  { kind: 'inbox', label: 'Suite du travail', description: 'Message en file ou attente humaine', config: { field: 'input', prompt: 'Sur quoi continuer ?', responseType: 'text', historyField: 'messages' } },
  { kind: 'tool', label: 'Outil', description: 'Appels et résultats · FunctionTool', config: {tool:'inspect_json',arguments:{value:{status:'ok'}},field:'output',ui:{renderer:'json'}} },
  { kind: 'set', label: 'Transformer', description: 'Écrire une valeur dans l’état', config: { field: 'output', value: '{{input}}' } },
  { kind: 'condition', label: 'Condition', description: 'Critères typés · sorties Oui / Non', config: { predicate: {kind:'compare',field:'input',operator:'eq',value:'oui'} } },
  { kind: 'input', label: 'Attendre une réponse', description: 'Suspendre · sauvegarder · reprendre', config: { field: 'input', prompt: 'Sur quoi continuer ?', responseType: 'text' } },
  { kind: 'output', label: 'Réponse', description: 'Publier un résultat', config: { text: '{{output}}' } },
  { kind: 'subgraph', label: 'Sous-graphe', description: 'Composition isolée · version embarquée', config: {} },
  { kind: 'route', label: 'Point de routage', description: 'Emprunter les routes apportées par les bridges', config: { branch: '', invocation: 'condition', inputField: 'input', field: 'output' } },
  { kind: 'await_route', label: 'Attendre une visite', description: 'Résultat d’un flow lancé par une route', config: { inputField: 'output', field: 'output' } },
  { kind: 'end', label: 'Fin', description: 'Terminer cette exécution', config: {} },
]
// Only the legacy reader and conversion fixtures use this combined node.
const legacyAgent = { kind: 'agent' as const, label: 'Appel modèle', config: { fanIn:'any', provider: 'fixture', inputField: 'input', field: 'output', attachments: instructionAttachments('Réponds de manière concise en français.') } }
function legacyDefinition(kind: Kind) { return kind === 'agent' ? legacyAgent : catalog.find(entry => entry.kind === kind)! }
// Rust omits empty maps from saved exports. Materialize them only in the editable copy.
function normalizeFlowExports(value: Record<string, any>): FlowExports {
  const contract=value.contract||{}
  return {...value,contract:{...contract,entries:contract.entries||{},branches:contract.branches||{},data:contract.data||{},requires:contract.requires||{},inferenceNodes:contract.inferenceNodes||{}},types:value.types||{},entries:value.entries||{},branches:value.branches||{},data:value.data||{},requires:value.requires||{},interactive:value.interactive??false}
}
export function legacyTemplate(interactive = true): Composition {
  const node = (id: string, kind: Kind, x: number, y: number): FlowNode => ({ id, type: 'flow', position: { x, y }, data: { kind, label: legacyDefinition(kind).label, config: structuredClone(legacyDefinition(kind).config) } })
  const nodes = [node('start','start',60,175), node('model','agent',300,100), node('response','output',750,165),node(interactive?'input':'end',interactive?'input':'end',750,410)]
  return { formatVersion:2, id: crypto.randomUUID(), name: interactive ? 'Assistant de workspace' : 'Analyse autonome', revision: 0, nodes, edges: [ { id:'e1',source:'start',target:'model' },{ id:'e2',source:'model',target:'response' },{ id:'e3',source:'response',target:interactive?'input':'end' },...(interactive?[{id:'e4',source:'input',target:'model'}]:[])] }
}

export function legacyHarnessTemplate(): Composition {
  const make = (id: string, kind: Kind, label: string, x: number, y: number, config: Record<string, unknown> = {}): FlowNode => ({ id, type: 'flow', position: { x, y }, data: { kind, label, config: { ...structuredClone(legacyDefinition(kind).config), ...config } } })
  const nodes = [
    make('start', 'start', 'Début', 30, 175),
    make('steering', 'steering', 'Réorientations', 240, 165),
    make('model', 'agent', 'Agent du workspace', 540, 100, { modelBinding: 'runtime', provider: undefined, historyField: 'messages', toolCallsField: 'toolCalls', attachments: {
      instructions:{items:[{id:'workspace-instructions',source:{kind:'workspace'},activation:'always'},{id:'agent-instructions',source:{kind:'text',text:'Aide l’utilisateur dans son workspace. Utilise les outils pour lire, modifier et vérifier le travail. Respecte les instructions attachées et charge les skills pertinents. Décris le résultat et les vérifications effectuées.'},mode:'literal',activation:'always'}]},
      skills:{items:[{id:'workspace-skills',source:{kind:'workspace'},activation:'explicit'}]},
      tools:{items:['read','write','edit','exec'].map(name=>({id:`tool-${name}`,name}))},
    } }),
    make('route', 'condition', 'Appels demandés ?', 990, 125, { predicate:{kind:'compare',field:'hasToolCalls',operator:'eq',value:true} }),
    make('tools', 'tool', 'Exécuter un outil', 990, 440, { fanIn: 'any', tool: 'execute_next_call', field: 'output', historyField: 'messages', toolCallsField: 'toolCalls', retry: { maxAttempts: 1 }, ui: { renderer: 'json', title: 'Outil du workspace' } }),
    make('remaining', 'condition', 'Appels restants ?', 600, 400, { predicate:{kind:'compare',field:'hasToolCalls',operator:'eq',value:true} }),
    make('response', 'output', 'Réponse', 1310, 165),
    make('inbox', 'inbox', 'Suite du travail', 1310, 440),
  ]
  const edge = (source: string, target: string, sourceHandle?: string) => ({ id: `${source}-${target}`, source, target, ...(sourceHandle ? { sourceHandle } : {}) })
  return { formatVersion:2, id: crypto.randomUUID(), name: 'Harness de workspace', revision: 0, settings: { maxConcurrency: 1, recursionLimit: 10000 }, nodes, edges: [edge('start', 'steering'), edge('steering', 'model'), edge('model', 'route'), edge('route', 'tools', 'true'), edge('route', 'response', 'false'), edge('tools', 'remaining'), edge('remaining', 'tools', 'true'), edge('remaining', 'steering', 'false'), edge('response', 'inbox'), edge('inbox', 'steering')] }
}

export function legacyToolTemplate(): Composition {
  const base=legacyTemplate(true)
  base.name='Assistant avec outils'
  const model=base.nodes.find(n=>n.id==='model')!
  model.data.config.attachments.tools={items:[{id:'inspect-json',name:'inspect_json'}]};model.data.config.historyField='messages'
  base.nodes.find(n=>n.id==='response')!.position={x:1070,y:165}
  base.nodes.find(n=>n.id==='input')!.position={x:1070,y:410}
  base.nodes.push({id:'route',type:'flow',position:{x:750,y:125},data:{kind:'condition',label:'Appels demandés ?',config:{predicate:{kind:'compare',field:'hasToolCalls',operator:'eq',value:true}}}},{id:'tools',type:'flow',position:{x:750,y:410},data:{kind:'tool',label:'Exécuter les outils',config:{tool:'execute_calls',field:'output',historyField:'messages',toolCallsField:'toolCalls',ui:{renderer:'table',title:'Résultats des outils'}}}})
  base.edges=base.edges.filter(e=>e.id!=='e2')
  base.edges.push({id:'m-route',source:'model',target:'route'},{id:'r-tools',source:'route',sourceHandle:'true',target:'tools'},{id:'r-response',source:'route',sourceHandle:'false',target:'response'},{id:'tools-m',source:'tools',target:'model'})
  return base
}

/** Sources are explicit on the context node; the strategy decides what is sent. */
export function defaultContextBindings(historyField = 'messages', inputField = 'input') {
  return {
    input: {kind: 'state', field: inputField},
    instructions: {kind: 'attachments', slot: 'instructions'},
    skills: {kind: 'attachments', slot: 'skills'},
    files: {kind: 'attachments', slot: 'files'},
    history: {kind: 'conversation', historyField, inputField},
  }
}

/** Create a new editable definition. The previous flow and its run snapshots stay intact. */
export function separateContextNodes(source: Composition, strategy = 'workspace-default', copy = true): Composition {
  const doc: Composition = JSON.parse(JSON.stringify(source))
  const startConfig=doc.nodes.find(node=>node.data.kind==='start')?.data.config
  if(startConfig?.exports)startConfig.exports=normalizeFlowExports(startConfig.exports)
  if (doc.nodes.some(node => node.data.kind === 'context') && (doc.formatVersion || 1) < 3) {
    throw new Error('Ce flow utilise un contexte historique partagé. Convertissez ses ressources explicitement avant de séparer les appels modèle.')
  }
  if ((doc.formatVersion || 1) < 2) throw new Error('Convertissez d’abord le flow historique en format 2 pour conserver ses ressources explicites.')
  doc.formatVersion = 3
  if (copy) { doc.id = crypto.randomUUID(); doc.name += ' · contexte séparé'; doc.revision = 0 }
  const agents = doc.nodes.filter(node => node.data.kind === 'agent')
  for (const agent of agents) {
    const config = agent.data.config
    let id = agent.id === 'model' ? 'context' : `${agent.id}-context`
    while (doc.nodes.some(node => node.id === id)) id += '-preparation'
    const contextConfig: Record<string, any> = {modelNode: agent.id, fanIn: config.fanIn || 'all'}
    for (const key of Object.keys(config)) {
      if (key.startsWith('context') || ['attachments', 'capabilityGrants', 'windowGrants'].includes(key)) {
        contextConfig[key] = config[key]; delete config[key]
      }
    }
    if (!contextConfig.contextStrategy && !contextConfig.contextProgram) {
      const tools = (contextConfig.attachments?.tools?.items || []).filter((item: {enabled?: boolean}) => item.enabled !== false).map((item: {name: string}) => item.name)
      const compatible = tools.length === 0 ? 'conversation-default' : tools.length === 1 && tools[0] === 'inspect_json' ? 'tools-default' : tools.length === 4 && ['read','write','edit','exec'].every(name => tools.includes(name)) ? strategy : undefined
      if (!compatible) throw new Error(`« ${agent.data.label} » utilise une sélection d’outils spécifique. Choisissez d’abord une stratégie qui expose ces mêmes capacités.`)
      contextConfig.contextStrategy = compatible
      contextConfig.contextBindings = defaultContextBindings(config.historyField || 'messages', config.inputField || 'input')
    }
    config.contextNode = id
    config.historyField ||= 'messages'
    agent.data.kind = 'model'
    if (agent.data.label === 'Agent du workspace' || agent.data.label === 'Appel modèle') agent.data.label = 'Modèle'
    let position = {x: agent.position.x, y: agent.position.y - 176}
    while (doc.nodes.some(node => Math.abs(node.position.x - position.x) < 260 && Math.abs(node.position.y - position.y) < 140)) position.y -= 176
    const context: FlowNode = {id, type: 'flow', position, data: {kind: 'context', label: 'Préparer le contexte', config: contextConfig}}
    for (const edge of doc.edges) if (edge.target === agent.id) edge.target = id
    doc.edges.push({id: `${id}-${agent.id}`, source: id, target: agent.id})
    doc.nodes.splice(doc.nodes.indexOf(agent), 0, context)
    const exports = doc.nodes.find(node => node.data.kind === 'start')?.data.config.exports
    if (exports) {
      if (exports.contract.inferenceNodes?.[agent.id]) exports.contract.inferenceNodes[agent.id].contextStrategy = typeof contextConfig.contextStrategy === 'string' ? contextConfig.contextStrategy : contextConfig.contextStrategy?.key || null
      for (const [port, node] of Object.entries(exports.branches || {})) {
        if (node === agent.id && exports.contract.branches[port]?.invocations?.includes('context')) exports.branches[port] = id
      }
      for (const entry of Object.values(exports.entries || {}) as {node: string}[]) if (entry.node === agent.id) entry.node = id
    }
  }
  return doc
}

export function template(interactive = true): Composition {
  const result = separateContextNodes(legacyTemplate(interactive), 'conversation-default', false)
  result.nodes.find(node => node.id === 'context')!.position = {x:280,y:160}
  result.nodes.find(node => node.id === 'model')!.position = {x:590,y:160}
  result.nodes.find(node => node.id === 'model')!.data.label = 'Modèle'
  result.nodes.find(node => node.id === 'response')!.position = {x:910,y:160}
  result.nodes.find(node => node.id === (interactive?'input':'end'))!.position = {x:910,y:410}
  return result
}
export function harnessTemplate(): Composition {
  const result = upgradeHarnessRouting(legacyHarnessTemplate(), false)
  result.nodes.find(node => node.id === 'model')!.data.label = 'Modèle du workspace'
  const positions:Record<string,{x:number;y:number}> = {dispatch:{x:540,y:165},context:{x:850,y:165},model:{x:1160,y:165},route:{x:1470,y:125},tools:{x:1470,y:440},remaining:{x:1100,y:400},response:{x:1790,y:165},inbox:{x:1790,y:440}}
  for (const node of result.nodes) if(positions[node.id]) node.position=positions[node.id]
  return result
}

/** Upgrade the known base loop; callers explicitly save this new definition. */
export function upgradeHarnessRouting(source: Composition, copy = true): Composition {
  const result = separateContextNodes(source, 'workspace-default', copy)
  const start=result.nodes.find(node=>node.data.kind==='start')
  const steering=result.nodes.find(node=>node.id==='steering'&&node.data.kind==='steering')
  const model=result.nodes.find(node=>node.id==='model'&&node.data.kind==='model')
  const context=result.nodes.find(node=>node.id===model?.data.config.contextNode&&node.data.kind==='context')
  if(!start||!steering||!model||!context)throw new Error('Cette définition ne possède pas la boucle Harness attendue : réorientations, contexte et modèle.')
  const inputField=model.data.config.inputField||'input'
  const inbox=result.nodes.find(node=>node.id==='inbox'&&node.data.kind==='inbox')
  if((steering.data.config.field||'input')!==inputField||inbox&&(inbox.data.config.field||'input')!==inputField)throw new Error('Les réorientations, la suite du travail et le modèle doivent utiliser le même champ d’entrée. Adaptez leur mapping explicitement.')
  const config=context.data.config,strategy=typeof config.contextStrategy==='string'?config.contextStrategy:config.contextStrategy?.key
  if(config.contextProgram||strategy&&!['workspace-default','harness-default'].includes(strategy))throw new Error('La stratégie personnalisée de ce Harness doit être adaptée explicitement pour recevoir le résultat de routage.')
  const existing=result.nodes.find(node=>node.id==='dispatch')
  if(existing&&(existing.data.kind!=='route'||existing.data.config.branch!=='work'))throw new Error('Le nœud dispatch existe déjà avec un autre rôle.')
  const routingConfig={branch:'work',invocation:'condition',inputField,field:'routeResult',fallback:''}
  if(existing){
    if(Object.keys(existing.data.config).length!==Object.keys(routingConfig).length||Object.entries(routingConfig).some(([key,value])=>existing.data.config[key]!==value))throw new Error('Le nœud dispatch possède une configuration personnalisée. Adaptez son routage explicitement.')
    const incoming=result.edges.filter(edge=>edge.target===existing.id),outgoing=result.edges.filter(edge=>edge.source===existing.id)
    if(incoming.length!==1||incoming[0].source!==steering.id||incoming[0].sourceHandle||outgoing.length!==1||outgoing[0].target!==context.id||outgoing[0].sourceHandle||result.edges.filter(edge=>edge.source===steering.id).length!==1||result.edges.filter(edge=>edge.target===context.id).length!==1)throw new Error('Le routage existant doit relier uniquement Réorientations → Routage → Contexte. Adaptez les connexions personnalisées explicitement.')
  }
  const channel=result.channels?.find(item=>item.name==='routeResult')
  if(channel&&(channel.reducer!=='overwrite'||channel.default!==''))throw new Error('Le canal routeResult existe déjà avec une autre définition.')
  const exports=normalizeFlowExports(start.data.config.exports||{interactive:true})
  if(exports.branches.work&&exports.branches.work!=='dispatch')throw new Error('Le branchement work est déjà exposé par un autre nœud.')
  const work=exports.contract.branches.work
  if(work&&(work.contract.input.kind!=='text'||work.contract.output?.kind!=='text'||work.invocations.length!==1||work.invocations[0]!=='condition'))throw new Error('Le contrat work possède des types ou des déclenchements personnalisés. Adaptez-le explicitement au routage du Harness.')
  if(exports.entries.main&&(exports.entries.main.node!==start.id||exports.entries.main.inputField!==inputField))throw new Error('L’entrée publique main possède un mapping personnalisé. Adaptez-la explicitement au champ d’entrée du Harness.')
  if(!existing){
    const incoming=result.edges.find(edge=>edge.source===steering.id&&edge.target===context.id)
    if(!incoming)throw new Error('La préparation du contexte ne suit pas directement les réorientations de ce Harness.')
    incoming.target='dispatch'
    result.nodes.splice(result.nodes.indexOf(context),0,{id:'dispatch',type:'flow',position:{x:context.position.x,y:context.position.y-176},data:{kind:'route',label:'Routage du travail',config:routingConfig}})
    result.edges.push({id:'dispatch-context',source:'dispatch',target:context.id})
  }
  if(!channel)(result.channels||=[]).push({name:'routeResult',reducer:'overwrite',default:''})
  config.contextStrategy='harness-default'
  config.contextBindings={...(config.contextBindings||defaultContextBindings(model.data.config.historyField||'messages',model.data.config.inputField||'input')),routeResult:{kind:'state',field:'routeResult'}}
  exports.contract.entries.main||={input:{kind:'text'},output:{kind:'text'}}
  exports.entries.main||={node:start.id,inputField,outputField:'response'}
  exports.contract.branches.work={contract:{input:{kind:'text'},output:{kind:'text'}},invocations:['condition']}
  exports.branches.work='dispatch'
  exports.contract.inferenceNodes[model.id]={model:model.data.config.modelBinding==='runtime'?{kind:'runtime'}:{kind:'fixed',provider:model.data.config.provider||'fixture',model:model.data.config.model||'fixture'},contextStrategy:'harness-default',resources:exports.contract.inferenceNodes[model.id]?.resources||[],capabilities:exports.contract.inferenceNodes[model.id]?.capabilities??['read','write','edit','exec']}
  exports.interactive=true
  start.data.config.exports=exports
  return result
}
export function toolTemplate(): Composition {
  const result = separateContextNodes(legacyToolTemplate(), 'tools-default', false)
  const positions:Record<string,{x:number;y:number}> = {context:{x:280,y:160},model:{x:590,y:160},route:{x:900,y:125},tools:{x:900,y:410},response:{x:1220,y:165},input:{x:1220,y:410}}
  for (const node of result.nodes) if(positions[node.id]) node.position=positions[node.id]
  return result
}
