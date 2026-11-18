import { execFile } from 'node:child_process';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';
import { expect, test } from 'vite-plus/test';

interface LintDiagnostic {
  readonly code: string;
  readonly message: string;
  readonly filename: string;
  readonly labels: readonly { readonly span: { readonly line: number; readonly column: number } }[];
}

const lintModule = async (source: string) => {
  const root = resolve('.artifacts/strict-code-limits/p8-t1');
  await mkdir(root, { recursive: true });
  const path = join(await mkdtemp(join(root, 'module-')), 'fixture.ts');
  await writeFile(path, source);
  let stdout: string;
  try {
    ({ stdout } = await promisify(execFile)(
      'pnpm',
      ['exec', 'vp', 'lint', '--format', 'json', path],
      {
        timeout: 20000,
      },
    ));
  } catch (error) {
    if (!(error instanceof Error) || !('stdout' in error) || typeof error.stdout !== 'string') {
      throw error;
    }
    stdout = error.stdout;
  }
  const output = JSON.parse(stdout) as { readonly diagnostics: readonly LintDiagnostic[] };
  return output.diagnostics;
};

const bareFunctions = (count: number) =>
  Array.from(
    { length: count },
    (_, index) => `export const f${index} = (): number => ${index};`,
  ).join('\n');

const wrapperCases = [
  { name: 'bare arrow', value: '(): number => 1' },
  { name: 'parenthesized arrow', value: '(((): number => 1))' },
  { name: 'satisfies arrow', value: '((): number => 1) satisfies (() => number)' },
  { name: 'as assertion', value: '((): number => 1) as (() => number)' },
  { name: 'angle-bracket assertion', value: '<() => number>(() => 1)' },
  { name: 'non-null assertion', value: '((): number => 1)!' },
  { name: 'instantiation expression', value: '(<T>(value: T): T => value)<number>' },
  {
    name: 'nested transparent wrappers',
    value: '(((<() => number>(() => 1))! satisfies (() => number)) as (() => number))!',
  },
  {
    name: 'wrapped function expression',
    value: '(function (): number { return 1; }) satisfies (() => number)',
  },
];

const functionRule = 'zf(max-functions-per-file)';
const excessMessage = 'This file declares 9 module-level functions. Maximum allowed is 8.';

test.each(wrapperCases)(
  'max-functions-per-file counts eight versus nine module functions: $name',
  async ({ value }) => {
    const declaration = (index: number) => `export const f${index} = ${value};`;
    const eight = Array.from({ length: 8 }, (_, index) => declaration(index)).join('\n');
    expect((await lintModule(eight)).filter(({ code }) => code === functionRule)).toEqual([]);
    const diagnostics = await lintModule(`${eight}\n${declaration(8)}`);
    expect(diagnostics.filter(({ code }) => code === functionRule)).toMatchObject([
      { message: excessMessage, labels: [{ span: { line: 9, column: 14 } }] },
    ]);
    expect(diagnostics.filter(({ code }) => code === functionRule)).toHaveLength(1);
  },
);

const declarationCases = [
  {
    name: 'mixed bare and wrapped declarations',
    source: `${bareFunctions(8)}\nexport const ninth = (() => 1) satisfies (() => number);`,
    line: 9,
    column: 14,
  },
  {
    name: 'multiple wrapped declarators',
    source: `${bareFunctions(7)}\nexport const eighth = (() => 1) as (() => number), ninth = (() => 2) satisfies (() => number);`,
    line: 8,
    column: 52,
  },
  {
    name: 'default exported wrapped arrow',
    source: `${bareFunctions(8)}\nexport default (((): number => 1) satisfies (() => number))!;`,
    line: 9,
    column: 16,
  },
  {
    name: 'default exported wrapped function expression',
    source: `${bareFunctions(8)}\nexport default (function (): number { return 1; }) as (() => number);`,
    line: 9,
    column: 16,
  },
  {
    name: 'named function declaration',
    source: `${bareFunctions(8)}\nexport function ninth(): number { return 1; }`,
    line: 9,
    column: 8,
  },
  {
    name: 'default function declaration',
    source: `${bareFunctions(8)}\nexport default function ninth(): number { return 1; }`,
    line: 9,
    column: 16,
  },
];

test.each(declarationCases)(
  'max-functions-per-file reports the excess declaration: $name',
  async ({ source, line, column }) => {
    const diagnostics = (await lintModule(source)).filter(({ code }) => code === functionRule);
    expect(diagnostics).toHaveLength(1);
    expect(diagnostics).toMatchObject([
      { message: excessMessage, labels: [{ span: { line, column } }] },
    ]);
  },
);

const nonFunctionCases = [
  { name: 'alias-only addition', extra: 'export const alias = f0;' },
  { name: 'wrapped alias', extra: 'export const alias = f0 satisfies (() => number);' },
  { name: 'wrapped call', extra: 'export const value = (f0() as number)!;' },
  {
    name: 'wrapped object',
    extra: 'export const value = ({ run: (): number => 1 }) satisfies { run: () => number };',
  },
  {
    name: 'assignment expression',
    extra: 'let assigned = f0; export const value = (assigned = (): number => 1);',
  },
  { name: 'call containing callback', extra: 'export const value = [1].map((n): number => n);' },
  {
    name: 'invoked arrow',
    extra: 'export const value = (((): number => 1) satisfies (() => number))();',
  },
  { name: 'object containing callback', extra: 'export const value = { run: (): number => 1 };' },
  { name: 'array containing callback', extra: 'export const value = [(): number => 1];' },
  {
    name: 'type-only signatures',
    extra: 'export type Fn = () => number; export interface API { readonly run: Fn }',
  },
  {
    name: 'conditional expression',
    extra: 'export const value = Math.random() ? ((): number => 1) : f0;',
  },
  { name: 'logical expression', extra: 'export const value = Math.random() && ((): number => 1);' },
  { name: 'sequence expression', extra: 'export const value = (f0(), (): number => 1);' },
  { name: 'member expression', extra: 'export const value = { run: (): number => 1 }.run;' },
  {
    name: 'nested namespace declaration',
    extra: 'export namespace Local { export const nested = (() => 1) satisfies (() => number); }',
  },
  { name: 'nested block declaration', extra: '{ const nested = (): number => 1; nested(); }' },
  {
    name: 'mutable wrapped declaration',
    extra: 'export let mutable = (() => 1) satisfies (() => number); mutable = () => 2;',
  },
];

test.each(nonFunctionCases)(
  'max-functions-per-file does not descend into non-function values: $name',
  async ({ extra }) => {
    const diagnostics = await lintModule(`${bareFunctions(8)}\n${extra}`);
    expect(diagnostics.filter(({ code }) => code === functionRule)).toEqual([]);
  },
);

test('max-functions-per-file allows a default wrapped arrow as the eighth function', async () => {
  const diagnostics = await lintModule(
    `${bareFunctions(7)}\nexport default (((): number => 1) satisfies (() => number))!;`,
  );
  expect(diagnostics.filter(({ code }) => code === functionRule)).toEqual([]);
});

test('max-functions-per-file excludes nested wrapped functions and callbacks', async () => {
  const source = `${bareFunctions(7)}
export const outer = (): number => {
  const nested = (() => 1) satisfies (() => number);
  return [1].map((n): number => nested() + n)[0] ?? 0;
};`;
  expect((await lintModule(source)).filter(({ code }) => code === functionRule)).toEqual([]);
});

test('max-functions-per-file reports the total count for every excess declaration', async () => {
  const source = `${bareFunctions(8)}
export const ninth = (() => 1) satisfies (() => number);
export const tenth = (() => 2) as (() => number);`;
  const diagnostics = (await lintModule(source)).filter(({ code }) => code === functionRule);
  expect(diagnostics).toHaveLength(2);
  expect(diagnostics).toMatchObject(
    [9, 10].map((line) => ({
      message: 'This file declares 10 module-level functions. Maximum allowed is 8.',
      labels: [{ span: { line, column: 14 } }],
    })),
  );
});

test('max-functions-per-file controls distinguish unrelated non-null assertion diagnostics', async () => {
  const diagnostics = await lintModule(`${bareFunctions(8)}\nexport const alias = f0!;`);
  expect(diagnostics.filter(({ code }) => code === functionRule)).toEqual([]);
  expect(diagnostics.map(({ code }) => code)).toContain('typescript(no-non-null-assertion)');
});
