import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createClient, executedDefinitionSchema, SyncProtocolError, HttpError } from '../dist/index.js';

const source = '// exact shared source\nfn flow() {}\n';
const exact = { exact: true, runId: 'run', instance: 'root', nodePath: 'out', occurrenceId: 'passage', key: 'flow', hash: 'source-hash', source, composition: { id: 'flow', name: 'Flow', revision: 0, nodes: [], edges: [] } };

test('executed definition keeps source identity distinct from optional executable revision', () => {
  assert.deepEqual(executedDefinitionSchema.parse(exact), exact);
  assert.deepEqual(executedDefinitionSchema.parse({ ...exact, definitionRevision: 'definition-revision' }), { ...exact, definitionRevision: 'definition-revision' });
  for (const definitionRevision of [null, 3, {}, []]) assert.equal(executedDefinitionSchema.safeParse({ ...exact, definitionRevision }).success, false);
});

test('definition pins and cache distinguish executable revisions with the same source', async () => {
  let calls = 0;
  const client = createClient({ baseUrl: 'https://fixture.invalid/api', protocol: 1, fetch: async url => {
    calls++;
    const query = new URL(url).searchParams;
    if (query.get('workspaceId') !== 'workspace') return new Response('{"error":"Not found"}', { status: 404 });
    const requested = query.get('hash');
    const revision = requested === 'definition-after' ? 'definition-after' : 'definition-before';
    return new Response(JSON.stringify({ ...exact, ...(requested === 'source-hash' ? {} : { definitionRevision: revision }) }));
  } });
  try {
    const scope = { runId: 'run', workspaceId: 'workspace' };
    const before = { ...scope, query: { nodePath: 'out', occurrenceId: 'passage', hash: 'definition-before' } };
    const after = { ...scope, query: { nodePath: 'out', occurrenceId: 'passage', hash: 'definition-after' } };
    const a = await client.definitions.load(before), b = await client.definitions.load(after);
    assert.equal(a.exact && a.source, source);
    assert.equal(b.exact && b.source, source);
    assert.notEqual(a, b);
    assert.equal(a.exact && a.definitionRevision, 'definition-before');
    assert.equal(b.exact && b.definitionRevision, 'definition-after');
    assert.equal(await client.definitions.load(before), a);
    assert.equal(calls, 2);
    const legacy = await client.definitions.load({ ...scope, query: { hash: 'source-hash' } });
    assert.equal(legacy.exact && legacy.hash, 'source-hash');
    await assert.rejects(client.definitions.load({ ...before, query: { ...before.query, hash: 'wrong-definition' } }), SyncProtocolError);
    await assert.rejects(client.definitions.load({ ...before, query: { ...before.query, occurrenceId: 'another-passage' } }), SyncProtocolError);
    await assert.rejects(client.definitions.load({ ...before, workspaceId: 'other-workspace' }), (error: unknown) => error instanceof HttpError && error.status === 404);
  } finally { client.dispose(); }
});

test('unidentified whole-run reads are not global latest caches; explicit admission identities share reads', async () => {
  let calls=0;
  const options={baseUrl:'https://fixture.invalid/api',protocol:1,fetch:async()=>{
    calls++;
    return new Response(JSON.stringify({...exact,nodePath:'',occurrenceId:null}));
  }};
  const client=createClient(options),other=createClient(options);
  const request={runId:'run',workspaceId:'workspace',query:{}};
  try {
    for(let i=0;i<3;i++) await client.definitions.load(request);
    assert.equal(calls,3,'F4 repro: unpinned reads are deliberately not reusable');
    const initial={...request,revision:'initial:content-reference'};
    for(let i=0;i<3;i++) await client.definitions.load(initial);
    assert.equal(calls,4);
    await other.definitions.load(initial);
    assert.equal(calls,5,'SDK instances never share caches');
    await client.definitions.load({...initial,revision:'initial:replacement-content'});
    assert.equal(calls,6);
    client.definitions.invalidate(initial);
    await client.definitions.load(initial);
    assert.equal(calls,7);
  } finally {client.dispose();other.dispose();}
});
