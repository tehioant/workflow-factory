import * as fs from 'node:fs/promises';
import { join } from 'node:path';
import { randomUUID } from 'node:crypto';
import { validateDefinition, validId } from './definition.ts';
import type { Definition } from './definition.ts';
export type Disk = Pick<typeof fs, 'mkdir' | 'readFile' | 'readdir' | 'open' | 'rename' | 'unlink'>;
export class DefinitionStore {
  private directory: string;
  private disk: Disk;
  constructor(directory: string, disk: Disk = fs) { this.directory = directory; this.disk = disk; }
  async list(): Promise<Definition[]> {
    let entries: string[];
    try { entries = await this.disk.readdir(this.directory); }
    catch (error) { if ((error as NodeJS.ErrnoException).code === 'ENOENT') return []; throw error; }
    const definitions = await Promise.all(entries.filter(name => name.endsWith('.json')).map(name => this.get(name.slice(0, -5))));
    return definitions.sort((a, b) => a.name.localeCompare(b.name));
  }
  async get(id: string): Promise<Definition> {
    validId(id);
    const value = validateDefinition(JSON.parse(await this.disk.readFile(join(this.directory, `${id}.json`), 'utf8')));
    if (value.id !== id) throw new Error('Definition identifier mismatch');
    return value;
  }
  async save(value: unknown): Promise<Definition> {
    const definition = validateDefinition(value);
    await this.disk.mkdir(this.directory, { recursive: true, mode: 0o700 });
    const temporary = join(this.directory, `.${randomUUID()}.tmp`);
    try {
      const handle = await this.disk.open(temporary, 'wx', 0o600);
      try { await handle.writeFile(JSON.stringify(definition)); await handle.sync(); }
      finally { await handle.close(); }
      await this.disk.rename(temporary, join(this.directory, `${definition.id}.json`));
    } finally { await this.disk.unlink(temporary).catch(() => {}); }
    return definition;
  }
}
