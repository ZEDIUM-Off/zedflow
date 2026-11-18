import { defineRule, type ESTree } from 'vite-plus/lint/plugins';
import { limitOption, limitSchema } from '../rule-options.ts';
import { SourceLines } from '../source-lines.ts';
import { MachineFactoryBindings } from './machine-factory-bindings.ts';

type FunctionNode = ESTree.Function | ESTree.ArrowFunctionExpression;

export const maxMachineConfigFunctionLines = defineRule({
  meta: {
    type: 'suggestion',
    schema: limitSchema(['max']),
    messages: {
      long: 'This machine config function has {{count}} lines. Maximum allowed is {{max}}: name it outside the config.',
    },
  },
  create: (context) => {
    const lines = new SourceLines(context.sourceCode);
    const factories = new MachineFactoryBindings(context.sourceCode);
    const configs: ESTree.Argument[][] = [];
    const check = (node: FunctionNode): void => {
      const inside = configs.some((config) =>
        config.some((argument) => argument.start <= node.start && node.end <= argument.end),
      );
      const count = lines.count(node);
      const max = limitOption(context, 'max');
      if (inside && count > max) {
        context.report({
          node,
          messageId: 'long',
          data: { count: String(count), max: String(max) },
        });
      }
    };
    return {
      CallExpression: (call) => {
        configs.push(factories.isCreateMachine(call.callee) ? call.arguments : []);
      },
      'CallExpression:exit': () => {
        configs.pop();
      },
      FunctionExpression: check,
      ArrowFunctionExpression: check,
    };
  },
});
