import { defineConfig } from 'vite-plus';
import { limits } from './scripts/lint/limits.ts';
import { rulesFor, structuralRules } from './scripts/lint/profile.ts';

export default defineConfig({
  fmt: {
    ignorePatterns: ['references/**', 'docs/**', 'AGENTS.md', 'README.md'],
    printWidth: 100,
    tabWidth: 2,
    useTabs: false,
    semi: true,
    singleQuote: true,
    trailingComma: 'all',
    endOfLine: 'lf',
  },
  lint: {
    ignorePatterns: ['references/**'],
    options: { typeAware: true, typeCheck: true },
    plugins: ['typescript', 'unicorn', 'import'],
    jsPlugins: [{ name: 'zf', specifier: './scripts/lint/plugin.ts' }],
    rules: {
      'zf/arrow-functions': 'error',
      'func-style': ['error', 'expression', { overrides: { namedExports: 'expression' } }],
      'prefer-const': 'error',
      'prefer-arrow-callback': 'error',
      eqeqeq: ['error', 'always'],
      curly: ['error', 'all'],
      'no-var': 'error',
      'typescript/no-explicit-any': 'error',
      'typescript/explicit-member-accessibility': ['error', { accessibility: 'explicit' }],
      'typescript/no-empty-object-type': 'error',
      'typescript/no-extraneous-class': 'error',
      'typescript/prefer-readonly': 'error',
      'typescript/no-non-null-assertion': 'error',
      'typescript/no-floating-promises': 'error',
      'typescript/no-misused-promises': 'error',
      'typescript/no-unsafe-assignment': 'error',
      'typescript/no-unsafe-argument': 'error',
      'typescript/no-unsafe-call': 'error',
      'typescript/no-unsafe-member-access': 'error',
      'typescript/no-unsafe-return': 'error',
      'typescript/consistent-type-imports': ['error', { prefer: 'type-imports' }],
      'typescript/consistent-type-definitions': ['error', 'interface'],
      'typescript/method-signature-style': ['error', 'property'],
      'typescript/explicit-function-return-type': [
        'error',
        {
          allowExpressions: true,
          allowTypedFunctionExpressions: true,
          allowHigherOrderFunctions: true,
        },
      ],
      ...rulesFor(limits.source),
      ...structuralRules,
    },
    overrides: [
      {
        files: ['**/test/**/*.ts'],
        excludeFiles: ['**/test/support/**'],
        rules: { ...rulesFor(limits.test), 'typescript/explicit-function-return-type': 'off' },
      },
      { files: ['**/machines/**/*.ts'], rules: rulesFor(limits.machine) },
    ],
  },
  test: { include: ['test/**/*.test.ts'], environment: 'node' },
});
