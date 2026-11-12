# Desktop resource observations (#39 / D16)

## Evidence status, not a performance claim

The parent's actual foundation release `0af716cc9f32d460d99c04268b746955558ff9d2`
was observed on Ubuntu **26.04 x86_64**, private Xvfb/private D-Bus, disposable
XDG data. External receipts are `checks-and-foundation-native.md` and
`native-foundation/observation.json` under the parent's report directory. Its
4,189,964-byte `.deb` declares 13531 KiB Installed-Size; it was **not installed**.
Declared package size excludes shared system WebKit/GTK dependencies and is not
actual allocated installed footprint.

After about two seconds warmup, one actual 10.012318114982918-second idle sample
ended at summed RSS **513,069,056 bytes**, including main and WebKit network/web
processes, and **0.39950788159778966% of one CPU core**. Summed RSS double-counts
shared pages. Observation infrastructure (Xvfb/private D-Bus/accessibility),
pre-existing services and workloads are excluded. The parent inspected a screenshot
of the empty-definition window. This is neither final-candidate canvas evidence,
first paint/startup latency, representative saved-canvas/log rendering nor an
Ubuntu 24.04/macOS acceptance observation. Original hardware metadata was incomplete.
Do not reinterpret this checkpoint as results from the new script.

This source adds one opt-in **Linux-only** process/window observer, outside routine
unit-test discovery. Metadata mode and a later explicit release idle launch were
exercised; the latter's actual evidence is recorded below. No arbitrary budget or
"lightweight" verdict follows. No macOS native host is available here; no macOS
startup, CPU, process-tree memory, install or interaction result exists. Linux
numbers are never substituted for it.

## Stabilized editor release: actual available-host observation

The release built and observed at `fd111c2d52afde5e24506948ccaf85fd0b8d0b03`
includes the canvas/configuration/runtime-settings source, but this observation
shows only its **empty saved-workflow screen**, not a loaded canvas or log view.
It ran on Ubuntu **26.04.1 x86_64**, Intel Xeon E3-1231 v3 at 3.40 GHz,
8 logical CPUs and 33,470,361,600 physical memory bytes, with private Xvfb,
private D-Bus and disposable HOME/XDG directories. No Docker/model/auth call or
UI interaction was performed.

- Release executable: **14,457,104 bytes**, SHA-256
  `31eb6b2a2f20a39863f58b5bd5d0941e2cf3320c1132e33dcc0e5f7fb3fa8769`.
- Local uninstalled release `.deb`: **4,400,860 bytes**, SHA-256
  `0128af16a60cd441fed729f19b7f14eb683de573c2c0ac3b6a837f3fdbf0765b`.
  Declared Installed-Size **14183 KiB**; actual installed/dependency footprint
  remains unmeasured. Its installed-path payload includes the full 1712-byte
  third-party notice, byte-identical to source.
- After five seconds of startup sampling, actual idle interval **10.1661 s**:
  **18.001% of one CPU core** and peak sampled summed RSS **551,063,552 bytes**
  (about **525.5 MiB**). Main, WebKit web and network processes are included;
  shared pages are double-counted. This single virtual-display sample is not a
  stable performance benchmark or proof of low overhead.
- The inspected screenshot shows the workflow list, configuration controls and
  runtime-readiness panel rendered, with execution explicitly disabled. Window-map
  timing is unavailable because xdotool/xwininfo were absent; screenshot capture
  is not a first-paint or startup-latency measurement.
- The observer completed successfully. Its cleanup list was empty, and an
  independent subsequent PID/start-time check found no observed processes alive.

Retained external receipts under `final-fd111c2/`: `checks.json` (including actual
release-build command/exit and unchanged-source fingerprints), `count.json`
(**151/151**), `idle-observation/observation.json`, `native-window.png`, and
`package-and-cleanup.json`. Later documentation-only corrections do not change
these measured application bytes. At that historical checkpoint, Ubuntu 24.04
ABI/install, macOS, saved-canvas/log scenarios and native interaction were unverified.

## Current-feature release: actual available-host observations

The unchanged release in `d15-build/artifact.json` was built from
`6155a1b9659202e7314556586ca3019bd06fde7e` (tree identical to integrated baseline
`ae0e5138b19f711796b92df50ae5f193271b22bb`). Observer/fixture preparation was pinned
at `3772857e78344adeb16cfb526c4cff0f109e1cab`; later documentation does not relabel
that release build. Executable: **18,804,416 bytes**, SHA-256
`05501b66f3881ff7d7fcbcb8f4df1dfbf6e7ebf2a06542fc9c626a02a45e2117`.

Two bounded private Xvfb/private D-Bus observations ran on Ubuntu **26.04.1 x86_64**,
Intel Xeon E3-1231 v3 at 3.40 GHz, **8 logical CPUs**, **33,470,361,600 bytes RAM**.
Each used fresh private HOME/XDG and the synthetic fixture below, startup polling
30 seconds and a 60-second fixed sample phase. That whole phase includes controller
interaction and is **not representative idle**. Conservative ready-only subsets
exclude interaction, one second of settling and clock-bracket uncertainty:

- Initial screen before first input: **10.2453 s**,
  **87.85% of one core**,
  peak summed RSS **589,459,456 bytes**.
  This includes startup; it is not idle or a first-paint measurement.
- Saved canvas after reopening: **17.2728 s**,
  **42.44% of one core**,
  peak summed RSS **637,759,488 bytes** (62 samples).
- Retained-log view after scrolling: **48.7459 s**,
  **13.31% of one core**,
  peak summed RSS **661,819,392 bytes**
  (173 samples).

All include the Rust host and WebKit web/network helpers; RSS double-counts shared
pages. Xvfb, private D-Bus/accessibility services and pre-existing services are not
charged to the app. No daemon/VM, egress helper or agent workload was started.
Both app logs contain DRI3/accelerated-rendering warnings. These headless, small
synthetic views are not an accelerated-desktop benchmark or a "lightweight" claim.

Actual native screenshots show the saved revision 1 two-block/one-edge canvas
reopening, its directional connection, document scrolling and retained synthetic
log text (entries 04–12 visible). A first click before window focus was a verified
no-op; focusing only the private window then selecting the saved definition worked.
Native next-page controls, prompt editing/save, pan/zoom, keyboard access and human
responsiveness were **not** exercised. Current readers separately validate the
40-entry journal as 32 + 8; that is projection evidence, not native pagination proof.

External `d16-native/` retains `canvas-result.json`, `log-result.json`, screenshots,
controller events, full raw samples/app logs in `canvas-observation/` and
`log-observation/`, and `payload-footprint.json`. Each successful observer exited 0
with no remaining owned identities. Independent checks found the private
observation/app sessions empty and no same-user WebKit processes, including
possible detached helpers. No workflow, runtime probe, auth or recovery was requested.
The first, rejected attempt is preserved: its 125-byte activation-socket path
failed before UI setup. An admitted 95-byte short path fixed the observation setup;
no product/sandbox/ownership guard was changed and the failed attempt is not a pass.

The current `.deb` is **5,584,754 bytes**, declared Installed-Size
**18,429 KiB**. Its **6 regular payload files** total
**18,810,041 apparent bytes** /
**18,825,216 allocated bytes**; including directory/link
metadata, the extracted tree uses **18,882,560 allocated bytes**.
The existing shared WebKit/GTK packages declare 94,896/9,123 KiB respectively; these
are pre-existing system costs, not incremental app costs. The product was **not
installed**. Actual package-manager installed/marginal dependency footprint,
Ubuntu 24.04 ABI/install and native macOS measurements remain **unverified**.

## Prepared synthetic data and reproducible observation

Opt-in `--prepared-fixture /absolute/private/synthetic-source` stages only with
`--observe-idle`, into new `data/com.workflowfactory.desktop` before spawn.
Metadata mode neither reads nor stages that fixture. No production-data discovery
occurs. Source paths must be explicit absolute paths without `.`/`..` or symlink
components. The root and payload directories/files must be owned/private; files
must be regular and single-link. Originals remain read-only inputs.

The <=16 KiB `synthetic-fixture.json` declares scenario
`SYNTHETIC-neverexecuted-A-B`, a canonical run UUID, `generator_sha256`, and exactly
two `files` entries with `bytes` and `sha256`: `definitions.sqlite3` (<=2 MiB) and
`inspection/<run UUID>/journal` (<=1 MiB). No other paths are copied. Exclusive new
destination directories use 0700/files 0600. The receipt binds manifest/generator,
payload hashes/sizes/source modes and scenario. Copy refusal prevents spawn.
A synthetic label/hash is **not universal proof of nonsensitivity**: review the
offline generator/provenance and current read-only projections before launch
admission. The helper is not a SQLite safety validator or arbitrary tree copier.

External `d16-build/` retains the offline seed source, compile/generate/read argv,
logs, manifest, original private fixture and separate copied projection. The seed
contains a saved 2-block/1-edge A→B definition, multiline prompts, named synthetic
input, nonsecret configuration/layout, terminal inactive synthetic capture/results
and 40 small journal entries (32 + 8). Success-looking names/results/diagnostics say
SYNTHETIC/neverexecuted; none is execution, cleanup or settlement evidence. No queue
rows, managed sessions, resource/recovery records or credentials are prepared.
Real saved-definition/configuration, history/detail/status and journal readers
validate copied data; this preparation is separate from the native evidence above.

For a separately admitted observation, append `--prepared-fixture` to the bounded
command below. Reopen the saved definition/retained view manually and record actual
content readiness and every input time. Accept the fixed idle aggregate only if
content was ready before it and no interaction occurs within it. Otherwise retain
raw samples and select a conservative fully ready, input-free subinterval, stating
clock uncertainty and actual duration; reject the scenario if none exists.
Never click Start, runtime readiness, sign-in, authorization, Stop or recovery.
Independently cross-check WebKit helpers and all observation infrastructure cleanup.

## Candidate-bound receipts

`../scripts/measure-release.py` uses Python 3's standard library, Git, and optional
read-only `dpkg-deb` / X11 utilities. It does **not** build, install, mount a DMG,
start Docker/services, inspect credentials, authenticate or call a model. Default
mode does not launch anything. Supply an executable and SHA-256 from a retained
successful **release-build receipt**; a caller-supplied hash binds bytes but cannot
prove compiler options or source provenance. Clean HEAD/index/untracked state and
exact expected HEAD are required. An old binary in an ignored target directory
still needs its real build receipt.

All paths must be absolute. Output must be a **new** directory outside checkout;
existing output is refused, never replaced. Parent directories can be created.
Keep the complete `OUTPUT/data/com.workflowfactory.desktop/activation.sock` path
within the host Unix-domain socket limit (below 108 encoded bytes on this Linux
host). Prefer a short, owned private scratch root and archive regular evidence
after cleanup; a deeply nested report directory can prevent application startup.
Receipts include argv, timestamps, HEAD/tree, binary hash/bytes, actual OS/CPU/memory,
optional package bytes/hash and `.deb` declared Installed-Size/Depends. Metadata
mode works on native macOS as well, but does not measure WKWebView processes.
On macOS `--observe-idle` records a precise unavailable reason and starts nothing;
no cross-platform sampler or Linux fallback is claimed.

```sh
python3 /absolute/checkout/desktop/scripts/measure-release.py --help
python3 /absolute/checkout/desktop/scripts/measure-release.py \
  --checkout /absolute/checkout --expected-head FULL_40_HEX_HEAD \
  --binary /absolute/release/workflow-desktop --binary-sha256 FULL_64_HEX_SHA256 \
  --package /absolute/local/package.deb --output /absolute/new-receipt-directory
```

Optional `--package` accepts the local `.deb`/`.dmg` for byte/hash inventory only;
`.deb` metadata is queried if `dpkg-deb` exists. Actual installed allocated bytes,
shared dependencies and VM/image storage remain **unmeasured**. On an explicitly
approved installed host, inventory package-owned files and allocated/apparent bytes
separately; on macOS inventory the installed `.app` separately from compressed DMG.
Do not charge all pre-existing shared WebKit dependencies to this app or hide them
by quoting only the archive. Record package-manager dependency versions and whether
already installed. This tool intentionally performs no installation or mounting.

## Explicit bounded idle observation (Linux, manual opt-in)

Use only an intentionally approved final candidate. The observer itself sends no
UI input and never executes workflows. Prepare a **private** Xvfb display and
private D-Bus session explicitly, not the user's active desktop. With
Xvfb/xvfb-run/xauth installed,
wrap the metadata command above with:

```sh
xvfb-run -a -s '-screen 0 1280x800x24 -nolisten tcp' \
  dbus-run-session -- sh -c 'exec python3 "$@" --observe-idle --display "$DISPLAY"' sh \
  /absolute/checkout/desktop/scripts/measure-release.py \
  --checkout /absolute/checkout --expected-head FULL_40_HEX_HEAD \
  --binary /absolute/release/workflow-desktop --binary-sha256 FULL_64_HEX_SHA256 \
  --output /absolute/new-idle-receipt-directory --startup-seconds 5 --idle-seconds 10
```

This wrapper intentionally starts only private observation infrastructure, never a
Docker daemon or persistent application service. `xvfb-run` removes its private
Xauthority/display; `dbus-run-session` owns its temporary bus. The tool passes the
selected display/session access paths without reading credential material and
clears other inherited environment variables. It gives the application a new
private HOME, XDG data/config/cache/runtime directories under output, retaining
those directories as evidence. Do not reuse production app-data paths. `app.log`
is local potentially sensitive application output; review before sharing. No
screenshot or window contents/title are collected.

Startup polling defaults to five seconds (allowed 1–30); idle defaults to ten
(1–60). `xdotool` plus `xwininfo`, if present, detect a visible window belonging to
the launched PID: the recorded time is a polling **upper bound to X11 window
mapping**, **not first paint**, content readiness or startup-to-usable latency.
Absent tools/no mapped window is an evidence gap, not a fabricated zero. No
Wayland/WKWebView equivalent is implemented. Sampling is about every 250 ms;
wall time includes read/probe/cleanup overhead, not a hard real-time deadline.

Linux `/proc/PID/stat` records PID, kernel start ticks, parent/session, CPU ticks,
name and RSS. Descendants and private-session processes include normal WebKit
helpers, even if reparented. Detached helpers starting an unrelated session may be
missed; cross-check the actual process list on a real native host. Each owned
identity gets a pidfd, rechecked against start ticks before admission; cleanup
signals those handles, not a potentially reused numeric PID or a global process
name. It waits/reaps its application and records surviving observed identities;
remaining zombies/uninterruptible processes are explicit cleanup limitations.
Nothing kills unrelated pre-existing services. Abrupt SIGKILL of the observer or
helpers escaping before discovery cannot guarantee cleanup; inspect that private
session explicitly, not production data/processes.

CPU is accumulated sampled ticks across identities during the idle interval,
including newly observed children, divided by actual elapsed seconds and clock
frequency, expressed as percent of **one core** (can exceed 100%). Processes that
start and exit between samples and final ticks after an observed process exits may
be missed; no PSS/private-memory or whole-machine attribution is claimed. RSS
peak is the maximum **sampled summed** RSS during idle, with shared-page double
counting. Raw process samples remain in `observation.json`. Inspect cleanup and
candidate-after fields before accepting a receipt. An error receipt is not a pass.

## Remaining scenario procedures — not measured results

- **Saved canvas:** once the final candidate is available, use a known non-secret
  saved sequential A→B definition with declared inputs/configuration references;
  record its size/block/edge counts and saved revision. Manually reopen/render,
  pan/zoom, select/focus inspector and edit a prompt in the actual native webview.
  Record responsiveness/visible errors, process-tree idle cost and manual timing
  method separately. Do not start a run or automate the UI. Reopen/render and
  document scrolling were observed above; pan/zoom, inspector focus, prompt
  editing/save and timed responsiveness remain unperformed.
- **Bounded logs:** delivered read-only history/inspection contracts support explicitly
  prepared non-secret retained records, never a model call to generate data.
  Record row/byte counts, caps/truncation and page/cursor behavior; manually inspect
  scrolling and bounded retention while checking that canvas state is unaffected.
  Native text rendering/scrolling were observed above; native page advance and
  cap/truncation interaction remain unverified. Feature/rendering absence is a
  completion gap, not a reason to invent evidence.
- **Isolation variants (future, separately authorized):** hold the same definition,
  image ID, input sizes and workload constant; measure one-agent-per-run shared
  environment versus fresh-agent-per-block with shared run workspace/output only.
  Report application/WebKit overhead, pre-existing Linux daemon or macOS Linux VM,
  image/disk cost, trusted per-run egress helper, and active agent CPU/memory/duration
  separately, including warm/cold conditions and transitions. Two blocks do not
  mean two simultaneously active containers. A private runtime network/helper and
  credential staging are not app-process-tree overhead. Docker, auth, models,
  egress/containment and workload checks are **not authorized or executed** here.
- **Native macOS:** collect OS/hardware/architecture, exact `.app`/DMG hashes,
  release-build/install receipt, native window readiness and a bounded Instruments
  or Activity Monitor sample that explicitly includes WKWebView content/network
  helpers (which need not be child processes), separately attributing Docker
  Desktop's VM. This Linux sampler declines rather than pretending `ps` ancestry
  is complete. Repeat on both Apple Silicon and Intel. Both remain unavailable.

Both isolation procedures remain unperformed methods, not measured execution.
Authentication feasibility, remaining native editing/pan/zoom/pagination/usability,
approved-host installation, actual full installed/dependency footprint, first paint,
live containment and workload measurements remain unverified. Existing
browser-app resource figures are historical and are not desktop measurements.
