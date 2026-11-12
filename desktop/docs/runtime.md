# Local runtime-readiness adapter (D05 module, not ticket completion)

The engine provides read-only Docker probes and an explicit setup **plan**, not an executor. Editing and retained data must remain usable when probes fail. No app launch, daemon call, build, login, network creation or container operation was used to implement or test this module.

## Selected contract

- Trusted **rootful Docker Engine 28+**, Linux daemon. Windows engines and rootless engines are unsupported. Version is a capability expectation, not an isolation measurement. The daemon, host kernel and socket owner are trusted; Docker access is effectively privileged host access. Labels do not authenticate malicious local images or a compromised daemon.
- The desktop host selects **`unix:///var/run/docker.sock` on both Linux and macOS**. No user endpoint selector, home-socket resolution, TCP/SSH endpoints, implicit contexts or environment-selected sockets are exposed. On macOS, explicitly enable Docker Desktop's [Allow the default Docker socket to be used](https://docs.docker.com/desktop/settings-and-maintenance/settings/#advanced-mac-only) setting if that path is absent; the app never creates it or silently switches to the home socket. The underlying engine accepts validated absolute Unix endpoints (no parent components or control characters), but that wider module interface is not a desktop setting. A local socket forwarding to another daemon violates the trusted-local deployment assumption; the CLI cannot attest the other end's physical location.
- CLI locations are deliberately selected, not PATH-discovered: `/usr/bin/docker` on Linux; Docker Desktop's `/usr/local/bin/docker` on macOS. Install the official CLI link there explicitly if absent; readiness never installs or changes it. The trusted CLI must support normal Docker 28+ commands.
- Runtime tag: `workflow-factory-runtime:codex-0.166.0-v1`. Required image labels: `org.workflow-factory.runtime=1` and `org.workflow-factory.codex=0.166.0`. Codex is pinned to **0.166.0** by the existing Dockerfile. Accept exactly one inspect record and a lowercase `sha256:` plus 64-hex image ID. Capture that immutable ID in future admission snapshots; compare supplied captured IDs on recheck. Never execute a mutable tag instead of the admitted ID. The existing base `node:24-bookworm-slim` is mutable: builds are not claimed reproducible or supply-chain certified.

## Readiness and guidance

`RuntimeStatus` is a camelCase serialized enum: `missing`, `stopped`, `unsupported`, `imageMissing`, `imageUnsupported`, `networkUnavailable`, `authUnsupported`.

- Missing CLI: explicitly install Docker Engine / Docker Desktop; editing continues.
- Stopped/inaccessible/failed probe: start the selected Linux daemon and check socket permissions. A timeout, oversized/invalid response or access denial is unavailable, not success; raw stdout/stderr never goes to the UI.
- Unsupported: upgrade to Docker 28+ with a rootful Linux daemon. No Windows/rootless/remote fallback.
- Missing/uninspectable image: review assets and explicitly build; readiness does not pull.
- Wrong labels/ID or ID drift: refuse it; review/rebuild/reselect explicitly. Existing admitted IDs must not silently change.
- `networkUnavailable`: refuse execution. Check the **owned immutable network ID**, `Driver=bridge`, `Internal=true`, `EnableIPv6=false`, and `Options[com.docker.network.bridge.gateway_mode_ipv4]=isolated`. Upgrade/reconfigure explicitly if unavailable. There is no ordinary-bridge fallback and no startup network probe that creates resources.
- `authUnsupported`: even valid runtime/image prerequisites do not establish secure container subscription authentication or concurrent refresh. Execution stays disabled pending a separately approved supported mechanism. No host credentials, Hermes credentials or API-key fallback are read or used here.

`Readiness` contains `{ status, message, imageId: string | null, isolation, executionReady }`. `executionReady` is **always false** in this module. `isolation` is `notChecked`, `expectedNotMeasured` or `measuredNetworkConfiguration`. Normal readiness reports only an expectation after version/image checks. `inspect_private_network` provides fresh measured Docker configuration evidence, not a live packet/egress test, a complete containment certificate or execution authorization. Its `networkUnavailable` error must be rendered using the actionable network guidance above.

## Explicit setup (not invoked)

`Docker::setup_plan()` returns `{ program, argv, instructions }` only. From the root of the existing **reviewed repository containing runtime/Dockerfile and runtime/egress.mjs**, its Linux argv is:

```text
program: /usr/bin/docker
argv: ["--host", "unix:///var/run/docker.sock", "build", "--file", "runtime/Dockerfile", "--tag", "workflow-factory-runtime:codex-0.166.0-v1", "."]
```

This explicitly requested build can download the base image and npm package, consume disk/network and create a local image. It is never passed to `Process`, run at startup or run by this adapter. Review assets first; confirm labels and capture the resulting immutable ID with a later read-only recheck. Do not copy the old application runtime or silently package/build its images with the desktop editor. The adapter uses the compatible existing image/helper assets in place and does not modify them.

## Outer-network contract for the future execution owner

Immediately before **each** future agent start, freshly call `inspect_private_network` on the durably recorded, positively owned network ID; fail closed on any error. Do not cache settings-time evidence as authority. Before any paid work, recheck the captured image identity too. This module contains no start function, so it cannot enforce a future caller that bypasses this seam.

Preserve the reviewed helper: only exact TLS CONNECT destinations `chatgpt.com`, `api.openai.com`, `auth.openai.com` on 443, vetted public IPv4 DNS results, exact resolved-IP connect, and matching TLS ClientHello SNI; no arbitrary HTTP, IP/private-address, alternate-host, IPv6 or encrypted/absent-SNI bypass. The agent has **only** the internal isolated network. A separate trusted per-run egress helper bridges that network to its separate egress network, with no agent workspace or credential mounts. Proxy variables alone do not constrain an agent. Future execution must verify agent network membership, helper wiring/ownership and all non-root/read-only/capability/resource/mount hardening; never expose the Docker socket or host home to workloads. No weaker substitute is allowed when containment is infeasible.

macOS needs Docker Desktop's Linux VM in addition to the Tauri/WKWebView app. Report VM/daemon memory, disk, startup and CPU costs separately, and include the per-run egress helper separately from one-agent-per-run/per-block workload counts. Linux uses a host daemon instead of that VM. None of these resource costs or live containment/auth claims have been measured here.

## Delivered integration seam

```rust
Docker::new(endpoint: &str, platform: Platform) -> Result<Docker, ProcessError>
Docker::readiness(&self, process: &mut impl Process,
                  pinned_image: Option<&str>) -> Readiness
Docker::setup_plan(&self) -> SetupPlan
Docker::inspect_private_network(&self, process: &mut impl Process,
                               expected_id: &str)
    -> Result<IsolationEvidence, RuntimeStatus>
verify_private_network(value: &serde_json::Value, expected_id: &str)
    -> Result<IsolationEvidence, RuntimeStatus>
Process::run(&mut self, command: &CommandSpec)
    -> Result<ProcessOutput, ProcessError>
```

`Platform::{Linux, MacOs}` and `RealProcess` are host-side only. `CommandSpec` fields are private; only read-only info/image/network specs can reach the production transport. Setup plans are a different type. `CommandSpec::{program, argv, process_command}` support inspection/conversion without spawning. `ProcessOutput` is a host-only bounded byte response, not a transport type. `Readiness`, `RuntimeStatus`, `IsolationEvidence`, `SetupPlan` derive serde serialization and `ts_rs::TS` and are included in the Rust type exporter.

The desktop registers and narrowly permissions the parameterless `runtime_readiness` and `runtime_setup_plan` Tauri commands. `RuntimeSettings.svelte` invokes them only on an explicit button action. The readiness command has an independent one-slot semaphore and a blocking worker, reaches `runtime_host::readiness_with(RealProcess)` without opening SQLite, and uses the fixed backend endpoint above. Setup returns the plan only. No `Process`, arbitrary argv/program, endpoint mutation, image/network mutation or generic shell/Docker interface is exposed to JS. Focused host-adapter and invoke checks cover the delivered consumer; startup never probes Docker. A registered command and passing mock checks do not establish live Docker or macOS acceptance.

## Process bounds and evidence limitations

Production uses `std::process::Command`, explicit argv, `env_clear`, fixed PATH/HOME/empty-config location, and null stdin (**0 bytes accepted**). It ignores inherited Docker contexts, proxy variables and login/config environment. Nonblocking Unix socket pairs drain both outputs without reader threads; total retained stdout+stderr is limited to **128 KiB**, with a **3-second per-probe deadline** (normal readiness has at most two probes). Each loop reads at most one chunk per stream so flood output cannot starve deadline checks. On every post-spawn error the read-only CLI is killed and synchronously waited/reaped; no detached/unjoined background client or reader is left. As with any process kill/wait, kernel-level uninterruptible process states are outside a userspace deadline guarantee. Killing this CLI is never interpreted as container termination.

Seven focused injected-response/argv/conversion cases only; no parameterized scenarios or execution journeys. Production process spawning, macOS compilation, live daemon capabilities, credential refresh, actual egress and containers were deliberately not exercised. Passing module checks do not close #28 or the parent desktop specification.

## Workflow agent isolation

Saved schema-1 definitions default to `perRun`. Workflow properties can save
`perBlock`; admission captures that choice with the immutable definition. The
existing host loop and executor consume only that snapshot. Per-run workspaces
remain container-private tmpfs. Per-block runs explicitly create one owned
local-driver tmpfs volume; it is never implicitly created or adopted.

At a successful durable block boundary, the stopped writer's current cache is
checkpointed and read back under the same lease, owner and generation. A fresh,
credential-free inert keeper overlaps the old live mount before old removal.
Live mount, start-epoch and exact-ID absence checks plus durable receipts precede
staging the exact checkpoint. The helper has no workspace or credential mounts.
No execution transcript, home directory or `/tmp` is copied. Captured responses
are resolved from durable block results, not predecessor container lifetime.

Unknown cache, storage, mount continuity, restart or removal outcomes hold the
resources and quarantine the session; no successor or rollback to old grants.
Final cleanup verifies exact owned agent/helper, volume and network removal.
Only final `finish_session` releases the lease and increments its generation.

These are source contracts and fake-boundary component checks, not live Docker
or native secure-store qualification. Local-driver tmpfs support, continuous
mount behavior and two-keeper resource headroom still require explicitly
authorized native qualification; no limits or platform gates are relaxed.
