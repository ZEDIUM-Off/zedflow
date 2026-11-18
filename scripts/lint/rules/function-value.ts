import type { ESTree } from 'vite-plus/lint/plugins';

/** These TypeScript expressions preserve the declared runtime value. */
const unwrapped = (node: ESTree.Node | null): ESTree.Node | null => {
  while (node) {
    switch (node.type) {
      case 'TSSatisfiesExpression':
      case 'TSAsExpression':
      case 'TSTypeAssertion':
      case 'TSNonNullExpression':
      case 'TSInstantiationExpression':
        node = node.expression;
        break;
      default:
        return node;
    }
  }
  return node;
};

export const isFunctionValue = (node: ESTree.Node | null): boolean => {
  const value = unwrapped(node);
  return value?.type === 'ArrowFunctionExpression' || value?.type === 'FunctionExpression';
};
