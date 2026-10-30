import { resolveInputs } from '../definition.ts';
import type { Definition } from '../definition.ts';
export type Launch = (definition: Definition, inputs: Record<string, string>) => Promise<void>;
export function mountRunInputs(root: HTMLElement, definition: Definition, launch?: Launch) {
 const document = root.ownerDocument;
 const form = document.createElement('form'); form.dataset.runInputs = ''; form.noValidate = true;
 const fields = new Map<string, HTMLTextAreaElement>();
 for (const input of definition.inputs ?? []) {
  const label = document.createElement('label'); label.textContent = `${input.name}${input.required ? ' (required)' : ' (optional)'}`;
  const field = document.createElement('textarea'); field.name = input.name; field.value = input.default ?? ''; field.maxLength = 100000; field.required = input.required;
  fields.set(input.name, field); label.append(field); form.append(label);
 }
 const submit = document.createElement('button'); submit.textContent = launch ? 'Launch run' : 'Validate run inputs';
 const message = document.createElement('p'); message.setAttribute('role', 'status');
 form.append(submit, message); root.replaceChildren(form);
 form.onsubmit = async event => {
  event.preventDefault(); submit.disabled = true;
  try {
   const snapshot = structuredClone(definition);
   const resolved = resolveInputs(snapshot, Object.fromEntries([...fields].map(([name, field]) => [name, field.value])));
   if (launch) { await launch(snapshot, resolved.inputs); message.textContent = 'Run accepted'; }
   else message.textContent = 'Inputs valid. Execution is not available in this increment; no run was started.';
  } catch (error) { message.textContent = error instanceof Error ? error.message : 'Could not accept run'; }
  finally { submit.disabled = false; }
 };
 return form;
}
