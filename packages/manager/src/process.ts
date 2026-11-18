import { InstallerRuntime } from './installer-runtime.ts';
import type { ProcessRequest, ProcessRunner } from './types.ts';

export { NativeProcessRunner } from './native-process-runner.ts';

/** pnpm flags that precede the dependency-policy hook; none depends on the runtime. */
const POLICY_FLAGS = [
  '--config.manage-package-manager-versions=false',
  '--config.package-manager-strict=false',
  '--config.auto-install-peers=false',
  '--config.strict-dep-builds=false',
  '--config.verify-store-integrity=true',
  '--config.block-exotic-subdeps=true',
  '--config.package-import-method=copy',
  '--config.side-effects-cache=false',
  '--config.fetch-retries=0',
  '--config.global-pnpmfile=',
] as const;

/** Installer commands run in a confined runtime against one validated HTTP(S) registry. */
export class InstallerProcess {
  readonly #runner: ProcessRunner;
  readonly #runtime: InstallerRuntime;
  readonly #registry: string;

  public constructor(options: { runner: ProcessRunner; runtime: string; registry: string }) {
    this.#runner = options.runner;
    this.#runtime = new InstallerRuntime(options.runtime);
    const registry = new URL(options.registry);
    if (
      !['http:', 'https:'].includes(registry.protocol) ||
      registry.username ||
      registry.password
    ) {
      throw new Error('Registry must be an HTTP(S) URL without credentials');
    }
    this.#registry = registry.href;
  }

  public async git(args: readonly string[], cwd: string): Promise<string> {
    return this.#run({ command: 'git', args, cwd });
  }

  public async install(cwd: string, frozen: boolean): Promise<void> {
    if (!frozen) {
      await this.#runtime.clearCache();
    }
    await this.#run({ command: 'pnpm', cwd, args: this.#installArguments(frozen) });
  }

  async #run(input: Omit<ProcessRequest, 'env'>): Promise<string> {
    return this.#runner.run({ ...input, env: await this.#runtime.prepare() });
  }

  #installArguments(frozen: boolean): readonly string[] {
    return [
      '--ignore-workspace',
      '--registry',
      this.#registry,
      '--store-dir',
      this.#runtime.path('store'),
      `--config.cache-dir=${this.#runtime.path('cache')}`,
      '--state-dir',
      this.#runtime.path('state'),
      ...POLICY_FLAGS,
      `--config.pnpmfile=${this.#runtime.dependencyPolicy}`,
      '--config.ignore-pnpmfile=false',
      '--config.config-dependencies={}',
      'install',
      '--prod',
      '--ignore-scripts',
      frozen ? '--frozen-lockfile' : '--no-frozen-lockfile',
    ];
  }
}
