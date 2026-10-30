# Workflow Factory

A local workflow definition editor for Linux and macOS. Create saved workflows,
drag AI blocks onto a canvas, edit their names/prompts, and connect them in order.
**This increment edits definitions and validates run inputs. It does not run agents.**

## Start from source

Prerequisites: Node.js **24 or newer**, npm, and a current browser (Firefox,
Chrome/Chromium, or Safari). No Electron, bundled browser, Rust, runtime npm
packages, container daemon, model credentials, or model calls are needed.

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

## Persistence and local access

### Reusable text inputs

Use **Add text input** to declare a name (letters, digits and underscores, starting
with a letter), required/optional status and an optional default. Each prompt has
an input-reference picker that inserts `{{inputs.name}}` at the caret, not its
current value. Renaming/removing a referenced input requires updating its prompts;
dangling references are rejected on save and input validation.

**Run inputs…** opens a separate value form. Required values cannot be blank;
defaults fill the form and optional blank values resolve to empty text. Values
and resolved prompts never overwrite the definition. **Validate run inputs**
only validates in this increment: it does not simulate execution or create a run.

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
Future execution settings and runs build on these boundaries, not on
browser-local persistence. No distribution, execution, configuration, history
or scheduler features are included in this increment.
