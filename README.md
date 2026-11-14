# Workflow Factory

A local **desktop-only** workflow editor built with Tauri 2, Svelte 5 and a Rust/SQLite engine. The canvas-first desktop is the sole application UI. There is no browser application, host Node HTTP backend, localhost application server or background worker service. Node is source-build/test tooling and remains inside the separately built runtime Docker image; it is not an installed desktop backend.

## Start from source

Use a clean checkout of the intended reviewed revision; preserve local edits. Provision Node **24+**, npm, pinned Rust **1.99.1** and native Tauri prerequisites first. On macOS 15+ these include Xcode command-line tools; Ubuntu 24.04 needs the [distribution guide's native development libraries](desktop/docs/distribution.md#build-explicitly-on-a-native-host). Nothing installs them automatically.

```sh
git clone https://github.com/tehioant/workflow-factory.git
cd workflow-factory
npm ci --ignore-scripts
npm ci --prefix desktop --ignore-scripts
npm start
```

`npm start` opens the native desktop development app using bundled assets. Its synchronous prelaunch hook builds the frontend before Tauri starts; `--no-dev-server` prevents the CLI's default loopback server. No URL needs opening and no navigation/CSP exception is added. Repeat with `npm start`; frontend changes require rebuilding/restarting, not a web dev server. This is guidance, **not fresh native launch evidence**. Installed local deb/dmg packages launch through their app/menu entry without Node or npm; see [distribution](desktop/docs/distribution.md).

## Editing and retained views

- Select **New workflow** or a saved workflow in the searchable list. Name it in workflow properties; add an **AI block** from the palette and edit its name, prompt, configuration and coordinates in the inspector. Drag/keyboard movement and explicit connection controls remain available. Only sequential AI chains are supported.
- Declare reusable text inputs in workflow properties. Prompt-reference controls insert `{{input.name}}` or a connected predecessor's `{{output.blockId.finalResponse}}`; substituted values stay literal.
- **Configurations** manages nonsecret model/reasoning settings. Model syntax validation is not live availability or subscription entitlement.
- Explicitly **Save workflow**. Failed saves retain edits; reopening uses saved revisions. Run preparation distinguishes saving current edits from running the last saved revision. An ambiguous admission retains its exact request for **Retry same request**, not a new launch.
- **Runs** shows the read-only global queue and selected workflow history. Select a retained run for its Output, Detail, Diagnostics, Inputs and Artifacts drawer. Captured-canvas inspection is read-only; **Return to editing** restores the draft. Recovery is explicit and selected, not a polling side effect.
- **Settings** owns queue capacities, Appearance (Dark default / Light), and explicit readiness/setup-plan actions. Capacity errors retain entered values; queue polls do not save settings.
- Use **Safe shutdown** or native close/Quit for coordinated draft-save/work-ownership decisions. Forced termination is not graceful shutdown; see [shutdown](desktop/docs/shutdown.md).

## Runtime and authentication limits

Editing, save/reopen and retained inspection need neither Docker nor authentication. Runtime setup is explicit, not part of startup/install. [Runtime policy](desktop/docs/runtime.md) specifies the fixed CLI/socket/image; [execution and managed auth](desktop/docs/single-block-run.md) describes the conditional Linux source path, native Secret Service, trusted rootful Docker 28+, host/cgroup protections and fresh native consent. The Settings readiness module only probes/plans and always reports `executionReady=false`; it is not execution authorization.

Only independently acquired **app-owned** managed sessions may authorize execution. No host Codex/Hermes credential import, API-key/paid fallback or automatic login is provided. Settings opens the separately permissioned native sign-in surface; controls are not live authentication qualification. **macOS execution remains fail-closed**; editor packaging does not enable it. Both isolation modes retain current-cache, ownership, egress and cleanup guards. Live auth, containment and tmpfs continuity remain separate qualification gates, not inferred from mocked tests.

## Saved data is preserved

Desktop identity and locations are unchanged:

- Linux: `${XDG_DATA_HOME:-$HOME/.local/share}/com.workflowfactory.desktop`
- macOS: `$HOME/Library/Application Support/com.workflowfactory.desktop`

Private `definitions.sqlite3`, adjacent backups/journals and webview preferences remain separate from retired browser data. The old `workflow-factory` data directory, including any previously configured `WORKFLOW_FACTORY_DATA_DIR`, is **not imported, migrated or deleted**. That variable is not a desktop data-directory selector. Uninstall only app files, preserving both data sets. Back up the entire desktop data directory while stopped; private permissions are not encryption. No cleanup command is part of this change.

## Development and delivery status

```sh
npm run check       # desktop Svelte/TypeScript; typecheck is an alias
npm run build       # desktop production frontend assets
# Only when the complete inherited suite is admitted for execution:
npm test            # retained root + desktop frontend + compiled Rust aggregate gate
```

The [combined runner](desktop/docs/checks.md) discovers retained tests, enforces the **200-case aggregate ceiling**, rejects incomplete cases and reconciles executed counts. Desktop-only static inventory: **111 registrations = 6 root helper/config + 36 frontend + 69 Rust source registrations**. Static counting is not native harness discovery or a full-suite pass. Ordinary local checks remain unit/small mounted selections; inherited journeys were not run locally. The existing PR56-only remote-CI exception does not authorize local full-suite/services/containers/auth/model/native GUI activity.

[Workspace status](desktop/docs/canvas-workspace.md) distinguishes historical redesign/light-theme evidence from this integration. Final independent review, exact-head Ubuntu/macOS CI and macOS manual native launch/edit/save/reopen/appearance/shutdown/retained-view checks remain delivery gates; no merge/release or new native pass is claimed. Preserve the [design system](DESIGN.md), [spec](docs/specs/canvas-first-redesign.md), [illustrative concepts](docs/design/canvas-first/README.md) and bundled legal notices. [Desktop resources](desktop/docs/resources.md) are candidate/platform-specific; [the retired Node-server report](RESOURCE_REPORT.md) is historical only.
