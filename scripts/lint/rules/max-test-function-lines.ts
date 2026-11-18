import { defineRule, type Definition, type ESTree, type SourceCode } from 'vite-plus/lint/plugins';
import { limitOption, limitSchema } from '../rule-options.ts';
import { SourceLines } from '../source-lines.ts';

type FunctionNode = ESTree.Function | ESTree.ArrowFunctionExpression;

const testModules = new Set(['vite-plus/test', 'vitest']);

/**
 * The identifier that a registration starts from: `describe`, `describe.skip`,
 * `describe.each(table)` or ``describe.each`table` ``.
 */
const registrationRoot = (callee: ESTree.Expression): ESTree.IdentifierReference | undefined => {
  switch (callee.type) {
    case 'Identifier':
      return callee;
    case 'MemberExpression':
      return registrationRoot(callee.object);
    case 'CallExpression':
      return registrationRoot(callee.callee);
    case 'TaggedTemplateExpression':
      return registrationRoot(callee.tag);
    default:
      return undefined;
  }
};

const exportName = (name: ESTree.ModuleExportName): string =>
  name.type === 'Literal' ? name.value : name.name;

/** Whether `definition` imports the named export `describe` of a test module. */
const importsDescribe = ({ type, node }: Definition): boolean =>
  type === 'ImportBinding' &&
  node.type === 'ImportSpecifier' &&
  exportName(node.imported) === 'describe' &&
  node.parent.type === 'ImportDeclaration' &&
  testModules.has(node.parent.source.value);

/**
 * Whether `identifier` resolves to an import of `describe`. The binding comes from the scope
 * analysis of the whole file, so a shadowing local or parameter does not match and an import
 * written below the call does.
 */
const isImportedDescribe = (
  sourceCode: SourceCode,
  identifier: ESTree.IdentifierReference,
): boolean =>
  sourceCode
    .getScope(identifier)
    .references.find((reference) => reference.identifier === identifier)
    ?.resolved?.defs.some(importsDescribe) === true;

export const maxTestFunctionLines = defineRule({
  meta: {
    type: 'suggestion',
    schema: limitSchema(['max']),
    messages: {
      long: 'This function has {{count}} lines. Maximum allowed is {{max}}.',
    },
  },
  create: (context) => {
    const lines = new SourceLines(context.sourceCode);
    const exempt = new WeakSet<ESTree.Node>();
    const check = (node: FunctionNode): void => {
      const count = lines.count(node);
      const max = limitOption(context, 'max');
      if (!exempt.has(node) && count > max) {
        context.report({
          node,
          messageId: 'long',
          data: { count: String(count), max: String(max) },
        });
      }
    };
    return {
      CallExpression: (call) => {
        const root = registrationRoot(call.callee);
        if (root !== undefined && isImportedDescribe(context.sourceCode, root)) {
          call.arguments.forEach((argument) => exempt.add(argument));
        }
      },
      FunctionDeclaration: check,
      FunctionExpression: check,
      ArrowFunctionExpression: check,
    };
  },
});
