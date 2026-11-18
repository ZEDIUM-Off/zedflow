import { expect, test } from 'vite-plus/test';
import { SecretInput } from '../../packages/tui/src/index.ts';
import { stripTerminalSequences } from '../../packages/tui/test/fixtures.ts';
import { CTRL_C, ENTER, ESCAPE, PASTE_END, PASTE_START } from '../support/tui/keys.ts';

test('SecretInput masks, edits by grapheme, accepts bracketed paste and clears on cancel', () => {
  const input = new SecretInput();
  input.focused = true;
  const submitted: string[] = [];
  let cancelled = 0;
  input.onSubmit = (value) => submitted.push(value);
  input.onCancel = () => {
    cancelled += 1;
  };
  for (const character of ['a', 'b', '👩‍👩‍👧', 'c']) {
    input.handleInput(character);
  }
  input.handleInput('\u001b[A');
  input.handleInput('\u0001');
  input.handleInput('\u001b[97u');
  input.handleInput('\u007f');
  input.handleInput('\u007f');
  input.handleInput(PASTE_START + 'pas');
  input.handleInput(`te-d\n${PASTE_END}`);
  input.focused = false;
  const rendered = stripTerminalSequences(input.render(40).join(''));
  expect(rendered).toContain('*');
  expect(rendered).not.toMatch(/[abcpaste]/u);
  input.handleInput(ENTER);
  expect(submitted).toEqual(['ab👩‍👩‍👧paste-d']);
  expect(input.render(40).join('')).not.toContain('*');
  input.handleInput('zzz');
  input.handleInput(ESCAPE);
  input.handleInput(ENTER);
  expect(cancelled).toBe(1);
  expect(submitted).toHaveLength(2);
  expect(submitted[1]).toBe('');
});

test('SecretInput bounds a bracketed paste, finds an end marker split across chunks and cancels mid-paste', () => {
  const input = new SecretInput();
  const submitted: string[] = [];
  let cancelled = 0;
  input.onSubmit = (value) => submitted.push(value);
  input.onCancel = () => {
    cancelled += 1;
  };
  input.handleInput(`${PASTE_START}abc\u001b[20`);
  input.handleInput('1~');
  input.handleInput(ENTER);
  expect(submitted).toEqual(['abc']);

  input.handleInput(PASTE_START);
  for (let chunk = 0; chunk < 400; chunk += 1) {
    input.handleInput('x'.repeat(10_000));
  }
  input.handleInput(`yy${PASTE_END}`);
  input.handleInput(ENTER);
  expect(submitted).toHaveLength(2);
  expect(submitted[1]?.length).toBe(8192);
  expect(submitted[1]).toMatch(/^x+$/u);

  // An unterminated paste does not trap the user.
  input.handleInput(`${PASTE_START}secret-fragment`);
  input.handleInput(ESCAPE);
  expect(cancelled).toBe(1);
  input.handleInput(ENTER);
  expect(submitted).toEqual(['abc', submitted[1], '']);
  input.handleInput(`${PASTE_START}more`);
  input.handleInput(CTRL_C);
  expect(cancelled).toBe(2);
});
