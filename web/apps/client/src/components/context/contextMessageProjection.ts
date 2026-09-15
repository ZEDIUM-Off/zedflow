import { textExpression } from '../../contextEngine'
import type { ContextExpr, ContextType } from '@zedflow/sdk'

const object: ContextType = { kind: 'record', fields: {} }
const list = (items: ContextExpr[]): ContextExpr => ({ kind: 'list', itemType: object, items })
const record = (fields: Record<string, ContextExpr>): ContextExpr => ({ kind: 'record', fields })
const field = (value: ContextExpr, name: string): ContextExpr => ({ kind: 'field', value, field: name })

function sameShape(left: unknown, right: unknown): boolean {
  if (left === right) return true
  if (!left || !right || typeof left !== 'object' || typeof right !== 'object') return false
  if (Array.isArray(left) || Array.isArray(right)) return Array.isArray(left) && Array.isArray(right) && left.length === right.length && left.every((value, index) => sameShape(value, right[index]))
  const a = left as Record<string, unknown>, b = right as Record<string, unknown>
  return Object.keys(a).length === Object.keys(b).length && Object.keys(a).every(key => Object.hasOwn(b, key) && sameShape(a[key], b[key]))
}

/** These are ordinary, editable engine expressions, not an alternate execution path. */
export function textMessage(value: ContextExpr, role: 'user' | 'model'): ContextExpr {
  return list([record({ role: textExpression(role), parts: list([record({ text: value })]) })])
}
export function toolExchange(call: ContextExpr, result: ContextExpr): ContextExpr {
  return list([
    record({ role: textExpression('model'), parts: list([record({ name: field(call, 'name'), id: field(call, 'id'), args: field(call, 'arguments') })]) }),
    record({ role: textExpression('function'), parts: list([record({ id: field(result, 'callId'), functionResponse: record({ name: field(call, 'name'), response: record({ content: field(result, 'content'), status: field(result, 'status') }) }) })]) }),
  ])
}
export function textMessageValue(expression: ContextExpr): { role: 'user' | 'model'; value: ContextExpr } | undefined {
  if (expression.kind !== 'list' || expression.items.length !== 1) return
  const message = expression.items[0]
  if (message.kind !== 'record' || Object.keys(message.fields).length !== 2) return
  const role = message.fields.role, parts = message.fields.parts
  if (role?.kind !== 'literal' || (role.value !== 'user' && role.value !== 'model') || parts?.kind !== 'list' || parts.items.length !== 1) return
  const part = parts.items[0]
  if (part.kind !== 'record' || Object.keys(part.fields).length !== 1 || !part.fields.text) return
  if (!sameShape(expression, textMessage(part.fields.text, role.value))) return
  return { role: role.value, value: part.fields.text }
}

/** Recognize only the exact editable projection so custom message fields stay visible. */
export function toolExchangeValue(expression: ContextExpr): { call: ContextExpr; result: ContextExpr } | undefined {
  if (expression.kind !== 'list' || expression.items.length !== 2) return
  const [callMessage, resultMessage] = expression.items
  if (callMessage.kind !== 'record' || resultMessage.kind !== 'record') return
  const callParts = callMessage.fields.parts, resultParts = resultMessage.fields.parts
  if (callParts?.kind !== 'list' || resultParts?.kind !== 'list') return
  const callPart = callParts.items[0], resultPart = resultParts.items[0]
  if (callPart?.kind !== 'record' || resultPart?.kind !== 'record') return
  const name = callPart.fields.name, callId = resultPart.fields.id
  if (name?.kind !== 'field' || name.field !== 'name' || callId?.kind !== 'field' || callId.field !== 'callId') return
  const call = name.value, result = callId.value
  return sameShape(expression, toolExchange(call, result)) ? { call, result } : undefined
}
