# Workflow Factory desktop

This Tauri 2 / Rust / SQLite application is the **sole supported UI**. Svelte 5 and pinned Svelte Flow provide the canvas. There is no separate browser app, host Node backend or localhost application server. Node remains source-build/test tooling and a dependency inside the runtime Docker image.

## Start

Provision Node 24+, Rust **1.99.1** (the committed toolchain), and your platform's native [Tauri prerequisites](docs/distribution.md#build-explicitly-on-a-native-host). From the repository root:

```sh
npm ci --ignore-scripts
npm ci --prefix desktop --ignore-scripts
npm start
```

From this directory, repeat-launch with `npm start`. The pinned CLI uses `tauri dev --no-dev-server`; its `beforeDevCommand` has `wait: true` and builds assets before native startup. No web server or permissive navigation workaround is needed. This is guidance, not fresh native rendering evidence. To package explicitly without launching, use the native-host commands in [distribution](docs/distribution.md). Installed bundles need neither npm nor Node on the host.

## Current source behavior

- Canvas-first workflow list, AI palette, graph connections, contextual block/workflow inspectors, prompt input/predecessor-output references, explicit revision-checked save/reopen and retained edits after failure.
- Nonsecret configurations with revision conflicts and referenced-delete refusal. Captured definitions/configurations stay immutable when current records change.
- Saved-revision run preparation, exact pending-request retry, FIFO queue, captured history and selected output/diagnostics/artifacts. Return to editing keeps the original draft separate from read-only snapshots.
- Settings owns queue-capacity edits and local Dark/Light preference. Runs owns read-only queue inspection. Runtime readiness/setup plans require explicit actions and do not launch Docker at startup.
- App-owned native sign-in, selected Stop/authorization/cleanup/output-storage recovery, and coordinated native shutdown have concrete gated source paths. Narrow capabilities separate main and sign-in windows; no generic shell/filesystem/credential IPC is exposed.

See [workspace behavior](docs/canvas-workspace.md), [execution](docs/single-block-run.md), [runtime](docs/runtime.md), [shutdown](docs/shutdown.md) and [auth boundaries](AUTH_READINESS.md). Settings separates runtime prerequisite checks, metadata-only app-owned managed-session status and dedicated native sign-in. The legacy host-file availability inspector is not an import route or execution authority. Conditional Linux and macOS execution source requires native secure storage, the supported trusted Docker host, resource/host protections and fresh native confirmation. Unknown or failed prerequisites remain fail-closed. Source/component evidence is not live authentication, containment or platform qualification.

## Storage and preservation

Tauri retains identity `com.workflowfactory.desktop` and private `definitions.sqlite3` plus adjacent journals/backups under:

- Linux: `${XDG_DATA_HOME:-$HOME/.local/share}/com.workflowfactory.desktop`
- macOS: `$HOME/Library/Application Support/com.workflowfactory.desktop`

No identity/schema/backend change is part of browser retirement. Old browser `workflow-factory` data and any prior custom root remain untouched; there is no importer or deletion/migration step. Back up the entire stopped app's data directory, not a live SQLite main file alone. Private is not encrypted. Fonts and their complete license notices remain bundled unchanged.

## Checks and remaining gates

Root `npm run check` / `npm run build` forward here; root `npm test` invokes the [combined aggregate runner](docs/checks.md). Build assets before native compilation because Tauri embeds `frontendDist`. The frontend-only `npm test` is not repository-wide coverage. Static source inventory: **113 = 6 root + 38 frontend + 69 Rust source registrations**, not a compiled-harness pass.

Desktop-only integration retains the redesigned palette/appearance and all runtime/native/security source behavior. The stale queue-capacity case is split between actual QueuePanel and Settings QueueCapacity owners. Focused receipts and exact source pins are external under `/home/ops/reports/workflow-factory-desktop-only-integration/`; no local canonical suite/native launch/service/container/auth/model action is claimed. Final independent review, exact-head CI on Ubuntu 24.04 and macOS 15 arm64/Intel, and manual macOS launch/edit/save/reopen/appearance/shutdown/retained-view interaction remain separately required. Historical native/resource observations are not new candidate acceptance.
