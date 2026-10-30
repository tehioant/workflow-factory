import { spawn } from 'node:child_process';
import * as fs from 'node:fs/promises';
import { join, resolve, isAbsolute } from 'node:path';
import { homedir } from 'node:os';
import { validId } from './definition.ts';
import { validateConfiguration } from './configuration.ts';
import type { CodexConfiguration } from './configuration.ts';
import { readSubscriptionAuth } from './auth.ts';
export interface CommandResult { code: number; stdout: string; stderr: string; }
export type Command = (args: string[], input?: string, timeout?: number, signal?: AbortSignal) => Promise<CommandResult>;
export interface Resources { id: string; network: string; egress: string; agent: string; proxy: string; workspace: string; credentials: string; image: string; uid: string; isolation?: 'workflow' | 'block'; agents?: string[]; }
export function blockResources(r: Resources, blockId: string): Resources {
 validId(blockId);
 return { ...r, agent: r.isolation === 'block' ? `wf-${r.id}-block-${blockId}` : r.agent };
}
export function dockerCommand(socket = process.env.WORKFLOW_FACTORY_DOCKER_SOCKET || (process.platform === 'darwin' ? join(homedir(), '.docker/run/docker.sock') : '/var/run/docker.sock')): Command {
 if (!isAbsolute(socket) || /[\r\n\0]/.test(socket)) throw new Error('Docker socket must be a local absolute path');
 return (args, input, timeout = 20000, signal) => new Promise((resolveResult, reject) => {
  const child = spawn('docker', ['--host', `unix://${socket}`, ...args], { shell: false, env: { PATH: process.env.PATH, HOME: homedir() }, stdio: ['pipe','pipe','pipe'], timeout, killSignal: 'SIGKILL', ...(signal ? { signal } : {}) });
  let stdout = '', stderr = '', size = 0, overflow = false;
  child.stdout.setEncoding('utf8'); child.stderr.setEncoding('utf8');
  const capture = (target: 'stdout' | 'stderr', bytes: string) => {
   size += Buffer.byteLength(bytes); if (size > 4_194_304) { overflow = true; child.kill('SIGKILL'); return; }
   if (target === 'stdout') stdout += bytes; else stderr += bytes;
  };
  child.stdout.on('data', bytes => capture('stdout', bytes)); child.stderr.on('data', bytes => capture('stderr', bytes));
  child.stdin.on('error', () => {}); child.stdin.end(input);
  child.on('error', () => reject(new Error('Docker CLI unavailable. Install Docker and ensure its local daemon is running.')));
  child.on('close', code => { if (overflow) reject(new Error('Execution output exceeded the 4 MiB safety limit')); else resolveResult({ code: code ?? 1, stdout, stderr }); });
 });
}
export async function requireRuntime(command: Command) {
 try {
  const result = await command(['info', '--format', '{{json .}}']);
  if (result.code !== 0 || JSON.parse(result.stdout).OSType !== 'linux') throw new Error('Unavailable');
 } catch { throw new Error('Docker must be installed with its local Linux engine running. Check Docker Desktop/Engine and WORKFLOW_FACTORY_DOCKER_SOCKET.'); }
}
export function verifyPrivateNetwork(value: unknown) {
 const n = value as { Driver?: string; Internal?: boolean; EnableIPv6?: boolean; Options?: Record<string,string> };
 if (!n || n.Driver !== 'bridge' || n.Internal !== true || n.EnableIPv6 !== false || n.Options?.['com.docker.network.bridge.gateway_mode_ipv4'] !== 'isolated') throw new Error('Docker must support internal isolated bridge networking; refusing an unrestricted network. Upgrade Docker (28+).');
}
const hardened = ['--pull=never', '--read-only', '--cap-drop=ALL', '--security-opt=no-new-privileges', '--pids-limit=128', '--memory=1g', '--cpus=1', '--init', '--log-driver=none', '--ipc=private', '--tmpfs', '/tmp:rw,nosuid,nodev,size=128m,mode=1777'];
export function agentArguments(r: Resources) {
 return ['run', '-d', '--name', r.agent, '--user', r.uid, '--network', r.network, ...hardened, '--mount', `type=bind,src=${r.workspace},dst=/workspace`, '--mount', `type=bind,src=${r.credentials},dst=/run/codex`, '--workdir', '/workspace', '--env', 'HOME=/tmp', '--env', 'CODEX_HOME=/run/codex', '--env', 'HTTP_PROXY=http://egress:3128', '--env', 'HTTPS_PROXY=http://egress:3128', '--env', 'ALL_PROXY=http://egress:3128', '--env', 'NO_PROXY=', r.image, 'node', '-e', 'setInterval(() => {}, 3600000)'];
}
export function codexArguments(value: CodexConfiguration) {
 const c = validateConfiguration(value);
 return ['codex', 'exec', '--json', '--ephemeral', '--skip-git-repo-check', '--sandbox', 'danger-full-access', '--model', c.model, '-c', `model_reasoning_effort=${JSON.stringify(c.reasoning)}`, '-c', 'approval_policy="never"', '-c', 'forced_login_method="chatgpt"', '-c', 'cli_auth_credentials_store="file"', '-'];
}
export function parseCodexOutput(stdout: string, code: number, stderr: string) {
 const logs: string[] = []; let output: string | undefined; let completed = false; let error: string | undefined;
 for (const line of stdout.split('\n').filter(line => line.trim())) {
  let event;
  try { event = JSON.parse(line); } catch { error = 'Codex emitted invalid JSONL; inspect the runtime version.'; break; }
  if (!event || typeof event.type !== 'string') { error = 'Codex emitted invalid JSONL events'; break; }
  // Persist event kinds, not tool output, raw errors, reasoning or credential-bearing stderr.
  logs.push(/^[a-z._]{1,80}$/.test(event.type) ? event.type : 'unknown.event');
  if (event.type === 'turn.failed' || event.type === 'error') error = 'Codex execution failed. Check subscription login, entitlement and allowed egress. No API-key fallback was attempted.';
  if (event.type === 'turn.completed') completed = true;
  if (event.type === 'item.completed' && event.item?.type === 'agent_message' && typeof event.item.text === 'string') output = event.item.text;
 }
 if (!error && (code !== 0 || !completed || output === undefined)) error = 'Codex did not complete successfully. Check login/model/runtime; no result was fabricated.';
 if (stderr) logs.push('Codex produced stderr diagnostics (content withheld to protect credentials)');
 return { output, logs: logs.join('\n'), ...(error ? { error } : {}) };
}
async function removeContainer(container: string, command: Command) {
  const result = await command(['rm', '-f', container]);
  if (result.code && !new RegExp(`^Error response from daemon: No such container: ${container}\\s*$`, 'i').test(result.stderr)) throw new Error('Could not stop owned containers; credential staging retained privately for safe cleanup');
}
export async function removeResources(r: Resources, command: Command, removeCredentials: (path: string) => Promise<void>) {
 for (const container of [...(r.agents ?? [r.agent]), r.proxy]) await removeContainer(container, command);
 for (const network of [r.network, r.egress]) {
  const result = await command(['network', 'rm', network]);
  if (result.code && (!result.stderr.includes(`network ${network} not found`) || !result.stderr.split('\n').filter(Boolean).every(line => line === `Error response from daemon: network ${network} not found` || line === `exit status 1`))) throw new Error('Could not remove owned networks');
 }
 await removeCredentials(r.credentials);
}
export class DockerRuntime {
 private root: string;
 private command: Command;
 private resources = new Map<string, Resources>();
 private secrets = new Map<string, string[]>();
 private credentials: (path?: string) => Promise<string>;
 constructor(root: string, command = dockerCommand(), credentials = readSubscriptionAuth) {
  this.root = resolve(root); if (/[,\r\n\0]/.test(this.root)) throw new Error('Runtime data path cannot contain commas or control characters');
  this.command = command;
  this.credentials = credentials;
 }
 private async checked(args: string[], message: string, signal?: AbortSignal) {
  signal?.throwIfAborted();
  // Do not kill setup's Docker client mid-create: await its daemon reply first,
  // so final cleanup cannot race a late container/network creation.
  const result = await this.command(args); signal?.throwIfAborted(); if (result.code !== 0) throw new Error(message); return result.stdout;
 }
 async prepare(id: string, isolation: 'workflow' | 'block' = 'workflow', signal?: AbortSignal): Promise<Resources> {
  signal?.throwIfAborted();
  validId(id); if (this.resources.has(id)) throw new Error('Run sandbox already exists');
  await requireRuntime(this.command); signal?.throwIfAborted();
  const imageInfo = JSON.parse(await this.checked(['image', 'inspect', 'workflow-factory-runtime:local'], 'Trusted runtime image missing. Build it explicitly: docker build -f runtime/Dockerfile -t workflow-factory-runtime:local .',signal))[0];
  if (imageInfo?.Config?.Labels?.['org.workflow-factory.runtime'] !== '1' || imageInfo?.Config?.Labels?.['org.workflow-factory.codex'] !== '0.166.0' || !/^sha256:[a-f0-9]{64}$/.test(imageInfo.Id)) throw new Error('Build the checked-in trusted runtime image; unsupported image refused');
  const uid = process.getuid?.() ?? 1000, gid = process.getgid?.() ?? 1000;
  if (uid === 0) throw new Error('Run Workflow Factory as a non-root user');
  const key = `wf-${id}`;
  const r: Resources = { id, network: `${key}-private`, egress: `${key}-egress`, agent: `${key}-agent`, proxy: `${key}-proxy`, workspace: join(this.root, id, 'workspace'), credentials: join(this.root, id, 'credentials'), image: imageInfo.Id, uid: `${uid}:${gid}`, isolation, agents: isolation === 'workflow' ? [`${key}-agent`] : [] };
  // Register before any owned resource creation so failure cleanup has exact targets.
  this.resources.set(id, r);
  const auth = await this.credentials(); signal?.throwIfAborted();
  this.secrets.set(id, Object.values(JSON.parse(auth).tokens));
  await fs.mkdir(join(this.root, id), { recursive: true, mode: 0o700 }); signal?.throwIfAborted();
  await fs.mkdir(r.workspace, { mode: 0o700 }); signal?.throwIfAborted(); await fs.mkdir(r.credentials, { mode: 0o700 }); signal?.throwIfAborted();
  await fs.writeFile(join(r.credentials, 'auth.json'), auth, { mode: 0o600, flag: 'wx' }); signal?.throwIfAborted();
  await this.checked(['network', 'create', '--internal', '--ipv6=false', '--opt', 'com.docker.network.bridge.gateway_mode_ipv4=isolated', r.network], 'Docker cannot create the required isolated network. No bridge fallback is allowed.',signal);
  verifyPrivateNetwork(JSON.parse(await this.checked(['network', 'inspect', r.network], 'Could not inspect private network',signal))[0]);
  await this.checked(['network', 'create', '--ipv6=false', r.egress], 'Could not create scoped egress network',signal);
  await this.checked(['run', '-d', '--name', r.proxy, '--network', r.egress, '--user', r.uid, ...hardened, r.image, 'node', '/opt/workflow-factory/egress.mjs'], 'Could not start the trusted egress helper',signal);
  await this.checked(['network', 'connect', '--alias', 'egress', r.network, r.proxy], 'Could not attach trusted egress helper',signal);
  if (isolation === 'workflow') await this.startAgent(r,signal);
  return r;
 }
 private async startAgent(r: Resources, signal?: AbortSignal) {
  await this.checked(agentArguments(r), 'Could not start the restricted Codex container',signal);
  const inspected = JSON.parse(await this.checked(['inspect', r.agent], 'Could not inspect agent containment',signal))[0];
  if (inspected?.HostConfig?.NetworkMode !== r.network || Object.keys(inspected?.NetworkSettings?.Networks ?? {}).join() !== r.network) throw new Error('Agent network containment was not honored; execution refused');
 }
 async execute(r: Resources, prompt: string, config: CodexConfiguration, blockId = 'single', signal?: AbortSignal) {
  signal?.throwIfAborted();
  if (this.resources.get(r.id) !== r) throw new Error('Unowned run sandbox');
  const agent = blockResources(r, blockId);
  if (r.isolation === 'block') { r.agents ??= []; r.agents.push(agent.agent); }
  let parsed: ReturnType<typeof parseCodexOutput> | undefined;
  try {
  if (r.isolation === 'block') await this.startAgent(agent,signal);
  signal?.throwIfAborted();
  const result = await this.command(['exec', '-i', '--workdir', '/workspace', agent.agent, ...codexArguments(config)], prompt, 600000,signal);
  // Codex may refresh its staged login. Include refreshed values in output redaction,
  // but never copy them back over the host's auth.json.
  const refreshed = JSON.parse(await this.credentials(join(r.credentials, 'auth.json')));
  this.secrets.set(r.id, [...(this.secrets.get(r.id) ?? []), ...Object.values(refreshed.tokens) as string[]]);
  parsed = parseCodexOutput(result.stdout, result.code, result.stderr);
  for (const secret of this.secrets.get(r.id) ?? []) if (parsed.output !== undefined) parsed.output = parsed.output.split(secret).join('[credential redacted]');
  return parsed;
  } finally {
   if (r.isolation === 'block') {
    try { await removeContainer(agent.agent, this.command); r.agents = r.agents!.filter(name => name !== agent.agent); }
    catch (error) { if (parsed) parsed.error = 'Block container cleanup failed; retained output is available, but no successor will start.'; else throw error; }
   }
  }
 }
 async stop(id: string) {
  const r = this.resources.get(id); if (!r) return;
  for (const agent of [...(r.agents ?? [r.agent])]) await removeContainer(agent,this.command);
 }
 async cleanup(id: string) {
  const r = this.resources.get(id); if (!r) return;
  await removeResources(r, this.command, path => fs.rm(path, { recursive: true, force: true }));
  this.resources.delete(id); this.secrets.delete(id);
 }
 async reconcileCleanup(id: string, blockIds: string[]) {
  validId(id); for (const block of blockIds) validId(block);
  if (this.resources.has(id)) { await this.cleanup(id); return; }
  await requireRuntime(this.command);
  // After restart, names alone do not prove ownership. Never kill a discovered
  // object: require manual removal, then confirm absence before clearing the hold.
  const containers = [`wf-${id}-agent`, ...blockIds.map(block => `wf-${id}-block-${block}`), `wf-${id}-proxy`];
  for (const name of containers) {
   const result = await this.command(['container','inspect',name]);
   if (result.code !== 1 || !new RegExp(`^(Error: No such (object|container): ${name}|Error response from daemon: No such container: ${name})\\s*$`, 'i').test(result.stderr)) throw new Error('Owned resources are present or cannot be verified. Stop only this run’s named resources manually, then recheck cleanup.');
  }
  for (const name of [`wf-${id}-private`,`wf-${id}-egress`]) {
   const result = await this.command(['network','inspect',name]);
   if (result.code !== 1 || !new RegExp(`^Error response from daemon: network ${name} not found\\s*$`, 'i').test(result.stderr)) throw new Error('Run networks are present or cannot be verified. Remove only this run’s networks manually, then recheck cleanup.');
  }
  await fs.rm(join(this.root,id,'credentials'),{ recursive: true, force: true });
 }
}
