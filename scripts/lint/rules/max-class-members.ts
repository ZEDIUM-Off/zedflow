import { defineRule, type ESTree } from 'vite-plus/lint/plugins';
import { limitOption, limitSchema } from '../rule-options.ts';
import { SourceLines } from '../source-lines.ts';
import { isFunctionValue } from './function-value.ts';

/** A method, an accessor or a function-valued property counts; the constructor does not. */
const isMethod = (member: ESTree.ClassElement): boolean => {
  switch (member.type) {
    case 'MethodDefinition':
    case 'TSAbstractMethodDefinition':
      return member.kind !== 'constructor';
    case 'PropertyDefinition':
    case 'AccessorProperty':
      return isFunctionValue(member.value);
    default:
      return false;
  }
};

export const maxClassMembers = defineRule({
  meta: {
    type: 'suggestion',
    schema: limitSchema(['lines', 'methods']),
    messages: {
      lines: 'This class body has {{count}} lines. Maximum allowed is {{max}}.',
      methods: 'This class has {{count}} methods. Maximum allowed is {{max}}.',
    },
  },
  create: (context) => {
    const lines = new SourceLines(context.sourceCode);
    return {
      ClassBody: (body) => {
        const lineCount = lines.count(body);
        const maxLines = limitOption(context, 'lines');
        if (lineCount > maxLines) {
          const data = { count: String(lineCount), max: String(maxLines) };
          context.report({ node: body.parent, messageId: 'lines', data });
        }
        const methodCount = body.body.filter(isMethod).length;
        const maxMethods = limitOption(context, 'methods');
        if (methodCount > maxMethods) {
          const data = { count: String(methodCount), max: String(maxMethods) };
          context.report({ node: body.parent, messageId: 'methods', data });
        }
      },
    };
  },
});
