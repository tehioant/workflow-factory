import { validateConfiguration } from '../configuration.ts';
import type { CodexConfiguration } from '../configuration.ts';
export interface ConfigurationAPI { list(): Promise<CodexConfiguration[]>; save(value: CodexConfiguration): Promise<CodexConfiguration>; authentication(): Promise<{ available: boolean; message: string }>; }
export async function mountConfigurations(root: HTMLElement, api: ConfigurationAPI, back: () => void) {
 root.innerHTML = '<h1>Codex configurations</h1><button data-back>← Workflows</button><p data-auth></p><div data-list></div><button data-new>New configuration</button><form><label>Name <input name="name" maxlength="120" required></label><label>Model <input name="model" maxlength="100" required placeholder="Your subscription-supported model"></label><label>Reasoning <select name="reasoning"><option>low</option><option selected>medium</option><option>high</option></select></label><button>Save configuration</button></form><p role="status"></p>';
 root.querySelector<HTMLButtonElement>('[data-back]')!.onclick = back;
 const form = root.querySelector('form')!;
 const name = form.querySelector<HTMLInputElement>('[name="name"]')!, model = form.querySelector<HTMLInputElement>('[name="model"]')!, reasoning = form.querySelector<HTMLSelectElement>('[name="reasoning"]')!;
 let id: string = crypto.randomUUID();
 root.querySelector<HTMLButtonElement>('[data-new]')!.onclick = () => { id = crypto.randomUUID(); form.reset(); };
 const status = root.querySelector('[role="status"]')!;
 async function refresh() {
  const list = root.querySelector('[data-list]')!; list.replaceChildren();
  for (const config of await api.list()) {
   const button = root.ownerDocument.createElement('button'); button.textContent = config.name;
   button.onclick = () => { id = config.id; name.value = config.name; model.value = config.model; reasoning.value = config.reasoning; };
   list.append(button);
  }
 }
 form.onsubmit = async event => {
  event.preventDefault(); const button = form.querySelector('button')!; button.disabled = true;
  try { await api.save(validateConfiguration({ version: 1, id, name: name.value, model: model.value, reasoning: reasoning.value })); status.textContent = 'Configuration saved'; await refresh(); }
  catch { status.textContent = 'Could not save. Check model/name/settings and the local data folder.'; }
  finally { button.disabled = false; }
 };
 try { const auth = await api.authentication(); root.querySelector('[data-auth]')!.textContent = auth.message; await refresh(); }
 catch { status.textContent = 'Could not load configurations/authentication status. Reload and check the local server.'; }
}
