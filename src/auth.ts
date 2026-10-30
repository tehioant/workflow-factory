import * as fs from 'node:fs/promises';
import { constants } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
const guidance = 'Codex needs a private file-based ChatGPT login. Set cli_auth_credentials_store = "file" in your Codex config, run codex login, and restrict auth.json to your user (chmod 600). Keyring-only and API-key logins are not supported. No paid fallback is used.';
export function authPath() { return join(process.env.CODEX_HOME || join(homedir(), '.codex'), 'auth.json'); }
// Server-only. Never return this value through HTTP or include it in diagnostics/snapshots.
export async function readSubscriptionAuth(path = authPath(), disk: Pick<typeof fs, 'open'> = fs): Promise<string> {
 try {
  const handle = await disk.open(path, constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK);
  try {
   const stat = await handle.stat();
   if (!stat.isFile() || stat.size > 65536 || (stat.mode & 0o077) !== 0 || (process.getuid && stat.uid !== process.getuid())) throw new Error('Unsafe credential file');
   const value = JSON.parse(await handle.readFile('utf8'));
   if (value.auth_mode !== 'chatgpt' || value.OPENAI_API_KEY || !value.tokens) throw new Error('Subscription login required');
   const tokens = Object.fromEntries(['access_token', 'refresh_token', 'id_token', 'account_id'].map(key => {
    const token = value.tokens[key]; if (typeof token !== 'string' || !token || token.length > 20000) throw new Error('Invalid tokens');
    return [key, token];
   }));
   return JSON.stringify({ auth_mode: 'chatgpt', tokens });
  } finally { await handle.close(); }
 } catch { throw new Error(guidance); }
}
export async function authenticationStatus() {
 try { await readSubscriptionAuth(); return { available: true, message: 'Private file-based ChatGPT login available; validity is checked by Codex at execution.' }; }
 catch { return { available: false, message: guidance }; }
}
