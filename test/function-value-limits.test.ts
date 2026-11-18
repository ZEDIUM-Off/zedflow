import { execFile } from 'node:child_process';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';
import { expect, test } from 'vite-plus/test';

interface LintDiagnostic {
  readonly code: string;
  readonly message: string;
  readonly labels: readonly { readonly span: { readonly line: number; readonly column: number } }[];
}

const lintFixture = async (source: string) => {
  const root = resolve('.artifacts/strict-code-limits/p9-t1');
  await mkdir(root, { recursive: true });
  const path = join(await mkdtemp(join(root, 'counter-')), 'fixture.ts');
  await writeFile(path, source);
  let stdout: string;
  try {
    ({ stdout } = await promisify(execFile)(
      'pnpm',
      ['exec', 'vp', 'lint', '--format', 'json', path],
      { timeout: 20000 },
    ));
  } catch (error) {
    if (!(error instanceof Error) || !('stdout' in error) || typeof error.stdout !== 'string') {
      throw error;
    }
    stdout = error.stdout;
  }
  const output = JSON.parse(stdout) as { readonly diagnostics: readonly LintDiagnostic[] };
  expect(output.diagnostics.some(({ code }) => code.includes('parser'))).toBe(false);
  return output.diagnostics;
};

const numbered = (count: number, line: (index: number) => string) =>
  Array.from({ length: count }, (_, index) => line(index)).join('\n');

const classSource = (count: number, value: string) =>
  `export class C {\n${numbered(count, (index) => `  f${index} = ${value};`)}\n}`;

const localSource = (count: number, value: string) =>
  `export const outer = () => {\n${numbered(count, (index) => `  const f${index} = ${value};`)}\n};`;

const moduleSource = (count: number, value: string) =>
  numbered(count, (index) => `export const f${index} = ${value};`);

const classRule = 'zf(max-class-members)';
const localRule = 'zf(max-variables-per-scope)';
const moduleRule = 'zf(max-functions-per-file)';
const counterDiagnostics = (diagnostics: readonly LintDiagnostic[]) =>
  diagnostics.filter(({ code }) => [classRule, localRule, moduleRule].includes(code));
const classMessage = 'This class has 11 methods. Maximum allowed is 10.';
const localMessage = 'This function declares 9 local variables. Maximum allowed is 8.';
const moduleMessage = 'This file declares 9 module-level functions. Maximum allowed is 8.';

const functionValues = [
  { name: 'bare arrow', value: '(): number => 1' },
  { name: 'parenthesized arrow', value: '(((): number => 1))' },
  { name: 'satisfies arrow', value: '((): number => 1) satisfies (() => number)' },
  { name: 'as assertion', value: '((): number => 1) as (() => number)' },
  { name: 'angle-bracket assertion', value: '<() => number>(() => 1)' },
  { name: 'non-null assertion', value: '((): number => 1)!' },
  { name: 'instantiation expression', value: '(<T>(value: T): T => value)<number>' },
  { name: 'bare function expression', value: 'function (): number { return 1; }' },
  {
    name: 'wrapped function expression',
    value: '(function (): number { return 1; }) satisfies (() => number)',
  },
  {
    name: 'nested transparent wrappers',
    value: '(((<() => number>(() => 1))! satisfies (() => number)) as (() => number))!',
  },
];

test.each(functionValues)(
  'max-class-members counts ten versus eleven function-valued properties: $name',
  async ({ value }) => {
    expect(counterDiagnostics(await lintFixture(classSource(10, value)))).toEqual([]);
    const diagnostics = counterDiagnostics(await lintFixture(classSource(11, value)));
    expect(diagnostics).toHaveLength(1);
    expect(diagnostics).toMatchObject([
      { code: classRule, message: classMessage, labels: [{ span: { line: 1, column: 8 } }] },
    ]);
  },
);

test.each(functionValues)(
  'max-variables-per-scope excludes nine function bindings among ordinary bindings: $name',
  async ({ value }) => {
    const functions = numbered(9, (index) => `  const fn${index} = ${value};`);
    const source = (count: number) =>
      `export const outer = () => {\n${functions}\n${numbered(count, (index) => `  const v${index} = 1;`)}\n};`;
    expect(counterDiagnostics(await lintFixture(localSource(9, value)))).toEqual([]);
    expect(counterDiagnostics(await lintFixture(source(8)))).toEqual([]);
    const diagnostics = counterDiagnostics(await lintFixture(source(9)));
    expect(diagnostics).toHaveLength(1);
    expect(diagnostics).toMatchObject([
      { code: localRule, message: localMessage, labels: [{ span: { line: 1, column: 22 } }] },
    ]);
  },
);

test.each(functionValues)(
  'max-functions-per-file retains eight versus nine function values with shared classification: $name',
  async ({ value }) => {
    expect(counterDiagnostics(await lintFixture(moduleSource(8, value)))).toEqual([]);
    const diagnostics = counterDiagnostics(await lintFixture(moduleSource(9, value)));
    expect(diagnostics).toHaveLength(1);
    expect(diagnostics).toMatchObject([
      { code: moduleRule, message: moduleMessage, labels: [{ span: { line: 9, column: 14 } }] },
    ]);
  },
);

const nonFunctionValues = [
  { name: 'wrapped number', value: '(1 as number)!' },
  { name: 'wrapped identifier', value: 'Math.random satisfies (() => number)' },
  { name: 'wrapped call', value: 'Math.random() as number' },
  { name: 'wrapped object with callback', value: '({ run: () => 1 }) satisfies object' },
  { name: 'array with callback', value: '[() => 1]' },
  { name: 'call with callback', value: '[1].map((n) => n)' },
  { name: 'invoked wrapped arrow', value: '((() => 1) satisfies (() => number))()' },
  { name: 'member expression', value: '({ run: () => 1 }).run' },
  { name: 'conditional expression', value: 'Math.random() ? (() => 1) : (() => 2)' },
  { name: 'logical expression', value: 'Math.random() && (() => 1)' },
  { name: 'sequence expression', value: '(Math.random(), () => 1)' },
];

test.each(nonFunctionValues)(
  'function-value counters do not descend into non-function expressions: $name',
  async ({ value }) => {
    expect(counterDiagnostics(await lintFixture(classSource(11, value)))).toEqual([]);
    const locals = counterDiagnostics(await lintFixture(localSource(9, value)));
    expect(locals).toHaveLength(1);
    expect(locals).toMatchObject([
      { code: localRule, message: localMessage, labels: [{ span: { line: 1, column: 22 } }] },
    ]);
    const module = `${moduleSource(8, '() => 1')}\nexport const value = ${value};`;
    expect(counterDiagnostics(await lintFixture(module))).toEqual([]);
  },
);

test('max-class-members preserves mixed methods, accessors, abstract methods and constructor exclusion', async () => {
  const source = `export abstract class C {
  public constructor() {}
  public get ready(): boolean { return true; }
  public set ready(value: boolean) { console.log(value); }
  public abstract run(): void;
${numbered(5, (index) => `  public m${index}(): void {}`)}
  public callback = () => 1;
  public accessor wrapped = (() => 1) satisfies (() => number);
`;
  expect(counterDiagnostics(await lintFixture(`${source}}`))).toEqual([]);
  const diagnostics = counterDiagnostics(
    await lintFixture(`${source}  public last = (function () { return 1; }) as (() => number);\n}`),
  );
  expect(diagnostics).toHaveLength(1);
  expect(diagnostics).toMatchObject([
    { code: classRule, message: classMessage, labels: [{ span: { line: 1, column: 8 } }] },
  ]);
});

test('max-class-members excludes uninitialized, abstract and type-only function fields', async () => {
  const source = `export abstract class C {
${numbered(11, (index) => `  public f${index}: () => number;`)}
${numbered(11, (index) => `  public abstract a${index}: () => number;`)}
  public accessor unset: () => number;
}
export interface API { run(): void; }
export type Fn = () => number;`;
  expect(counterDiagnostics(await lintFixture(source))).toEqual([]);
});

test('max-variables-per-scope preserves binding patterns, multiple declarators and block ownership', async () => {
  const source = `export const outer = () => {
  const { a, b: renamed, c = 1, ...rest } = { a: 1, b: 2, c: 3, d: 4 };
  const [first, , second = 1, ...tail] = [1, 2, 3];
  { const eighth = 1; }
  const fn = (() => 1) satisfies (() => number), other = (function () {}) as (() => void);
`;
  expect(counterDiagnostics(await lintFixture(`${source}};`))).toEqual([]);
  const diagnostics = counterDiagnostics(await lintFixture(`${source}  const ninth = 1;\n};`));
  expect(diagnostics).toHaveLength(1);
  expect(diagnostics).toMatchObject([
    { code: localRule, message: localMessage, labels: [{ span: { line: 1, column: 22 } }] },
  ]);
});

test('max-variables-per-scope assigns nested wrapped function variables only to their own function', async () => {
  const source = `export const outer = () => {
  const inner = (() => {
${numbered(9, (index) => `    const v${index} = 1;`)}
  }) satisfies (() => void);
${numbered(8, (index) => `  const v${index} = 1;`)}
};`;
  const diagnostics = counterDiagnostics(await lintFixture(source));
  expect(diagnostics).toHaveLength(1);
  expect(diagnostics).toMatchObject([
    { code: localRule, message: localMessage, labels: [{ span: { line: 2, column: 18 } }] },
  ]);
});

test('function-value counters report independent owners and full counts without absorbing nested callbacks', async () => {
  const source = `${moduleSource(8, '() => 1')}
export const outer = (() => {
${numbered(9, (index) => `  const v${index} = 1;`)}
  return { run: (() => 1) satisfies (() => number) };
}) satisfies (() => object);
${classSource(12, '(() => 1) satisfies (() => number)')}`;
  const diagnostics = counterDiagnostics(await lintFixture(source));
  expect(diagnostics).toHaveLength(3);
  expect(diagnostics.filter(({ code }) => code === moduleRule)).toMatchObject([
    { message: moduleMessage, labels: [{ span: { line: 9, column: 14 } }] },
  ]);
  expect(diagnostics.filter(({ code }) => code === localRule)).toMatchObject([
    { message: localMessage, labels: [{ span: { line: 9, column: 23 } }] },
  ]);
  expect(diagnostics.filter(({ code }) => code === classRule)).toMatchObject([
    {
      message: 'This class has 12 methods. Maximum allowed is 10.',
      labels: [{ span: { line: 21, column: 8 } }],
    },
  ]);
});

test('function-value counters distinguish unrelated function-expression and non-null diagnostics', async () => {
  const source = classSource(11, '(function () { return 1; })!');
  const diagnostics = await lintFixture(source);
  expect(counterDiagnostics(diagnostics)).toHaveLength(1);
  expect(counterDiagnostics(diagnostics)[0]).toMatchObject({
    code: classRule,
    message: classMessage,
  });
  expect(diagnostics.map(({ code }) => code)).toContain('typescript(no-non-null-assertion)');
  expect(diagnostics.map(({ code }) => code)).toContain('zf(arrow-functions)');
});
