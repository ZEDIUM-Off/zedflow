// Adapted from Pi 1.5.2 coding-agent/src/extensions/codemode/execute.ts (MIT).
// Copyright Mario Zechner and Earendil Works; see ../LICENSE and ../PROVENANCE.json.
import type { ImageContent, TextContent } from '@earendil-works/pi-ai';
import { errorText } from './definition.ts';
import { writeOutputFile } from './output-files.ts';

const extensions: Readonly<Record<string, string>> = {
  'image/png': '.png',
  'image/jpeg': '.jpg',
  'image/gif': '.gif',
  'image/webp': '.webp',
};

/** Owns per-output image deduplication, including cached failure labels. */
export class CodemodeImages {
  readonly #directory: string;
  readonly #saved = new Map<string, string>();

  public constructor(directory: string) {
    this.#directory = directory;
  }

  public async save(
    items: (TextContent | ImageContent)[],
  ): Promise<(TextContent | ImageContent)[]> {
    const output: (TextContent | ImageContent)[] = [];
    for (const item of items) {
      if (item.type !== 'image') {
        output.push(item);
        continue;
      }
      let label = this.#saved.get(item.data);
      if (!label) {
        label = await this.#save(item);
        this.#saved.set(item.data, label);
      }
      output.push({ type: 'text', text: label }, item);
    }
    return output;
  }

  async #save(item: ImageContent): Promise<string> {
    try {
      const extension = extensions[item.mimeType];
      if (!extension) {
        throw new Error(`Unsupported image type: ${item.mimeType}`);
      }
      const path = await writeOutputFile(
        'zf-codemode',
        extension,
        Buffer.from(item.data, 'base64'),
        this.#directory,
      );
      return `[Image saved to ${path} (${item.mimeType})]`;
    } catch (error) {
      return `[Image could not be saved: ${errorText(error)}]`;
    }
  }
}
