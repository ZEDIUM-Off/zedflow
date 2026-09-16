import { isNodeConfig, historicalModelSelectionSchema } from '@zedflow/sdk'

import type { Composition, FlowNode, ModelEntry, ModelSelection, WorkspaceContext } from '@zedflow/sdk'

export type { ModelSelection } from '@zedflow/sdk'

export type SkillEntry = WorkspaceContext['skills'][number]
export interface ModelNode { path: string; group: string; node: FlowNode; runtime: boolean; context?: FlowNode; contextPath?: string }

export function modelNodes(composition: Composition, prefix = '', group = ''): ModelNode[] {
  return composition.nodes.flatMap(node => {
    if (!isNodeConfig(node.data.config)) return []
    const config = node.data.config
    const path = prefix ? `${prefix}/${node.id}` : node.id
    if (['agent','model'].includes(node.data.kind)) {
      const context = node.data.kind === 'model' ? composition.nodes.find(candidate => candidate.id === config.contextNode) : undefined
      return [{ path, group, node, runtime: node.data.config.modelBinding === 'runtime', context, contextPath: context ? (prefix ? `${prefix}/${context.id}` : context.id) : undefined }]
    }
    if (node.data.kind === 'subgraph' && node.data.config.composition) {
      return modelNodes(node.data.config.composition, path, group ? `${group} / ${node.data.label}` : node.data.label)
    }
    return []
  })
}

export function fixedSelection(entry: ModelNode): import('@zedflow/sdk').HistoricalModelSelection {
  const config = entry.node.data.config
  if (!isNodeConfig(config)) throw new Error(`Configuration de modèle invalide pour ${entry.path}`)
  return historicalModelSelectionSchema.parse({ provider: config.provider || 'fixture', model: config.model || (config.provider === 'fixture' ? 'fixture' : ''), reasoningEffort: config.reasoningEffort, reasoningSummary: config.reasoningSummary, textVerbosity: config.textVerbosity, thinkingBudget: config.thinkingBudget })
}
