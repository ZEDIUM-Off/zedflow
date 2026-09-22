
import type { JsonValue, ResourceBinding } from '@zedflow/sdk'
import type { ContextType, ContextExpr, ContextPredicate, ContextBlock, ContextCapability, ContextStrategy, ContextFunction, ContextLibrary, ContextDiagnostic, ContextLibraryFile, ContextTypesFile, ContextFile, ContextItem, ContextTraceEntry, ContextEvaluation, ContextPreview, ContextPreviewProfile } from '@zedflow/sdk'

import { useClient } from '@zedflow/vue'
import { HttpError, contextDiagnosticSchema, contextPreviewProfileSchema } from '@zedflow/sdk'
import { useReloadDrafts, managedReload } from './composables/reloadState'
import { computed, onUnmounted, reactive, ref, watch, type Ref } from 'vue'
import { conversationToolsDocumentsExample } from './contextExamples'

export type ContextPreviewForm = Omit<ContextPreviewProfile, 'config' | 'tools' | 'media'> & { config: string; tools: string; media: string }

export interface ContextDraft {
  strategy: ContextStrategy; file?: ContextFile; saved: string; source: string; sourceSignature: string
  types: Record<string, ContextType>; typesFile?:ContextTypesFile; library: ContextLibrary; libraryFile?: ContextLibraryFile; resources: Record<string, JsonValue>; grants: string[]
  bindings?:Record<string,ResourceBinding>; preview?: ContextPreview; previewSignature?: string; selectedBlock: string; selectedPreviewItem?: string
  diagnostics: ContextDiagnostic[]; error: string; notice: string; pending: string; conflict: boolean
  noticeSignature?: string
  previewPending?: boolean; previewAttempt?: string; fixtureName?: string
  previewProfile?: ContextPreviewForm
  fixtures?: { id: string; name: string; resources: Record<string, JsonValue> }[]
}
export type ContextExample = 'instructions' | 'tool-result' | 'structured'
interface ContextWorkspaceState { files: ContextFile[]; drafts: Record<string, ContextDraft>; selected: string; loading: boolean; error: string; initialized: boolean; pristineDraft: string }
export function httpDiagnostics(error: unknown): ContextDiagnostic[] {
  if (!(error instanceof HttpError) || !error.body || typeof error.body !== 'object' || !('diagnostics' in error.body)) return []
  const parsed = contextDiagnosticSchema.array().safeParse(error.body.diagnostics)
  return parsed.success ? parsed.data : []
}
export function contextId(prefix = 'block') { return `${prefix}-${crypto.randomUUID().slice(0, 8)}` }
export function cloneContext<T>(value: T): T { return JSON.parse(JSON.stringify(value)) }
export function textExpression(text = ''): ContextExpr { return { kind: 'literal', dataType: { kind: 'text' }, value: text } }
export function contextType(kind: ContextType['kind']): ContextType {
  if (kind === 'list') return { kind, item: { kind: 'text' } }
  if (kind === 'record') return { kind, fields: {} }
  if (kind === 'media') return { kind, mediaType: 'image/png' }
  if (kind === 'named') return { kind, name: '' }
  return { kind }
}
export function typeLabel(type: ContextType): string {
  if (type.kind === 'list') return `Liste<${typeLabel(type.item)}>`
  if (type.kind === 'record') return 'Objet'
  if (type.kind === 'media') return type.mediaType || 'Média'
  if (type.kind === 'named') return type.name || 'Type nommé'
  return { boolean: 'Booléen', number: 'Nombre', text: 'Texte' }[type.kind]
}
export function defaultContextValue(type: ContextType, registry: Record<string, ContextType> = {}, seen = new Set<string>()): JsonValue {
  switch (type.kind) {
    case 'text': return ''
    case 'number': return 0
    case 'boolean': return false
    case 'list': return []
    case 'record': return Object.fromEntries(Object.entries(type.fields).map(([name, field]) => [name, defaultContextValue(field, registry, new Set(seen))]))
    case 'media': return { contentRef: '', mediaType: type.mediaType }
    case 'named': if (seen.has(type.name) || !registry[type.name]) return null; seen.add(type.name); return defaultContextValue(registry[type.name], registry, seen)
  }
}
export function defaultExpression(kind: ContextExpr['kind'], resources: string[] = [], previous?: ContextExpr): ContextExpr {
  const value = previous ? cloneContext(previous) : { kind: 'resource', name: resources[0] || '' } as ContextExpr
  const key: ContextExpr = { kind: 'variable', name: 'item' }
  switch (kind) {
    case 'resource': return { kind, name: resources[0] || '' }
    case 'variable': return { kind, name: 'item' }
    case 'literal': return textExpression()
    case 'field': return { kind, value, field: '' }
    case 'project': return { kind, value, fields: [] }
    case 'filter': return { kind, value, item: 'item', condition: { kind: 'present', value: key } }
    case 'sort': return { kind, value, item: 'item', key, descending: false }
    case 'take': return { kind, value, count: 10 }
    case 'truncate': return { kind, value, count: 1200 }
    case 'map': return { kind, value, item: 'item', body: key }
    case 'groupBy': case 'dedup': return { kind, value, item: 'item', key }
    case 'record': return { kind, fields: {} }
    case 'list': return { kind, itemType: { kind: 'text' }, items: [] }
    case 'template': return { kind, template: '{{value}}', values: { value: textExpression() } }
    case 'construct': return {kind,name:'',value}
    case 'toJson': return { kind, value }
    case 'measure': return { kind, value, unit: 'bytes' }
    case 'call': return { kind, catalog: 'projection', name: '', arguments: {} }
  }
}
export function defaultPredicate(kind: ContextPredicate['kind'] = 'present', resources: string[] = []): ContextPredicate {
  const value: ContextExpr = { kind: 'resource', name: resources[0] || '' }
  if (kind === 'eq') return { kind, left: value, right: textExpression() }
  if (kind === 'compare') return { kind, left: { kind: 'measure', value, unit: 'bytes' }, operator: 'gt', right: { kind: 'literal', dataType: { kind: 'number' }, value: 1000 } }
  if (kind === 'and' || kind === 'or') return { kind, items: [{ kind: 'present', value }] }
  if (kind === 'not') return { kind, item: { kind: 'present', value } }
  if (kind === 'contains') return { kind, value, item: textExpression() }
  return { kind, value }
}
export function newContextBlock(kind: ContextBlock['kind'], resources: string[] = []): ContextBlock {
  const id = contextId(kind)
  if (kind === 'group') return { kind, id, label: 'Nouveau groupe', items: [] }
  if (kind === 'if') return { kind, id, condition: defaultPredicate('present', resources), then: [], else: [] }
  if (kind === 'forEach') return { kind, id, value: { kind: 'resource', name: resources[0] || '' }, item: 'item', items: [] }
  return { kind, id, role: 'data', format: 'text', value: textExpression() }
}
export function findContextBlock(blocks: ContextBlock[], id: string): ContextBlock | undefined {
  for (const block of blocks) {
    if (block.id === id) return block
    const child = block.kind === 'group' || block.kind === 'forEach' ? findContextBlock(block.items, id) : block.kind === 'if' ? findContextBlock([...block.then, ...block.else], id) : undefined
    if (child) return child
  }
}
export function diagnosticBlock(strategy: ContextStrategy, path: string): string | undefined {
  const parts = path.replace(/\[(\d+)\]/g, '.$1').split('.')
  let value: unknown = strategy, id: string | undefined
  for (const part of parts) {
    if (value === null || typeof value !== 'object') break
    value = (value as Record<string, unknown>)[part]
    if (value && typeof value === 'object' && 'id' in value) id = String(value.id)
  }
  return id
}
function newDraft(strategy?: ContextStrategy, file?: ContextFile): ContextDraft {
  const value = cloneContext(strategy || { version: 2, id: contextId('context'), name: 'Nouvelle stratégie', requirements: {}, capabilities: [], program: [] })
  return { strategy: value, file, saved: file ? JSON.stringify(value) : '', source: file?.source || '', sourceSignature: file ? JSON.stringify(value) : '', types: cloneContext(value.types || {}), library: { projections: {}, subprograms: {} }, resources: {}, grants: [], selectedBlock: '', diagnostics: file?.diagnostics || [], error: '', notice: '', pending: '', conflict: false }
}
export function useContextStudio(workspaceId: Ref<string>, active: Ref<boolean>) {
  const client=useClient()
  // Catalog arrival chooses data; only an explicit user action switches editor tabs.
  const navigationIntent = ref(0)
  const states = reactive<Record<string, ContextWorkspaceState>>({})
  const editingDrafts = new WeakSet<ContextDraft>()
  useReloadDrafts('context-drafts',states)
  function workspaceState(id = workspaceId.value) {
    if (!states[id]) {
      const initial = newDraft()
      states[id] = { files: [], drafts: { new: initial }, selected: 'new', loading: false, error: '', initialized: false, pristineDraft: JSON.stringify(initial) }
    }
    return states[id]
  }
  const session = computed(() => workspaceState())
  const current = computed(() => session.value.drafts[session.value.selected])
  // Focus/selection already belongs to this document, before the first input event.
  function beginEditing() { editingDrafts.add(current.value) }
  // The schema used by the editor travels with the strategy. A catalog selection
  // copies definitions; it never makes the saved strategy depend on an open tab.
  watch(() => current.value.types, types => {
    if (Object.keys(types).length) {
      if (JSON.stringify(current.value.strategy.types) !== JSON.stringify(types)) current.value.strategy.types = cloneContext(types)
    } else if (current.value.strategy.types) delete current.value.strategy.types
  }, { deep: true, flush: 'sync' })
  const dirty = computed(() => JSON.stringify(current.value.strategy) !== current.value.saved)
  const signature = (draft: ContextDraft) => JSON.stringify({ strategy: draft.strategy, types: draft.types, library: draft.library, resources: draft.resources, grants: draft.grants,profile:draft.previewProfile })
  watch(() => [workspaceId.value, current.value, navigationIntent.value, signature(current.value)] as const, (_, previous) => {
    const draft = previous?.[1]
    if (draft?.noticeSignature && (draft !== current.value || previous[0] !== workspaceId.value || previous[2] !== navigationIntent.value || draft.noticeSignature !== signature(draft))) {
      draft.notice = ''; draft.noticeSignature = undefined
    }
  }, { flush: 'sync' })
  async function refresh() {
    const id = workspaceId.value; if (!id) return
    const state = workspaceState(id); if (state.loading) return
    state.loading = true; state.error = ''
    try {
      state.files = await client.context.list({workspaceId:id})
      const initial = state.drafts.new
      const untouched = state.selected === 'new' && initial && !editingDrafts.has(initial) && JSON.stringify(initial) === state.pristineDraft
      const defaultFile = state.files.find(file => file.key === 'workspace-default' && file.strategy)
      if (!state.initialized && untouched && defaultFile) {
        // The list response already includes the parsed definition. Do not open a file
        // through the current workspace after an asynchronous workspace switch.
        state.drafts[defaultFile.key] = newDraft(defaultFile.strategy, defaultFile)
        state.selected = defaultFile.key
      }
      state.initialized = true
    }
    catch (error) { state.error = String(error instanceof Error ? error.message : error) }
    finally { state.loading = false }
  }
  function select(key: string) {
    if (!session.value.drafts[key]) return
    session.value.selected = key
    navigationIntent.value++
  }
  function create(copy = false) {
    navigationIntent.value++
    const state = session.value, draft = copy ? newDraft(current.value.strategy) : newDraft()
    if (copy) { draft.strategy.id = contextId('context'); draft.strategy.name += ' · copie'; draft.types = cloneContext(current.value.types); draft.library = cloneContext(current.value.library); draft.libraryFile = current.value.libraryFile; draft.resources = cloneContext(current.value.resources); draft.grants = [...current.value.grants] }
    const key = `draft:${draft.strategy.id}`; state.drafts[key] = draft; state.selected = key
  }
  function createExample(example: ContextExample) {
    navigationIntent.value++
    if (example === 'structured') {
      const value = conversationToolsDocumentsExample(contextId('context'))
      const draft = newDraft(value.strategy)
      draft.resources = value.resources
      const key = `draft:${draft.strategy.id}`
      session.value.drafts[key] = draft; session.value.selected = key
      return
    }
    const resource = (name: string): ContextExpr => ({ kind: 'resource', name })
    const fragment = (id: string, name: string, role: 'instruction' | 'data' = 'data'): ContextBlock => ({ kind: 'emit', id, role, format: 'text', value: resource(name) })
    const draft = newDraft({
      version: 2, id: contextId('context'),
      name: example === 'instructions' ? 'Instructions et demande' : 'Résultat d’outil conditionnel',
      requirements: example === 'instructions'
        ? { instructions: { kind: 'text' }, question: { kind: 'text' } }
        : { question: { kind: 'text' }, toolResult: { kind: 'text' } },
      capabilities: [],
      program: example === 'instructions'
        ? [fragment('instructions', 'instructions', 'instruction'), fragment('question', 'question')]
        : [fragment('question', 'question'), {
          kind: 'if', id: 'tool-available', condition: { kind: 'present', value: resource('toolResult') },
          then: [fragment('tool-result', 'toolResult')],
          else: [{ kind: 'emit', id: 'without-tool', role: 'instruction', format: 'text', value: textExpression('Aucun résultat d’outil disponible. Précisez ce qu’il reste à vérifier.') }],
        }],
    })
    draft.resources = example === 'instructions'
      ? { instructions: 'Répondez en français. Appuyez les conclusions sur les données fournies.', question: 'Quels documents faut-il relire dans le workspace ?' }
      : { question: 'Que nous apprend la lecture du fichier ?', toolResult: 'La documentation décrit trois étapes : explorer, modifier, vérifier.' }
    draft.selectedBlock = draft.strategy.program[0].id
    draft.notice = example === 'instructions'
      ? 'Exemple éditable : modifiez les valeurs dans Sources, puis prévisualisez. L’enregistrement conserve le programme ; les valeurs d’essai restent dans ce brouillon.'
      : 'Exemple éditable : décochez « Fournir toolResult pour l’aperçu », puis prévisualisez pour essayer la branche Sinon.'
    const key = `draft:${draft.strategy.id}`
    session.value.drafts[key] = draft
    session.value.selected = key
  }
  function fromFrozen(program:{strategy:ContextStrategy;types:Record<string,ContextType>;library:ContextLibrary;bindings?:Record<string,ResourceBinding>},blockId='') {
    navigationIntent.value++
    const draft=newDraft(program.strategy);draft.strategy.id=contextId('context');draft.strategy.name+=' · copie';draft.types=cloneContext(program.types);draft.library=cloneContext(program.library);draft.bindings=cloneContext(program.bindings||{});draft.selectedBlock=blockId;draft.notice='Brouillon créé depuis une définition exécutée. Enregistrez-le puis sélectionnez-le explicitement sur le nœud.'
    const key=`draft:${draft.strategy.id}`;session.value.drafts[key]=draft;session.value.selected=key
  }
  async function convert(formats:Record<string,'json'|'adkMessages'>){
    const draft=current.value,state=session.value,id=workspaceId.value,slot=state.selected
    if(draft.pending)return false
    draft.pending='Conversion';draft.error='';draft.diagnostics=[]
    try{const result=await client.context.convert({workspaceId:id,strategy:cloneContext(draft.strategy),bindings:draft.bindings,formats})
      if(!result.valid||!result.strategy){draft.diagnostics=result.diagnostics;draft.error='Précisez la représentation des fragments signalés avant de créer la copie.';return false}
      const copy=newDraft(result.strategy);copy.types=cloneContext(draft.types);copy.typesFile=draft.typesFile;copy.library=cloneContext(draft.library);copy.libraryFile=draft.libraryFile;copy.resources=cloneContext(draft.resources);copy.grants=[...draft.grants];copy.bindings=cloneContext(draft.bindings||{});copy.notice='Copie v2 créée. Enregistrez-la puis sélectionnez-la explicitement sur le nœud.'
      copy.noticeSignature=signature(copy)
      const key=`draft:${contextId('converted')}`;state.drafts[key]=copy;if(workspaceId.value===id&&state.selected===slot)state.selected=key
      return true
    }catch(cause){draft.error=cause instanceof Error?cause.message:String(cause);return false}finally{draft.pending=''}
  }
  async function open(file: ContextFile, reload = false) {
    if (!reload) navigationIntent.value++
    const id = workspaceId.value, state = session.value
    if (!reload && state.drafts[file.key]) { state.selected = file.key; return }
    const draft = state.drafts[file.key] ||= newDraft(file.strategy, file)
    state.selected = file.key
    draft.pending = 'Chargement'; draft.error = ''
    try {
      const result = await client.context.read(file.key,{workspaceId:id})
      // Replace this slot only: navigation and other workspace drafts remain untouched.
      state.drafts[file.key] = newDraft(result.strategy, result)
    } catch (error) { draft.error = error instanceof Error ? error.message : String(error) }
    finally { draft.pending = '' }
  }
  async function run(action: 'save' | 'preview' | 'source', automatic = false) {
    const id = workspaceId.value, state = session.value, key = state.selected, draft = current.value, intent = navigationIntent.value
    if (!id || draft.pending || (draft.file && !draft.file.strategy)) return
    if (draft.previewPending) {
      if (action === 'preview') return
      await new Promise<void>(resolve => { const stop = watch(() => draft.previewPending, pending => { if (!pending) { stop(); resolve() } }) })
      if (draft.pending || state.drafts[key] !== draft || workspaceId.value !== id || navigationIntent.value !== intent) return
    }
    if (action === 'preview') draft.previewPending = true
    else draft.pending = action === 'save' ? 'Enregistrement' : 'Génération Rust'
    draft.error = ''; draft.diagnostics = []; draft.conflict = false
    if (!automatic || draft.noticeSignature !== signature(draft)) { draft.notice = ''; draft.noticeSignature = undefined }
    const strategy = cloneContext(draft.strategy), captured = JSON.stringify(strategy), snapshot = signature(draft)
    try {
      if (action === 'save') {
        const result = await client.context.save({ workspaceId: id, strategy, types: cloneContext(draft.types), library: cloneContext(draft.library), ...(draft.file ? { expectedHash: draft.file.hash } : {}) })
        draft.file = result; draft.saved = captured; draft.source = result.source || ''; draft.sourceSignature = captured
        if (signature(draft) === snapshot && workspaceId.value === id && navigationIntent.value === intent && current.value === draft) {
          draft.notice = 'Stratégie enregistrée en Rust'; draft.noticeSignature = snapshot
        }
        state.files = [...state.files.filter(file => file.key !== result.key), result].sort((a, b) => (a.strategy?.name || a.key).localeCompare(b.strategy?.name || b.key))
        if (key !== result.key) { state.drafts[result.key] = draft; if (state.selected === key) state.selected = result.key; delete state.drafts[key] }
      } else {
        const trial=draft.previewProfile
        const profile=trial ? contextPreviewProfileSchema.parse({provider:trial.provider,model:trial.model,config:JSON.parse(trial.config),tools:JSON.parse(trial.tools),media:JSON.parse(trial.media),reasoningEffort:trial.reasoningEffort||undefined,reasoningSummary:trial.reasoningSummary||undefined,textVerbosity:trial.textVerbosity||undefined}):undefined
        const result = await client.context.preview({ workspaceId: id, selection: { kind: 'draft', strategy }, types: cloneContext(draft.types), library: cloneContext(draft.library), resources: cloneContext(draft.resources), grantedCapabilities: [...draft.grants],profile })
        if (signature(draft) === snapshot) {
          draft.source = result.selection.source; draft.sourceSignature = captured
          draft.preview = result; draft.previewSignature = snapshot
          draft.diagnostics = result.evaluation.diagnostics
          if (draft.diagnostics.length) { draft.notice = ''; draft.noticeSignature = undefined }
        }
      }
    } catch (error) {
      if (action !== 'preview' || signature(draft) === snapshot) {
        draft.notice = ''; draft.noticeSignature = undefined
        draft.error = error instanceof Error ? error.message : String(error)
        if (error instanceof HttpError) { draft.diagnostics = httpDiagnostics(error); draft.conflict = error.status === 409 }
      }
    } finally {
      draft.pending = ''; draft.previewPending = false
      if (action === 'preview') draft.previewAttempt = snapshot
    }
  }
  let previewTimer: ReturnType<typeof setTimeout> | undefined
  watch(() => [active.value, signature(current.value), current.value.pending, current.value.previewPending], () => {
    clearTimeout(previewTimer)
    const draft = current.value
    if (!active.value || draft.pending || draft.previewPending || (!draft.strategy.program.length && !draft.preview) || draft.previewAttempt === signature(draft)) return
    previewTimer = setTimeout(() => { void run('preview', true) }, 450)
  })
  const focus = () => { if (active.value) void refresh() }
  watch([workspaceId, active], ([id, enabled]) => { if (id && enabled) void refresh() }, { immediate: true })
  window.addEventListener('focus', focus)
  const beforeUnload = (event: BeforeUnloadEvent) => { if (!managedReload && Object.values(states).some(state => Object.values(state.drafts).some(draft => JSON.stringify(draft.strategy) !== draft.saved && (draft.file || draft.strategy.program.length || Object.keys(draft.strategy.requirements).length)))) event.preventDefault() }
  window.addEventListener('beforeunload', beforeUnload)
  onUnmounted(() => { clearTimeout(previewTimer); window.removeEventListener('focus', focus); window.removeEventListener('beforeunload', beforeUnload) })
  return reactive({ session, current, dirty, workspaceId, navigationIntent, beginEditing, select, refresh, create, createExample, fromFrozen, convert, open, save: () => run('save'), preview: () => run('preview'), source: () => run('source'), get previewStale() { return !!current.value.preview && current.value.previewSignature !== signature(current.value) } })
}
export type ContextStudioController = ReturnType<typeof useContextStudio>
