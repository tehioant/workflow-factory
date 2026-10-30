# Resource evidence and reproduction

This is a source-delivered POC, not a claim of proven lightweightness. There is no
agreed numeric performance budget. Do not infer an agent workload's cost from an
idle Node process, or treat focused CI as live sandbox/authentication evidence.

## Observed Linux scenario

Observed at **2026-10-30 20:10:38 UTC**. The measured application source is the
issue-10 main tree `4996e7153c1e14d38d6b37393b53b0079631fae6`; this documentation
slice does not change application behavior.

- Machine: available Linux development server, Intel Xeon E3-1231 v3 @ 3.40 GHz,
  8 logical CPUs / 8 available, 31.172 GiB RAM.
- OS: Ubuntu 26.04.1 LTS, Linux 7.0.0-38-generic, x86_64.
- Node v26.9.0, npm 12.2.1; preinstalled native Docker Engine/CLI 29.9.2,
  containerd 2.4.2. No Docker Desktop VM on this Linux host.
- Fresh private empty data directory; one idle foreground app on loopback. No
  browser opened, API authentication check, credential read, image build, workflow,
  model request or new container. The observer terminates only its app child with
  SIGTERM, waits for it, and removes only its own temporary data directory.
- Existing Docker was **not clean/idle**: 3 unrelated containers were running at
  the initial metadata read; 0 Workflow Factory containers. They were not stopped
  or inspected for content. Daemon/shim cost is confounded by those workloads.
- One-second settling period followed by a requested five-second idle interval;
  actual app sample interval 5.079268 seconds. CPU is `/proc` user+system tick
  delta at 100 ticks/s, divided by actual monotonic elapsed time. Runtime processes
  have individual endpoint timestamps. Zero observed ticks means below sample
  resolution, not proof of zero ongoing CPU cost.

| Category | Actual observation | Interpretation |
| --- | ---: | --- |
| App startup wall time | 72.518 ms | Spawn to ready log, empty history, one sample; not UI-ready or cold Docker startup |
| App CPU accumulated at readiness | 70 ms | Sampled process CPU at the observer's ready read |
| App RSS at readiness / idle end | 61.621 / 61.621 MiB | Node server only; no browser or npm wrapper |
| App lifetime RSS high-water mark | 62.863 MiB | `/proc/PID/status` VmHWM, includes startup |
| App idle CPU delta | 0 ms / 0 observed ticks | 0% of one core at this sampling resolution |
| Existing dockerd RSS | 93.438 MiB | Background control plane, with unrelated workloads |
| Existing containerd RSS | 38.500 MiB | Same limitation |
| Three existing shim RSS values | 10.609 / 10.480 / 11.078 MiB | Existing unrelated container infrastructure |
| Runtime control-plane RSS sum | 164.105 MiB | Readable daemon/containerd/shims/proxies; no workload processes |
| Runtime sampled CPU | 10 ms, about 0.197% of one core | Individual process-window percentages summed |
| App + observed runtime RSS sum | 225.727 MiB | Not unique physical memory, not whole-host or agent-workload RAM |
| Host MemAvailable at observation | 17.191 GiB | Context only; unrelated host activity and caches included |
| Host 1/5/15-minute load averages | 0.44 / 0.37 / 0.44 | Context, not app-attributed load |
| Observed idle app exit | code 0, no terminating signal | Graceful SIGTERM on this empty-data idle process only |

RSS sums double-count shared pages and exclude the unrelated containers' workload
processes, browser memory and filesystem cache. Container memory is not secretly
being counted as zero; it is **outside this app-idle measurement**. This observed
runtime baseline is not the incremental cost of starting a clean Docker engine.
No performance threshold or cross-platform comparison follows from one sample.

After the build, `du -sk dist node_modules runtime` reported 144 / 48,168 / 16 KiB
(0.141 / 47.039 / 0.016 MiB). These are local allocated directory sizes: generated
app assets, development dependencies, and checked-in runtime sources respectively.
They exclude the Git object store, Node installation, Docker installation/image
layers, browser, retained user data and workspace growth. They are not an installer
size. Both full `npm audit` and runtime-only audit reported zero vulnerabilities
at the observation; this does not audit the unbuilt container image.

## Reproduce the Linux idle observation

From the checkout, with Node/npm installed and the local Docker engine already
running if its baseline is wanted:

```sh
npm ci --ignore-scripts
npm run build
PORT=4319 MEASURE_SECONDS=5 node scripts/measure-idle.mjs > idle-observation.json
du -sk dist node_modules runtime
```

The port must be free. The manual observer creates a private directory under the
OS temporary directory (`TMPDIR` can point to a private scratch folder), launches
`node dist/server.js` directly, reads only process/system resource counters and
Docker version/container-count metadata, then performs the idle cleanup above.
It does not load the user's existing data/authentication or open a browser. Read
its JSON rather than assuming a run succeeded: errors exit nonzero. It is **not**
part of `npm test` or CI. Only its pure stat parsing and CPU-unit arithmetic have
unit tests. `MEASURE_SECONDS` accepts integers 2–60; no fixed result is promised.

For app startup comparisons, repeat manually under the same recorded conditions
and report the distribution, existing-history size and cold/warm cache policy.
Keep Node's startup separate from dependency install/build, browser startup,
Docker engine/VM startup and image build/pull. `npm start` also retains npm's
wrapper process, unlike the direct-Node observation above; count it separately.
Never stop unrelated containers merely to improve a number. If a genuinely quiet
runtime baseline is unavailable, keep that limitation explicit as here.

## Execution measurement procedure — not performed

Only after separate authorization for live model usage/resource work:

1. Record host/OS/Node, Docker engine/Desktop version and socket, existing workloads,
   selected model/reasoning, fixed nonsecret inputs/prompts, history size and image
   ID. Confirm a valid private ChatGPT file login without recording its contents.
2. Capture app, browser, daemon (or macOS VM) idle/startup baselines separately.
   Record image size with `docker --host "unix://$WORKFLOW_FACTORY_DOCKER_SOCKET"
   image inspect workflow-factory-runtime:local --format '{{.Size}}'`; distinguish
   shared/reused layers from additional disk allocation. No image size is measured here.
3. Manually run one agreed representative chain in each workflow-level isolation
   mode, separately, not as an automated acceptance campaign. Use each actual run
   ID to select only `wf-<run-id>-agent` (or `wf-<run-id>-block-<block-id>`) and
   `wf-<run-id>-proxy` in Docker stats. Sample while they exist, noting that a
   block's agent disappears at block cleanup. Count both helper and agent cost;
   do not substitute the entire Docker host's container total for this run.
4. Record elapsed startup-to-admission, queue wait, sandbox preparation, each block,
   cleanup and total wall time with an external monotonic observer. UI polling is
   1.5 seconds and is not precise latency instrumentation. Keep subscription/model
   latency distinct from local compute. Retain safe metrics, never auth/tool content.
5. Record app/browser RSS or PSS, daemon/VM memory and CPU, selected container stats,
   private workspace growth (`du -sk` on that run's workspace), retained history
   disk growth and image allocation. Explain overlapping accounting; adding a VM's
   resident memory to its contained workloads double-counts them.
6. Verify only the run's resources stop, outputs remain, and the post-run baseline
   returns as expected. Report both raw data and the still-confounded/unmeasured
   parts; do not turn this procedure into an E2E test or claim it was executed here.

Both isolation modes share only their own run workspace. Workflow mode has one
agent plus one trusted helper per active run. Block mode uses a distinct agent
per block sequentially plus that helper; it does not create a simultaneous agent
for every block. Both currently cap each agent **and helper** at 1 GiB memory,
1 CPU and 128 PIDs; those are ceilings, not observed use, and exclude daemon/VM,
browser, workspace disk and other processes. The default two-run concurrency can
therefore admit four containers. No workspace disk quota is implemented.

## macOS procedure and verification boundary

The source commands in README apply on macOS with Docker Desktop's Linux engine,
its local Unix socket and shared data-directory access. Include Docker Desktop's
Linux VM and backend/virtualization processes, not just the Node app or menu-bar
UI. Use Activity Monitor or `ps -axo pid,ppid,rss,time,comm` to identify the actual
Node/npm/browser and Docker Desktop/backend/VM processes for the installed version.
Sample cumulative CPU deltas over a recorded interval; use Docker stats only for
selected containers and explain overlap with the VM total. Do not assume a Linux
`/proc` observer works on macOS or that process names are stable across Desktop
versions. Record VM configured limits separately from resident host memory.

No native macOS UI, Docker Desktop startup/VM measurement, image build, live
containment, authenticated execution, workload cost or signal-during-Docker-create
exercise has been performed. Linux/macOS CI checks only installation of test/dev
dependencies, focused tests, typecheck and source build. The Linux idle process
observation above is real but does not fill those live verification gaps.

Delivery is source checkout only: no binary packaging, signing, uploads, release
pipeline or purchased service is introduced.
