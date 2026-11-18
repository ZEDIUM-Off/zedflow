import { basename } from 'node:path';
import { defineRule } from 'vite-plus/lint/plugins';

export const indexExportsOnly = defineRule({
  meta: {
    type: 'suggestion',
    schema: [],
    messages: {
      code: 'An index.ts file holds re-exports only: move this statement into its own module.',
    },
  },
  create: (context) => ({
    Program: (program) => {
      if (basename(context.filename) !== 'index.ts') {
        return;
      }
      for (const statement of program.body) {
        const reexport =
          statement.type === 'ExportAllDeclaration' ||
          (statement.type === 'ExportNamedDeclaration' && statement.source !== null);
        if (!reexport) {
          context.report({ node: statement, messageId: 'code' });
        }
      }
    },
  }),
});
