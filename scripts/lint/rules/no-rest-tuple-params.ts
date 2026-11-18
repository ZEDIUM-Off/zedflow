import { defineRule, type ESTree } from 'vite-plus/lint/plugins';
import { TupleTypeResolution } from './tuple-type-resolution.ts';

// Direct arrays cannot be tuples; defer named/compound forms to the compiler once per file.
const isTuple = (type: ESTree.TSType): boolean | undefined => {
  switch (type.type) {
    case 'TSTupleType':
      return true;
    case 'TSTypeOperator':
    case 'TSParenthesizedType':
      return isTuple(type.typeAnnotation);
    case 'TSArrayType':
      return false;
    default:
      return undefined;
  }
};

export const noRestTupleParams = defineRule({
  meta: {
    type: 'suggestion',
    schema: [],
    messages: {
      destructured: 'Declare named parameters instead of destructuring a rest parameter.',
      tuple: 'Declare named parameters instead of typing a rest parameter as a tuple.',
    },
  },
  create: (context) => {
    let types: TupleTypeResolution | undefined;
    const resolvesTuple = (start: number): boolean => {
      types ??= new TupleTypeResolution(context.physicalFilename, context.sourceCode.text);
      return types.isTuple(start);
    };
    return {
      RestElement: (node) => {
        if (node.parent.type === 'ArrayPattern' || node.parent.type === 'ObjectPattern') {
          return;
        }
        if (node.argument.type === 'ArrayPattern') {
          context.report({ node, messageId: 'destructured' });
        } else if (
          (node.typeAnnotation && isTuple(node.typeAnnotation.typeAnnotation)) ??
          resolvesTuple(node.range[0])
        ) {
          context.report({ node, messageId: 'tuple' });
        }
      },
    };
  },
});
