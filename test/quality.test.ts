import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { expect, test } from 'vite-plus/test';
import ts from 'typescript';

const runCheck = async (path: string, option: string) => {
  try {
    const result = await promisify(execFile)('pnpm', ['exec', 'vp', 'check', option, path], {
      timeout: 20000,
    });
    return { failed: false, output: result.stdout + result.stderr };
  } catch (error) {
    if (!(error instanceof Error) || !('stdout' in error) || !('stderr' in error)) {
      throw error;
    }
    return { failed: true, output: String(error.stdout) + String(error.stderr) };
  }
};

test('the actual lint pipeline rejects forbidden syntax, excessive parameters, unsafe types and floating promises', async () => {
  const root = resolve('.artifacts/workspace-loop/quality');
  await mkdir(root, { recursive: true });
  const directory = await mkdtemp(join(root, 'rules-'));
  const bad = join(directory, 'negative.ts');
  await writeFile(
    bad,
    `
    export function bad(a: any, b: number, c: number, d: number, e: number) { return a.x + b + c + d + e; }
    export const legacy = function (): number { return 1; };
    export let mutable = (): number => 1; mutable = (): number => 2;
    export const floating = (): void => { Promise.resolve(1); };
    export const assertion = (value: { x: number } | undefined): number => value!.x;
  `,
  );
  const result = await runCheck(bad, '--no-fmt');
  expect(result.failed).toBe(true);
  for (const rule of [
    'func-style',
    'max-params',
    'explicit-function-return-type',
    'no-explicit-any',
    'no-unsafe-member-access',
    'arrow-functions',
    'no-floating-promises',
    'no-non-null-assertion',
  ]) {
    expect(result.output).toContain(rule);
  }
  const good = join(directory, 'positive.ts');
  await writeFile(good, 'export const add = (a: number, b: number): number => a + b;\n');
  expect((await runCheck(good, '--no-fmt')).failed).toBe(false);
});

const numbered = (count: number, line: (index: number) => string) =>
  Array.from({ length: count }, (_, index) => line(index)).join('\n');

const customRuleCases = [
  { rule: 'index-exports-only', file: 'index.ts', source: 'export const value = 1;\n' },
  {
    rule: 'max-functions-per-file',
    file: 'functions.ts',
    source: numbered(9, (index) => `export const f${index} = (): number => ${index};`),
  },
  {
    rule: 'max-class-members',
    file: 'members.ts',
    source: `export class Wide {\n${numbered(11, (index) => `  public m${index}(): number { return ${index}; }`)}\n}\n`,
  },
  {
    rule: 'max-variables-per-scope',
    file: 'variables.ts',
    source: `export const many = (): number => {
      const [a, b, c] = [1, 2, 3];
      const { d, e } = { d: 4, e: 5 };
      const f = 6, g = 7, h = 8, i = 9;
      return a + b + c + d + e + f + g + h + i;
    };\n`,
  },
  {
    rule: 'no-rest-tuple-params',
    file: 'rest.ts',
    source: 'export const join = (...[a, b]: [string, string]): string => a + b;\n',
  },
  {
    rule: 'max-machine-config-function-lines',
    file: 'machines/machine.ts',
    source: `import { createMachine } from 'xstate';
    export default createMachine({
      entry: () => {
        const total = 1 + 2;
        console.log(total);
      },
    });\n`,
  },
  {
    rule: 'max-test-function-lines',
    file: 'test/long.test.ts',
    source: `import { expect, test } from 'vite-plus/test';
    test('long', () => {\n${numbered(51, (index) => `  expect(${index}).toBe(${index});`)}\n});\n`,
  },
];

test.each(customRuleCases)(
  'the actual lint pipeline rejects a violation of the custom rule zf/$rule',
  async ({ rule, file, source }) => {
    const root = resolve('.artifacts/workspace-loop/quality');
    await mkdir(root, { recursive: true });
    const path = join(await mkdtemp(join(root, 'custom-')), file);
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, source);
    const result = await runCheck(path, '--no-fmt');
    expect(result.failed).toBe(true);
    expect(result.output).toContain(`zf(${rule})`);
  },
);

test('the actual formatter rejects code that violates the committed formatting convention', async () => {
  const root = resolve('.artifacts/workspace-loop/quality');
  await mkdir(root, { recursive: true });
  const directory = await mkdtemp(join(root, 'format-'));
  const bad = join(directory, 'negative.ts');
  await writeFile(bad, 'export const add=(a:number,b:number):number=>{return a+b}');
  const result = await runCheck(bad, '--no-lint');
  expect(result.failed).toBe(true);
  expect(result.output.toLowerCase()).toContain('formatting issues');
});

test('the inspector is development-only and package dependencies preserve the architecture direction', async () => {
  const root: unknown = JSON.parse(await readFile('package.json', 'utf8'));
  expect(root).toHaveProperty('dependencies', { '@zf/cli': 'workspace:*' });
  expect(root).toMatchObject({
    packageManager: 'pnpm@12.18.0',
    devDependencies: {
      '@statelyai/sdk': '0.48.0',
      'vite-plus': '1.6.0',
      '@zf/capabilities': 'workspace:*',
      '@zf/core': 'workspace:*',
      '@zf/machines': 'workspace:*',
      '@zf/run': 'workspace:*',
      '@zf/storage': 'workspace:*',
      '@zf/xstate': 'workspace:*',
    },
  });
  const expected = {
    core: ['@earendil-works/pi-ai'],
    run: ['@zf/core'],
    storage: ['@zf/core', '@earendil-works/pi-ai', 'proper-lockfile'],
    capabilities: [
      '@zf/core',
      '@zf/config',
      'jiti',
      'yaml',
      '@earendil-works/pi-ai',
      '@earendil-works/pi-schema',
    ],
    xstate: ['@zf/core', 'xstate'],
    machines: ['@zf/core', '@earendil-works/pi-ai', 'xstate'],
    tui: ['@zf/core', '@earendil-works/pi-ai', '@earendil-works/pi-tui'],
  };
  for (const [name, dependencies] of Object.entries(expected)) {
    const parsed: unknown = JSON.parse(
      await readFile(join('packages', name, 'package.json'), 'utf8'),
    );
    if (
      !parsed ||
      typeof parsed !== 'object' ||
      !('dependencies' in parsed) ||
      !parsed.dependencies ||
      typeof parsed.dependencies !== 'object'
    ) {
      throw new Error('Invalid package manifest');
    }
    expect(Object.keys(parsed.dependencies).sort()).toEqual([...dependencies].sort());
    expect(JSON.stringify(parsed)).not.toContain('@statelyai/sdk');
  }
});

test('class methods are allowed while empty abstractions and implicit visibility are rejected', async () => {
  const root = resolve('.artifacts/oop-refactor');
  await mkdir(root, { recursive: true });
  const directory = await mkdtemp(join(root, 'oop-rules-'));
  const good = join(directory, 'good.ts');
  await writeFile(
    good,
    `export class Counter {
    #count = 0;
    public increment(): number { this.#count += 1; return this.#count; }
  }`,
  );
  expect((await runCheck(good, '--no-fmt')).failed).toBe(false);
  const bad = join(directory, 'bad.ts');
  await writeFile(
    bad,
    `export interface Empty {}
    export class EmptyClass {}
    export class Counter { count = 0; increment(): number { return ++this.count; } }`,
  );
  const result = await runCheck(bad, '--no-fmt');
  expect(result.failed).toBe(true);
  for (const rule of [
    'no-empty-object-type',
    'no-extraneous-class',
    'explicit-member-accessibility',
  ]) {
    expect(result.output).toContain(rule);
  }
});

test('the committed compiler options enforce initialization, override, readonly, private fields and implements', async () => {
  const root = resolve('.artifacts/oop-refactor');
  await mkdir(root, { recursive: true });
  const directory = await mkdtemp(join(root, 'oop-types-'));
  const file = join(directory, 'invalid.ts');
  await writeFile(
    file,
    `class Base { public run(): void {} }
    export class Derived extends Base { public run(): void {} }
    export class Broken {
      public value: number;
      readonly #secret = 1;
      public read(): number { return this.#secret; }
    }
    export const leaked = new Broken().#secret;
    export interface Data { readonly value: number }
    export const mutate = (data: Data): void => { data.value = 2; };
    export interface Readable { readonly read: () => number }
    export class Wrong implements Readable { public readonly value = 1; }
  `,
  );
  const config = ts.readConfigFile('tsconfig.json', (path) => ts.sys.readFile(path));
  const parsed = ts.parseJsonConfigFileContent(config.config, ts.sys, process.cwd());
  const program = ts.createProgram([file], { ...parsed.options, types: [] });
  const codes = ts.getPreEmitDiagnostics(program).map((diagnostic) => diagnostic.code);
  for (const code of [4114, 2564, 18013, 2540, 2420]) {
    expect(codes).toContain(code);
  }
});

const tupleAliasCases = [
  { name: 'named tuple', source: 'type Args = [string, string];' },
  { name: 'alias chain', source: 'type Pair = [string, string]; type Args = Pair;' },
  { name: 'readonly tuple alias', source: 'type Args = readonly [string, string];' },
  { name: 'generic tuple alias', source: 'type Pair<T> = [T, T]; type Args = Pair<string>;' },
  {
    name: 'imported alias chain',
    source: "import type { Pair } from './types.ts'; type Args = Pair<string>;",
  },
  {
    name: 'namespace imported alias',
    source: "import type * as Types from './types.ts'; type Args = Types.Pair<string>;",
  },
  { name: 'tuple union', source: 'type Args = [string] | [string, string];' },
  { name: 'array alias', source: 'type Args = string[];', legal: true },
  {
    name: 'array alias chain',
    source: 'type List<T> = T[]; type Args = List<string>;',
    legal: true,
  },
  {
    name: 'imported array alias',
    source: "import type { List } from './types.ts'; type Args = List<string>;",
    legal: true,
  },
  { name: 'readonly array alias', source: 'type Args = readonly string[];', legal: true },
];

test.each(tupleAliasCases)(
  'the actual lint pipeline resolves rest parameter types: $name',
  async ({ source, legal }) => {
    const root = resolve('.artifacts/workspace-loop/quality');
    await mkdir(root, { recursive: true });
    const directory = await mkdtemp(join(root, 'tuple-alias-'));
    await writeFile(
      join(directory, 'types.ts'),
      'export type Pair<T> = readonly [T, T]; export type List<T> = T[];',
    );
    const path = join(directory, 'rest.ts');
    await writeFile(
      path,
      `${source}\nexport const collect = (...values: Args): string => values.join('');\n`,
    );
    const result = await runCheck(path, '--no-fmt');
    expect(result.failed).toBe(!legal);
    expect(result.output.includes('zf(no-rest-tuple-params)')).toBe(!legal);
  },
);

const tupleBindingCases = [
  {
    name: 'alias declared below the rest',
    source:
      "export const collect = (...values: Args): string => values.join(''); type Args = [string];",
  },
  {
    name: 'direct nondestructured tuple',
    source: "export const collect = (...values: [string, string]): string => values.join('');",
  },
  {
    name: 'generic tuple constraint',
    source:
      "export const collect = <T extends [string, string]>(...values: T): string => values.join('');",
  },
  {
    name: 'inner tuple shadows outer array',
    source:
      "export type Args = string[]; export namespace Local { type Args = [string, string]; export const collect = (...values: Args): string => values.join(''); }",
  },
  {
    name: 'inner array shadows imported tuple',
    source:
      "import type { Pair as Args } from './types.ts'; export type Saved = Args<string>; export namespace Local { type Args = string[]; export const collect = (...values: Args): string => values.join(''); }",
    legal: true,
  },
  {
    name: 'generic array parameter shadows tuple alias',
    source:
      "export type Args = [string, string]; export const collect = <Args extends string[]>(...values: Args): string => values.join('');",
    legal: true,
  },
  {
    name: 'tuple alias declarations are legal without rest parameters',
    source: 'export type Pair<T> = readonly [T, T]; export type Args = Pair<string>;',
    legal: true,
  },
];

test.each(tupleBindingCases)(
  'the actual lint pipeline uses lexical rest type identity: $name',
  async ({ source, legal }) => {
    const root = resolve('.artifacts/workspace-loop/quality');
    await mkdir(root, { recursive: true });
    const directory = await mkdtemp(join(root, 'tuple-binding-'));
    await writeFile(join(directory, 'types.ts'), 'export type Pair<T> = [T, T];');
    const path = join(directory, 'rest.ts');
    await writeFile(path, source);
    const result = await runCheck(path, '--no-fmt');
    expect(result.failed).toBe(!legal);
    expect(result.output.includes('zf(no-rest-tuple-params)')).toBe(!legal);
  },
);

const contextualRestCases = [
  { name: 'fixed', signature: '(n: number, s: string) => string' },
  { name: 'optional', signature: '(n: number, s?: string) => string' },
  { name: 'mixed variadic', signature: '(n: number, ...s: string[]) => string' },
  { name: 'empty', signature: '() => string' },
  { name: 'array', signature: '(...s: string[]) => string', legal: true },
];

test.each(contextualRestCases)(
  'the actual lint pipeline checks contextual rest types in assignments, callbacks, methods and initializers: $name',
  async ({ signature, legal }) => {
    const root = resolve('.artifacts/workspace-loop/quality');
    await mkdir(root, { recursive: true });
    const path = join(await mkdtemp(join(root, 'contextual-rest-')), 'rest.ts');
    await writeFile(
      path,
      `
      type Collect = ${signature};
      export const fixed: ${signature} = (...args): string => args.join();
      export let assigned: Collect = fixed;
      assigned = (...args): string => args.join();
      const callback = (collect: Collect): string => String(collect);
      export const called = callback((...args): string => args.join());
      export const initializer: { run: Collect } = { run: (...args): string => args.join() };
      export class Collector { public readonly run: Collect = (...args): string => args.join(); }
      export const bindings = (values: string[], item: { value: string; extra: string }): string => {
        const [first, ...rest] = values;
        const { value, ...others } = item;
        return [first, ...rest, value, others.extra].join();
      };
    `,
    );
    const result = await runCheck(path, '--no-fmt');
    expect(result.failed).toBe(!legal);
    expect(result.output.match(/zf\(no-rest-tuple-params\)/g) ?? []).toHaveLength(legal ? 0 : 5);
    const method = join(dirname(path), 'method.ts');
    await writeFile(
      method,
      `
      export const method: { run: ${signature} } = { run(...args): string { return args.join(); } };
    `,
    );
    const methodResult = await runCheck(method, '--no-fmt');
    expect(methodResult.output.match(/zf\(no-rest-tuple-params\)/g) ?? []).toHaveLength(
      legal ? 0 : 1,
    );
    expect(methodResult.output.match(/zf\(arrow-functions\)/g) ?? []).toHaveLength(1);
  },
);
