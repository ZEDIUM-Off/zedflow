import type { Definition, ESTree, SourceCode, Variable } from 'vite-plus/lint/plugins';

type FactoryKind = 'namespace' | 'setup' | 'configured' | 'machine' | undefined;

const unsafeMemberUses = new Set([
  'UpdateExpression',
  'ArrayPattern',
  'RestElement',
  'ForInStatement',
  'ForOfStatement',
  'TaggedTemplateExpression',
]);

const unwrapped = (node: ESTree.Node): ESTree.Node => {
  while (
    node.type === 'TSAsExpression' ||
    node.type === 'TSSatisfiesExpression' ||
    node.type === 'TSTypeAssertion' ||
    node.type === 'TSNonNullExpression' ||
    node.type === 'TSInstantiationExpression' ||
    node.type === 'ChainExpression'
  ) {
    node = node.expression;
  }
  return node;
};

const propertyName = (node: ESTree.Node, computed: boolean): string | undefined => {
  const value = unwrapped(node);
  if (value.type === 'Literal' && typeof value.value === 'string') {
    return value.value;
  }
  return !computed && value.type === 'Identifier' ? value.name : undefined;
};

const memberKind = (object: FactoryKind, name: string | undefined): FactoryKind => {
  if (object === 'namespace' && name === 'setup') {
    return 'setup';
  }
  return (object === 'namespace' || object === 'configured') && name === 'createMachine'
    ? 'machine'
    : undefined;
};

const isValueImport = (node: ESTree.Node): boolean => {
  const parent = node.parent;
  return (
    parent?.type === 'ImportDeclaration' &&
    parent.source.value === 'xstate' &&
    parent.importKind !== 'type'
  );
};

const importedKind = ({ type, node }: Definition): FactoryKind => {
  if (type !== 'ImportBinding' || !isValueImport(node)) {
    return undefined;
  }
  if (node.type === 'ImportNamespaceSpecifier') {
    return 'namespace';
  }
  return node.type === 'ImportSpecifier' && node.importKind !== 'type'
    ? memberKind('namespace', propertyName(node.imported, false))
    : undefined;
};

const outerValue = (node: ESTree.Node): ESTree.Node => {
  while (node.parent && unwrapped(node.parent) === unwrapped(node)) {
    node = node.parent;
  }
  return node;
};

const resolvedBinding = (source: SourceCode, node: ESTree.Node): Variable | null | undefined =>
  source.getScope(node).references.find((ref) => ref.identifier === node)?.resolved;

const singleDefinition = (binding: Variable): Definition | undefined =>
  binding.defs.length === 1 && !binding.references.some((ref) => ref.isWrite() && !ref.init)
    ? binding.defs[0]
    : undefined;

/** Resolves only lexical imports and immutable aliases, never arbitrary runtime return values. */
export class MachineFactoryBindings {
  readonly #source: SourceCode;
  readonly #resolving = new Set<Variable>();

  public constructor(source: SourceCode) {
    this.#source = source;
  }

  public isCreateMachine(node: ESTree.Expression): boolean {
    return this.#kind(node) === 'machine';
  }

  #kind(expression: ESTree.Node): FactoryKind {
    const node = unwrapped(expression);
    switch (node.type) {
      case 'Identifier':
        return this.#identifier(node);
      case 'MemberExpression':
        return memberKind(this.#kind(node.object), propertyName(node.property, node.computed));
      case 'CallExpression':
        return this.#kind(node.callee) === 'setup' ? 'configured' : undefined;
      default:
        return undefined;
    }
  }

  #identifier(node: ESTree.Node): FactoryKind {
    const binding = resolvedBinding(this.#source, node);
    if (!binding || this.#resolving.has(binding)) {
      return undefined;
    }
    const definition = singleDefinition(binding);
    if (!definition) {
      return undefined;
    }
    this.#resolving.add(binding);
    const kind = importedKind(definition) ?? this.#alias(definition);
    this.#resolving.delete(binding);
    if (kind === 'configured' && !this.#readOnly(binding)) {
      return undefined;
    }
    return kind;
  }

  #alias({ type, node, name }: Definition): FactoryKind {
    if (
      type !== 'Variable' ||
      node.type !== 'VariableDeclarator' ||
      node.parent.type !== 'VariableDeclaration' ||
      node.parent.kind !== 'const' ||
      !node.init
    ) {
      return undefined;
    }
    const kind = this.#kind(node.init);
    if (node.id.type === 'Identifier') {
      return kind;
    }
    return this.#destructured(node.id, name, kind);
  }

  // Object bindings must not escape to unknown consumers or acquire replacement factories.
  // Follow const object aliases too, so mutation through an alias cannot prove the source safe.
  #readOnly(binding: Variable, seen = new Set<Variable>()): boolean {
    if (seen.has(binding)) {
      return true;
    }
    seen.add(binding);
    return binding.references.every((ref) => !ref.isRead() || this.#safeRead(ref.identifier, seen));
  }

  #safeRead(identifier: ESTree.Node, seen: Set<Variable>): boolean {
    const node = outerValue(identifier);
    const parent = node.parent;
    if (!parent) {
      return false;
    }
    if (parent.type === 'TSTypeQuery' || parent.type === 'TSQualifiedName') {
      return true;
    }
    if (parent.type === 'MemberExpression' && parent.object === node) {
      return this.#memberRead(parent);
    }
    return this.#safeAlias(parent, node, seen);
  }

  #safeAlias(parent: ESTree.Node, value: ESTree.Node, seen: Set<Variable>): boolean {
    if (parent.type !== 'VariableDeclarator' || parent.init !== value) {
      return false;
    }
    if (parent.parent.type !== 'VariableDeclaration' || parent.parent.kind !== 'const') {
      return false;
    }
    return (
      parent.id.type === 'ObjectPattern' ||
      this.#source.getDeclaredVariables(parent).every((alias) => this.#readOnly(alias, seen))
    );
  }

  #memberRead(member: ESTree.MemberExpression): boolean {
    const node = outerValue(member);
    const parent = node.parent;
    if (!parent) {
      return true;
    }
    switch (parent.type) {
      case 'CallExpression':
      case 'NewExpression':
        // Only the known XState factory may be invoked without losing receiver provenance.
        return (
          parent.callee !== node ||
          propertyName(member.property, member.computed) === 'createMachine'
        );
      case 'UnaryExpression':
        return parent.operator !== 'delete';
      case 'Property':
        return parent.parent.type !== 'ObjectPattern';
      case 'AssignmentExpression':
        return parent.left !== node;
      default:
        return !unsafeMemberUses.has(parent.type);
    }
  }

  #destructured(id: ESTree.BindingPattern, name: ESTree.Node, kind: FactoryKind): FactoryKind {
    if (id.type !== 'ObjectPattern') {
      return undefined;
    }
    const property = id.properties.find(
      (entry) => entry.type === 'Property' && entry.value === name,
    );
    return property?.type === 'Property'
      ? memberKind(kind, propertyName(property.key, property.computed))
      : undefined;
  }
}
