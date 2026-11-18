import { lstat, readFile } from 'node:fs/promises';
import { parse } from 'yaml';
import type { Skill } from '@zf/core';

export const validSkillName = (name: string): boolean =>
  name.length <= 64 && /^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(name);

const invalidMetadata = 'Invalid skill metadata';

/**
 * One `SKILL.md` file: a regular file of at most 64 KiB whose YAML front matter names the skill
 * after its directory and carries a description of at most 1024 characters.
 */
export class SkillFile {
  readonly #path: string;
  readonly #expectedName: string;

  public constructor(path: string, expectedName: string) {
    this.#path = path;
    this.#expectedName = expectedName;
  }

  public async read(): Promise<Skill> {
    const stat = await lstat(this.#path);
    if (!stat.isFile() || stat.size > 65536) {
      throw new Error('Skills must be regular files of at most 64 KiB');
    }
    const source = await readFile(this.#path, 'utf8');
    const match = /^---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)/.exec(source);
    if (!match?.[1]) {
      throw new Error('Missing skill frontmatter');
    }
    const metadata: unknown = parse(match[1], { maxAliasCount: 0 });
    if (!metadata || typeof metadata !== 'object') {
      throw new Error(invalidMetadata);
    }
    return {
      name: this.#name(metadata),
      description: this.#description(metadata),
      path: this.#path,
      content: source.slice(match[0].length),
    };
  }

  #name(metadata: object): string {
    if (
      !('name' in metadata) ||
      typeof metadata.name !== 'string' ||
      metadata.name !== this.#expectedName ||
      !validSkillName(metadata.name)
    ) {
      throw new Error(invalidMetadata);
    }
    return metadata.name;
  }

  #description(metadata: object): string {
    if (
      !('description' in metadata) ||
      typeof metadata.description !== 'string' ||
      !metadata.description.trim() ||
      metadata.description.length > 1024
    ) {
      throw new Error(invalidMetadata);
    }
    return metadata.description;
  }
}
