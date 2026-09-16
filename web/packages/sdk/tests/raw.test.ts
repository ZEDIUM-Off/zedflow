import assert from 'node:assert/strict';
import test from 'node:test';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createClient, FetchTransport, HttpError, RequestValidationError } from '../dist/index.js';
import { createSessionsClient } from '../dist/sessions/client.js';
import { createGenerationClient } from '../dist/generation/client.js';

test('session export validates the workspace and unique selection before sending a command', async () => {
  const response = { exports: [{ sessionId: 'run-one', path: '/workspace/.zedflow/sessions/run-one', archiveHash: 'a'.repeat(64) }], downloadUrl: '/api/sessions/exports/827ae2ae-c0e6-4389-abde-ed8401597c23.zip?workspaceId=workspace-one' };
  const sent: {url: string; body: string}[] = [];
  const sessions = createSessionsClient(new FetchTransport({ baseUrl: 'https://fixture.test/gateway/api', protocol: 1,
    fetch: async (url, init) => { sent.push({ url, body: String(init?.body) }); return new Response(JSON.stringify(response)); },
  }));
  assert.deepEqual(await sessions.export({ workspaceId: 'workspace-one', sessionIds: ['run-one'] }), response);
  assert.deepEqual(sent, [{ url: 'https://fixture.test/gateway/api/sessions/export', body: '{"workspaceId":"workspace-one","sessionIds":["run-one"]}' }]);
  for (const input of [{ workspaceId: '', sessionIds: ['run-one'] }, { workspaceId: 'workspace-one', sessionIds: [] }, { workspaceId: 'workspace-one', sessionIds: ['run-one', 'run-one'] }]) {
    await assert.rejects(sessions.export(input), RequestValidationError);
  }
  assert.equal(sent.length, 1);
});

test('session import preserves the waiting run and sends no implicit resume command', async () => {
  const run = { id: 'run-one', name: 'Imported', status: 'waiting', interactive: true, workspaceId: 'target', workspacePath: '/target',
    composition: { id: 'flow', name: 'Flow', revision: 0, nodes: [], edges: [] }, state: { custom: { nested: ['keep', 42] } }, messages: [],
    wait: { id: 'wait-one', node: 'question', config: { prompt: 'Continue?', responseType: 'text' } },
    import: { archiveHash: 'hash', sourceSessionId: 'run-one', sourceWorkspace: { id: null, path: '/source' }, importedAt: 123, resumeBlocked: ['missing resource'] },
  };
  const result = { runs: [run], imported: 1, unchanged: 0 };
  const sent: {url: string; body: string}[] = [];
  const sessions = createSessionsClient(new FetchTransport({ baseUrl: 'https://fixture.test/api', protocol: 1,
    fetch: async (url, init) => { sent.push({ url, body: String(init?.body) }); return new Response(JSON.stringify(result)); },
  }));
  assert.deepEqual(await sessions.import({ workspaceId: 'target', path: '../exports/Été.zip' }), result);
  assert.deepEqual(sent, [{ url: 'https://fixture.test/api/sessions/import', body: '{"workspaceId":"target","path":"../exports/Été.zip"}' }]);
  await assert.rejects(sessions.import({ workspaceId: '', path: 'bundle.zip' }), RequestValidationError);
  assert.equal(sent.length, 1);
});

test('generated source keeps exact contents and selects the requested historical passage', async () => {
  const result = { files: [{ path: 'src/main.rs', content: '// Été\r\nfn main() { println!("🦀"); }\r\n' }], executionRevision: { runId: 'run-one', nodePath: 'root/model', occurrenceId: 'visit-2', hash: 'frozen', graphRef: null } };
  let sent: {url: string; body: string} | undefined;
  const generation = createGenerationClient(new FetchTransport({ baseUrl: 'https://fixture.test/api', protocol: 1,
    fetch: async (url, init) => { sent = { url, body: String(init?.body) }; return new Response(JSON.stringify(result)); },
  }));
  const input = { runId: 'run-one', workspaceId: 'workspace-one', nodePath: 'root/model', occurrenceId: 'visit-2', hash: 'frozen' };
  assert.deepEqual(await generation.generate(input), result);
  assert.equal(sent?.url, 'https://fixture.test/api/generate');
  assert.deepEqual(JSON.parse(sent!.body), input);
  await assert.rejects(generation.generate({ workspaceId: 'workspace-one', flowKey: 'saved-flow' }), RequestValidationError);
  const unavailable = createGenerationClient(new FetchTransport({ baseUrl: 'https://fixture.test/api', protocol: 1,
    fetch: async () => new Response('{"error":"Flow changed"}', { status: 409 }),
  }));
  await assert.rejects(unavailable.generate({ workspaceId: 'workspace-one', flowKey: 'saved-flow', flowHash: 'old-hash' }), error => error instanceof HttpError && error.status === 409);
});

test('draft generation and compilation preserve the authored graph and compiler diagnostics', async () => {
  const composition = { id: 'draft', name: 'Draft', revision: 1, nodes: [], edges: [], channels: [{ name: 'custom', reducer: 'overwrite', default: { nested: ['Été', 42] } }] };
  const requests: { url: string; body: string }[] = [];
  const failure = { success: false, output: 'error: fixture compiler diagnostic\r\n', directory: '/temporary/build', files: [{ path: 'src/main.rs', content: 'fn main() {}\n' }] };
  const generation = createGenerationClient(new FetchTransport({ baseUrl: 'https://fixture.test/api', protocol: 1,
    fetch: async (url, init) => { requests.push({ url, body: String(init?.body) }); return new Response(JSON.stringify(url.endsWith('/validate') ? { valid: true, adk: '2.2.0' } : url.endsWith('/generate') ? { files: failure.files } : failure)); },
  }));
  assert.deepEqual(await generation.validate(composition), { valid: true, adk: '2.2.0' });
  assert.deepEqual(await generation.generate(composition), { files: failure.files });
  assert.deepEqual(await generation.build({ workspaceId: 'workspace-one', composition }), failure);
  assert.deepEqual(JSON.parse(requests[1]!.body), composition);
  assert.deepEqual(JSON.parse(requests[2]!.body), { workspaceId: 'workspace-one', composition });
});

test('session downloads retain original binary bytes and scope when an export link is used behind a prefix', async () => {
  const original = new Uint8Array([0x50, 0x4b, 0x03, 0x04, 0, 0xff, 0xc3, 0x28, 13, 10]);
  const requests: string[] = [];
  const sessions = createSessionsClient(new FetchTransport({ baseUrl: 'https://fixture.test/gateway/api', protocol: 1,
    fetch: async url => { requests.push(url); return new Response(original); },
  }));
  const link = '/api/sessions/exports/827ae2ae-c0e6-4389-abde-ed8401597c23.zip?workspaceId=docs%2F%C3%A9';
  assert.deepEqual(await sessions.downloadExport(link), original);
  assert.deepEqual(requests, ['https://fixture.test/gateway/api/sessions/exports/827ae2ae-c0e6-4389-abde-ed8401597c23.zip?workspaceId=docs%2F%C3%A9']);
  for (const invalid of ['https://other.test' + link, link + '&workspaceId=other', link.replace('workspaceId=', 'other='), '/api/../private?workspaceId=one']) {
    await assert.rejects(sessions.downloadExport(invalid), RequestValidationError);
  }
  assert.equal(requests.length, 1);
});

test('invocation raw reads preserve key order, whitespace and Unicode byte for byte', async () => {
  const bytes = new TextEncoder().encode('{\r\n  "z": "Été 🦀",  "a": [1, 2]\r\n}\r\n');
  const seen: string[] = [];
  const client = createClient({ baseUrl: 'https://fixture.test/gateway/api', protocol: 1,
    fetch: async url => { seen.push(url); return new Response(bytes, { headers: { 'Content-Type': 'application/json' } }); },
  });
  assert.deepEqual(await client.runs.requestRaw('run/one', 'invocation/2?', { workspaceId: 'workspace/é' }), bytes);
  assert.deepEqual(seen, ['https://fixture.test/gateway/api/runs/run%2Fone/requests/invocation%2F2%3F/raw?workspaceId=workspace%2F%C3%A9']);
});

test('generated declarations retain typed source selections and session commands', () => {
  const directory = mkdtempSync(join(tmpdir(), 'zedflow-domain-types-'));
  const require = createRequire(import.meta.url);
  try {
    const input = join(directory, 'consumer.mts');
    writeFileSync(input, `
      import type { GenerateInput } from ${JSON.stringify(resolve('dist/generation/commands.js'))};
      import type { ExportSessionsInput } from ${JSON.stringify(resolve('dist/sessions/commands.js'))};
      const source: GenerateInput = { workspaceId:'w', flowKey:'flow', flowHash:'hash' };
      const sessions: ExportSessionsInput = { workspaceId:'w', sessionIds:['run'] };
      // @ts-expect-error Source selections must not become unknown through a schema annotation.
      const primitive: GenerateInput = 42;
      // @ts-expect-error Selecting a saved flow requires its loaded hash.
      const missingHash: GenerateInput = { flowKey:'flow' };
      // @ts-expect-error Export identities are text, not numeric values.
      const badSession: ExportSessionsInput = { workspaceId:'w', sessionIds:[42] };
    `);
    execFileSync(process.execPath, [require.resolve('typescript/bin/tsc'), '--noEmit', '--strict', '--target', 'ES2022', '--module', 'NodeNext', input], { encoding: 'utf8' });
  } finally { rmSync(directory, { recursive: true, force: true }); }
});

test('generated package files preserve binary assets and Unicode source bytes', async () => {
  const { generatedFileSchema, generatedFileBytes } = await import('../dist/generation/model.js');
  const file = generatedFileSchema.parse({ path: 'assets/data.bin', content: 'AP8qgA==', encoding: 'base64' });
  assert.deepEqual([...generatedFileBytes(file)], [0, 255, 42, 128]);
  const text = 'réponse 🦀\n';
  assert.equal(new TextDecoder().decode(generatedFileBytes({ path: 'flow.rs', content: text })), text);
  assert.equal(generatedFileSchema.safeParse({ path: 'x', content: '', encoding: 'unknown' }).success, false);
});
