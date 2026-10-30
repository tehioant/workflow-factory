import { validateDefinition, validId } from './definition.ts';
import type { Definition } from './definition.ts';
import { validateConfiguration } from './configuration.ts';
import type { CodexConfiguration } from './configuration.ts';
import type { Run } from './runs.ts';
export interface Authority { host: string; token: string; }
export interface DefinitionRepository { list(): Promise<Definition[]>; get(id: string): Promise<Definition>; save(value: unknown): Promise<Definition>; }
export function authorize(request: Request, authority: Authority): boolean {
 const origin = request.headers.get('origin');
 return request.headers.get('host') === authority.host
  && (!origin || origin === `http://${authority.host}`)
  && request.headers.get('x-workflow-token') === authority.token
  && (!request.headers.has('sec-fetch-site') || request.headers.get('sec-fetch-site') === 'same-origin');
}
export interface ConfigurationRepository { list(): Promise<CodexConfiguration[]>; save(value: unknown): Promise<CodexConfiguration>; }
export interface RunAPI { list(): Run[]; get(id: string): Run; launch(definition: Definition, inputs: Record<string,string>, configs: CodexConfiguration[]): Promise<Run>; reconcileCleanup(id: string): Promise<void>; cancel(id: string): Promise<Run>; }
export interface Services { configurations?: ConfigurationRepository; authentication?: () => Promise<{ available: boolean; message: string }>; runs?: RunAPI; settings?: { concurrency: number }; lifecycle?: { status(): { pending: number }; quit(): Promise<void> }; }
export async function handleRequest(request: Request, authority: Authority, store: DefinitionRepository, services: Services = {}): Promise<Response> {
 if (!authorize(request, authority)) return Response.json({ error: 'Local session authorization required. Reload the app.' }, { status: 403 });
 const path = new URL(request.url).pathname;
 try {
  if (path === '/api/lifecycle' && request.method === 'GET' && services.lifecycle) return Response.json(services.lifecycle.status());
  if (path === '/api/quit' && request.method === 'POST' && services.lifecycle) {
   if (request.headers.get('origin') !== `http://${authority.host}` || request.headers.get('content-type') !== 'application/json') return Response.json({ error: 'Same-origin JSON required' }, { status: 403 });
   let body;
   try { body = await request.json(); if (typeof body?.confirmed !== 'boolean') throw new Error(); }
   catch { return Response.json({error:'Quit requires JSON with a boolean confirmed field.'},{status:400}); }
   try {
    if (services.lifecycle.status().pending > 0 && body?.confirmed !== true) return Response.json({error:'Runs are now pending. Retry Quit application and confirm cancellation.'},{status:409});
    await services.lifecycle.quit(); return Response.json({closed:true});
   } catch { return Response.json({error:'Quit incomplete. Admission is frozen; check Docker/storage, resolve retained holds and retry.'},{status:503}); }
  }
  if (path === '/api/runtime-settings' && request.method === 'GET' && services.settings) return Response.json(services.settings);
  if (services.runs) {
   const cancelMatch = /^\/api\/runs\/([a-zA-Z0-9_-]{1,80})\/cancel$/.exec(path);
   if (cancelMatch && request.method === 'POST') {
    if (request.headers.get('origin') !== `http://${authority.host}` || request.headers.get('content-type') !== 'application/json') return Response.json({ error: 'Same-origin JSON required' }, { status: 403 });
    try { return Response.json(await services.runs.cancel(cancelMatch[1]!)); }
    catch (error) { return Response.json({error:error instanceof Error ? error.message : 'Could not cancel run'}, {status:400}); }
   }
   const cleanupMatch = /^\/api\/runs\/([a-zA-Z0-9_-]{1,80})\/recheck-cleanup$/.exec(path);
   if (cleanupMatch && request.method === 'POST') {
    if (request.headers.get('origin') !== `http://${authority.host}` || request.headers.get('content-type') !== 'application/json') return Response.json({ error: 'Same-origin JSON required' }, { status: 403 });
    try { await services.runs.reconcileCleanup(cleanupMatch[1]!); return Response.json({ reconciled: true }); }
    catch (error) { return Response.json({ error: error instanceof Error ? error.message : 'Could not reconcile cleanup' }, { status: 400 }); }
   }
   if (path === '/api/runs' && request.method === 'GET') return Response.json(services.runs.list());
   const runMatch = /^\/api\/runs\/([a-zA-Z0-9_-]{1,80})$/.exec(path);
   if (runMatch && request.method === 'GET') {
    try { return Response.json(services.runs.get(runMatch[1]!)); } catch { return Response.json({ error: 'Run not found' }, { status: 404 }); }
   }
   if (path === '/api/runs' && request.method === 'POST' && services.configurations) {
    if (request.headers.get('origin') !== `http://${authority.host}` || request.headers.get('content-type') !== 'application/json') return Response.json({ error: 'Same-origin JSON required' }, { status: 403 });
    const body = await request.text(); if (Buffer.byteLength(body) > 1_048_576) return Response.json({ error: 'Launch too large' }, { status: 413 });
    try { const value = JSON.parse(body); return Response.json(await services.runs.launch(value.definition, value.inputs, await services.configurations.list()), { status: 202 }); }
    catch (error) { return Response.json({ error: error instanceof Error ? error.message : 'Could not accept run' }, { status: 400 }); }
   }
  }
  if (path === '/api/authentication' && request.method === 'GET' && services.authentication) return Response.json(await services.authentication());
  if (path === '/api/configurations' && services.configurations) {
   if (request.method === 'GET') return Response.json(await services.configurations.list());
   if (request.method === 'PUT') {
    if (request.headers.get('origin') !== `http://${authority.host}` || request.headers.get('content-type') !== 'application/json') return Response.json({ error: 'Same-origin JSON required' }, { status: 403 });
    const body = await request.text();
    if (Buffer.byteLength(body) > 1_048_576) return Response.json({ error: 'Configuration too large' }, { status: 413 });
    let config: CodexConfiguration;
    try { config = validateConfiguration(JSON.parse(body)); } catch { return Response.json({ error: 'Invalid or unsupported Codex configuration' }, { status: 400 }); }
    return Response.json(await services.configurations.save(config));
   }
  }
  if (path === '/api/definitions' && request.method === 'GET') return Response.json(await store.list());
  const match = /^\/api\/definitions\/([a-zA-Z0-9_-]{1,80})$/.exec(path);
  if (!match) return Response.json({ error: 'Not found' }, { status: 404 });
  const id = match[1]!; validId(id);
  if (request.method === 'GET') return Response.json(await store.get(id));
  if (request.method === 'PUT') {
   if (request.headers.get('origin') !== `http://${authority.host}` || request.headers.get('content-type') !== 'application/json') return Response.json({ error: 'Same-origin JSON required' }, { status: 403 });
   const body = await request.text();
   if (Buffer.byteLength(body) > 1_048_576) return Response.json({ error: 'Definition too large' }, { status: 413 });
   let definition: Definition;
   try { definition = validateDefinition(JSON.parse(body)); if (definition.id !== id) throw new Error('Identifier mismatch'); }
   catch { return Response.json({ error: 'Invalid definition or execution order' }, { status: 400 }); }
   return Response.json(await store.save(definition));
  }
  return Response.json({ error: 'Method not allowed' }, { status: 405 });
 } catch { return Response.json({ error: 'Storage unavailable or definition damaged. Check the local data folder.' }, { status: 500 }); }
}
