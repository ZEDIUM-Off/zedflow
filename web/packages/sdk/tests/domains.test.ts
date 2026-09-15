import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createClient, RequestValidationError } from '@zedflow/sdk';

test('workspace operations validate commands and encode identities before transport', async () => {
  const requests: { url: string; method?: string; body?: BodyInit | null }[] = [];
  const workspace = { id: 'work/é', name: 'Docs', path: '/fixture/docs', open: true };
  const client = createClient({ baseUrl: 'https://fixture.test/api', protocol: 1,
    fetch: async (url, init) => {
      requests.push({ url, method: init?.method, body: init?.body });
      return new Response(JSON.stringify(workspace));
    },
  });
  assert.deepEqual(await client.workspaces.open({ path: '/fixture/docs' }), workspace);
  assert.equal(requests[0]?.method, 'POST');
  assert.equal(requests[0]?.body, '{"path":"/fixture/docs"}');
  assert.deepEqual(await client.workspaces.update('work/é', { name: 'Docs' }), workspace);
  assert.equal(requests[1]?.url, 'https://fixture.test/api/workspaces/work%2F%C3%A9');
  assert.equal(requests[1]?.method, 'PATCH');
  await assert.rejects(client.workspaces.open({ path: '' }), RequestValidationError);
  await assert.rejects(client.workspaces.update('work/é', { name: '   ' }), RequestValidationError);
  assert.equal(requests.length, 2);
});

test('workspace and directory listings preserve declared fields and daemon diagnostics', async () => {
  const directory = { path: '/docs', parent: '/', home: '/fixture', entries: [{ name: 'api', path: '/docs/api', directory: true }], diagnostics: ['unreadable hidden folder'] };
  const workspace = { id: 'one', name: 'Docs', path: '/docs', open: false };
  const urls: string[] = [];
  const client = createClient({ baseUrl: 'https://fixture.test/api', protocol: 1,
    fetch: async url => { urls.push(url); return new Response(JSON.stringify(url.includes('/filesystem') ? directory : [workspace])); },
  });
  assert.deepEqual(await client.workspaces.list(), [workspace]);
  assert.deepEqual(await client.workspaces.browse({ path: '/docs', showHidden: true }), directory);
  assert.equal(urls[1], 'https://fixture.test/api/filesystem?path=%2Fdocs&showHidden=true');
});
