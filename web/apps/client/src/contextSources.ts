import type { ContextBlock, ContextExpr, ContextType } from '@zedflow/sdk'

export const CONTEXT_SOURCE_MIME = 'application/x-zedflow-context-source'
export const CONTEXT_SOURCE_MAX_DEPTH = 64
const CONTEXT_SOURCE_MAX_NODES = 4096
export type ContextSourceCategory = 'messages' | 'tools' | 'documents' | 'execution' | 'data' | 'media' | 'custom'
export interface ContextSourceEntry {
  id: string
  label: string
  category: ContextSourceCategory
  type: ContextType
  types: Record<string, ContextType>
  origin: string
  providers: string[]
  description?: string
  alias?: string
}
export interface ContextSourceCatalog { entries: ContextSourceEntry[]; diagnostics?: { path: string; message: string }[] }
export interface ContextSourceField {
  source: string
  path: string[]
  type: ContextType
  typeId: string
  label: string
  expression: ContextExpr
}
export const contextSourceCategories: { id: ContextSourceCategory | 'all'; label: string }[] = [
  { id: 'all', label: 'Tous' }, { id: 'messages', label: 'Messages' }, { id: 'tools', label: 'Outils' },
  { id: 'documents', label: 'Documents' }, { id: 'execution', label: 'Exécution' },
  { id: 'data', label: 'Données' }, { id: 'media', label: 'Médias' }, { id: 'custom', label: 'Personnalisés' },
]

function canonical(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`
  if (value && typeof value === 'object') return `{${Object.entries(value).sort(([a], [b]) => a.localeCompare(b)).map(([key, item]) => `${JSON.stringify(key)}:${canonical(item)}`).join(',')}}`
  return JSON.stringify(value)
}
function hash(value: string): number {
  let result = 2166136261
  for (const char of value) result = Math.imul(result ^ char.charCodeAt(0), 16777619)
  return result >>> 0
}
export function contextSourceTypeId(type: ContextType): string {
  return type.kind === 'list' ? `list:${contextSourceTypeId(type.item)}` : type.kind === 'named' ? type.name : type.kind === 'record' ? `${type.kind}:${hash(canonical(type)).toString(36)}` : type.kind === 'media' ? `media:${type.mediaType}` : type.kind
}
export function contextSourceColor(typeId: string): string {
  const family = typeId.replace(/^(?:list:)+/, '').replace(/^Zedflow[.:/]/, '')
  const name = family.toLowerCase().replace(/[^a-z]/g, '')
  if (/^(instructions?|workspaceinstructions)$/.test(name)) return '#8eafff'
  if (/^(usermessage|messageuser|messageutilisateur|userinput)$/.test(name)) return '#e9bb83'
  if (name === 'conversationmessage') return '#99acd5'
  if (name === 'routeresult') return '#b8c992'
  if (name === 'skillcatalog') return '#a6bf91'
  if (/^(modeloutput|outputmodel|sortiemodele)$/.test(name)) return '#b69af5'
  if (/^(toolcall|calltool|appeloutil)$/.test(name)) return '#e99f8f'
  if (/^(toolresult|resulttool|resultatoutil)$/.test(name)) return '#83c6aa'
  if (/^(tooldefinition|definitionoutil)$/.test(name)) return '#d3ad73'
  if (name === 'document' || name === 'selectedfiles') return '#80c4d1'
  const identity = hash(family)
  return `hsl(${(identity % 36000) / 100} ${45 + (identity >>> 16) % 15}% ${72 + (identity >>> 24) % 8}%)`
}
export function contextSourceStyle(typeId: string): Record<string, string> {
  return { '--ctx-source-color': contextSourceColor(typeId) }
}
export function contextSourceOrder(type: ContextType): number {
  const family = contextSourceTypeId(type).replace(/^(?:list:)+/, '').replace(/^Zedflow[.:/]/, '')
  const order = ['Instructions', 'WorkspaceInstructions', 'SkillCatalog', 'SelectedFiles', 'RouteResult', 'UserInput', 'ConversationMessage', 'UserMessage', 'ModelOutput', 'ToolDefinition', 'ToolCall', 'ToolResult', 'Document', 'Skill', 'Passage', 'Run', 'State', 'Image', 'Audio', 'Video'].indexOf(family)
  return order < 0 ? 100 : order
}
export function sourceAppearance(alias: string, type: ContextType) {
  const typeId = contextSourceTypeId(type)
  const labels: Record<string, string> = { Instructions: 'Instructions', WorkspaceInstructions: 'Instructions du workspace', SkillCatalog: 'Catalogue et skills actifs', SelectedFiles: 'Fichiers sélectionnés', RouteResult: 'Résultat de routage', UserInput: 'Saisie utilisateur', ConversationMessage: 'Message de conversation', UserMessage: 'Message utilisateur', ModelOutput: 'Sortie du modèle', ToolDefinition: 'Définition d’outil', ToolCall: 'Appel d’outil', ToolResult: 'Résultat d’outil', Document: 'Document', Skill: 'Skill', Passage: 'Passage de nœud', Run: 'Exécution', State: 'État', Image: 'Image', Audio: 'Audio', Video: 'Vidéo' }
  const plurals: Record<string, string> = { Instructions: 'Instructions', WorkspaceInstructions: 'Instructions des workspaces', SkillCatalog: 'Catalogues et skills actifs', SelectedFiles: 'Fichiers sélectionnés', RouteResult: 'Résultat de routage', UserInput: 'Saisies utilisateur', ConversationMessage: 'Messages de conversation', UserMessage: 'Messages utilisateur', ModelOutput: 'Sorties du modèle', ToolDefinition: 'Définitions d’outils', ToolCall: 'Appels d’outils', ToolResult: 'Résultats d’outils', Document: 'Documents', Skill: 'Skills', Passage: 'Passages de nœuds', Run: 'Exécutions', State: 'États', Image: 'Images', Audio: 'Audios', Video: 'Vidéos' }
  let element = type
  while (element.kind === 'list') element = element.item
  const nameWithoutPrefix = element.kind === 'named' ? element.name.replace(/^Zedflow[.:/]/, '') : ''
  const builtin = Object.hasOwn(labels, nameWithoutPrefix)
  const label = builtin ? (type.kind === 'list' ? plurals[nameWithoutPrefix] : labels[nameWithoutPrefix]) : type.kind === 'named' ? nameWithoutPrefix.replace(/([a-z])([A-Z])/g, '$1 $2') : alias
  const name = typeId.toLowerCase()
  const icon = /instruction|document|selectedfiles/.test(name) ? 'file' : /message|user/.test(name) ? 'message' : /model|skill/.test(name) ? 'sparkles' : /tool|outil/.test(name) ? 'tool' : type.kind === 'media' || /image|audio|video|media/.test(name) ? 'media' : 'data'
  return { alias, label, builtin, typeId, color: contextSourceColor(typeId), icon }
}
export function resolveSourceType(type: ContextType, types: Record<string, ContextType> = {}): ContextType {
  const seen = new Set<string>()
  while (type.kind === 'named' && types[type.name] && !seen.has(type.name)) {
    seen.add(type.name)
    type = types[type.name]
  }
  return type
}
export function sourceFieldExpression(source: string, path: string[] = []): ContextExpr {
  return path.reduce<ContextExpr>((value, field) => ({ kind: 'field', value, field }), { kind: 'resource', name: source })
}
export function createContextSourceField(source: string, path: string[], type: ContextType, rootType: ContextType): ContextSourceField {
  return { source, path, type, typeId: contextSourceTypeId(rootType), label: [source, ...path].join(' · '), expression: sourceFieldExpression(source, path) }
}
export function contextSourceFields(source: string, rootType: ContextType, types: Record<string, ContextType> = {}): ContextSourceField[] {
  const fields: ContextSourceField[] = []
  function visit(type: ContextType, path: string[], ancestors: Set<string>) {
    if (fields.length >= CONTEXT_SOURCE_MAX_NODES || path.length > CONTEXT_SOURCE_MAX_DEPTH) return
    fields.push(createContextSourceField(source, path, type, rootType))
    if (type.kind === 'named' && ancestors.has(type.name)) return
    const next = new Set(ancestors)
    if (type.kind === 'named') next.add(type.name)
    const resolved = resolveSourceType(type, types)
    // A list is inserted as a list. Its member fields are only valid in a loop's scope.
    if (resolved.kind === 'record') for (const [name, item] of Object.entries(resolved.fields)) visit(item, [...path, name], next)
  }
  visit(rootType, [], new Set())
  return fields
}
export function contextTypesCompatible(source: ContextType, target: ContextType, types: Record<string, ContextType> = {}): boolean {
  function valid(type: ContextType, seen = new Set<string>(), depth = 0, budget = { remaining: CONTEXT_SOURCE_MAX_NODES }): boolean {
    if (depth > CONTEXT_SOURCE_MAX_DEPTH || budget.remaining-- <= 0) return false
    if (type.kind === 'named') {
      if (seen.has(type.name) || !types[type.name]) return false
      return valid(types[type.name], new Set([...seen, type.name]), depth + 1, budget)
    }
    if (type.kind === 'record') return Object.values(type.fields).every(field => valid(field, seen, depth + 1, budget))
    return type.kind !== 'list' || valid(type.item, seen, depth + 1, budget)
  }
  function accepts(from: ContextType, to: ContextType): boolean {
    if (from.kind !== to.kind) return false
    if (from.kind === 'record' && to.kind === 'record') return Object.entries(to.fields).every(([key, type]) => Object.hasOwn(from.fields, key) && accepts(from.fields[key], type))
    if (from.kind === 'list' && to.kind === 'list') return accepts(from.item, to.item)
    return canonical(from) === canonical(to)
  }
  return valid(source) && valid(target) && accepts(source, target)
}
export function writeContextSourceDrag(event: DragEvent, field: ContextSourceField): void {
  if (!event.dataTransfer) return
  event.dataTransfer.effectAllowed = 'copy'
  event.dataTransfer.setData(CONTEXT_SOURCE_MIME, JSON.stringify(field))
  event.dataTransfer.setData('text/plain', field.label)
}
export function readContextSourceDrag(event: DragEvent): ContextSourceField | null {
  try {
    const raw = event.dataTransfer?.getData(CONTEXT_SOURCE_MIME)
    if (!raw || raw.length > 65536) return null
    const value = JSON.parse(raw) as ContextSourceField
    if (typeof value.source !== 'string' || !value.source || !Array.isArray(value.path) || value.path.length > CONTEXT_SOURCE_MAX_DEPTH || !value.path.every(field => typeof field === 'string') || typeof value.typeId !== 'string' || !value.type || typeof value.type.kind !== 'string') return null
    return { ...value, expression: sourceFieldExpression(value.source, value.path), label: [value.source, ...value.path].join(' · ') }
  } catch { return null }
}
export function expressionResources(expression: unknown): string[] {
  const names = new Set<string>()
  function visit(value: unknown) {
    if (!value || typeof value !== 'object') return
    if (Array.isArray(value)) { value.forEach(visit); return }
    const item = value as Record<string, unknown>
    if (item.kind === 'resource' && typeof item.name === 'string') names.add(item.name)
    // Literal contents are data, even when they happen to resemble an expression.
    if (item.kind === 'literal') return
    Object.values(item).forEach(visit)
  }
  visit(expression)
  return [...names]
}
export function sourceUseCount(blocks: ContextBlock[], source: string): number {
  return blocks.reduce((count, block) => {
    if (block.kind === 'group') return count + sourceUseCount(block.items, source)
    if (block.kind === 'if') return count + Number(expressionResources(block.condition).includes(source)) + sourceUseCount(block.then, source) + sourceUseCount(block.else, source)
    if (block.kind === 'forEach') return count + Number(expressionResources(block.value).includes(source)) + sourceUseCount(block.items, source)
    return count + Number(expressionResources(block.value).includes(source))
  }, 0)
}
export function contextTypeDependencies(type: ContextType, registry: Record<string, ContextType>): Record<string, ContextType> {
  const dependencies: Record<string, ContextType> = {}
  function visit(item: ContextType) {
    if (item.kind === 'named' && !Object.hasOwn(dependencies, item.name) && registry[item.name]) {
      dependencies[item.name] = registry[item.name]
      visit(registry[item.name])
    } else if (item.kind === 'record') Object.values(item.fields).forEach(visit)
    else if (item.kind === 'list') visit(item.item)
  }
  visit(type)
  return dependencies
}
export function sameContextType(first: ContextType, second: ContextType): boolean { return canonical(first) === canonical(second) }

/** One visual choice per nominal/structural type and complete dependency schema.
 * Providers remain inspectable; schemas with different transitive definitions
 * remain separate choices so the gallery can report their actual conflict.
 */
export function normalizeContextSourceEntries(entries: ContextSourceEntry[]): ContextSourceEntry[] {
  const groups = new Map<string, { entry: ContextSourceEntry; origins: Set<string>; providers: Set<string> }>()
  for (const entry of entries) {
    const types = contextTypeDependencies(entry.type, entry.types)
    const identity = canonical({ type: entry.type, types })
    const existing = groups.get(identity)
    if (!existing) {
      groups.set(identity, { entry: { ...entry, types }, origins: new Set([entry.origin]), providers: new Set(entry.providers) })
      continue
    }
    existing.origins.add(entry.origin)
    entry.providers.forEach(provider => existing.providers.add(provider))
    if (entry.id.startsWith('builtin:') && !existing.entry.id.startsWith('builtin:')) existing.entry = { ...entry, types }
  }
  return [...groups.values()].map(({ entry, origins, providers }) => ({ ...entry, origin: [...origins].filter(Boolean).join(' · '), providers: [...providers] }))
}
