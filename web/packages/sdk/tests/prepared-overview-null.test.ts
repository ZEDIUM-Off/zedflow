import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { preparedCompositionSchema } from '../dist/index.js';

const capture = JSON.parse(readFileSync(new URL('./fixtures/daemon-prepared-overviews.json', import.meta.url), 'utf8'));
const fields = ['contextProgramHash', 'contextPath', 'context'] as const;

test('prepared daemon overviews preserve null or paired inference context without inventing values', () => {
  for (const fixture of [capture.unpaired, capture.paired]) assert.deepEqual(preparedCompositionSchema.parse(fixture), fixture);
  const missing = structuredClone(capture.unpaired);
  for (const field of fields) {
    assert.equal(capture.unpaired.overview.inferences['root/model'][field], null);
    assert.notEqual(capture.paired.overview.inferences['root/model'][field], null);
    delete missing.overview.inferences['root/model'][field];
  }
  assert.deepEqual(preparedCompositionSchema.parse(missing), missing);
});

test('prepared overview nullable fields still reject incorrect nonnull data', () => {
  for (const [field, invalid] of [
    ['contextProgramHash', 3], ['contextProgramHash', {}], ['contextPath', []], ['contextPath', false],
    ['context', 'text'], ['context', []], ['context', {}], ['context', { node: 3, label: 'bad', config: {} }],
    ['context', { node: 'context', label: 'bad', config: [] }],
  ] as const) {
    const fixture = structuredClone(capture.unpaired);
    fixture.overview.inferences['root/model'][field] = invalid;
    assert.equal(preparedCompositionSchema.safeParse(fixture).success, false, field);
  }
});
