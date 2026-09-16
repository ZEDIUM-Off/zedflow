import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createClient, RequestValidationError, HttpError, modelSelectionSchema } from '@zedflow/sdk';
test('workspace operations validate commands and encode identities before transport', async () => {
    const requests: {
        url: string;
        method?: string;
        body?: BodyInit | null;
    }[] = [];
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
test('model catalog is workspace-scoped and selections respect provider options', async () => {
    const catalog = { models: [{ provider: 'codex', id: 'example', label: 'Example', reasoningLevels: ['high'], reasoningDefault: null }], providers: [{ id: 'codex', label: 'Codex' }] };
    const auth = { installed: true, authenticated: false, authMode: 'none', message: 'Login required', credentialFilePresent: false, loginCommand: 'codex login', transport: 'codex-responses-compatibility' };
    let url = '';
    const client = createClient({ baseUrl: 'https://fixture.test/api', protocol: 1, fetch: async (current) => { url = current; return new Response(JSON.stringify(current.includes('auth/codex') ? auth : catalog)); } });
    assert.deepEqual(await client.models.list({ workspaceId: 'work/é' }), catalog);
    assert.equal(url, 'https://fixture.test/api/models?workspaceId=work%2F%C3%A9');
    assert.deepEqual(await client.models.codexStatus(), auth);
    assert.equal(modelSelectionSchema.safeParse({ provider: 'fixture', model: 'fixture', reasoningEffort: 'high' }).success, false);
    assert.equal(modelSelectionSchema.safeParse({ provider: 'gemini', model: 'example', thinkingBudget: 1024 }).success, false);
    assert.deepEqual(modelSelectionSchema.parse({ provider: 'codex', model: 'example', reasoningEffort: 'high' }), { provider: 'codex', model: 'example', reasoningEffort: 'high' });
});
test('daemon health and capability inventory expose typed integration metadata', async () => {
    const health = { name: 'Zedflow', daemon: { id: 'daemon-1', host: 'fixture', instanceId: 'process-1' }, defaultWorkspaceId: 'workspace-1', adk: '2.2.0', capabilities: ['context', 'model'] };
    const capabilities = { adkVersion: '2.2.0', graph: { settings: {}, channelReducers: ['overwrite'], builtInChannels: [], retryFields: {} }, nodes: [{ kind: 'model', label: 'Modèle', fields: ['contextNode'], supported: true }], tools: {}, nodePolicies: [], renderers: [], toolDispatch: 'execute_calls', toolDispatchModes: ['execute_calls'], harness: {}, fanIn: {}, providers: [], unsupported: [{ id: 'pending', reason: 'Not integrated' }] };
    const client = createClient({ baseUrl: 'https://fixture.test/api', protocol: 1, fetch: async (url) => new Response(JSON.stringify(url.endsWith('/health') ? health : capabilities)) });
    assert.deepEqual(await client.daemon.health(), health);
    assert.deepEqual(await client.daemon.capabilities(), capabilities);
});
test('daemon version preserves nullable release state and validates activation identities', async () => {
    const build = { component: 'daemon', version: '0.2.0-dev', buildId: 'd'.repeat(64), protocol: 1, storageEpoch: 1, revision: null };
    const status = { daemon: build, client: null, releaseId: null, managed: false, candidate: null, previous: null, operation: null, maintenance: false, activeExecutions: -1, channel: 'local' };
    const requests: {
        url: string;
        body?: BodyInit | null;
    }[] = [];
    const ack = { id: 'request-1', target: 'a'.repeat(64), source: null, expectedDaemonBuildId: build.buildId, createdAt: 123 };
    const client = createClient({ baseUrl: 'https://fixture.test/api', protocol: 1,
        fetch: async (url, init) => { requests.push({ url, body: init?.body }); return new Response(JSON.stringify(url.endsWith('/version') ? status : ack)); },
    });
    assert.deepEqual(await client.daemon.version(), status);
    assert.deepEqual(await client.daemon.applyUpdate({ releaseId: ack.target, expectedDaemonBuildId: build.buildId }), ack);
    await assert.rejects(client.daemon.applyUpdate({ releaseId: '../escape', expectedDaemonBuildId: build.buildId }), RequestValidationError);
    assert.equal(requests.length, 2);
    const legacy = createClient({ baseUrl: 'https://fixture.test/api', protocol: 1, fetch: async () => new Response('missing', { status: 404 }) });
    await assert.rejects(legacy.daemon.version(), error => error instanceof HttpError && error.status === 404);
});
test('workspace and directory listings preserve declared fields and daemon diagnostics', async () => {
    const directory = { path: '/docs', parent: '/', home: '/fixture', entries: [{ name: 'api', path: '/docs/api', directory: true }], diagnostics: ['unreadable hidden folder'] };
    const workspace = { id: 'one', name: 'Docs', path: '/docs', open: false };
    const urls: string[] = [];
    const client = createClient({ baseUrl: 'https://fixture.test/api', protocol: 1,
        fetch: async (url) => { urls.push(url); return new Response(JSON.stringify(url.includes('/filesystem') ? directory : [workspace])); },
    });
    assert.deepEqual(await client.workspaces.list(), [workspace]);
    assert.deepEqual(await client.workspaces.browse({ path: '/docs', showHidden: true }), directory);
    assert.equal(urls[1], 'https://fixture.test/api/filesystem?path=%2Fdocs&showHidden=true');
});
test('context preview validates recursive expressions and preserves open resource values', async () => {
    let sent = '';
    const strategy = { version: 2, id: 'example', name: 'Example', requirements: { rows: { kind: 'list', item: { kind: 'record', fields: { title: { kind: 'text' } } } } }, capabilities: [], program: [{ kind: 'emit', id: 'rows', role: 'data', format: 'json', value: { kind: 'filter', value: { kind: 'resource', name: 'rows' }, item: 'row', condition: { kind: 'present', value: { kind: 'field', value: { kind: 'variable', name: 'row' }, field: 'title' } } } }] };
    const result = { selection: { strategy, source: '// exact\r\n', hash: 'hash' }, evaluation: { complete: true, items: [], capabilities: [], needs: [], diagnostics: [] } };
    const client = createClient({ baseUrl: 'https://fixture.test/api', protocol: 1, fetch: async (_, init) => { sent = String(init?.body); return new Response(JSON.stringify(result)); } });
    assert.deepEqual(await client.context.preview({ workspaceId: 'w', selection: { kind: 'draft', strategy }, resources: { rows: [{ title: 'é', extension: { future: [null, true] } }] } }), result);
    assert.deepEqual(JSON.parse(sent).resources.rows[0].extension, { future: [null, true] });
    await assert.rejects(client.context.preview({ selection: { kind: 'draft', strategy: { ...strategy, program: [{ kind: 'emit', id: 'bad', role: 'data', format: 'json', value: { kind: 'take', value: { kind: 'resource', name: 'rows' }, count: -1 } }] } } }), RequestValidationError);
});
test('flow source authoring carries optimistic hashes and nested channel JSON without loss', async () => {
    const composition = { id: 'flow', name: 'Flow', revision: 0, nodes: [{ id: 'start', type: 'flow', position: { x: 0, y: 0 }, data: { label: 'Start', kind: 'start', config: { extra: { value: [1, null] } } } }], edges: [], channels: [{ name: 'custom', reducer: 'overwrite', default: { nested: ['é'] } }] };
    const file = { key: 'workspace/path', id: 'flow', name: 'Flow', path: '/fixture/flow.rs', hash: 'new-hash', scope: 'workspace', composition, diagnostics: [], source: '// source\r\n' };
    const seen: string[] = [];
    const client = createClient({ baseUrl: 'https://fixture.test/api', protocol: 1, fetch: async (url, init) => { seen.push(url); if (init?.method === 'POST')
            assert.equal(JSON.parse(String(init.body)).expectedHash, 'old-hash'); return new Response(JSON.stringify(file)); } });
    assert.deepEqual(await client.flows.save({ workspaceId: 'w', key: file.key, expectedHash: 'old-hash', composition }), file);
    assert.deepEqual(await client.flows.read(file.key, { workspaceId: 'w' }), file);
    assert.equal(seen[1], 'https://fixture.test/api/flows/workspace%2Fpath?workspaceId=w');
    await assert.rejects(client.flows.save({ composition: { ...composition, nodes: [{ ...composition.nodes[0], position: { x: 'invalid', y: 0 } }] } }), RequestValidationError);
});
test('composition analysis validates bridge predicates before making a request', async () => {
    let calls = 0;
    const diagnostic = { code: 'flow_missing', path: 'request.flow', message: 'Missing flow' };
    const client = createClient({ baseUrl: 'https://fixture.test/api', protocol: 1, fetch: async () => { calls++; return new Response(JSON.stringify({ graph: null, diagnostics: [diagnostic] })); } });
    const bridge = { requires: [], imports: {}, connections: { entry: { from: { instance: 'root', port: 'next' }, to: { instance: 'worker', port: 'main' }, mode: 'callAwait', invocation: 'condition', condition: { kind: 'all', items: [{ kind: 'compare', field: 'ready', operator: 'eq', value: true }] } } }, bindings: {} };
    assert.deepEqual(await client.composition.analyze({ catalog: { types: {}, flows: {}, bridges: { example: bridge } }, request: { flow: 'missing', entry: 'main', bridges: ['example'] } }), { graph: null, diagnostics: [diagnostic] });
    await assert.rejects(client.composition.analyze({ catalog: { types: {}, flows: {}, bridges: { example: { ...bridge, connections: { entry: { ...bridge.connections.entry, condition: { kind: 'all', items: [{ kind: 'compare', field: 'ready', operator: 'invented' }] } } } } } }, request: { flow: 'missing', entry: 'main', bridges: [] } }), RequestValidationError);
    assert.equal(calls, 1);
});
test('context packages keep exact source text and expose validation diagnostics', async () => {
    const bundle = { version: 2, artifacts: [{ kind: 'example', key: 'sample', source: '{ "é": 1 }\r\n', hash: 'source-hash' }] };
    const result = { valid: false, prerequisites: [{ kind: 'flow', key: 'worker' }], diagnostics: [{ code: 'package_hash', path: 'artifacts[0]', message: 'Mismatched source hash' }] };
    let sent = '';
    const client = createClient({ baseUrl: 'https://fixture.test/api', protocol: 1, fetch: async (_, init) => { sent = String(init?.body); return new Response(JSON.stringify(result)); } });
    assert.deepEqual(await client.context.validatePackage({ workspaceId: 'w', package: bundle }), result);
    assert.equal(JSON.parse(sent).package.artifacts[0].source, '{ "é": 1 }\r\n');
    await assert.rejects(client.context.validatePackage({ package: { ...bundle, artifacts: [{ ...bundle.artifacts[0], kind: 'invented' }] } }), RequestValidationError);
});
test('runs keep autonomous identity, validate waits and preserve historical provider data', async () => {
    const ack = { id: 'run/1', workspaceId: 'work/é', revision: 1 };
    const run = { id: ack.id, name: 'Autonomous', status: 'waiting', interactive: false, composition: { id: 'flow', name: 'Flow', revision: 0, nodes: [], edges: [] }, state: { custom: { deep: ['é', null] } }, messages: [], wait: { id: 'wait-1', kind: 'context_production', node: 'context', nodePath: 'root/context', config: { future: true } }, modelBindings: { 'root/model': { provider: 'old-provider', model: 'old-model', thinkingBudget: 1024 } }, workspaceId: ack.workspaceId };
    const requests: {
        url: string;
        body?: string;
    }[] = [];
    const client = createClient({ baseUrl: 'https://fixture.test/api', protocol: 1, fetch: async (url, init) => { requests.push({ url, body: init?.body ? String(init.body) : undefined }); return new Response(JSON.stringify(init?.method === 'POST' ? ack : { type: 'bootstrap', run, revision: 1, cursor: 1 })); } });
    assert.deepEqual(await client.runs.start({ workspaceId: ack.workspaceId, composition: run.composition, input: { input: 'hello' }, modelBindings: {} }), ack);
    assert.deepEqual((await client.runs.snapshot(ack.id, { workspaceId: ack.workspaceId })).run, run);
    assert.equal(requests[1].url, 'https://fixture.test/api/runs/run%2F1/snapshot?workspaceId=work%2F%C3%A9');
    await assert.rejects(client.runs.answer(ack.id, { waitId: '', value: 'yes' }, { workspaceId: ack.workspaceId }), RequestValidationError);
    assert.deepEqual(await client.runs.answer(ack.id, { waitId: 'wait-1', value: { choice: 'yes' } }, { workspaceId: ack.workspaceId }), ack);
});
test('run history exposes scoped pagination, nullable details and immutable window revisions', async () => {
    const window = { nodePath: 'root/context', alias: 'window', entityId: 'entity', revision: 'rev-2', contentRef: 'ref', window: { strategyId: 's', strategyRevision: 'h', items: [], sourceRevisions: {}, capabilities: [] } };
    const urls: string[] = [];
    const client = createClient({ baseUrl: 'https://fixture.test/api', protocol: 1, fetch: async (url) => { urls.push(url); return new Response(JSON.stringify(url.includes('/context-window') ? window : url.includes('/timeline') ? { entries: [], hasMore: false, before: null } : url.includes('/requests/') ? { invocationId: 'inv', manifest: { invocationId: 'inv', nodePath: 'root/model', origin: null, request: { model: 'fixture', contents: [], tools: {}, config: {} }, selection: { provider: 'fixture', model: 'fixture' } }, capture: null, status: 'unavailable' } : { callId: 'call', nodePath: null, name: null, status: 'completed', error: null, result: null })); } });
    assert.deepEqual(await client.runs.timeline('run', { workspaceId: 'w', before: 44 }), { entries: [], hasMore: false, before: null });
    assert.equal(urls[0], 'https://fixture.test/api/runs/run/timeline?workspaceId=w&before=44');
    assert.equal((await client.runs.tool('run', 'call', { workspaceId: 'w' })).error, null);
    assert.equal((await client.runs.request('run', 'inv', { workspaceId: 'w' })).capture, null);
    assert.deepEqual(await client.runs.contextWindow('run', { workspaceId: 'w', nodePath: 'root/context', alias: 'window', revision: 'rev-2' }), window);
    await assert.rejects(client.runs.patchContextWindow('run', { id: 'invalid-uuid', nodePath: 'root/context', alias: 'window', expectedRevision: 'rev-2', patches: [{ kind: 'remove', id: 'item' }] }, { workspaceId: 'w' }), RequestValidationError);
});
test('run deltas validate collection entities and never accept an invalid nested timeline entry', async () => {
    const invalid = { type: 'delta', runId: 'r', workspaceId: 'w', baseRevision: 1, revision: 2, cursor: 2, ops: [{ collection: 'timeline', id: 'm', value: { id: 'm', seq: 2, kind: 'message', role: 'user', text: 42 } }] };
    const client = createClient({ baseUrl: 'https://fixture.test/api', protocol: 1, fetch: async () => new Response(JSON.stringify(invalid)) });
    await assert.rejects(client.runs.snapshot('r', { workspaceId: 'w', after: 1 }), { name: 'ResponseValidationError' });
});
test('node configuration and request options expose typed boundaries while catalogues retain invalid authored nodes', async () => {
    const { nodeConfigSchema, generateContentConfigSchema } = await import('@zedflow/sdk');
    assert.equal(nodeConfigSchema.safeParse({ modelBinding: 'runtime', contextBindings: { source: { kind: 'reader', reader: 'file', input: { kind: 'state', field: 'path' } } }, attachments: { instructions: { items: [{ id: 'i', source: { kind: 'text', text: 'literal' } }] } } }).success, true);
    assert.equal(nodeConfigSchema.safeParse({ contextBindings: { source: { kind: 'reader', reader: 'file', input: { kind: 'invented' } } } }).success, false);
    assert.equal(generateContentConfigSchema.safeParse({ max_output_tokens: 100, extensions: { vendor: { feature: true } } }).success, true);
    assert.equal(generateContentConfigSchema.safeParse({ invented: 100 }).success, false);
    const file = { key: 'broken', id: 'broken', name: 'Broken', path: '/fixture/broken.rs', hash: 'hash', scope: 'workspace', diagnostics: ['Unknown node kind'], composition: { id: 'broken', name: 'Broken', revision: 1, nodes: [{ id: 'node', type: 'flow', position: { x: 0, y: 0 }, data: { kind: 'future-node', label: 'Future', config: null } }], edges: [] } };
    const client = createClient({ baseUrl: 'https://fixture.test/api', protocol: 1, fetch: async () => new Response(JSON.stringify([file])) });
    assert.deepEqual(await client.flows.list(), [file]);
});
test('optional command fields are omitted and RTC signalling preserves scoped transport cancellation', async () => {
    const controller = new AbortController();
    const urls: string[] = [];
    const client = createClient({ baseUrl: 'https://fixture.test/gateway/api', protocol: 7, fetch: async (url, init) => {
            urls.push(url);
            assert.equal(new Headers(init?.headers).get('x-zedflow-protocol'), '7');
            assert.equal(init?.signal, controller.signal);
            if (url.endsWith('/flows')) {
                assert.equal(JSON.parse(String(init?.body)).expectedHash, undefined);
                return new Response(JSON.stringify({ key: 'flow', id: 'flow', name: 'Flow', path: '/fixture/flow.rs', hash: 'h', scope: 'workspace', diagnostics: [] }));
            }
            assert.deepEqual(JSON.parse(String(init?.body)), { type: 'offer', sdp: 'fixture-sdp', after: 4 });
            return new Response(JSON.stringify({ type: 'answer', sdp: 'fixture-answer' }));
        } });
    await client.flows.save({ composition: { id: 'flow', name: 'Flow', revision: 0, nodes: [], edges: [] }, expectedHash: undefined }, controller.signal);
    assert.deepEqual(await client.runs.offerRtc('run/one', { type: 'offer', sdp: 'fixture-sdp', after: 4 }, { workspaceId: 'w/é' }, controller.signal), { type: 'answer', sdp: 'fixture-answer' });
    assert.equal(urls[1], 'https://fixture.test/gateway/api/runs/run%2Fone/rtc?workspaceId=w%2F%C3%A9');
    controller.abort('fixture cancellation');
    await assert.rejects(client.runs.timeline('run/one', { workspaceId: 'w/é' }, controller.signal), { name: 'RequestAbortedError' });
    assert.equal(urls.length, 2);
});

test('node configuration retains authored source references and optional pinned revisions', async () => {
    const { nodeConfigSchema } = await import('@zedflow/sdk');
    const config = {
        contextStrategy: { key: 'conversation', hash: 'a'.repeat(64) },
        contextLibraryRef: { key: 'shared', hash: null },
        contextTypesRef: { key: 'workspace-types' },
    };
    assert.deepEqual(nodeConfigSchema.parse(config), config);
    assert.deepEqual(nodeConfigSchema.parse({ contextStrategy: 'conversation' }), { contextStrategy: 'conversation' });
    assert.equal(nodeConfigSchema.safeParse({ contextStrategy: { key: 'conversation', hash: 12 } }).success, false);
    assert.equal(nodeConfigSchema.safeParse({ contextTypesRef: { hash: 'a'.repeat(64) } }).success, false);
});


test('package catalogue preserves Rust, binary bytes and revision metadata; conversion is explicit', async () => {
    const { flowFileSchema, flowPackageInventory, packageFileBytes } = await import('@zedflow/sdk');
    const { createHash } = await import('node:crypto');
    const rust = '// exact Unicode é\nfn flow() {}\n';
    const manifestSource = JSON.stringify({formatVersion:1,id:'fixture',name:'Fixture',entry:'flow.rs',files:['flow.rs','asset.bin']});
    const revision = 'a'.repeat(64);
    const file = {key:'new-key',id:'fixture',name:'Fixture',path:'/fixture/.zedflow/flows/fixture',scope:'workspace',workspaceId:'workspace',hash:revision,sourceHash:'b'.repeat(64),source:rust,diagnostics:['fixture diagnostic'],package:{root:revision,packages:{[revision]:{manifestSource,files:{'flow.rs':rust,'asset.bin':{base64:'AP/+AA=='}},dependencies:{}}}}};
    const decoded = flowFileSchema.parse(file);
    assert.deepEqual(decoded, file);
    const inventory = await flowPackageInventory(decoded.package!);
    assert.equal(inventory[0].manifest.entry, 'flow.rs');
    const bytes = Uint8Array.of(0,255,254,0);
    assert.deepEqual(packageFileBytes({base64:'AP/+AA=='}), bytes);
    assert.equal(inventory[0].files.find(item=>item.path==='asset.bin')?.sha256, createHash('sha256').update(bytes).digest('hex'));
    assert.equal(inventory[0].files.find(item=>item.path==='flow.rs')?.byteLength, new TextEncoder().encode(rust).byteLength);
    let requests = 0;
    const conversion = {oldKey:'legacy-key',newKey:file.key,oldRevision:'old-hash',newRevision:revision,flow:file,changedConsumers:[{workspaceId:'workspace',key:'bridge',path:'/fixture/bridge.rs'}]};
    const client = createClient({baseUrl:'https://fixture.test/api',protocol:1,fetch:async(url,init)=>{
        requests++;
        assert.equal(url, 'https://fixture.test/api/flows/convert');
        assert.equal(init?.method, 'POST');
        assert.deepEqual(JSON.parse(String(init?.body)), {workspaceId:'workspace',key:'legacy-key',expectedHash:'old-hash'});
        return new Response(JSON.stringify(conversion));
    }});
    assert.equal(requests, 0);
    assert.deepEqual(await client.flows.convertPackage({workspaceId:'workspace',key:'legacy-key',expectedHash:'old-hash'}), conversion);
    await assert.rejects(client.flows.convertPackage({key:'legacy-key',expectedHash:''}), RequestValidationError);
    assert.equal(requests, 1);
});

test('node editor validates structure while preserving mutable drafts and unknown extensions', async () => {
    const { requireNodeConfig, resourceBindingSchema, windowPreparationSchema, flowNodeSchema } = await import('@zedflow/sdk');
    const value = {contextBindings:{input:{kind:'state',field:''}},contextWindow:{alias:'',prepare:{branch:'',routeId:''}},extension:{preserved:true}};
    const draft = requireNodeConfig(value);
    assert.equal(draft, value);
    assert.equal(requireNodeConfig(draft), value);
    assert.equal(resourceBindingSchema.safeParse(value.contextBindings.input).success, false);
    assert.equal(windowPreparationSchema.safeParse(value.contextWindow).success, false);
    value.contextBindings.input.field='input';value.contextWindow.alias='working';value.contextWindow.prepare.branch='prepare';value.contextWindow.prepare.routeId='bridge/prepare';
    assert.equal(requireNodeConfig(value), value);
    assert.equal(resourceBindingSchema.safeParse(value.contextBindings.input).success, true);
    assert.equal(windowPreparationSchema.safeParse(value.contextWindow).success, true);
    assert.deepEqual(draft.extension, {preserved:true});
    assert.throws(()=>requireNodeConfig({contextBindings:{input:{kind:'state',field:42}}}), /Invalid node configuration/);
    const raw = {id:'invalid-draft',type:'flow',position:{x:0,y:0},data:{kind:'model',label:'Draft',config:{contextBindings:{input:{kind:'state',field:42}}}}};
    assert.deepEqual(flowNodeSchema.parse(raw), raw);
});


test('numeric editor drafts stay editable while strict settings reject invalid authored limits', async () => {
    const { requireNodeConfig, retrySettingsSchema, fileItemSchema } = await import('@zedflow/sdk');
    assert.equal(requireNodeConfig({retry:null}).retry, null);
    const value={maxOutputTokens:1.5,topK:-1,retry:{maxAttempts:-1.5},attachments:{files:{items:[{id:'file',path:'/fixture',maxChars:-1,startLine:0.5}]}}};
    assert.equal(requireNodeConfig(value),value);
    assert.equal(retrySettingsSchema.safeParse(value.retry).success,false);
    assert.equal(fileItemSchema.safeParse(value.attachments.files.items[0]).success,false);
    const editable=requireNodeConfig(value);
    delete editable.retry?.maxAttempts;
    assert.equal(requireNodeConfig(value),value);
    editable.maxOutputTokens=null;
    assert.equal(requireNodeConfig(value),value);
    assert.throws(()=>requireNodeConfig({maxOutputTokens:'not a number'}),/Invalid node configuration/);
});


test('legacy public exports use Rust defaults only in detached editor copies', async () => {
    const { compositionSchema, editableComposition, requireNodeConfig, flowExportsReadSchema } = await import('@zedflow/sdk');
    // Exact exports from zf-flows/tests/formats.rs public_entry_validates_its_type...
    // and zf-compiler/tests/portable_export.rs authored source fixture.
    const fixtures=[
        {contract:{entries:{main:{input:{kind:'text'}}}},entries:{main:{node:'context',inputField:'request'}}},
        {contract:{entries:{main:{input:{kind:'text'},output:{kind:'text'}}}},entries:{main:{node:'start',inputField:'input',outputField:'response'}},interactive:false},
    ];
    for(const exports of fixtures){
        const raw=compositionSchema.parse({formatVersion:4,id:'legacy',name:'Legacy',revision:0,nodes:[{id:'start',type:'flow',position:{x:0,y:0},data:{label:'Start',kind:'start',config:{exports,extension:{keep:true}}}}],edges:[]});
        const original=JSON.stringify(raw);
        assert.throws(()=>requireNodeConfig(raw.nodes[0].data.config),/Invalid node configuration/);
        const copy=editableComposition(raw);
        assert.notEqual(copy,raw);assert.equal(JSON.stringify(raw),original);
        const config=requireNodeConfig(copy.nodes[0].data.config);
        assert.deepEqual(config.exports,flowExportsReadSchema.parse(exports));
        assert.deepEqual(config.exports?.contract.branches,{});assert.deepEqual(config.exports?.types,{});
        assert.deepEqual(config.extension,{keep:true});
        assert.equal(requireNodeConfig(copy.nodes[0].data.config),copy.nodes[0].data.config);
        config.exports!.interactive=true;
        assert.equal(requireNodeConfig(copy.nodes[0].data.config).exports?.interactive,true);
        assert.equal(JSON.stringify(raw),original);
    }
});
