import { appendFileSync, mkdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { json } from '@zf/core';
import type { Json, RecordEntry, RecordStore } from '@zf/core';

// One writer and bounded records. Persistence is not effect-safe replay.
export class JsonlRunStore implements RecordStore {
  readonly #directory: string;

  public constructor(directory: string) {
    this.#directory = directory;
    mkdirSync(directory, { recursive: true });
  }

  public append(runId: string, kind: string, data: Json): void {
    const seq = this.read(runId).length + 1;
    appendFileSync(this.#file(runId), JSON.stringify({ seq, kind, data }) + '\n', { mode: 0o600 });
  }

  public read(runId: string): RecordEntry[] {
    let content: string;
    try {
      content = readFileSync(this.#file(runId), 'utf8');
    } catch (error) {
      if (error instanceof Error && 'code' in error && error.code === 'ENOENT') {
        return [];
      }
      throw error;
    }
    if (content && !content.endsWith('\n')) {
      throw new Error('Incomplete run record');
    }
    return content
      .split('\n')
      .filter(Boolean)
      .map((line, index) => this.#decode(line, index + 1));
  }

  #file(runId: string): string {
    if (!/^[a-zA-Z0-9-]+$/.test(runId)) {
      throw new Error('Invalid run ID');
    }
    return join(this.#directory, runId + '.jsonl');
  }

  #decode(line: string, sequence: number): RecordEntry {
    const value: unknown = JSON.parse(line);
    if (
      !value ||
      typeof value !== 'object' ||
      !('seq' in value) ||
      value.seq !== sequence ||
      !('kind' in value) ||
      typeof value.kind !== 'string' ||
      !('data' in value)
    ) {
      throw new Error('Invalid run record');
    }
    return { seq: sequence, kind: value.kind, data: json(value.data) };
  }
}
