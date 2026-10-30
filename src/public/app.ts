import { mountHome } from './editor.ts';
import type { Definition } from '../definition.ts';
const token = document.querySelector<HTMLMetaElement>('meta[name="workflow-token"]')!.content;
async function request(path: string, method = 'GET', value?: unknown) {
 const response = await fetch(path, { method, headers: { 'x-workflow-token': token, 'content-type': 'application/json' }, body: value ? JSON.stringify(value) : undefined });
 if (!response.ok) throw new Error('Local request failed');
 return response.json();
}
void mountHome(document.querySelector<HTMLElement>('main')!, {
 list: () => request('/api/definitions'),
 save: definition => request(`/api/definitions/${definition.id}`, 'PUT', definition),
 configurations: { list: () => request('/api/configurations'), save: config => request('/api/configurations', 'PUT', config), authentication: () => request('/api/authentication') },
});
