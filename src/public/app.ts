import { mountHome } from './editor.ts';
import { renderRun } from './runs.ts';
import type { Run } from '../runs.ts';
const token = document.querySelector<HTMLMetaElement>('meta[name="workflow-token"]')!.content;
async function request(path: string, method = 'GET', value?: unknown) {
 const response = await fetch(path, { method, headers: { 'x-workflow-token': token, 'content-type': 'application/json' }, body: value ? JSON.stringify(value) : undefined });
 const result = await response.json();
 if (!response.ok) throw new Error(result.error || 'Local request failed');
 return result;
}
const runArea = document.createElement('aside'); const runList = document.createElement('div'); const selectedRun = document.createElement('div');
const workflowFilter = document.createElement('select'); workflowFilter.setAttribute('aria-label', 'Run history workflow');
workflowFilter.onchange = () => { selected = undefined; selectedRun.replaceChildren(); };
runArea.append(workflowFilter, runList, selectedRun); document.body.append(runArea);
let selected: string | undefined;
function showRun(run: Run) { renderRun(selectedRun,run,{ recheckCleanup: id => request(`/api/runs/${id}/recheck-cleanup`,'POST',{}) }); }
let timer: ReturnType<typeof setTimeout>;
async function refreshRuns() {
 try {
  const runs: Run[] = await request('/api/runs'); runList.replaceChildren();
  const filter = workflowFilter.value;
  workflowFilter.replaceChildren(new Option('All workflow runs',''));
  for (const [id,name] of new Map(runs.map(run => [run.definition.id,run.name]))) workflowFilter.append(new Option(name,id));
  workflowFilter.value = filter;
  for (const run of runs.filter(run => !filter || run.definition.id === filter).reverse()) { const button = document.createElement('button'); button.textContent = `${run.name} — ${run.createdAt} — ${run.status}`; button.onclick = () => { selected = run.id; showRun(run); }; runList.append(button); }
  const run = runs.find(run => run.id === selected); if (run) showRun(run);
 } catch { runList.textContent = 'Could not refresh runs. Check the local server.'; }
 timer = setTimeout(() => { void refreshRuns(); }, 1500);
}
window.addEventListener('pagehide', () => clearTimeout(timer));
void refreshRuns();
void mountHome(document.querySelector<HTMLElement>('main')!, {
 list: () => request('/api/definitions'),
 save: definition => request(`/api/definitions/${definition.id}`, 'PUT', definition),
 configurations: { list: () => request('/api/configurations'), save: config => request('/api/configurations', 'PUT', config), authentication: () => request('/api/authentication') },
 launch: async (definition, inputs) => { const run: Run = await request('/api/runs', 'POST', { definition, inputs }); selected = run.id; showRun(run); },
});
