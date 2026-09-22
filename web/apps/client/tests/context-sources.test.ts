import { test } from 'node:test'
import assert from 'node:assert/strict'
import type { ContextType } from '@zedflow/sdk'
import { contextSourceFields, contextTypesCompatible, readContextSourceDrag } from '../src/contextSources'
test('deep imported fields respect type, traversal and drag limits',()=>{
  function nestedType(length: number): ContextType {
    let type: ContextType = { kind: 'text' }
    for (let index = length; index > 0; index--) type = { kind: 'record', fields: { [`level_${index}`]: type } }
    return type
  }
  const deepest = nestedType(64)
  const fields = contextSourceFields('deep', deepest)
  assert.equal((fields.at(-1)?.path).length, 64)
  assert.deepEqual(fields.at(-1)?.type, { kind: 'text' })
  assert.ok(contextTypesCompatible(deepest, deepest))
  assert.ok(!(contextTypesCompatible(nestedType(65), nestedType(65))))
  assert.ok(!(contextSourceFields('deep', nestedType(65)).some(field => field.path.length > 64)))
  function drag(value: unknown) { return { dataTransfer: { getData: () => JSON.stringify(value) } } as unknown as DragEvent }
  assert.equal((readContextSourceDrag(drag(fields.at(-1)))?.path).length, 64)
  assert.equal(readContextSourceDrag(drag({ ...fields.at(-1), path: [...fields.at(-1)!.path, 'too_deep'] })), null)

})
