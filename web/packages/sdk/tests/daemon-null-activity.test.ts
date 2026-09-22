import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { flowRevisionSchema, nodeActivitySchema } from '../dist/index.js';

// Captured from the isolated fixture daemon, not a model of its serializer.
const activity = JSON.parse(readFileSync(new URL('./fixtures/daemon-null-activity.json', import.meta.url), 'utf8'));

test('daemon activity preserves explicit null graph pin and renderer', () => {
  const parsed = nodeActivitySchema.parse(activity);
  assert.deepEqual(parsed, activity);
  assert.equal(parsed.ui, null);
  assert.equal(parsed.flowRevision!.graphRef, null);
});

test('optional graph pin and renderer retain absence and valid values', () => {
  const { graphRef: _graph, ...revision } = activity.flowRevision;
  const { ui: _ui, ...withoutUi } = activity;
  assert.equal(Object.hasOwn(flowRevisionSchema.parse(revision), 'graphRef'), false);
  assert.equal(Object.hasOwn(nodeActivitySchema.parse(withoutUi), 'ui'), false);
  assert.equal(flowRevisionSchema.parse({ ...revision, graphRef: 'exact-pin' }).graphRef, 'exact-pin');
  const ui = { renderer: 'table', title: 'Fixture', language: 'json', extension: [null, 4] };
  assert.deepEqual(nodeActivitySchema.parse({ ...activity, ui }).ui, ui);
});

test('non-null graph pins and renderers still reject invalid types', () => {
  for (const graphRef of [false, 4, [], {}]) {
    assert.equal(flowRevisionSchema.safeParse({ ...activity.flowRevision, graphRef }).success, false);
  }
  for (const ui of [false, 4, [], 'table', { renderer: 4 }, { title: false }]) {
    assert.equal(nodeActivitySchema.safeParse({ ...activity, ui }).success, false);
  }
});
