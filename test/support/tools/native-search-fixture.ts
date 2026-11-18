import { execFile } from 'node:child_process';
import { mkdir, readFile, realpath, rm, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';
import { fixture } from './invocation.ts';

export interface NativeBinaries {
  readonly rg: string;
  readonly fd: string;
}
// Fixture dependencies only: production selection is exercised separately against public Pi.
export const nativeSearchBinary = async (name: 'rg' | 'fd'): Promise<string> => {
  const located = await promisify(execFile)('which', [name]).catch(() =>
    promisify(execFile)(
      process.execPath,
      [
        '-e',
        `import('@earendil-works/pi-coding-agent').then(pi => console.log(pi.getAgentDir() + '/bin/${name}'))`,
      ],
      { cwd: resolve('packages/tools') },
    ),
  );
  return realpath(located.stdout.trim());
};

export const quoteShell = (value: string): string => `'${value.replaceAll("'", "'\\''")}'`;

/** Owns private executable fixtures and their actual (not version-only) invocation markers. */
export class NativeSearchFixture {
  readonly #real: NativeBinaries;
  public readonly root: string;
  public readonly cwd: string;
  public readonly agent: string;
  public readonly tmp: string;
  public readonly path: string;
  public readonly bin: string;
  public readonly blockingResource: string;

  public constructor(root: string, real: NativeBinaries) {
    this.#real = real;
    this.root = root;
    this.cwd = join(root, 'files');
    this.agent = join(root, 'agent');
    this.tmp = join(root, 'tmp');
    this.path = join(root, 'path');
    this.bin = join(this.agent, 'bin');
    this.blockingResource = join(root, 'blocking-search');
  }

  public async initialize(): Promise<void> {
    await Promise.all([this.cwd, this.agent, this.tmp, this.path].map((dir) => mkdir(dir)));
    await mkdir(join(this.agent, 'bin'));
    await writeFile(join(this.cwd, 'notes.txt'), 'before\nneedle\nafter\nneedle\n');
    await writeFile(this.blockingResource, '');
  }

  public environment(): NodeJS.ProcessEnv {
    return { ...process.env, PATH: this.path, PI_CODING_AGENT_DIR: this.agent, TMPDIR: this.tmp };
  }

  public async install(folder: string, label: string, alias = false): Promise<void> {
    await mkdir(folder, { recursive: true });
    for (const name of ['rg', 'fd'] as const) {
      const target = join(folder, alias && name === 'fd' ? 'fdfind' : name);
      await writeFile(target, this.#script(name, label), { mode: 0o700 });
    }
  }

  public async broken(folder: string, body: string): Promise<void> {
    await mkdir(folder, { recursive: true });
    await Promise.all(
      ['rg', 'fd', 'fdfind'].map((name) =>
        writeFile(join(folder, name), `#!/bin/sh\n${body}\n`, { mode: 0o700 }),
      ),
    );
  }

  public async launches(): Promise<string[]> {
    return (await readFile(join(this.root, 'launched'), 'utf8').catch(() => ''))
      .trim()
      .split('\n')
      .filter(Boolean);
  }

  /** Releases only this fixture's lifetime resource, even after its child or host has exited. */
  public async releaseBlockingSearch(): Promise<void> {
    await rm(this.blockingResource, { force: true });
  }

  #script(name: keyof NativeBinaries, label: string): string {
    return `#!/bin/sh
if [ "$1" = --version ]; then exec ${quoteShell(this.#real[name])} "$@"; fi
printf '%s\\n' ${quoteShell(`${label}:${name}`)} >> ${quoteShell(join(this.root, 'launched'))}
case "$*" in *BLOCK_MARKER*) exec ${quoteShell(process.execPath)} ${quoteShell(fileURLToPath(new URL('./native-search-blocker.ts', import.meta.url)))} ${quoteShell(join(this.root, `${name}.pid`))} ${quoteShell(this.blockingResource)} ;; esac
exec ${quoteShell(this.#real[name])} "$@"
`;
  }
}

export const nativeSearchFixture = async (real: NativeBinaries): Promise<NativeSearchFixture> => {
  const owned = new NativeSearchFixture(await fixture(), real);
  await owned.initialize();
  return owned;
};
