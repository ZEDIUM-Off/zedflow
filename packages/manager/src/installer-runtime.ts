import { mkdir, rm, writeFile } from 'node:fs/promises';
import { delimiter, isAbsolute, join } from 'node:path';
import { DEPENDENCY_POLICY } from './dependency-policy.ts';
import { safePath } from './files.ts';

const LAYOUT = [
  'home',
  'tmp',
  'config',
  'cache',
  'state',
  'store',
  'empty-hooks',
  'empty-template',
] as const;

/** Do not inherit relative or package-local executables. */
const executablePath = (): string =>
  (process.env['PATH'] ?? '')
    .split(delimiter)
    .filter((path) => isAbsolute(path) && !path.includes('node_modules'))
    .join(delimiter);

const gitConfigVariables = (
  entries: readonly (readonly [key: string, value: string])[],
): Record<string, string> => {
  const variables: Record<string, string> = { GIT_CONFIG_COUNT: String(entries.length) };
  for (const [index, [key, value]] of entries.entries()) {
    variables[`GIT_CONFIG_KEY_${index}`] = key;
    variables[`GIT_CONFIG_VALUE_${index}`] = value;
  }
  return variables;
};

/**
 * The manager-owned runtime directory that every installer child process is confined to. Each path
 * is checked against symlinks below the directory before the runtime writes to it, and children
 * receive only the environment derived from this directory.
 */
export class InstallerRuntime {
  readonly #directory: string;

  public constructor(directory: string) {
    this.#directory = directory;
  }

  public path(...segments: readonly string[]): string {
    return join(this.#directory, ...segments);
  }

  public get dependencyPolicy(): string {
    return this.path('config', 'dependency-policy.cjs');
  }

  public async clearCache(): Promise<void> {
    await safePath(this.#directory, this.path('cache'));
    await rm(this.path('cache'), { recursive: true, force: true });
  }

  /** Creates the layout and policy files; returns the environment for one child process. */
  public async prepare(): Promise<NodeJS.ProcessEnv> {
    for (const name of LAYOUT) {
      await safePath(this.#directory, this.path(name));
      await mkdir(this.path(name), { recursive: true });
    }
    const emptyConfig = this.path('config', 'empty');
    await safePath(this.#directory, emptyConfig);
    await writeFile(emptyConfig, '');
    await safePath(this.#directory, this.dependencyPolicy);
    await writeFile(this.dependencyPolicy, DEPENDENCY_POLICY, { mode: 0o600 });
    return this.#environment(emptyConfig);
  }

  // Do not inherit NODE_OPTIONS, Git helpers/config, authentication, npmrc or pnpm settings.
  #environment(emptyConfig: string): NodeJS.ProcessEnv {
    return {
      PATH: executablePath(),
      HOME: this.path('home'),
      USERPROFILE: this.path('home'),
      TMPDIR: this.path('tmp'),
      XDG_CONFIG_HOME: this.path('config'),
      XDG_CACHE_HOME: this.path('cache'),
      XDG_DATA_HOME: this.path('state'),
      PNPM_HOME: this.path('home'),
      NPM_CONFIG_USERCONFIG: emptyConfig,
      NPM_CONFIG_GLOBALCONFIG: emptyConfig,
      CI: 'true',
      LANG: 'C.UTF-8',
      ...this.#gitEnvironment(emptyConfig),
    };
  }

  #gitEnvironment(emptyConfig: string): NodeJS.ProcessEnv {
    return {
      GIT_CONFIG_NOSYSTEM: '1',
      GIT_CONFIG_SYSTEM: emptyConfig,
      GIT_CONFIG_GLOBAL: emptyConfig,
      GIT_TERMINAL_PROMPT: '0',
      GIT_ALLOW_PROTOCOL: 'http:https:file',
      ...gitConfigVariables([
        ['core.hooksPath', this.path('empty-hooks')],
        ['core.fsmonitor', 'false'],
        ['credential.helper', ''],
        ['protocol.ext.allow', 'never'],
        ['init.templateDir', this.path('empty-template')],
        ['protocol.ssh.allow', 'never'],
      ]),
    };
  }
}
