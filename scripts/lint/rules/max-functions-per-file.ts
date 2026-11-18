import { defineRule, type ESTree } from 'vite-plus/lint/plugins';
import { limitOption, limitSchema } from '../rule-options.ts';
import { isFunctionValue } from './function-value.ts';

type TopLevel = ESTree.Program['body'][number];

const unexported = (statement: TopLevel): ESTree.Node | null =>
  statement.type === 'ExportNamedDeclaration' || statement.type === 'ExportDefaultDeclaration'
    ? statement.declaration
    : statement;

/** Module-level functions declared by one top-level statement. */
const declaredFunctions = (statement: TopLevel): ESTree.Node[] => {
  const declaration = unexported(statement);
  if (declaration && (declaration.type === 'FunctionDeclaration' || isFunctionValue(declaration))) {
    return [declaration];
  }
  if (declaration?.type !== 'VariableDeclaration' || declaration.kind !== 'const') {
    return [];
  }
  return declaration.declarations.filter((declarator) => isFunctionValue(declarator.init));
};

export const maxFunctionsPerFile = defineRule({
  meta: {
    type: 'suggestion',
    schema: limitSchema(['max']),
    messages: {
      excess: 'This file declares {{count}} module-level functions. Maximum allowed is {{max}}.',
    },
  },
  create: (context) => ({
    Program: (program) => {
      const max = limitOption(context, 'max');
      const functions = program.body.flatMap(declaredFunctions);
      for (const node of functions.slice(max)) {
        context.report({
          node,
          messageId: 'excess',
          data: { count: String(functions.length), max: String(max) },
        });
      }
    },
  }),
});
