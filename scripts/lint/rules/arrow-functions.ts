import { defineRule } from 'vite-plus/lint/plugins';

export const arrowFunctions = defineRule({
  meta: {
    type: 'suggestion',
    schema: [],
    messages: {
      arrow: 'Use a const arrow function instead of a function expression.',
      constant: 'Bind arrow functions with const, not let or var.',
    },
  },
  create: (context) => ({
    FunctionExpression: (node) => {
      if (node.parent.type !== 'MethodDefinition') {
        context.report({ node, messageId: 'arrow' });
      }
    },
    VariableDeclarator: (node) => {
      if (
        node.init?.type === 'ArrowFunctionExpression' &&
        node.parent.type === 'VariableDeclaration' &&
        node.parent.kind !== 'const'
      ) {
        context.report({ node, messageId: 'constant' });
      }
    },
  }),
});
