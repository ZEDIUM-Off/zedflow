import { mkdir, mkdtemp } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { execute } from './process.ts';

export const isolatedAuth = { env: async () => undefined, fileExists: async () => false };

export class CliFixture {
  readonly #cwd: string;
  readonly #home: string;

  public constructor(cwd: string, home: string) {
    this.#cwd = cwd;
    this.#home = home;
  }

  public get cwd(): string {
    return this.#cwd;
  }
  public get home(): string {
    return this.#home;
  }
  public get globalDirectory(): string {
    return join(this.#home, '.zedflow');
  }
  public get flags(): string[] {
    return ['--cwd', this.#cwd, '--global-dir', this.globalDirectory];
  }

  public async run(
    args: readonly string[],
  ): Promise<{ code: number; stdout: string; stderr: string }> {
    try {
      const result = await execute(
        process.execPath,
        [resolve('packages/cli/bin/zf.ts'), ...this.flags, ...args],
        {
          env: {
            PATH: process.env['PATH'],
            HOME: this.#home,
            USERPROFILE: this.#home,
            TMPDIR: this.#cwd,
          },
          timeout: 15000,
        },
      );
      return { code: 0, ...result };
    } catch (error) {
      if (error instanceof Error && 'stdout' in error && 'stderr' in error) {
        return { code: 1, stdout: String(error.stdout), stderr: String(error.stderr) };
      }
      throw error;
    }
  }
}

export const fixture = async (): Promise<CliFixture> => {
  const root = resolve('.artifacts/cli-goal/cli-tests');
  await mkdir(root, { recursive: true });
  const cwd = await mkdtemp(join(root, 'case-'));
  const home = join(cwd, 'home');
  await mkdir(home);
  return new CliFixture(cwd, home);
};
