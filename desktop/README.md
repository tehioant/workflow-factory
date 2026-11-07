# Workflow Factory desktop source

Separate Tauri 2 / Rust engine / SQLite application. Svelte 5 + pinned Svelte Flow
MIT core provide the canvas; surrounding controls use shared Tailwind tokens and
the reduced shadcn-svelte button. Node is build tooling, not an installed backend.

## Implemented source increments

- D01: explicit create/name/save/reopen with optimistic revisions and retained edits.
- #25 / D02: compact AI nodes, directional connections, inspector prompts, explicit
  graph/domain mapping, drag and non-drag coordinate editing, keyboard-accessible
  palette/select/connect/remove controls. Rust rejects invalid identities, dangling
  edges, branching, duplicate connections, disconnected chains and cycles.
- #26 / D03: named required/default text declarations, `{{input.name}}` picker,
  reusable preparation form and literal single-pass Rust substitution. A supplied
  empty value overrides the default and fails if required. Preview is not execution.
- #27 / D04: SQLite non-secret model/reasoning configuration CRUD, reference selection,
  revision checks and refusal to delete a currently referenced configuration.
  Auth reports **unsupportedConcurrentAuth / notInspected / executionEnabled=false**.
  No supported staging route is approved: see `AUTH_READINESS.md`. No credentials
  are read; file/keyring presence is not misrepresented as sandbox authentication.

The left sidebar contains workflows, AI palette and reusable configurations;
workflow properties appear above the central canvas, with the block inspector at
right. Model identifiers are syntax-validated, not checked against a live catalog.
Only sequential AI chains are supported; unfinished block types are not exposed.

## Reproducible local checks

Node 24+, Rust stable and native Tauri 2 prerequisites are required. Linux needs
GTK 3/WebKitGTK 4.1 development packages; macOS needs Xcode command-line tools.
Direct package versions and lockfiles are pinned. No native prerequisites or runtime
images are installed automatically.

```sh
cd desktop
npm ci --ignore-scripts
cargo run -p workflow-engine --bin export-types -- --check
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p workflow-engine
npm test
npm run check
npm run build
cargo check -p workflow-desktop
# Explicit local bundle, not an application launch:
npm exec -- tauri build --debug --bundles deb
# On each appropriate macOS host:
# npm exec -- tauri build --debug --bundles dmg
```

## Persistence and command boundary

Tauri resolves the private `com.workflowfactory.desktop` application data directory:
Linux `${XDG_DATA_HOME:-$HOME/.local/share}/com.workflowfactory.desktop`; macOS
`$HOME/Library/Application Support/com.workflowfactory.desktop`. The SQLite database
is separate from browser data; final symlinks are refused and Unix directory/file
permissions are 0700/0600. Private does not mean encrypted.

SQLite schema 0→1 initializes only empty storage; additive transactional 1→2 adds
`configurations`. Unsupported versions or populated unversioned storage receive a
private SQLite backup and refusal. No destructive migration exists. Definition
schema remains 1: old name-only records default blocks/connections/inputs to empty
and old blocks default configuration references to null. Historical revision bodies
are not rewritten.

`revisions` retains immutable definition bodies. Revision-checked save/configuration
update uses `BEGIN IMMEDIATE`; no force overwrite exists. Config deletion checks
latest saved definitions in the same transaction. Configuration selection remains
editable after errors; explicitly save or discard before changing configuration
editors. Definition navigation requires stay or explicit discard; failed load/save
retains the working copy. Svelte state is snapshotted before structured cloning.

The local-main capability explicitly allows only `list_definitions`,
`load_definition`, `save_definition`, `prepare_inputs`, `list_configurations`,
`save_configuration`, `delete_configuration`, `auth_readiness`. These are registered
in the host and declared in the build manifest. Storage uses the existing bounded
blocking-worker gate. Input preparation runs off the UI thread. No general shell,
filesystem, arbitrary credential path, remote capability or execution command exists.
Transport types are generated from Rust; run `npm run types` after contract edits.

Limits: 500 listed definitions/configurations, 100 blocks/connections/inputs, names
200 bytes, prompts/defaults/supplied input values 64 KiB, input names 64 ASCII bytes,
prepared prompt expansion 1 MiB. Invalid graph shapes cannot be saved; intermediate
disconnected edits remain in the unsaved working copy until connected or removed.
No browser source/data migration, server, sidecar, Docker/model/login call is added.

## Focused evidence and remaining gates

Desktop suite: **33 cases (22 engine + 11 frontend)**, including D01's 12.
The three increments add exactly **21 (9 + 6 + 6)**; no parameterized/automatic
per-type tests or application journeys. The parent owns aggregate discovery/CI and
independent review. External report: `editor-increments.md` beside `d01.md` under
`/home/ops/reports/workflow-factory-desktop/`.

Approved targets remain Ubuntu 24.04 x86_64 and macOS 15+ arm64/Intel. Available
Ubuntu 26.04 builds do not establish their ABI or interaction compatibility.
No desktop window was launched; actual WebKitGTK/WKWebView drag, connection,
pan/zoom, keyboard, inspector-focus, dialogs and scrolling gates remain open.
Native common close/Quit is a later lifecycle increment; beforeunload is only a
best-effort dirty signal, not native-close protection evidence. No authentication,
container containment, resource/performance, package-install or macOS evidence is
claimed. npm audit reports two low-severity development-tool findings; no paid
fallback or silent installation is introduced.
