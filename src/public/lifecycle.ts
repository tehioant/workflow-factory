export interface QuitActions {
 pending(): boolean;
 status(): Promise<{ pending: number }>;
 confirm(message: string): boolean;
 quit(confirmed: boolean): Promise<void>;
 closed(): void;
}
export function mountQuit(root: HTMLElement, actions: QuitActions) {
 const document = root.ownerDocument;
 const button = document.createElement('button'); button.textContent = 'Quit application';
 const message = document.createElement('p'); message.setAttribute('role','status'); message.textContent = 'Save definition edits before quitting. Closing this tab alone does not stop the server.';
 root.replaceChildren(button,message);
 const warn = (event: BeforeUnloadEvent) => { if (actions.pending()) { event.preventDefault(); event.returnValue = ''; } };
 document.defaultView?.addEventListener('beforeunload',warn);
 button.onclick = async () => {
  button.disabled = true;
  try {
   const status = await actions.status();
   const confirmed = status.pending > 0;
   if (confirmed && !actions.confirm(`${status.pending} pending runs or cleanup/storage holds. Quit will stop active runs and cancel queued runs. OK quits; Cancel keeps running.`)) { message.textContent = 'Kept running. No work was cancelled.'; return; }
   message.textContent = 'Stopping owned work and saving terminal history…';
   await actions.quit(confirmed); actions.closed();
   document.defaultView?.removeEventListener('beforeunload',warn);
   message.textContent = 'Application stopped. Close this tab.'; button.remove();
  } catch (error) { message.textContent = error instanceof Error ? error.message : 'Quit incomplete. Check Docker/storage, resolve retained holds, and retry.'; }
  finally { button.disabled = false; }
 };
 return () => document.defaultView?.removeEventListener('beforeunload',warn);
}
