import { expect, test } from 'vite-plus/test';
import { AuthPanel, TuiAuthInteraction } from '../../packages/tui/src/index.ts';
import {
  counterMachine,
  createAuthFixture,
  OAUTH_CODE,
  SECRET,
} from '../../packages/tui/test/fixtures.ts';
import { CTRL_C, DOWN, ENTER, ESCAPE, PASTE_END, PASTE_START } from '../support/tui/keys.ts';
import { Session } from '../support/tui/session.ts';

test('/login api-key reads a masked key through the native provider flow and /auth, /logout reflect the store', async () => {
  const session = await Session.open('counter', counterMachine);
  const { terminal, auth } = session;
  await session.screen.waitFor('"count": 0');
  session.screen.submit('/auth');
  await session.screen.waitFor('OpenAI: not signed in');
  session.screen.submit('/login api-key');
  const prompt = await session.screen.waitFor('Enter OpenAI API key');
  expect(prompt).toContain('Sign in to OpenAI');
  session.screen.type(SECRET);
  const masked = await session.screen.screen();
  expect(masked).toContain('*'.repeat(SECRET.length));
  expect(masked).not.toContain(SECRET);
  terminal.sendInput(ENTER);
  const done = await session.screen.waitFor('Signed in to OpenAI (api_key).');
  expect(done).not.toContain('Enter OpenAI API key');
  expect(await auth.credentials.read('openai')).toEqual({ type: 'api_key', key: SECRET });
  session.screen.submit('/auth');
  await session.screen.waitFor('signed in (api_key');
  // The editor is back and focused: events still reach the machine.
  session.screen.submit('{"type":"INCREMENT"}');
  await session.screen.waitFor('"count": 1');
  session.screen.submit('/logout');
  await session.screen.waitFor('Signed out of OpenAI.');
  expect(await auth.credentials.read('openai')).toBeUndefined();
  session.screen.submit('/auth');
  await session.screen.waitFor('OpenAI: not signed in');
  const everything = terminal.writes.join('') + (await terminal.scrollback()).join('\n');
  expect(everything).not.toContain(SECRET);
  expect(JSON.stringify(session.run.runs.records(session.run.runId))).not.toContain(SECRET);
  await session.finish();
  session.run.runs.stop(session.run.runId);
});

test('/login with no argument offers the provider methods and runs OAuth with a masked pasted code', async () => {
  const session = await Session.open('counter', counterMachine);
  const { terminal, auth } = session;
  await session.screen.waitFor('"count": 0');

  // Arrow keys move the native selection: the second method is the API key.
  session.screen.submit('/login');
  const choice = await session.screen.waitFor('Choose a sign-in method');
  expect(choice).toContain('Sign in with ChatGPT');
  expect(choice).toContain('OpenAI API key');
  terminal.sendInput(DOWN);
  terminal.sendInput(ENTER);
  await session.screen.waitFor('Enter OpenAI API key');
  session.screen.submit(SECRET);
  await session.screen.waitFor('Signed in to OpenAI (api_key).');
  expect(await auth.credentials.read('openai')).toEqual({ type: 'api_key', key: SECRET });

  auth.arm();
  session.screen.submit('/login');
  await session.screen.waitFor('Choose a sign-in method');
  terminal.sendInput(ENTER);
  const url = await session.screen.waitFor('Open this URL to sign in:');
  expect(url).toContain('Complete sign-in in your browser');
  expect(url).toContain('paste the final redirect URL');
  expect(url).toContain('Complete login in your browser');
  expect(url).toContain('(http://localhost:1455/auth/callback)');
  expect(terminal.writes.join('')).toContain(
    '\u001b]8;;https://auth.example.test/authorize?state=abc',
  );
  terminal.sendInput(
    `${PASTE_START}http://localhost:1455/auth/callback?code=${OAUTH_CODE}&state=abc${PASTE_END}`,
  );
  const masked = await session.screen.screen();
  expect(masked).not.toContain(OAUTH_CODE);
  expect(masked).toContain('*');
  terminal.sendInput(ENTER);
  await session.screen.waitFor('Signed in to OpenAI (oauth).');
  expect(await auth.credentials.read('openai')).toMatchObject({
    type: 'oauth',
    access: 'access-token',
  });
  // Models.login saw the injected options, and the pasted-code prompt had its own signal.
  expect(auth.loginOptions).toHaveLength(1);
  expect(auth.loginOptions[0]?.getDeviceId?.()).toBe('00000000-0000-4000-8000-000000000000');
  expect(auth.promptSignals).toHaveLength(1);
  expect(auth.promptSignals[0]?.aborted).toBe(true);
  const everything = terminal.writes.join('') + (await terminal.scrollback()).join('\n');
  expect(everything).not.toContain(OAUTH_CODE);
  expect(everything).not.toContain(SECRET);
  await session.finish();
  session.run.runs.stop(session.run.runId);
});

test('a provider-cancelled prompt disappears, Ctrl+C cancels the flow and failures redact entered codes', async () => {
  const session = await Session.open('counter', counterMachine);
  const { terminal, auth } = session;
  await session.screen.waitFor('"count": 0');

  // The browser callback wins: only the pasted-code prompt is cancelled, the login completes.
  session.screen.submit('/login oauth');
  await session.screen.waitFor('Complete login in your browser');
  auth.callback.resolve('callback-code');
  const won = await session.screen.waitFor('Signed in to OpenAI (oauth).');
  expect(won).not.toContain('Complete login in your browser');
  expect(auth.promptSignals[0]?.aborted).toBe(true);

  // Ctrl+C aborts the whole flow; nothing is stored.
  await auth.credentials.delete('openai');
  auth.arm();
  session.screen.submit('/login oauth');
  await session.screen.waitFor('Complete login in your browser');
  session.screen.type('half-typed-secret');
  terminal.sendInput(CTRL_C);
  const cancelled = await session.screen.waitFor('Login cancelled.');
  expect(cancelled).not.toContain('Complete login in your browser');
  expect(await auth.credentials.read('openai')).toBeUndefined();
  expect(terminal.running).toBe(true);

  // Escape inside the prompt cancels too.
  session.screen.submit('/login api-key');
  await session.screen.waitFor('Enter OpenAI API key');
  terminal.sendInput(ESCAPE);
  await session.screen.waitFor('Login cancelled.');
  expect(await auth.credentials.read('openai')).toBeUndefined();

  // A failing flow reports a message that never carries the pasted code.
  auth.arm();
  auth.failure = new Error('token exchange rejected');
  session.screen.submit('/login oauth');
  await session.screen.waitFor('Complete login in your browser');
  terminal.sendInput(`${PASTE_START}${OAUTH_CODE}${PASTE_END}`);
  terminal.sendInput(ENTER);
  const failed = await session.screen.waitFor('Login failed.');
  expect(failed).not.toContain(OAUTH_CODE);
  expect(terminal.writes.join('')).not.toContain(OAUTH_CODE);

  session.screen.submit('/login nonsense');
  await session.screen.waitFor('Usage: /login');
  await session.finish();
  session.run.runs.stop(session.run.runId);
});

test('the native interaction survives Models.login copying it and keeps prompt signals separate', async () => {
  const auth = createAuthFixture();
  const panel = new AuthPanel({ requestRender: () => {}, onCancel: () => {} });
  const controller = new AbortController();
  const interaction = new TuiAuthInteraction({ panel, signal: controller.signal });
  expect(Object.keys(interaction).sort()).toEqual(['notify', 'prompt', 'signal']);
  const login = auth.models.login('openai', 'api_key', interaction);
  await new Promise((resolve) => setTimeout(resolve, 10));
  expect(panel.render(60).join('\n')).toContain('Enter OpenAI API key');
  controller.abort();
  await expect(login).rejects.toThrow();
  expect(await auth.credentials.read('openai')).toBeUndefined();

  const own = new AbortController();
  const pending = interaction.prompt({ type: 'secret', message: 'x', signal: own.signal });
  await expect(pending).rejects.toThrow('Login cancelled');
});
