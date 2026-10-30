import type { Run } from '../runs.ts';
export function renderRun(root: HTMLElement, run: Run, actions?: { recheckCleanup?(id: string): Promise<void>; cancel?(id: string): Promise<void> }) {
 const document = root.ownerDocument;
 const expanded = root.dataset.runId === run.id && root.querySelector('details')?.open;
 root.dataset.runId = run.id;
 const title = document.createElement('h2'); title.textContent = `${run.name} — ${run.status}`;
 const id = document.createElement('p'); id.textContent = `Run ${run.id}`;
 root.replaceChildren(title, id);
 const captured = document.createElement('details'); const summary = document.createElement('summary'); summary.textContent = 'Captured definition, inputs, prompts and settings';
 captured.open = expanded === true;
 const snapshot = document.createElement('pre'); snapshot.textContent = JSON.stringify({ createdAt: run.createdAt, definition: run.definition, inputs: run.inputs, prompts: run.prompts, configurations: run.configurations }, null, 2);
 captured.append(summary,snapshot); root.append(captured);
 if (run.error) { const error = document.createElement('p'); error.setAttribute('role', 'alert'); error.textContent = run.error; root.append(error); }
 if (['queued','running'].includes(run.status) && actions?.cancel) {
  const button = document.createElement('button'); button.dataset.cancelRun = ''; button.textContent = run.cancelRequested ? 'Cancelling…' : 'Cancel run'; button.disabled = run.cancelRequested === true;
  button.onclick = async () => { button.disabled = true; try { await actions.cancel!(run.id); button.textContent = 'Cancelled'; } catch (error) { button.textContent = error instanceof Error ? error.message : 'Cancellation failed'; button.disabled = false; } };
  root.append(button);
 }
 if (run.cleanupPending && actions?.recheckCleanup && !['queued','running'].includes(run.status)) {
  const button = document.createElement('button'); button.textContent = 'Recheck cleanup';
  button.onclick = async () => { button.disabled = true; try { await actions.recheckCleanup!(run.id); button.textContent = 'Cleanup verified'; } catch (error) { button.textContent = error instanceof Error ? error.message : 'Cleanup could not be verified'; } finally { button.disabled = false; } };
  root.append(button);
 }
 for (const [blockId, result] of Object.entries(run.blocks)) {
  const section = document.createElement('section');
  const heading = document.createElement('h3'); heading.textContent = `${run.definition.blocks.find(b => b.id === blockId)?.name ?? blockId} — ${result.status}`;
  const output = document.createElement('pre'); output.dataset.output = ''; output.textContent = result.output ?? '';
  const logs = document.createElement('pre'); logs.dataset.logs = ''; logs.textContent = result.logs;
  section.append(heading, output, logs);
  const prompt = document.createElement('pre'); prompt.textContent = result.prompt; prompt.dataset.prompt = ''; section.append(prompt);
  if (result.error) { const error = document.createElement('p'); error.textContent = result.error; section.append(error); }
  root.append(section);
 }
}
