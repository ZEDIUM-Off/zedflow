import type { InjectionKey, Ref } from 'vue'
import type { ContextExpr, ContextType } from '@zedflow/sdk'
import { CONTEXT_SOURCE_MAX_DEPTH, resolveSourceType, type ContextSourceField } from '../../contextSources'

export interface ContextSocketTarget {
  id: string
  label: string
  accepts(field: ContextSourceField): boolean
  insert(field: ContextSourceField): boolean
}
export interface ContextSocketSelection {
  selectedId: Ref<string | null>
  register(target: ContextSocketTarget): void
  select(target: ContextSocketTarget): void
  clear(id: string): void
}
export const CONTEXT_SOCKET_SELECTION: InjectionKey<ContextSocketSelection> = Symbol('context-socket-selection')

/** A known expression type is used for insertion feedback, never as runtime validation. */
export function contextExpressionType(expression: ContextExpr, resources: Record<string, ContextType>, types: Record<string, ContextType> = {}, variables: Record<string, ContextType> = {}, depth = 0): ContextType | undefined {
  if (depth > CONTEXT_SOURCE_MAX_DEPTH) return undefined
  const infer = (value: ContextExpr, locals = variables) => contextExpressionType(value, resources, types, locals, depth + 1)
  switch (expression.kind) {
    case 'resource': return resources[expression.name]
    case 'variable': return variables[expression.name]
    case 'literal': return expression.dataType
    case 'list': return { kind: 'list', item: expression.itemType }
    case 'construct': return { kind: 'named', name: expression.name }
    case 'template': case 'toJson': case 'truncate': return { kind: 'text' }
    case 'measure': return { kind: 'number' }
    case 'field': {
      const source = infer(expression.value)
      const resolved = source && resolveSourceType(source, types)
      return resolved?.kind === 'record' ? resolved.fields[expression.field] : undefined
    }
    case 'project': {
      const source = infer(expression.value)
      const resolved = source && resolveSourceType(source, types)
      if (resolved?.kind !== 'record' || expression.fields.some(field => !resolved.fields[field])) return undefined
      return { kind: 'record', fields: Object.fromEntries(expression.fields.map(field => [field, resolved.fields[field]])) }
    }
    case 'record': {
      const entries = Object.entries(expression.fields).map(([name, value]) => [name, infer(value)] as const)
      if (entries.some(([, type]) => !type)) return undefined
      return { kind: 'record', fields: Object.fromEntries(entries) as Record<string, ContextType> }
    }
    case 'map': {
      const source = infer(expression.value)
      const resolved = source && resolveSourceType(source, types)
      const body = infer(expression.body, resolved?.kind === 'list' ? { ...variables, [expression.item]: resolved.item } : variables)
      return body ? { kind: 'list', item: body } : undefined
    }
    case 'filter': case 'sort': case 'take': case 'dedup': return infer(expression.value)
    case 'groupBy': case 'call': return undefined
  }
}

export function contextExpressionDescription(expression: ContextExpr): string {
  switch (expression.kind) {
    case 'resource': return expression.name || 'Choisir une source'
    case 'variable': return expression.name || 'Élément courant'
    case 'field': return `${contextExpressionDescription(expression.value)} · ${expression.field || 'champ'}`
    case 'literal': return typeof expression.value === 'string' ? expression.value || 'Saisir une valeur' : JSON.stringify(expression.value)
    case 'template': return 'Composer un texte'
    case 'record': return 'Assembler des champs'
    case 'list': return `Assembler une liste · ${expression.items.length} éléments`
    case 'construct': return expression.name || 'Construire un type'
    case 'project': return `Champs : ${expression.fields.join(', ') || 'à choisir'}`
    case 'filter': return 'Filtrer les éléments'
    case 'sort': return expression.descending ? 'Trier · décroissant' : 'Trier · croissant'
    case 'take': return `Garder ${expression.count} éléments`
    case 'truncate': return `Extrait · ${expression.count} caractères`
    case 'map': return 'Transformer chaque élément'
    case 'groupBy': return 'Regrouper par clé'
    case 'dedup': return 'Retirer les doublons'
    case 'toJson': return 'Convertir en texte JSON'
    case 'measure': return `Mesurer · ${{ bytes: 'octets', items: 'éléments', media: 'médias' }[expression.unit]}`
    case 'call': return expression.name || 'Appeler une fonction'
  }
}
