import { execFile } from 'node:child_process';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';
import { RuleTester } from 'vite-plus/lint/plugins-dev';
import { expect, test } from 'vite-plus/test';
import { maxMachineConfigFunctionLines } from '../scripts/lint/rules/max-machine-config-function-lines.ts';

// Execute RuleTester's assertions within each named Vitest regression.
RuleTester.describe = (_name, run) => run();
RuleTester.it = (_name, run) => run();
const tester = new RuleTester();
const named = "import { createMachine as machine } from 'xstate';";
const namespace = "import * as xs from 'xstate';";
const setup = "import { setup as build } from 'xstate';";
const configured = `${setup} const configured = build({});`;
const callback = '() => {\nconst value = 1;\nreturn value;\n}';
const shortCallback = '() => {\nreturn 1;\n}';
const ruleName = 'max-machine-config-function-lines';
const message =
  'This machine config function has 4 lines. Maximum allowed is 3: name it outside the config.';

interface FactoryCase {
  readonly name: string;
  readonly head: string;
  readonly callee: string;
  readonly tail?: string;
}
const sourceFor = ({ head, callee, tail = '' }: FactoryCase, value = callback) =>
  `${head}\n${callee}({ context: ${value} });\n${tail}`;

const factoryCase = (name: string, head: string, callee: string, tail = ''): FactoryCase => ({
  name,
  head,
  callee,
  tail,
});

const factories = (
  [
    ['direct named import', "import { createMachine } from 'xstate';", 'createMachine'],
    ['aliased named import', named, 'machine'],
    ['import declared after use', '', 'machine', named],
    ['namespace dot member', namespace, 'xs.createMachine'],
    ['namespace identity', `${namespace} inspect(xs);`, 'xs.createMachine'],
    ['namespace single-quoted member', namespace, "xs['createMachine']"],
    ['namespace double-quoted member', namespace, 'xs["createMachine"]'],
    ['wrapped literal member key', namespace, "xs['createMachine' as const]"],
    ['direct setup factory', "import { setup } from 'xstate';", 'setup({}).createMachine'],
    ['aliased setup factory', setup, 'build({}).createMachine'],
    ['namespace setup factory', namespace, 'xs.setup({}).createMachine'],
    ['literal setup and machine members', namespace, 'xs["setup"]({})["createMachine"]'],
    [
      'immutable machine alias chain',
      `${named} const first = machine; const second = first;`,
      'second',
    ],
    ['immutable alias declared after use', named, 'alias', 'const alias = machine;'],
    ['immutable namespace alias', `${namespace} const local = xs;`, 'local.createMachine'],
    [
      'destructured namespace factory',
      `${namespace} const { createMachine: local } = xs;`,
      'local',
    ],
    [
      'literal destructured namespace factory',
      `${namespace} const { ['createMachine']: local } = xs;`,
      'local',
    ],
    [
      'destructured setup factory',
      `${namespace} const { setup: local } = xs;`,
      'local({}).createMachine',
    ],
    ['immutable setup alias', `${setup} const local = build;`, 'local({}).createMachine'],
    ['bound setup result', `${configured}`, 'configured.createMachine'],
    ['optional namespace member', namespace, '(xs?.createMachine)'],
    ['optional setup member', configured, '(configured?.createMachine)'],
    ['optional computed factory', configured, '(configured?.["createMachine"])'],
    ['optional setup chain', setup, '(build?.({})?.createMachine)'],
    ['optional factory TS non-null', configured, '(configured?.createMachine)!'],
    ['optional namespace alias', `${namespace} const local = xs?.createMachine;`, 'local'],
    [
      'optional setup factory alias',
      `${configured} const local = configured?.createMachine;`,
      'local',
    ],
    [
      'setup result alias chain',
      `${configured} const local = configured;`,
      'local["createMachine"]',
    ],
    ['destructured setup result', `${setup} const { createMachine: local } = build({});`, 'local'],
    ['alias of setup machine member', `${setup} const local = build({}).createMachine;`, 'local'],
    ['block-scoped imported alias', `${named} { const local = machine;`, 'local', '}'],
    ['parenthesized callee', named, '((machine))'],
    ['as-wrapped callee', named, '(machine as typeof machine)'],
    ['satisfies-wrapped callee', named, '(machine satisfies typeof machine)'],
    ['angle-asserted callee', named, '(<typeof machine>machine)'],
    ['non-null callee', named, 'machine!'],
    ['instantiated callee', named, '(machine<never>)'],
    [
      'nested transparent callee wrappers',
      named,
      '(((<typeof machine>machine)! satisfies typeof machine) as typeof machine)!',
    ],
    ['wrapped namespace and computed member', namespace, '(xs as typeof xs)["createMachine"]'],
    ['wrapped setup call and result', setup, '((build as typeof build)({})!).createMachine'],
    ['wrapped immutable alias', `${named} const local = (machine as typeof machine)!;`, 'local'],
    [
      'wrapped destructure source',
      `${namespace} const { createMachine: local } = (xs as typeof xs)!;`,
      'local',
    ],
  ] satisfies readonly Parameters<typeof factoryCase>[]
).map(([name, head, callee, tail]) => factoryCase(name, head, callee, tail));

test.each(factories)(
  'machine factory bindings enforce three versus four callback lines: $name',
  (entry) => {
    validSource(sourceFor(entry, shortCallback));
    invalidSource(sourceFor(entry), [
      {
        messageId: 'long',
        data: { count: '4', max: '3' },
        line: 2,
        column: entry.callee.length + 12,
        endLine: 5,
        endColumn: 1,
      },
    ]);
  },
);

const unrelated = (
  [
    ['unbound same-spelled identifier', '', 'createMachine'],
    ['foreign named import', "import { createMachine } from 'foreign';", 'createMachine'],
    ['foreign aliased import', "import { createMachine as machine } from 'foreign';", 'machine'],
    ['foreign namespace', "import * as xs from 'foreign';", 'xs.createMachine'],
    ['foreign setup', "import { setup } from 'foreign';", 'setup({}).createMachine'],
    [
      'unrelated member',
      'const local = { createMachine: (value) => value };',
      'local.createMachine',
    ],
    ['unrelated local function', 'const createMachine = (value) => value;', 'createMachine'],
    ['shadowing parameter', `${named} const run = (machine) => {`, 'machine', '};'],
    [
      'shadowing declaration below call',
      `${named} {`,
      'machine',
      'const machine = (value) => value; }',
    ],
    [
      'shadowing namespace parameter',
      `${namespace} const run = (xs) => {`,
      'xs["createMachine"]',
      '};',
    ],
    [
      'shadowing setup parameter',
      `${setup} const run = (build) => {`,
      'build({}).createMachine',
      '};',
    ],
    [
      'shadowing destructured parameter',
      `${named} const run = ({ machine }) => {`,
      'machine',
      '};',
    ],
    ['mutable let factory alias', `${named} let local = machine;`, 'local'],
    ['mutable var factory alias', `${named} var local = machine;`, 'local'],
    ['reassigned factory alias', `${named} const local = machine; local = other;`, 'local'],
    ['mutable destructure', `${namespace} let { createMachine: local } = xs;`, 'local'],
    ['mutable setup object', `${setup} let configured = build({});`, 'configured.createMachine'],
    [
      'overwritten setup member',
      `${configured} configured.createMachine = other;`,
      'configured.createMachine',
    ],
    [
      'setup member overwritten through alias',
      `${configured} const alias = configured; alias.createMachine = other;`,
      'configured.createMachine',
    ],
    [
      'setup object escapes to unknown mutator',
      `${configured} mutate(configured);`,
      'configured.createMachine',
    ],
    [
      'deleted setup member',
      `${configured} delete configured.createMachine;`,
      'configured.createMachine',
    ],
    ['namespace dynamic key', `${namespace} const key = 'createMachine';`, 'xs[key]'],
    ['dynamic setup key', `${namespace} const key = 'setup';`, 'xs[key]({}).createMachine'],
    ['dynamic setup result key', `${setup} const key = 'createMachine';`, 'build({})[key]'],
    ['template member key', namespace, 'xs[`createMachine`]'],
    [
      'defaulted destructure',
      `${namespace} const { createMachine: local = fallback } = xs;`,
      'local',
    ],
    [
      'dynamic destructure',
      `${namespace} const key = 'createMachine'; const { [key]: local } = xs;`,
      'local',
    ],
    [
      'ambiguous conditional factory',
      `${named} const local = condition ? machine : other;`,
      'local',
    ],
    ['unknown factory result', `${named} const local = wrap(machine);`, 'local'],
    ['cyclic immutable aliases', 'const first = second; const second = first;', 'first'],
    [
      'type-only named import',
      "import type { createMachine as machine } from 'xstate';",
      'machine',
    ],
    [
      'type-only import specifier',
      "import { type createMachine as machine } from 'xstate';",
      'machine',
    ],
    ['type-only namespace import', "import type * as xs from 'xstate';", 'xs.createMachine'],
    [
      'default XState import without proven namespace',
      "import xs from 'xstate';",
      'xs.createMachine',
    ],
    ['other XState return value', namespace, 'xs.createActor({}).createMachine'],
  ] satisfies readonly Parameters<typeof factoryCase>[]
).map(([name, head, callee, tail]) => factoryCase(name, head, callee, tail));

const validSource = (code: string, max = 3): void => {
  tester.run(ruleName, maxMachineConfigFunctionLines, {
    valid: [{ filename: 'machine.ts', code, options: [{ max }] }],
    invalid: [],
  });
};
type ExpectedErrors = RuleTester.InvalidTestCase['errors'];
const invalidSource = (code: string, errors: ExpectedErrors, max = 3): void => {
  tester.run(ruleName, maxMachineConfigFunctionLines, {
    valid: [],
    invalid: [{ filename: 'machine.ts', code, options: [{ max }], errors }],
  });
};

test.each(unrelated)('machine factory bindings reject guessed identity: $name', (entry) => {
  validSource(sourceFor(entry));
});

test('machine factory bindings preserve configurable limits and invalid option rejection', () => {
  const code = sourceFor(factoryCase('configured limit', named, 'machine'));
  validSource(code, 4);
  invalidSource(code, [{ messageId: 'long', data: { count: '4', max: '2' } }], 2);
  for (const options of [
    [{ max: -1 }],
    [{ max: 1.5 }],
    [{ max: '3' }],
    [{}],
    [{ max: 3, extra: true }],
  ]) {
    expect(() =>
      tester.run(ruleName, maxMachineConfigFunctionLines, {
        valid: [{ code, options }],
        invalid: [],
      }),
    ).toThrow();
  }
});

test('machine config lines skip blank and comment-only lines without moving the function report', () => {
  const code = `${named}\nmachine({ context: () => {\n// comment\n\nconst value = 1; /* comment */\n/* block\ncomment */\nreturn value;\n} });`;
  invalidSource(code, [{ message, line: 2, column: 19, endLine: 9, endColumn: 1 }]);
});

test('machine config traversal covers every argument, nested callbacks, methods and function expressions', () => {
  const code = `${named}\nmachine({ context: ${callback}, states: { ready: { entry: wrap(${callback}) } } }, { actions: { run: function () {\nconst value = 1;\nreturn value;\n}, method() {\nconst value = 1;\nreturn value;\n} } });`;
  invalidSource(
    code,
    [2, 5, 8, 11].map((line) => ({ message, line })),
  );
});

test('machine config traversal reports outer and descendant functions once each', () => {
  const code = `${named}\nmachine({ context: () => {\nreturn ${callback};\n} });`;
  invalidSource(code, [
    { messageId: 'long', data: { count: '6', max: '3' }, line: 2 },
    { message, line: 3 },
  ]);
});

test('machine config traversal does not scan setup options, named outside callbacks or later calls', () => {
  const code = `${setup}\nconst outside = ${callback};\nbuild({ actions: { outside: ${callback} } }).createMachine({ context: outside });\nother({ context: ${callback} });`;
  validSource(code);
});

interface LintDiagnostic {
  readonly code: string;
  readonly message: string;
  readonly labels: readonly { readonly span: { readonly line: number; readonly column: number } }[];
}
const lintFixture = async (source: string, machineProfile = true) => {
  const root = resolve('.artifacts/strict-code-limits/p10-t1');
  await mkdir(root, { recursive: true });
  const directory = await mkdtemp(join(root, 'factory-'));
  const path = join(directory, machineProfile ? 'machines/fixture.ts' : 'fixture.ts');
  await mkdir(join(directory, 'machines'));
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
  return output.diagnostics.filter(({ code }) => code === 'zf(max-machine-config-function-lines)');
};

test.each(factories.filter(({ name }) => !name.includes('wrapped') && !name.includes('callee')))(
  'machine profile pipeline enforces the callback boundary: $name',
  async (entry) => {
    expect(await lintFixture(sourceFor(entry, shortCallback))).toEqual([]);
    const diagnostics = await lintFixture(sourceFor(entry));
    expect(diagnostics).toMatchObject([
      { message, labels: [{ span: { line: 2, column: entry.callee.length + 13 } }] },
    ]);
    expect(diagnostics).toHaveLength(1);
  },
);

test.each(
  unrelated.filter(({ name }) =>
    [
      'foreign named import',
      'foreign namespace',
      'unrelated member',
      'shadowing parameter',
      'shadowing declaration below call',
      'mutable let factory alias',
      'overwritten setup member',
      'ambiguous conditional factory',
      'namespace dynamic key',
    ].includes(name),
  ),
)('machine profile pipeline excludes unrelated factories: $name', async (entry) => {
  expect(await lintFixture(sourceFor(entry))).toEqual([]);
});

test('machine factory rule remains off outside the machine profile', async () => {
  expect(await lintFixture(sourceFor(factoryCase('source', named, 'machine')), false)).toEqual([]);
});

test.each([
  'configured.valueOf()',
  'configured.unknown()',
  'configured["unknown"]()',
  'configured.unknown?.()',
  '(configured.unknown as (() => object))()',
  'configured.unknown!()',
  'configured.unknown<never>()',
  'configured.valueOf`tag`',
  '(configured.unknown as (() => object))``',
  'new configured.unknown()',
  '(configured?.valueOf)()',
  '(configured?.valueOf)`tag`',
  '(configured?.["valueOf"])()',
  '(configured?.["valueOf"])`tag`',
  '((configured?.valueOf) as (() => object))()',
  '((configured?.valueOf) as (() => object))`tag`',
  '((configured as typeof configured)?.valueOf)!()',
  '((configured as typeof configured)?.["valueOf"])!`tag`',
  'new (configured?.unknown)()',
])('machine receiver provenance rejects unproven invocations: %s', async (invocation) => {
  const head = `${configured} const escaped = ${invocation}; escaped.createMachine = value => value;
const { createMachine: make } = configured;`;
  const code = sourceFor(factoryCase('receiver escape', head, 'make'));
  validSource(code);
  expect(await lintFixture(code)).toEqual([]);
});

const configuredSource = (usage: string, value = callback) =>
  `${configured} ${usage};\nconfigured.createMachine({ context: ${value} });\n`;

test.each([
  'configured.createMachine({})',
  'configured["createMachine"]({})',
  'configured["createMachine" as const]({})',
  '(configured.createMachine as typeof configured.createMachine)({})',
  'configured.createMachine?.({})',
  'configured.createMachine({}); configured.createMachine({})',
  'void configured.valueOf',
  'inspect(configured.valueOf)',
  'new Reader(configured.valueOf)',
  'typeof configured.valueOf',
  'void configured["unknown"]',
  '(configured?.createMachine)({})',
  '((configured?.createMachine) as typeof configured.createMachine)({})',
  '(configured?.["createMachine" as const])({})',
  'inspect(configured?.valueOf)',
  'new Reader(configured?.valueOf)',
  'typeof (configured?.valueOf)',
])('machine receiver provenance retains safe factory calls and member reads: %s', async (usage) => {
  const code = configuredSource(usage);
  invalidSource(code, [{ message, line: 2 }]);
  expect(await lintFixture(code)).toMatchObject([{ message }]);
  expect(await lintFixture(configuredSource(usage, shortCallback))).toEqual([]);
});
