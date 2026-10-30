import type { Definition } from '../definition.ts';
import { validateDefinition } from '../definition.ts';
import { mountRunInputs } from './inputs.ts';
import type { Launch } from './inputs.ts';
import { mountConfigurations } from './configurations.ts';
import type { ConfigurationAPI } from './configurations.ts';
import { predecessors } from '../flow.ts';
export interface DefinitionAPI { list(): Promise<Definition[]>; save(value: Definition): Promise<Definition>; launch?: Launch; configurations?: ConfigurationAPI; }
function element<K extends keyof HTMLElementTagNameMap>(root: HTMLElement, tag: K, text = '') {
 const node = root.ownerDocument.createElement(tag); node.textContent = text; return node;
}
export async function mountHome(root: HTMLElement, api: DefinitionAPI) {
 root.replaceChildren(element(root, 'h1', 'Workflow Factory'), element(root, 'p', 'Local workflows. Your prompts, connected.'));
 const create = element(root, 'button', 'New workflow'); create.dataset.create = '';
 create.onclick = () => mountEditor(root, { version: 1, id: crypto.randomUUID(), name: 'Untitled workflow', blocks: [], connections: [] }, api);
 root.append(create);
 if (api.configurations) { const settings = element(root, 'button', 'Codex configurations'); settings.onclick = () => { void mountConfigurations(root, api.configurations!, () => { void mountHome(root, api); }); }; root.append(settings); }
 const list = element(root, 'div'); root.append(list);
 try {
  const definitions = await api.list();
  if (!definitions.length) list.append(element(root, 'p', 'No saved workflows yet. Create your first workflow.'));
  for (const definition of definitions) {
   const button = element(root, 'button', definition.name); button.dataset.workflow = definition.id;
   button.onclick = () => mountEditor(root, definition, api); list.append(button);
  }
 } catch { list.append(element(root, 'p', 'Could not load workflows. Check the server and data folder, then reload.')); }
}
export function mountEditor(root: HTMLElement, definition: Definition, api: DefinitionAPI) {
 const state = structuredClone(definition);
 let savedJSON = JSON.stringify(definition);
 const view = root.ownerDocument.defaultView!;
 const isDirty = () => JSON.stringify(state) !== savedJSON;
 view.onbeforeunload = event => { if (isDirty()) { event.preventDefault(); event.returnValue = ''; } };
 root.innerHTML = `<header><button data-home>← Workflows</button><input aria-label="Workflow name" maxlength="120"><button data-save>Save workflow</button><span role="status" aria-live="polite">Not saved this session</span></header>
 <aside><button data-add draggable="true">＋ AI block</button><p>Drag onto the canvas, or click to add. Move blocks by their handle; arrow keys move a focused handle.</p></aside>
 <section><h2>Workflow inputs</h2><div data-inputs></div><button data-add-input>Add text input</button><button data-run>Run inputs…</button><div data-run-form></div></section>
 <form data-connect><label>From <select aria-label="From block"></select></label><span> → </span><label>To <select aria-label="To block"></select></label><button>Connect blocks</button></form><ul class="edges"></ul>
 <div class="viewport"><div class="canvas" aria-label="Workflow canvas"><svg aria-label="Connections"></svg><div class="nodes"></div></div></div>`;
 const status = root.querySelector<HTMLElement>('[role="status"]')!;
 const save = root.querySelector<HTMLButtonElement>('[data-save]')!;
 const changed = () => { status.textContent = 'Unsaved changes'; root.querySelector('[data-run-form]')!.replaceChildren(); };
 function renderInputs() {
  const list = root.querySelector('[data-inputs]')!; list.replaceChildren();
  for (const input of state.inputs ?? []) {
   const row = element(root, 'div');
   const name = element(root, 'input'); name.value = input.name; name.setAttribute('aria-label', 'Input name'); name.maxLength = 80;
   name.oninput = () => { input.name = name.value; changed(); refreshPickers(); };
   const required = element(root, 'input'); required.type = 'checkbox'; required.checked = input.required;
   const label = element(root, 'label', 'Required'); label.append(required);
   required.onchange = () => { input.required = required.checked; changed(); };
   const defaults = element(root, 'textarea'); defaults.value = input.default ?? ''; defaults.maxLength = 100000; defaults.setAttribute('aria-label', 'Default text');
   defaults.oninput = () => { input.default = defaults.value; changed(); };
   const remove = element(root, 'button', 'Remove input'); remove.onclick = () => { state.inputs = state.inputs!.filter(i => i !== input); changed(); renderInputs(); refreshPickers(); };
   row.append(name, label, defaults, remove); list.append(row);
  }
 }
 root.querySelector<HTMLButtonElement>('[data-add-input]')!.onclick = () => {
  state.inputs ??= []; if (state.inputs.length >= 50) { status.textContent = 'Maximum 50 text inputs'; return; }
  let index = 1; while (state.inputs.some(i => i.name === `input${index}`)) index++;
  state.inputs.push({ name: `input${index}`, type: 'text', required: false }); changed(); renderInputs(); refreshPickers();
 };
 root.querySelector<HTMLButtonElement>('[data-run]')!.onclick = () => mountRunInputs(root.querySelector<HTMLElement>('[data-run-form]')!, state, api.launch);
 save.onclick = async () => {
  save.disabled = true; status.textContent = 'Saving…';
  try {
   const snapshot = validateDefinition(state);
   await api.save(snapshot); savedJSON = JSON.stringify(snapshot);
   status.textContent = isDirty() ? 'Saved earlier changes; newer changes are unsaved' : 'Saved to disk';
  }
  catch (error) { status.textContent = `Save failed. ${error instanceof Error ? error.message : 'Check properties/server and retry.'} Your changes are still here.`; }
  finally { save.disabled = false; }
 };
 const canvas = root.querySelector<HTMLElement>('.canvas')!;
 const nodes = root.querySelector<HTMLElement>('.nodes')!;
 const workflowName = root.querySelector<HTMLInputElement>('input')!;
 workflowName.value = state.name;
 workflowName.oninput = () => { state.name = workflowName.value; status.textContent = 'Unsaved changes'; };
 root.querySelector<HTMLButtonElement>('[data-home]')!.onclick = () => {
  if (save.disabled || (isDirty() && !view.confirm('Discard unsaved workflow changes?'))) return;
  view.onbeforeunload = null; void mountHome(root, api);
 };
 function drawEdges() {
  const svg = root.querySelector('svg')!;
  svg.innerHTML = '<defs><marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="8" markerHeight="8" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" /></marker></defs>';
  const list = root.querySelector<HTMLElement>('.edges')!; list.replaceChildren();
  for (const edge of state.connections) {
   const from = state.blocks.find(b => b.id === edge.from)!, to = state.blocks.find(b => b.id === edge.to)!;
   const path = root.ownerDocument.createElementNS('http://www.w3.org/2000/svg', 'path');
   const x = from.position.x + 260, y = from.position.y + 95, end = to.position.x, bottom = to.position.y + 95;
   path.setAttribute('d', `M ${x} ${y} C ${x+70} ${y}, ${end-70} ${bottom}, ${end} ${bottom}`);
   path.setAttribute('marker-end', 'url(#arrow)'); path.dataset.edge = `${edge.from}:${edge.to}`; svg.append(path);
   const item = element(root, 'li', `${from.name} → ${to.name} `);
   const remove = element(root, 'button', 'Remove connection'); remove.onclick = () => { state.connections = state.connections.filter(e => e !== edge); drawEdges(); status.textContent = 'Unsaved changes'; };
   item.append(remove); list.append(item);
  }
  refreshPickers();
 }
 root.querySelector<HTMLFormElement>('[data-connect]')!.onsubmit = event => {
  event.preventDefault();
  const from = root.querySelector<HTMLSelectElement>('[aria-label="From block"]')!.value;
  const to = root.querySelector<HTMLSelectElement>('[aria-label="To block"]')!.value;
  try { const next = validateDefinition({ ...state, connections: [...state.connections, { from, to }] }); state.connections = next.connections; drawEdges(); status.textContent = 'Unsaved changes'; }
  catch (error) { status.textContent = error instanceof Error ? error.message : 'Invalid connection'; }
 };
 function refreshPickers() {
  for (const picker of root.querySelectorAll<HTMLSelectElement>('[data-reference]')) {
   picker.replaceChildren(element(root, 'option', 'Insert input reference…'));
   picker.options[0]!.value = '';
   for (const input of state.inputs ?? []) { const option = element(root, 'option', input.name); option.value = input.name; picker.append(option); }
   const id = picker.closest<HTMLElement>('.block')?.dataset.id;
   for (const block of predecessors(state, id ?? '')) { const option = element(root, 'option', `${block.name} → final response`); option.value = `blocks.${block.id}.final`; picker.append(option); }
  }
 }
 function render() {
  for (const select of root.querySelectorAll<HTMLSelectElement>('[data-connect] select')) {
   select.replaceChildren();
   for (const block of state.blocks) { const option = element(root, 'option', block.name); option.value = block.id; select.append(option); }
  }
  drawEdges();
  nodes.replaceChildren();
  for (const block of state.blocks) {
   const card = element(root, 'section'); card.className = 'block'; card.dataset.id = block.id;
   card.style.left = `${block.position.x}px`; card.style.top = `${block.position.y}px`;
   const handle = element(root, 'button', '⠿ Move AI block'); handle.className = 'handle';
   const name = element(root, 'input'); name.value = block.name; name.maxLength = 120; name.setAttribute('aria-label', 'Block name');
   const prompt = element(root, 'textarea'); prompt.value = block.prompt; prompt.maxLength = 100000; prompt.setAttribute('aria-label', `Prompt for ${block.name}`);
   name.oninput = () => {
    block.name = name.value; status.textContent = 'Unsaved changes';
    prompt.setAttribute('aria-label', `Prompt for ${block.name}`);
    for (const option of root.querySelectorAll<HTMLOptionElement>('[data-connect] select option')) if (option.value === block.id) option.textContent = block.name;
    drawEdges();
   };
   prompt.oninput = () => { block.prompt = prompt.value; status.textContent = 'Unsaved changes'; };
   const configPicker = element(root, 'select'); configPicker.setAttribute('aria-label', `Codex configuration for ${block.name}`);
   const empty = element(root, 'option', block.configurationId ? `Unavailable configuration: ${block.configurationId}` : 'Choose Codex configuration…'); empty.value = block.configurationId ?? ''; configPicker.append(empty);
   if (api.configurations) void api.configurations.list().then(configs => {
    if (!configPicker.isConnected) return;
    for (const config of configs) { const option = element(root, 'option', config.name); option.value = config.id; configPicker.append(option); }
    if (configs.some(c => c.id === block.configurationId)) { empty.value = ''; empty.textContent = 'Choose Codex configuration…'; }
    configPicker.value = block.configurationId ?? '';
   }).catch(() => { status.textContent = 'Could not load Codex configurations'; });
   configPicker.onchange = () => { if (configPicker.value) block.configurationId = configPicker.value; else delete block.configurationId; changed(); };
   const picker = element(root, 'select'); picker.dataset.reference = ''; picker.setAttribute('aria-label', `Prompt reference for ${block.name}`);
   picker.onchange = () => {
    if (!picker.value) return;
    const reference = picker.value.startsWith('blocks.') ? `{{${picker.value}}}` : `{{inputs.${picker.value}}}`;
    prompt.setRangeText(reference, prompt.selectionStart, prompt.selectionEnd, 'end');
    block.prompt = prompt.value; picker.value = ''; changed(); prompt.focus();
   };
   function move(x: number, y: number) {
    block.position = { x: Math.max(0, Math.min(3000, x)), y: Math.max(0, Math.min(2000, y)) };
    card.style.left = `${block.position.x}px`; card.style.top = `${block.position.y}px`; drawEdges(); status.textContent = 'Unsaved changes';
   }
   let drag: { x: number; y: number; left: number; top: number } | undefined;
   handle.onpointerdown = event => {
    if (event.button !== 0) return;
    drag = { x: event.clientX, y: event.clientY, left: block.position.x, top: block.position.y };
    handle.setPointerCapture?.(event.pointerId); event.preventDefault();
   };
   handle.onpointermove = event => { if (drag) move(drag.left + event.clientX - drag.x, drag.top + event.clientY - drag.y); };
   handle.onpointerup = handle.onpointercancel = () => { drag = undefined; };
   handle.onkeydown = event => {
    const delta: Record<string, [number, number]> = { ArrowLeft: [-10, 0], ArrowRight: [10, 0], ArrowUp: [0, -10], ArrowDown: [0, 10] };
    const offset = delta[event.key]; if (offset) { event.preventDefault(); move(block.position.x + offset[0], block.position.y + offset[1]); }
   };
   const remove = element(root, 'button', 'Remove block'); remove.dataset.removeBlock = '';
   remove.onclick = () => { state.blocks = state.blocks.filter(b => b.id !== block.id); state.connections = state.connections.filter(e => e.from !== block.id && e.to !== block.id); render(); status.textContent = 'Unsaved changes'; };
   card.append(handle, name, prompt, configPicker, picker, remove); nodes.append(card);
  }
  refreshPickers();
 }
 function add(x: number, y: number) {
  if (state.blocks.length >= 200) { status.textContent = 'Maximum 200 blocks per definition'; return; }
  status.textContent = 'Unsaved changes';
  state.blocks.push({ id: crypto.randomUUID(), type: 'ai', name: `AI block ${state.blocks.length + 1}`, prompt: '', position: { x: Math.max(0, Math.min(3000, x)), y: Math.max(0, Math.min(2000, y)) } }); render();
 }
 const palette = root.querySelector<HTMLButtonElement>('[data-add]')!;
 palette.ondragstart = event => event.dataTransfer?.setData('application/workflow-block', 'ai');
 palette.onclick = () => add(40 + state.blocks.length * 30, 40 + state.blocks.length * 30);
 canvas.ondragover = event => { event.preventDefault(); };
 canvas.ondrop = event => {
  event.preventDefault(); if (event.dataTransfer?.getData('application/workflow-block') !== 'ai') return;
  const box = canvas.getBoundingClientRect(); add(event.clientX - box.left, event.clientY - box.top);
 };
 renderInputs(); render();
 return { snapshot: () => structuredClone(state) };
}
