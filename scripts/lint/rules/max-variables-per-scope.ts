import { defineRule, type Context, type ESTree } from 'vite-plus/lint/plugins';
import { limitOption, limitSchema } from '../rule-options.ts';
import { isFunctionValue } from './function-value.ts';

type FunctionNode = ESTree.Function | ESTree.ArrowFunctionExpression;

/** Number of identifiers that one binding pattern declares. */
const boundNames = (pattern: ESTree.BindingPattern | ESTree.BindingRestElement): number => {
  switch (pattern.type) {
    case 'Identifier':
      return 1;
    case 'AssignmentPattern':
      return boundNames(pattern.left);
    case 'RestElement':
      return boundNames(pattern.argument);
    case 'ArrayPattern':
      return pattern.elements.reduce(
        (total, element) => total + (element === null ? 0 : boundNames(element)),
        0,
      );
    case 'ObjectPattern':
      return pattern.properties.reduce(
        (total, property) =>
          total + boundNames(property.type === 'Property' ? property.value : property),
        0,
      );
  }
};

/** Variable counts of the functions that enclose the visited node, innermost last. */
class FunctionScopes {
  readonly #context: Context;
  readonly #counts: number[] = [];

  public constructor(context: Context) {
    this.#context = context;
  }

  public enter(): void {
    this.#counts.push(0);
  }

  public record(declarator: ESTree.VariableDeclarator): void {
    const last = this.#counts.length - 1;
    if (last >= 0 && !isFunctionValue(declarator.init)) {
      this.#counts[last] = (this.#counts[last] ?? 0) + boundNames(declarator.id);
    }
  }

  public exit(node: FunctionNode): void {
    const count = this.#counts.pop() ?? 0;
    const max = limitOption(this.#context, 'max');
    if (count > max) {
      const data = { count: String(count), max: String(max) };
      this.#context.report({ node, messageId: 'excess', data });
    }
  }
}

export const maxVariablesPerScope = defineRule({
  meta: {
    type: 'suggestion',
    schema: limitSchema(['max']),
    messages: {
      excess: 'This function declares {{count}} local variables. Maximum allowed is {{max}}.',
    },
  },
  create: (context) => {
    const scopes = new FunctionScopes(context);
    const enter = (): void => scopes.enter();
    const exit = (node: FunctionNode): void => scopes.exit(node);
    return {
      FunctionDeclaration: enter,
      FunctionExpression: enter,
      ArrowFunctionExpression: enter,
      'FunctionDeclaration:exit': exit,
      'FunctionExpression:exit': exit,
      'ArrowFunctionExpression:exit': exit,
      VariableDeclarator: (declarator) => scopes.record(declarator),
    };
  },
});
