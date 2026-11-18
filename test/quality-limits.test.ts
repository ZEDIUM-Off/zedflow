import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { lstat, mkdir, mkdtemp, readdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { expect, test } from 'vite-plus/test';
import config from '../vite.config.ts';
import { limits } from '../scripts/lint/limits.ts';
import { rulesFor } from '../scripts/lint/profile.ts';

const run = promisify(execFile);

const runCheck = async (path: string) => {
  try {
    const result = await run('pnpm', ['exec', 'vp', 'check', '--no-fmt', path], { timeout: 20000 });
    return { failed: false, output: result.stdout + result.stderr };
  } catch (error) {
    if (!(error instanceof Error) || !('stdout' in error) || !('stderr' in error)) {
      throw error;
    }
    return { failed: true, output: String(error.stdout) + String(error.stderr) };
  }
};

/** Writes `source` at `file` inside a fresh directory of the quality workspace and checks it. */
const checkFixture = async (file: string, source: string) => {
  const root = resolve('.artifacts/workspace-loop/quality');
  await mkdir(root, { recursive: true });
  const path = join(await mkdtemp(join(root, 'limits-')), file);
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, source);
  return runCheck(path);
};

const numbered = (count: number, line: (index: number) => string) =>
  Array.from({ length: count }, (_, index) => line(index)).join('\n');

/** A function of `lines` lines with one statement, so that only its length can exceed a limit. */
const longFunction = (lines: number) =>
  `export const values = (): number[] => [\n${numbered(lines - 2, (index) => `  ${index},`)}\n];\n`;

/** `count` short test registrations, so that only the enclosing callback is long. */
const shortTests = (count: number) =>
  numbered(
    count,
    (index) => `  test('case ${index}', () => {\n    expect(${index}).toBe(${index});\n  });`,
  );

/** A test whose body holds `count` code lines between blank and comment-only lines. */
const sparseTest = (count: number) =>
  `test('sparse', () => {\n${numbered(count, (index) => `  expect(${index}).toBe(${index});\n\n  // note ${index}\n  /* block */`)}\n});`;

const imports = `import { describe, expect, test } from 'vite-plus/test';\n`;

/** A function of `lines` lines that is invoked where it is written. */
const iife = (lines: number) =>
  `export const values = ((): number[] => [\n${numbered(lines - 2, (index) => `  ${index},`)}\n])();\n`;

/** A tagged-template table of two rows, as `describe.each` accepts it. */
const table = '`\n  value\n  ${1}\n  ${2}\n`';

const acceptedCases = [
  {
    name: 'a long describe callback',
    file: 'test/describe.test.ts',
    source: `${imports}describe('suite', () => {\n${shortTests(30)}\n});\n`,
  },
  {
    name: 'an aliased describe callback',
    file: 'test/alias.test.ts',
    source: `import { describe as suite, expect, test } from 'vite-plus/test';\nsuite('suite', () => {\n${shortTests(30)}\n});\n`,
  },
  {
    name: 'a describe.each callback',
    file: 'test/each.test.ts',
    source: `${imports}describe.each([1, 2])('suite %s', (value) => {\n  expect(value).toBeGreaterThan(0);\n${shortTests(30)}\n});\n`,
  },
  {
    name: 'a test of 50 code lines among blank and comment lines',
    file: 'test/sparse.test.ts',
    source: `${imports}${sparseTest(48)}\n`,
  },
  {
    name: 'a 40-line function in a test directory',
    file: 'test/values.ts',
    source: longFunction(40),
  },
  {
    name: 'a 40-line function in a machines directory',
    file: 'machines/values.ts',
    source: longFunction(40),
  },
  {
    name: 'a source function of 30 code lines among blank and comment lines',
    file: 'src/values.ts',
    source: `export const values = (): number[] => [\n${numbered(28, (index) => `  ${index},\n\n  // ${index}`)}\n];\n`,
  },
  {
    name: 'a 3-line inline function inside createMachine',
    file: 'machines/machine.ts',
    source: `import { createMachine } from 'xstate';\nexport default createMachine({\n  entry: () => {\n    console.log('entered');\n  },\n});\n`,
  },
  {
    name: 'a tagged-template describe.each callback',
    file: 'test/tagged.test.ts',
    source: `${imports}describe.each${table}('suite $value', () => {\n${shortTests(30)}\n});\n`,
  },
  {
    name: 'an aliased tagged-template describe.each callback',
    file: 'test/tagged-alias.test.ts',
    source: `import { describe as suite, expect, test } from 'vite-plus/test';\nsuite.each${table}('suite $value', () => {\n${shortTests(30)}\n});\n`,
  },
  {
    name: 'a describe callback registered above its import declaration',
    file: 'test/hoisted.test.ts',
    source: `describe('suite', () => {\n${shortTests(30)}\n});\n${imports}`,
  },
];

test.each(acceptedCases)('the limit profiles accept $name', async ({ file, source }) => {
  const result = await checkFixture(file, source);
  expect(result.output).not.toContain('max-test-function-lines');
  expect(result.output).not.toContain('max-lines-per-function');
  expect(result.failed).toBe(false);
});

const rejectedCases = [
  {
    name: 'a long test inside a describe callback',
    file: 'test/long.test.ts',
    rule: 'zf(max-test-function-lines)',
    source: `${imports}describe('suite', () => {\n  ${sparseTest(49)}\n});\n`,
  },
  {
    name: 'a long callback of a describe that is not imported from a test module',
    file: 'test/local.test.ts',
    rule: 'zf(max-test-function-lines)',
    source: `import { expect, test } from 'vite-plus/test';\nconst describe = (name: string, body: () => void): void => {\n  test(name, body);\n};\ndescribe('suite', () => {\n${shortTests(30)}\n});\n`,
  },
  {
    name: 'a long helper of a test file',
    file: 'test/helper.test.ts',
    rule: 'zf(max-test-function-lines)',
    source: longFunction(51),
  },
  {
    name: 'a 40-line test support helper',
    file: 'test/support/values.ts',
    rule: 'eslint(max-lines-per-function)',
    source: longFunction(40),
  },
  {
    name: 'a 40-line function in a plain directory',
    file: 'src/values.ts',
    rule: 'eslint(max-lines-per-function)',
    source: longFunction(40),
  },
  {
    name: 'a 4-line inline function inside createMachine',
    file: 'machines/machine.ts',
    rule: 'zf(max-machine-config-function-lines)',
    source: `import { createMachine } from 'xstate';\nexport default createMachine({\n  entry: () => {\n    const total = 1 + 2;\n    console.log(total);\n  },\n});\n`,
  },
  {
    name: 'a long test inside a tagged-template describe.each callback',
    file: 'test/tagged-long.test.ts',
    rule: 'zf(max-test-function-lines)',
    source: `${imports}describe.each${table}('suite $value', () => {\n  ${sparseTest(49)}\n});\n`,
  },
  {
    name: 'a long callback of a block-local describe that shadows the import',
    file: 'test/block.test.ts',
    rule: 'zf(max-test-function-lines)',
    source: `${imports}describe('suite', () => {\n  test('short', () => {\n    expect(1).toBe(1);\n  });\n});\n{\n  const describe = (name: string, body: () => void): void => {\n    test(name, body);\n  };\n  describe('shadowed', () => {\n${shortTests(30)}\n  });\n}\n`,
  },
  {
    name: 'a long callback of a parameter that shadows the imported describe',
    file: 'test/parameter.test.ts',
    rule: 'zf(max-test-function-lines)',
    source: `${imports}const register = (name: string, body: () => void): void => {\n  test(name, body);\n};\ndescribe.each([register])('suite %#', (describe) => {\n  describe('shadowed', () => {\n${shortTests(30)}\n  });\n});\n`,
  },
  {
    name: 'a 51-line immediately invoked function in a test file',
    file: 'test/iife.test.ts',
    rule: 'zf(max-test-function-lines)',
    source: iife(51),
  },
  {
    name: 'a 31-line immediately invoked function in a plain directory',
    file: 'src/iife.ts',
    rule: 'eslint(max-lines-per-function)',
    source: iife(31),
  },
];

test.each(rejectedCases)('the limit profiles reject $name', async ({ file, rule, source }) => {
  const result = await checkFixture(file, source);
  expect(result.failed).toBe(true);
  expect(result.output).toContain(rule);
});

test('a describe callback imported from vitest is exempt from the test function length', async () => {
  const source = `import { describe, expect, test } from 'vitest';\ndescribe('suite', () => {\n${shortTests(30)}\n});\n`;
  const result = await checkFixture('test/vitest.test.ts', source);
  expect(result.output).not.toContain('max-test-function-lines');
});

test('lint has no debt, disable directives or per-file exceptions', async () => {
  await expect(lstat('scripts/lint/debt')).rejects.toMatchObject({ code: 'ENOENT' });
  const tracked = await run('git', [
    'ls-files',
    '-z',
    '--',
    'packages/',
    'src/',
    'dev/',
    'scripts/',
    'test/',
  ]);
  const files = tracked.stdout.split('\0').filter((file) => file.endsWith('.ts'));
  expect(files.length).toBeGreaterThan(0);
  const directive = new RegExp(`(?:${['oxlint', 'eslint'].join('|')})-${'disable'}`);
  for (const file of files) {
    expect(await readFile(file, 'utf8'), file).not.toMatch(directive);
  }
  expect(config.lint?.overrides).toEqual([
    {
      files: ['**/test/**/*.ts'],
      excludeFiles: ['**/test/support/**'],
      rules: { ...rulesFor(limits.test), 'typescript/explicit-function-return-type': 'off' },
    },
    { files: ['**/machines/**/*.ts'], rules: rulesFor(limits.machine) },
  ]);
});

const piPackage = '@earendil-works/pi-coding-agent';

const typeScriptFiles = async (directory: string): Promise<string[]> => {
  const entries = await readdir(directory, { withFileTypes: true, recursive: true });
  return entries
    .filter(
      (entry) =>
        entry.isFile() && entry.name.endsWith('.ts') && !entry.parentPath.includes('node_modules'),
    )
    .map((entry) => join(entry.parentPath, entry.name));
};

test('only packages/tools depends on the Pi coding agent, through its package root', async () => {
  const manifests = [
    'package.json',
    ...(await readdir('packages')).map((name) => join('packages', name, 'package.json')),
  ];
  const dependents: string[] = [];
  for (const manifest of manifests) {
    const parsed: unknown = JSON.parse(await readFile(manifest, 'utf8'));
    if (JSON.stringify(parsed).includes(`"${piPackage}"`)) {
      dependents.push(manifest);
    }
  }
  expect(dependents).toEqual(['packages/tools/package.json']);
  for (const file of await typeScriptFiles('packages')) {
    expect(await readFile(file, 'utf8'), file).not.toMatch(new RegExp(`['"]${piPackage}/`));
  }
});
