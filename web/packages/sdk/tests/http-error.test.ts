import { test } from 'node:test';
import assert from 'node:assert/strict';
import { HttpError } from '../dist/index.js';

test('HttpError keeps a useful daemon conflict message without losing its typed evidence', () => {
  const body = { error: 'Le flow a été modifié', extra: [null, 42] }, raw = JSON.stringify(body);
  const error = new HttpError('flows.save', 409, raw, body);
  assert.equal(error.message, 'HTTP 409 during flows.save: Le flow a été modifié');
  assert.equal(error.kind, 'http');
  assert.equal(error.operation, 'flows.save');
  assert.equal(error.status, 409);
  assert.equal(error.rawBody, raw);
  assert.equal(error.body, body);
});

test('HttpError does not stringify arbitrary server bodies as human diagnostics', () => {
  for (const body of [null, 'raw text', [42], 42, {}, { error: null }, { error: {} }, { error: '' }, { error: '  ' }]) {
    const error = new HttpError('fixture', 400, 'original bytes', body);
    assert.equal(error.message, 'HTTP 400 during fixture');
    assert.equal(error.body, body);
    assert.equal(error.rawBody, 'original bytes');
  }
});
