# Desktop foundation (D01)

This separate application creates, names, explicitly saves and reopens desktop definitions. The D01 record has an identity, name and definition schema version only. Canvas blocks, inputs, configurations and execution belong to later tickets; no simulated runtime is included.

## Reproducible local checks

Use Node 24+ (verified here with 26.9.0), Rust stable (verified 1.99.1), and the native Tauri 2 prerequisites. Linux requires GTK 3, WebKitGTK 4.1 and development packages; macOS requires Xcode command-line tools. All direct npm/Cargo versions and lockfiles are pinned.

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
# Local development bundle; never launches the application:
npm exec -- tauri build --debug --bundles deb
# On appropriate macOS 15+ hosts (arm64 and x86_64 separately):
# npm exec -- tauri build --debug --bundles dmg
```

A display is required for an intentional interactive `npm exec -- tauri dev` session, not for the checks above. Development also loads bundled local assets; it does not start a Vite HTTP service. No installed Node backend, sidecar, daemon, shell/fs plugin, Docker prerequisite or updater exists.

## Persistence and error contract

The host owns `com.workflowfactory.desktop` and obtains Tauri's application data directory. Linux: `${XDG_DATA_HOME:-$HOME/.local/share}/com.workflowfactory.desktop`; macOS: `$HOME/Library/Application Support/com.workflowfactory.desktop`. `definitions.sqlite3` is separate from all browser data. Directory/file permissions are 0700/0600 on supported Unix targets; symlink final paths are refused. Private is not encrypted.

`Store::open` initializes SQLite user_version 0 → 1 transactionally **only for an empty database**. Unknown versions or populated unversioned databases are backed up with SQLite's backup API to a private `backup-<UUID>.sqlite3` alongside the database and refused without schema modification. If backup fails, no migration is applied and storage failure is reported. There is no destructive migration in D01: future destructive migrations must add a backup-before-transaction step, not repurpose the initial migration.

`revisions(id, revision, body)` retains immutable revision bodies. Definition `schemaVersion` and database `user_version` are separate from optimistic `revision`. Creating requires `expectedRevision: null`; updating requires the current integer revision. `BEGIN IMMEDIATE` protects comparison and insertion together. No force overwrite is exposed. Name validation, conflict and storage failures retain the UI working copy/revision. Navigation requires stay or explicit discard. Native common close/Quit handling is D13; browser beforeunload is only a best-effort dirty signal, not native close evidence.

The host registers only `list_definitions`, `load_definition`, `save_definition`. Its local-main-only capability explicitly permits only those commands; no remote capability exists. The single permit admits at most one blocking database task, rejects excess calls with `busy`, and holds that permit until the worker completes. All database/filesystem work runs off the UI thread. CSP limits assets and IPC; the window navigation allowlist rejects remote pages. Names/errors are rendered as text. Names are limited to 200 UTF-8 bytes and identities to canonical-length UUIDs. List reads refuse beyond 500 records rather than silently truncate; pagination is not implemented in this foundation.

## Interfaces for the next increment

- `engine/src/lib.rs`: `Definition`, `SavedDefinition`, `SaveDefinition`, typed `Error`, and public `Store::{open,list,load,save}`. Extend Definition deliberately in D02; provide compatible reads of existing schema-1 name-only records, and bump definition schema where necessary. Do not discard historical revision bodies.
- `engine/src/bin/export-types.rs`: the one explicit Rust → TypeScript exporter. No `#[ts(export)]` or automatic per-type export tests. Run `npm run types` after Rust contract edits; `--check` refuses drift without editing files.
- `src/lib/commands.ts`: typed invocation boundary used by actual Svelte UI and mocked by focused action tests.
- `src/lib/editor.ts`: one-operation save/open actions; `src/App.svelte`: mounting, list/create/name controls and explicit dirty decision.
- `src-tauri/src/main.rs`, `build.rs`, `permissions/`, `capabilities/`: actual command registration, command ACL declaration and bounded worker seam.

There are **12 new cases: 9 engine + 3 frontend**; generated exporter/library/bin/doc targets add zero cases. Test setup seeds records directly; no desktop is launched and no save/restart/run journey is tested. The integration owner owns the aggregate retained+desktop cap gate and CI, not this directory's frontend test command.

## Evidence limits

Compilation/bundling on Ubuntu 26.04 is not Ubuntu 24.04 ABI acceptance. Approved targets remain Ubuntu 24.04 x86_64 and macOS 15+ Apple Silicon/Intel. No native window interaction, approved-host package install/launch, macOS compile, signing/notarization or resource measurement is claimed. See the external D01 report for actual receipts. npm audit currently reports two low-severity development-tool findings through the Vite Svelte plugin/deepmerge; upstream reports no fix. They are not concealed by disabling audit.
