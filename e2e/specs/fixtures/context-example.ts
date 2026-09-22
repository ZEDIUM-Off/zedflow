// Frozen public context request, independent of the client implementation.
import { readFileSync } from 'node:fs'
import type { ContextStrategy, JsonValue } from '@zedflow/sdk'
export function conversationToolsDocumentsExample(id: string): { strategy: ContextStrategy; resources: Record<string, JsonValue> } {
  const value = JSON.parse(readFileSync(new URL('contextExample.json', import.meta.url), 'utf8'))
  value.strategy.id = id
  return value
}
