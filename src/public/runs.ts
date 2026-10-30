import type { Run } from '../runs.ts';
export function renderRun(root: HTMLElement, run: Run) {
 const document = root.ownerDocument;
 const title = document.createElement('h2'); title.textContent = `${run.name} — ${run.status}`;
 const id = document.createElement('p'); id.textContent = `Run ${run.id}`;
 root.replaceChildren(title, id);
 if (run.error) { const error = document.createElement('p'); error.setAttribute('role', 'alert'); error.textContent = run.error; root.append(error); }
 for (const [blockId, result] of Object.entries(run.blocks)) {
  const section = document.createElement('section');
  const heading = document.createElement('h3'); heading.textContent = `${run.definition.blocks.find(b => b.id === blockId)?.name ?? blockId} — ${result.status}`;
  const output = document.createElement('pre'); output.dataset.output = ''; output.textContent = result.output ?? '';
  const logs = document.createElement('pre'); logs.dataset.logs = ''; logs.textContent = result.logs;
  section.append(heading, output, logs);
  if (result.error) { const error = document.createElement('p'); error.textContent = result.error; section.append(error); }
  root.append(section);
 }
}
