# Workflow Factory

A local workflow definition editor for Linux and macOS. Create saved workflows,
drag AI blocks onto a canvas, edit their names/prompts, and connect them in order.
Connected Codex AI blocks execute in order inside a constrained local run container.

## Start from source

Prerequisites: Node.js **24 or newer**, npm, and a current browser (Firefox,
Chrome/Chromium, or Safari). No Electron, bundled browser, Rust, runtime npm
packages are needed for editing. Execution additionally needs the runtime/login below.

```sh
npm ci
npm run build
npm start
```

Open **http://127.0.0.1:4317** in your existing browser. Use that exact address,
not `localhost`. Stop the foreground server with Ctrl+C; closing a browser tab
alone does not stop the server. Restart with `npm start` and reopen the address
to see your saved definitions. The same source commands apply on Linux/macOS;
macOS execution has not yet been verified on a macOS host.

To change the port or use a dedicated data directory:

```sh
PORT=4320 WORKFLOW_FACTORY_DATA_DIR="$HOME/workflow-factory-data" npm start
```

## Editing

1. Choose **New workflow**, name it, or select an existing workflow from home.
2. Drag **＋ AI block** onto the dotted canvas. Clicking it also adds a block.
3. Drag a block's **Move AI block** handle to reposition it. A focused handle
   supports arrow keys (10 pixels per press). Scroll to reach the wider canvas.
4. Edit the block name and prompt directly on its card. AI is the only block type.
5. Select **From** and **To**, then **Connect blocks**. Arrows show execution
   order; duplicate, missing, self-connected, and cyclic edges are rejected.
   Remove connections or blocks to revise the graph.
6. Click **Save workflow** and wait for **Saved to disk** before quitting. Saving
   is explicit, not automatic. Unsaved edits warn before tab close or returning
   home; a failed save keeps the working copy available to retry. Stopping the
   server cannot save unsaved browser edits.

## Codex configurations and authentication

From home choose **Codex configurations**, create a named model configuration,
then select it on each AI block. Supported settings are an explicit model ID
and reasoning effort (low/medium/high). Model availability depends on your ChatGPT
subscription; a saved model name is not proof of entitlement. Configuration files
use the same private atomic store under `configurations/`. Resolution clones
non-secret settings per block so later configuration edits cannot alter snapshots.
No arbitrary CLI flags, API keys, custom providers or billing fallback are accepted.

The server-only credential provider reads just `$CODEX_HOME/auth.json` (default
`~/.codex/auth.json`), never the rest of the home directory or Hermes credentials.
Use an existing Codex CLI with `cli_auth_credentials_store = "file"` in its
`config.toml`, run `codex login` using ChatGPT, then `chmod 600 ~/.codex/auth.json`
(adjust for CODEX_HOME). Only owner-private regular files and ChatGPT token login
are supported; keyring-only and API-key logins are refused with setup guidance.
The browser gets availability/guidance only, never credentials. Availability does
not validate token expiry or entitlement. The runtime integration will stage only
these credentials separately from the workspace; it never mounts the host home
or overwrite host login after refresh. This host has no authenticated Codex CLI;
authenticated execution is unverified, not simulated.

## Persistence and local access

### Reusable text inputs

Use **Add text input** to declare a name (letters, digits and underscores, starting
with a letter), required/optional status and an optional default. Each prompt has
an input-reference picker that inserts `{{inputs.name}}` at the caret, not its
current value. Renaming/removing a referenced input requires updating its prompts;
dangling references are rejected on save and input validation.

**Run inputs…** opens a separate value form. Required values cannot be blank;
defaults fill the form and optional blank values resolve to empty text. Values
and resolved prompts never overwrite the definition. **Launch run** captures the
current definition (including unsaved edits), inputs and resolved nonsecret settings.

## Executing a block

Install and start Docker Engine on Linux or Docker Desktop's Linux engine on macOS
separately. Docker **28+** with internal isolated bridge support is required; the
app inspects the created network and refuses unsupported engines instead of using
an unrestricted bridge. Access to Docker is privileged host authority: use only
your own trusted local daemon. Remote TCP/SSH endpoints are not supported.
The default socket is `/var/run/docker.sock` on Linux and `~/.docker/run/docker.sock`
on macOS. For another local socket, set `WORKFLOW_FACTORY_DOCKER_SOCKET` to its
absolute path (for example a Linux Docker Desktop/rootless socket). Run the app as
a non-root user. Share its data directory with Docker Desktop when prompted.

Build the trusted image explicitly from this checkout; the app never installs,
builds, pulls or starts a daemon for you:

```sh
docker build -f runtime/Dockerfile -t workflow-factory-runtime:local .
```

The image pins `@openai/codex@0.166.0`; the Node 24 Debian base tag is mutable and
should be rebuilt deliberately for OS fixes. Local execution pins the inspected
image ID and checks its labels, not a user-selectable image. Configure ChatGPT file
login as above, create a workflow with one block or a connected chain, choose each configuration,
and use **Run inputs… → Launch run**. Its status, final answer, event-kind diagnostics
and actionable errors appear below the editor. There is no retry or paid fallback.
This slice accepts one active run; concurrency arrives in a later slice.
Connect A → B and select A's **final
response** in B's prompt-reference picker to insert `{{blocks.A_ID.final}}`.
Only earlier connected blocks may be referenced; disconnected graphs, forks/joins,
future/missing outputs and unsupported output properties are refused at launch.
Each block gets its own prompt with selected outputs and inputs, never the previous
conversation. Substituted values stay literal, even if they contain reference-like
text. Successful completion alone advances the chain; failure stops it without
retry. All blocks reuse only that run's workspace.

### Isolation modes

Workflow properties offer **One agent container per run** (the default for older
definitions) or **Separate agent container per block**. The first keeps one agent
container for the chain; the second creates and removes a distinct agent container
for each block. No mixed per-block strategy is offered. In both modes, only that
run's private workspace is shared; each block has a fresh Codex invocation, not
conversation continuation. Final responses live in the app's run record, so B can
use A's response after A's agent container is gone. Cleanup failures stop the chain
without discarding already captured output. The per-run trusted egress helper and
two private networks persist through the chain and are removed at run cleanup;
this infrastructure overhead is additional to the agent container count.

Each run gets a private workspace and separate credential staging under
`run-resources/<run-id>/`. Only those two directories are mounted. The agent is
non-root, read-only-rootfs, capability-free, no-new-privileges, with private IPC,
bounded tmpfs/PIDs/memory/CPU, no published ports, no home mount and no Docker socket.
Its sole network is an inspected internal **isolated** bridge, not ordinary Docker
bridge networking. A trusted per-run CONNECT helper is the only external route;
it admits exact `chatgpt.com`, `api.openai.com`, `auth.openai.com` TLS destinations
on port 443, rejects all IANA special-purpose/reserved IPv4 DNS answers and connects
the vetted IP only after the first TLS ClientHello SNI equals the CONNECT hostname.
Non-TLS, absent/encrypted SNI and fragmented/oversized ClientHello records fail
closed. The helper does not decrypt application traffic or inspect encrypted HTTP
headers; it enforces the destination IP and clear TLS hostname, not URL paths.
IPv6 is disabled. Proxy environment variables are convenience, not enforcement:
the outer network removes the direct route. The helper has a separate per-run
egress bridge and no workspace/credential mounts. No general web/package egress is
offered. Backend hostname changes require a reviewed helper change.

Workflow isolation means one **agent** container, plus one trusted infrastructure
helper per run; it does not mean one total container. Codex's inner
`danger-full-access` is used only behind that enforced outer boundary. A container
is not a VM security guarantee; a compromised daemon/kernel is outside this model.
Workspace disk consumption is not quota-enforced: monitor the private data volume.
Commands use argv with `shell:false`, clean environment and prompt stdin. The last
completed JSONL `agent_message` is the answer; event kinds are diagnostics. Tool
output/raw stderr are withheld to avoid credential disclosure; staged token strings
are redacted from final output, including refreshed tokens. Auth staging is deleted
only after agent/helper removal; refreshed login never overwrites the host login.
Workspaces and completed in-memory results survive container removal. Cleanup
failure is visible; if Docker becomes unavailable, staging remains owner-private
until the owned containers can be stopped safely. Do not quit during an active run
in this increment; graceful cancellation/quit comes later.

The image build, live containment, authenticated model execution and native macOS
runtime behaviour have **not** been exercised. Focused mocked tests are not proof
of those live properties. No container/model smoke campaign is part of CI.

Definitions use versioned JSON records with block IDs, type, name, prompt,
positions, and directed connections. Files live outside the checkout:

- Linux: `${XDG_DATA_HOME:-$HOME/.local/share}/workflow-factory/definitions/`
- macOS: `~/Library/Application Support/workflow-factory/definitions/`
- Override: `$WORKFLOW_FACTORY_DATA_DIR/definitions/`

The store creates private directories (0700), writes a new private file (0600),
flushes and atomically renames it over the old definition. No credentials are
stored. Keep backups of this data folder; prompts are **not encrypted**. Invalid
or damaged files produce a visible load/storage error rather than being reset.
Concurrent editors use last successful save wins; use one tab per definition.

The standard-library HTTP server binds only to `127.0.0.1`, checks the exact
Host and Origin, rejects cross-site requests, and requires a random per-process
session token for API access. It serves only allowlisted assets, limits request
bodies to 1 MiB, and disables caching/framing. Do not proxy it publicly. These
controls are not an OS sandbox or protection from software running as your user.
If you restart the server while a page is open, reload to obtain its new session.

## Durable run history

Runs are atomically saved as private JSON under `runs/` in the same app data folder.
Admission is persisted before execution. Running/block-start/completed-output and
terminal transitions are serialized per run; a storage failure stops advancement
and still attempts scoped cleanup. After reopening, the run list includes retained
history. Filter by workflow, select an individual dated run and expand **Captured
definition, inputs, prompts and settings**. Historical data comes from the captured
record, never a current definition/configuration lookup; each block also shows its
actual resolved prompt, final response, safe event-kind logs and error.

Credentials and runtime resource objects are never added to the persisted contract;
unknown run fields/settings are rejected. Prompts/inputs/outputs are private files,
not encrypted, so do not place secrets in workflow text. Back up the data folder.
History has no automatic retention policy; its disk and in-memory use grow with
retained runs. Damaged records fail visibly instead of being silently discarded.
Previously active/queued records become interrupted failures on abnormal restart,
never auto-resumed; completed outputs are retained. After a crash, inspect/stop
only Docker resources named `wf-<run-id>-…` before starting further work; normal
graceful cancellation/quit is delivered in the lifecycle slices.
If a retained run has a cleanup hold, use **Recheck cleanup**. In the same app
session this retries only its recorded cleanup. After restart it never kills
discovered resources based on a name alone: manually remove only that run's owned
containers/networks, then recheck. The hold clears only after Docker confirms their
absence and the updated history is saved; unavailable Docker does not clear it.

## Development and checks

```sh
npm test
npm run typecheck
npm run build
```

`npm test` discovers every `test/**/*.test.mjs` file and checks the Node test
runner's actual test summary against a repository-wide ceiling of **200**.
Issue #1 stays within its allocation of **18** focused tests. Keep that aggregate
cap for later issues. No E2E tests, browser-to-runtime journeys, full controller
journeys, real containers, or model calls are permitted in the ordinary suite.
Use explicit records and fake external/storage boundaries, one editor action or
store operation at a time. Follow red → green TDD and conventional commits.

The runtime has zero dependencies. TypeScript builds native ES modules; happy-dom
is test-only. `src/definition.ts` owns the persisted contract; `src/store.ts`
owns atomic storage; `src/http.ts` owns the focused API; `src/server.ts` adapts
Node HTTP and filesystem assets; `src/public/` owns the vanilla DOM/SVG editor.
Execution uses `src/runs.ts` admission and `src/runtime.ts` Docker adapters plus
the trusted `runtime/egress.mjs` helper. No distribution or concurrent
scheduler features are included in this increment.
