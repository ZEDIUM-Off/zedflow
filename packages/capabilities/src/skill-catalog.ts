import { readdir } from 'node:fs/promises';
import { join } from 'node:path';
import type { Skill, SkillMetadata } from '@zf/core';
import { SkillFile, validSkillName } from './skill-file.ts';

export class SkillCatalog {
  readonly #roots: readonly string[];

  public constructor(roots: readonly string[]) {
    this.#roots = [...roots];
  }

  public async list(): Promise<Map<string, SkillMetadata>> {
    const skills = new Map<string, SkillMetadata>();
    for (const root of this.#roots) {
      await this.#readRoot(root, skills);
    }
    return skills;
  }

  public async read(name: string): Promise<Skill> {
    if (!validSkillName(name)) {
      throw new Error('Invalid skill name');
    }
    const metadata = (await this.list()).get(name);
    if (!metadata) {
      throw new Error('Unknown skill');
    }
    return new SkillFile(metadata.path, name).read();
  }

  async #readRoot(root: string, skills: Map<string, SkillMetadata>): Promise<void> {
    let entries;
    try {
      entries = await readdir(join(root, 'skills'), { withFileTypes: true });
    } catch (error) {
      if (error instanceof Error && 'code' in error && error.code === 'ENOENT') {
        return;
      }
      throw error;
    }
    for (const entry of entries) {
      if (!entry.isDirectory()) {
        continue;
      }
      const path = join(root, 'skills', entry.name, 'SKILL.md');
      const skill = await new SkillFile(path, entry.name).read();
      skills.set(skill.name, {
        name: skill.name,
        description: skill.description,
        path: skill.path,
      });
    }
  }
}
