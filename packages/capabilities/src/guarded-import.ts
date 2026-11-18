import type { Hash } from 'node:crypto';
import { readFileSync, realpathSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { extname, sep } from 'node:path';
import { isBuiltin } from 'node:module';
import { fileURLToPath } from 'node:url';
import { createJiti } from 'jiti';
import type { Jiti, TransformOptions, TransformResult } from 'jiti';

interface ImportBindings {
  readonly require: Jiti;
  readonly jitiImport: Jiti['import'];
  readonly jitiESMResolve: Jiti['esmResolve'];
}

const withoutPreamble = (value: string): string =>
  value.replace(/^\uFEFF/, '').replace(/^#![^\n]*(?:\n|$)/, '');

// Native JS/CJS/JSON imports use Node's cache even with Jiti's caches disabled.
// Enforce the cache-free local TypeScript profile at actual import/require calls.
const guardedCode = (code: string): string => {
  const directive = code.match(/^["']use strict["'];?/)?.[0] ?? '';
  return `${directive}({require,jitiImport}=require('virtual:zedflow-imports').guard({require,jitiImport,jitiESMResolve}));${code.slice(directive.length)}`;
};

/**
 * One cache-free import of a machine entry. It owns the closure of local sources that the import
 * read, each recorded once and required to stay unchanged, and guards every import or require
 * of a local module so that it resolves to TypeScript. Use one instance per import.
 */
export class GuardedImport {
  readonly #sharedModules: Record<string, unknown>;
  readonly #sources = new Map<string, string>();
  readonly #transformer = createJiti(import.meta.url, {
    moduleCache: false,
    fsCache: false,
    tryNative: false,
  });

  public constructor(sharedModules: Record<string, unknown>) {
    this.#sharedModules = sharedModules;
  }

  public async module(entry: string): Promise<unknown> {
    const jiti = createJiti(import.meta.url, {
      virtualModules: {
        ...this.#sharedModules,
        'virtual:zedflow-imports': { guard: (bindings: ImportBindings) => this.#guard(bindings) },
      },
      transform: (options) => this.#transform(options),
      moduleCache: false,
      fsCache: false,
      tryNative: false,
    });
    const module: unknown = await jiti.import(entry, { default: true });
    return module;
  }

  /** Adds each local source to `revision` in path order; fails when one changed since its import. */
  public async hashClosure(revision: Hash): Promise<void> {
    for (const [filename, source] of [...this.#sources].sort(([a], [b]) => a.localeCompare(b))) {
      if ((await readFile(filename, 'utf8')) !== source) {
        throw new Error('Machine dependency changed while loading');
      }
      revision.update(filename).update('\0').update(source).update('\0');
    }
  }

  #transform(options: TransformOptions): TransformResult {
    const installed = options.filename?.split(sep).includes('node_modules') === true;
    if (options.filename && !installed) {
      this.#record(options.filename, options.source);
    }
    const code = this.#transformer.transform(options);
    return { code: installed ? code : guardedCode(code) };
  }

  #record(filename: string, transformed: string): void {
    const path = realpathSync(filename);
    const source = readFileSync(path, 'utf8');
    if (
      withoutPreamble(source) !== withoutPreamble(transformed) ||
      (this.#sources.has(path) && this.#sources.get(path) !== source)
    ) {
      throw new Error('Machine dependency changed while loading');
    }
    this.#sources.set(path, source);
  }

  #guard(bindings: ImportBindings): Pick<ImportBindings, 'require' | 'jitiImport'> {
    return {
      require: new Proxy(bindings.require, {
        apply: (target, receiver, args: [string]) => {
          this.#checkImport(args[0], bindings.require.resolve);
          const result: unknown = Reflect.apply(target, receiver, args);
          return result;
        },
      }),
      jitiImport: new Proxy(bindings.jitiImport, {
        apply: (target, receiver, args: [string, unknown?]) => {
          this.#checkImport(args[0], bindings.jitiESMResolve);
          const result: unknown = Reflect.apply(target, receiver, args);
          return result;
        },
      }),
    };
  }

  #checkImport(id: string, resolveImport: (id: string) => string): void {
    if (isBuiltin(id) || Object.hasOwn(this.#sharedModules, id)) {
      return;
    }
    const resolved = resolveImport(id);
    const path = resolved.startsWith('file:') ? fileURLToPath(resolved) : resolved;
    if (path.split(sep).includes('node_modules')) {
      return;
    }
    if (!['.ts', '.mts', '.cts', '.tsx', '.mtsx', '.ctsx'].includes(extname(path))) {
      throw new Error(
        'Local machine dependencies must use TypeScript for cache-free reload; native installed packages remain supported',
      );
    }
  }
}
