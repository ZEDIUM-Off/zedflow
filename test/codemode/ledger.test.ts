import { richResult, streamingTool } from '../support/codemode/streaming-tool.ts';
import { join } from 'node:path';
import type { ToolResultMessage } from '@earendil-works/pi-ai';
import { expect, test } from 'vite-plus/test';
import { createCodemodeTool } from '../../packages/tools/src/codemode.ts';
import { fixture, execute, text, png } from '../support/codemode/invocation.ts';
import { customTool } from '../support/codemode/custom-tool.ts';

test('Code Mode bounds large nested results and streaming ledgers without losing explicit script output', async () => {
  const cwd = await fixture();
  const payload = 'payload-'.repeat(128 * 1024);
  const rich = richResult(payload);
  const updates: ToolResultMessage[] = [];
  const nested = streamingTool(rich);
  const tool = createCodemodeTool({ tools: [nested], artifactDirectory: join(cwd, 'output') });
  const result = await execute(
    tool,
    'const result = await tools.custom({value:"x".repeat(5000)}); text(result.content[0].text.length); image(result.content[1]); return result.content[0].text.slice(0,8);',
    new AbortController().signal,
    (update) => updates.push(update),
  );
  expect(result.isError).toBe(false);
  expect(text(result)).toContain(String(payload.length));
  expect(text(result)).toContain('payload-');
  expect(result.content).toContainEqual({ type: 'image', data: png, mimeType: 'image/png' });
  expect(result.nestedCalls).toMatchObject({
    calls: [{ status: 'ok', argumentsBytes: 5012 }],
    complete: false,
  });
  expect(updates).toHaveLength(4);
  for (const update of updates) {
    expect(update.content).toEqual([]);
    expect(JSON.stringify(update).length).toBeLessThan(3000);
    expect(update.nestedCalls?.calls.length).toBeLessThanOrEqual(1);
  }
  expect(JSON.stringify(result.nestedCalls).length).toBeLessThan(3000);
  expect(JSON.stringify(result.nestedCalls)).not.toContain(png);
  expect(JSON.stringify(result.nestedCalls)).not.toContain('payload');
  expect(result.details).toBeUndefined();
});

test('Code Mode caps native ledger records, aggregate argument bytes and error previews', async () => {
  const updates: ToolResultMessage[] = [];
  const nested = customTool(async () => ({
    role: 'toolResult',
    toolCallId: 'nested',
    toolName: 'custom',
    timestamp: 1,
    isError: false,
    content: [{ type: 'text', text: 'ok' }],
  }));
  const tool = createCodemodeTool({ tools: [nested], maxToolCalls: 110 });
  const result = await execute(
    tool,
    'for(let i=0;i<110;i++) { await tools.custom({value:"x".repeat(500)}); } return "all ran";',
    new AbortController().signal,
    (update) => updates.push(update),
  );
  expect(result.isError).toBe(false);
  expect(text(result)).toContain('all ran');
  expect(result.nestedCalls?.calls).toHaveLength(100);
  expect(result.nestedCalls?.complete).toBe(false);
  const argumentBytes = result.nestedCalls?.calls.reduce(
    (sum, call) => sum + (call.arguments ? Buffer.byteLength(JSON.stringify(call.arguments)) : 0),
    0,
  );
  expect(argumentBytes).toBeLessThanOrEqual(16 * 1024);
  expect(result.nestedCalls?.calls.some((call) => call.argumentsBytes !== undefined)).toBe(true);
  expect(updates.length).toBeLessThanOrEqual(3 * 110 + 1);
  expect(updates.every((update) => (update.nestedCalls?.calls.length ?? 0) <= 1)).toBe(true);
  const failed = createCodemodeTool({
    tools: [
      customTool(async () => {
        throw new Error('x'.repeat(50000));
      }),
    ],
  });
  const error = await execute(
    failed,
    'try { await tools.custom({value:"x"}); } catch(e) { return e.message.length; }',
  );
  expect(text(error)).toContain('50000');
  expect(error.nestedCalls?.calls[0]?.error).toHaveLength(500);
});
