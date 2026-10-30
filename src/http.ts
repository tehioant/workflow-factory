import { validateDefinition, validId } from './definition.ts';
import type { Definition } from './definition.ts';
export interface Authority { host: string; token: string; }
export interface DefinitionRepository { list(): Promise<Definition[]>; get(id: string): Promise<Definition>; save(value: unknown): Promise<Definition>; }
export function authorize(request: Request, authority: Authority): boolean {
 const origin = request.headers.get('origin');
 return request.headers.get('host') === authority.host
  && (!origin || origin === `http://${authority.host}`)
  && request.headers.get('x-workflow-token') === authority.token
  && (!request.headers.has('sec-fetch-site') || request.headers.get('sec-fetch-site') === 'same-origin');
}
export async function handleRequest(request: Request, authority: Authority, store: DefinitionRepository): Promise<Response> {
 if (!authorize(request, authority)) return Response.json({ error: 'Local session authorization required. Reload the app.' }, { status: 403 });
 const path = new URL(request.url).pathname;
 try {
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
