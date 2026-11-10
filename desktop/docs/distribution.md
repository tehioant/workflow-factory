# Local desktop distribution contract (#38 / D15)

## Targets and current evidence

The approved **targets**, not verified compatibility claims, are Ubuntu **24.04
x86_64** (`x86_64-unknown-linux-gnu`, local `.deb`) and macOS **15+**, separately
Apple Silicon (`aarch64-apple-darwin`) and Intel (`x86_64-apple-darwin`, local
`.dmg`). Build on each corresponding native host. A Linux package built against
newer system libraries is not evidence of Ubuntu 24.04 compatibility; a configured
macOS minimum version is not an executed compatibility check. No universal binary,
Windows, other Linux distribution or cross-compiled DMG is promised.

At the source checkpoint `f47a67aae2c588dc82d98021cedf64ae41ff2371`, CI declares
Ubuntu 24.04, `macos-15` and `macos-15-intel`, pinned action SHAs, Node 24,
Rust 1.99.1, focused checks and **debug** native packaging. It neither launches nor
uploads packages. A matrix declaration is not a successful remote run. README's
older case counts/launch statements are historical; use candidate-bound receipts.

Available development host: Ubuntu **26.04 x86_64**, not the approved baseline.
D01 debug bundles exist locally. The parent also built an actual **foundation**
release at `0af716cc9f32d460d99c04268b746955558ff9d2`: local uninstalled
`Workflow Factory Desktop_0.1.0_amd64.deb`, 4,189,964 bytes, SHA-256
`76367fff0653e993ffd24d72dc16a1523a6ca52c5878b8346c9d00cdf856ad83`,
Declared Installed-Size 13531 KiB. This predates the final canvas/configuration
candidate. The parent's `checks-and-foundation-native.md` and
`native-foundation/observation.json` are external historical receipts, not bundled
release artifacts. No Ubuntu 24.04/macOS native host, installed-package launch or
final-candidate package acceptance is verified here. Neither ticket is closed.

## Build explicitly, on a native host

Use a clean, pinned checkout and retain `git rev-parse HEAD`, `git status
--porcelain`, OS/CPU architecture, tool versions, complete command/exit, artifact
size and SHA-256. Never label an old target-directory binary as a new build.

- Rust **1.99.1**, minimal profile plus rustfmt/clippy, is selected by
  `desktop/rust-toolchain.toml`. Do not replace it with floating `stable`.
- Node **24** is build tooling only; record the exact patch and npm version used.
  CI pins the major, not a patch-level Node/npm environment. `npm ci` consumes the
  committed `desktop/package-lock.json`; Tauri CLI is exactly **2.12.4** in
  `desktop/package.json`. Do not use a global/latest Tauri CLI.
- Cargo dependencies are committed in `desktop/Cargo.lock`; use `--locked` for
  direct Cargo checks. Retain lockfiles unchanged after bundling.
- On Ubuntu 24.04, explicitly provision the C/C++ build toolchain/pkg-config and
  CI's `libwebkit2gtk-4.1-dev libxdo-dev libssl-dev
  libayatana-appindicator3-dev librsvg2-dev patchelf`. These pull GTK 3 development
  dependencies. Review apt changes before installing; the app does not install them.
- On each macOS 15 native host, explicitly provision Xcode command-line tools and
  record `xcodebuild -version` / `xcrun --show-sdk-version`. WKWebView comes from
  the OS; do not bundle Chromium or substitute Linux WebKit numbers.

From the repository's `desktop/` directory, after those prerequisites are present:

```sh
node --version; npm --version; rustc --version; cargo --version
npm ci --ignore-scripts
npm run check
npm run build # Tauri's generate_context! needs these local assets during checks
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p workflow-engine --locked --bin export-types -- --check
# Ubuntu 24.04 x86_64 native host, RELEASE (no --debug):
npm exec -- tauri build --bundles deb --target x86_64-unknown-linux-gnu
# macOS 15 Apple Silicon native host, local ad-hoc signing only:
APPLE_SIGNING_IDENTITY=- npm exec -- tauri build --bundles dmg --target aarch64-apple-darwin
# macOS 15 Intel native host:
APPLE_SIGNING_IDENTITY=- npm exec -- tauri build --bundles dmg --target x86_64-apple-darwin
```

Run only the one platform-appropriate bundle command. Its configured
`beforeBuildCommand` builds bundled Vite assets. Explicit targets place artifacts
under `desktop/target/<target>/release/bundle/{deb,dmg}/`; default native builds
without `--target` use `desktop/target/release/`. Inspect actual filenames rather
than guessing them. Do not execute a stale artifact if a command fails. The
canonical focused checks/count gate is documented in [checks.md](checks.md);
this observation source adds no routine test cases. Compilation/package creation
is separate from installation, rendering and manual native interaction acceptance.

## Local installation and removal (manual, not run by the observer)

**Ubuntu:** inspect the exact file with `dpkg-deb --info /absolute/package.deb`
and `dpkg-deb --contents /absolute/package.deb`. Review its package name,
architecture and Depends. The foundation metadata requires
`libwebkit2gtk-4.1-0` and `libgtk-3-0`; on Ubuntu 24.04 the actual apt provider may
be a t64 transition package. Let apt resolve the native distribution's declared
runtime dependencies; do not copy system libraries from Ubuntu 26.04. For a
reviewed 24.04 package: `sudo apt install /absolute/package.deb`. Launch its
installed desktop-menu entry. To remove only application files, use
`sudo apt remove <exact-package-name-from-dpkg-deb>`; do not run recursive data
removal or an unrelated autoremove. This is guidance, not an installation receipt.

**macOS:** locally open the exact DMG in Finder, copy **Workflow Factory
Desktop.app** to Applications (or the user's Applications directory), eject the
image and launch that installed app. Remove only the app bundle to uninstall.
No signing identity purchase, Developer ID setup or notarization was performed.
Ad-hoc signing is local integrity signing, **not** trusted Developer ID or
notarization. Gatekeeper/quarantine may prevent first launch of an unnotarized
app; report the exact failure. Where local policy permits, use Apple's explicit
Privacy & Security / Open Anyway approval for that reviewed local build; do not
disable Gatekeeper, strip quarantine indiscriminately or promise frictionless
third-party installation. No public distribution is authorized.

On every actual installed host, separately record launch without Node/a dev
server, bundled asset rendering, storage creation, reopen/save behavior, and manual
WebKitGTK/WKWebView drag/connect/pan/zoom, keyboard/focus, dialogs and scrolling.
No automated UI interaction/E2E is supplied. Test native close/Quit only against
the actually delivered lifecycle contract; source build success proves none of
these behaviors.

## Data preservation and separately installed prerequisites

The installed application is bundled local frontend assets plus a Rust engine;
there is **no Node backend, localhost application server, sidecar or worker
service**. Node is not an installed-host prerequisite. Linux uses system WebKitGTK;
macOS uses WKWebView. No runtime daemon, image pull/build or login happens as part
of these packaging instructions or the observation script.

Tauri app data identity is `com.workflowfactory.desktop`:

- Linux: `${XDG_DATA_HOME:-$HOME/.local/share}/com.workflowfactory.desktop`.
- macOS: `$HOME/Library/Application Support/com.workflowfactory.desktop`.
- `definitions.sqlite3` and any adjacent database backups/journals remain in that
  private application-data area. Native webview caches/storage may also live there
  or in platform cache directories. Private permissions are not encryption.

Keep these paths and platform webview data when uninstalling. Back up the entire
private app-data directory while the app is stopped; never copy a live SQLite
main file alone and assume journals are unnecessary. Do not delete or migrate old
browser-app data, host Codex data, Docker images/volumes or workspaces. No importer,
old-app retirement, updater, tray/background service or destructive migration is
part of distribution.

Editing/persistence do not require Docker or authentication. Future execution
requires a separately and explicitly installed trusted rootful Linux Docker
Engine 28+ (macOS Docker Desktop's Linux VM), selected local socket/CLI, reviewed
pinned runtime image and supported subscription authentication. See
[runtime.md](runtime.md) and `../AUTH_READINESS.md`. The current runtime module is
read-only/planning-only, not a launcher; execution remains disabled pending an
approved authentication route. No API-key fallback or Hermes credential reuse.

Both **planned** isolation contracts must eventually be documented and measured:
(1) one agent container per run, sharing its environment across sequential blocks;
(2) a fresh agent container per block, preserving only that run's workspace and
captured outputs. Neither shares workspaces across runs. Both also require a
separate trusted per-run egress helper, fail-closed enforced outer-network policy,
and hardened agents. These are not implemented execution claims. Container/VM,
image, helper and credential-staging costs are separate from installing the editor;
no implicit setup is allowed to conceal them. See [resources.md](resources.md).
