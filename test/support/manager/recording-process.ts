import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import type { ProcessRequest, ProcessRunner } from '../../../packages/manager/src/index.ts';
const execute = promisify(execFile);
export class RecordingProcess implements ProcessRunner {
  readonly #requests: ProcessRequest[] = [];
  #intercept: ((request: ProcessRequest) => Promise<void>) | undefined;

  public requests(): readonly ProcessRequest[] {
    return this.#requests;
  }
  public intercept(callback: (request: ProcessRequest) => Promise<void>): void {
    this.#intercept = callback;
  }
  public async run(request: ProcessRequest): Promise<string> {
    this.#requests.push(request);
    await this.#intercept?.(request);
    const result = await execute(request.command, [...request.args], {
      cwd: request.cwd,
      env: request.env,
      shell: false,
      timeout: 30000,
      maxBuffer: 16 * 1024 * 1024,
    });
    return result.stdout;
  }
}
