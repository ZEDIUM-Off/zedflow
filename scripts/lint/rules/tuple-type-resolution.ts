import ts from 'typescript';
import { resolve } from 'node:path';

const configPath = ts.findConfigFile(process.cwd(), (path) => ts.sys.fileExists(path));
const config: unknown = configPath
  ? ts.readConfigFile(configPath, (path) => ts.sys.readFile(path)).config
  : {};
const options = ts.parseJsonConfigFileContent(config, ts.sys, process.cwd()).options;

/** One lazy compiler snapshot per linted file, including contextually typed rest parameters. */
export class TupleTypeResolution {
  readonly #checker: ts.TypeChecker;
  readonly #source: ts.SourceFile;
  readonly #types = new Map<number, ts.Type>();

  public constructor(filename: string, text: string) {
    const path = resolve(filename);
    const host = ts.createCompilerHost(options);
    const getSourceFile = host.getSourceFile.bind(host);
    host.getSourceFile = (file, languageVersion, onError, shouldCreateNewSourceFile) =>
      resolve(file) === path
        ? ts.createSourceFile(file, text, languageVersion, true)
        : getSourceFile(file, languageVersion, onError, shouldCreateNewSourceFile);
    const program = ts.createProgram([path], options, host);
    const source = program.getSourceFile(path);
    if (!source) {
      throw new Error(`Cannot resolve tuple types in ${filename}`);
    }
    this.#source = source;
    this.#checker = program.getTypeChecker();
    this.#collect(source);
  }

  #collect(node: ts.Node): void {
    if (ts.isParameter(node) && node.dotDotDotToken) {
      this.#types.set(node.getStart(this.#source), this.#checker.getTypeAtLocation(node));
    }
    ts.forEachChild(node, (child) => this.#collect(child));
  }

  #isTuple(type: ts.Type, seen: Set<ts.Type>): boolean {
    if (seen.has(type)) {
      return false;
    }
    seen.add(type);
    if (this.#checker.isTupleType(type)) {
      return true;
    }
    if (type.isUnionOrIntersection()) {
      return type.types.some((part) => this.#isTuple(part, seen));
    }
    const constraint = this.#checker.getBaseConstraintOfType(type);
    return constraint !== undefined && this.#isTuple(constraint, seen);
  }

  public isTuple(start: number): boolean {
    const type = this.#types.get(start);
    return type !== undefined && this.#isTuple(type, new Set());
  }
}
