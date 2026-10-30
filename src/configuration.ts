import { validId } from './definition.ts';
import type { Definition } from './definition.ts';
export interface CodexConfiguration { version: 1; id: string; name: string; model: string; reasoning: 'low' | 'medium' | 'high'; }
export function validateConfiguration(value: unknown): CodexConfiguration {
 const c = value as CodexConfiguration;
 if (!c || c.version !== 1) throw new Error('Invalid configuration');
 if (Object.keys(c).some(key => !['version', 'id', 'name', 'model', 'reasoning'].includes(key))) throw new Error('Unsupported configuration setting');
 validId(c.id);
 if (typeof c.name !== 'string' || !c.name.trim() || c.name.length > 120) throw new Error('Invalid configuration name');
 if (typeof c.model !== 'string' || !/^[a-zA-Z0-9][a-zA-Z0-9._-]{0,99}$/.test(c.model)) throw new Error('Invalid Codex model');
 if (!['low', 'medium', 'high'].includes(c.reasoning)) throw new Error('Unsupported reasoning setting');
 return structuredClone(c);
}
export function resolveConfigurations(definition: Definition, configurations: CodexConfiguration[]) {
 return Object.fromEntries(definition.blocks.map(block => {
  const config = configurations.find(c => c.id === block.configurationId);
  if (!config) throw new Error(`Choose an available Codex configuration for ${block.name}`);
  return [block.id, validateConfiguration(config)];
 }));
}
