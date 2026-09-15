
import type { JsonValue } from '@zedflow/sdk'

import type { ContextBlock, ContextExpr, ContextStrategy, ContextType } from '@zedflow/sdk'
import { textMessage, toolExchange } from './components/context/contextMessageProjection'

/** A complete, editable example. These values are fixtures, never acquired resources. */
export function conversationToolsDocumentsExample(id: string): {
  strategy: ContextStrategy
  resources: Record<string, JsonValue>
} {
  const text: ContextType = { kind: 'text' }
  const object: ContextType = { kind: 'record', fields: {} }
  const literal = (value: string): ContextExpr => ({ kind: 'literal', dataType: text, value })
  const resource = (name: string): ContextExpr => ({ kind: 'resource', name })
  const field = (name: string, key: string, variable = false): ContextExpr => ({
    kind: 'field', value: variable ? { kind: 'variable', name } : resource(name), field: key,
  })
  const fragment = (
    blockId: string,
    value: ContextExpr,
    format: 'text' | 'adkMessages' = 'text',
    role: 'data' | 'instruction' = 'data',
  ): ContextBlock => ({ kind: 'emit', id: blockId, role, format, value })

  const strategy: ContextStrategy = {
    version: 2,
    id,
    name: 'Conversation, outils et documents',
    types: {
      Instructions: { kind: 'record', fields: { content: text, origin: text } },
      UserMessage: { kind: 'record', fields: { content: text } },
      ModelOutput: { kind: 'record', fields: { content: text, model: text, usage: object } },
      ToolCall: { kind: 'record', fields: { id: text, name: text, arguments: object } },
      ToolResult: { kind: 'record', fields: { callId: text, status: text, content: text } },
      Document: { kind: 'record', fields: { title: text, content: text, path: text } },
    },
    requirements: {
      instructions: { kind: 'named', name: 'Instructions' },
      userMessage: { kind: 'named', name: 'UserMessage' },
      modelOutput: { kind: 'named', name: 'ModelOutput' },
      toolCall: { kind: 'named', name: 'ToolCall' },
      toolResult: { kind: 'named', name: 'ToolResult' },
      documents: { kind: 'list', item: { kind: 'named', name: 'Document' } },
    },
    capabilities: [],
    program: [
      fragment('instructions', field('instructions', 'content'), 'text', 'instruction'),
      {
        kind: 'group', id: 'conversation', label: 'Conversation',
        items: [
          fragment('user-message', textMessage(field('userMessage', 'content'), 'user'), 'adkMessages'),
          fragment('model-output', textMessage(field('modelOutput', 'content'), 'model'), 'adkMessages'),
        ],
      },
      {
        kind: 'if', id: 'tool-status',
        condition: { kind: 'eq', left: field('toolResult', 'status'), right: literal('erreur') },
        then: [fragment('tool-exchange', toolExchange(resource('toolCall'), resource('toolResult')), 'adkMessages')],
        else: [],
      },
      {
        kind: 'forEach', id: 'documents-loop', value: resource('documents'), item: 'document',
        items: [fragment('document-excerpt', {
          kind: 'template', template: '{{title}}\n{{content}}\nSource : {{path}}',
          values: {
            title: field('document', 'title', true),
            content: { kind: 'truncate', value: field('document', 'content', true), count: 1200 },
            path: field('document', 'path', true),
          },
        })],
      },
    ],
  }

  return {
    // Detach editable schemas and expressions from each other and the builders.
    strategy: JSON.parse(JSON.stringify(strategy)) as ContextStrategy,
    resources: {
      instructions: {
        content: 'Répondez en français. Distinguez les informations fournies des données manquantes et conservez les références des documents.',
        origin: 'Données d’essai de l’exemple',
      },
      userMessage: { content: 'Peux-tu lire le fichier de termes et expliquer comment les flows se composent ?' },
      modelOutput: { content: 'Je vais consulter le document demandé.', model: 'fixture', usage: {} },
      toolCall: { id: '17', name: 'read', arguments: { path: 'docs/terms.md' } },
      toolResult: { callId: '17', status: 'erreur', content: 'Fichier indisponible : docs/terms.md' },
      documents: [{
        title: 'Working System · Documentation d’exemple',
        path: 'docs/composition-example.md',
        content: [
          'Un flow décrit un comportement indépendant, ses points d’entrée et les données qu’il expose. Ses nœuds peuvent préparer un contexte, appeler un modèle, exécuter un outil ou attendre une entrée. La définition reste distincte de ses passages exécutés.',
          'Un bridge compose les points exposés par plusieurs flows. Il choisit les routes disponibles, les données partagées et les droits de leurs destinataires. Une route peut appeler un autre flow et attendre son résultat, le lancer en parallèle ou lui passer la main.',
          'Une stratégie de contexte déclare les types de ressources qu’elle attend. Le flow fournit leurs valeurs par des bindings explicites. Le programme sélectionne les champs, applique ses conditions et produit les fragments transmis au modèle. Déclarer un document ne signifie pas envoyer tout son contenu.',
          'Les sources de cet exemple sont des données d’essai. Le résultat de lecture en erreur est conservé avec son identifiant d’appel, sans exécuter de commande. Les documents fournis séparément restent utilisables ; cette distinction permet de montrer une erreur d’outil sans produire une erreur du moteur de contexte.',
          'Chaque document est traité par le bloc Pour chaque. Le titre et le chemin sont conservés, tandis que le contenu est limité à 1 200 caractères Unicode. Modifier cette limite ou ajouter un second document permet d’observer les fragments et leurs occurrences dans l’aperçu.',
        ].join('\n\n'),
      }],
    },
  }
}
