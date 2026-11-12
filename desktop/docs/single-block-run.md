# Desktop execution and bounded FIFO source checkpoint (#29–#34; not live-qualified)

The main window uses generated transport and locally scoped Tauri commands for immutable admission, retained status and scoped stop. Admission binds request identity, exact saved definition/configuration revisions, literal inputs/prompts and the inspected immutable image ID in one SQLite transaction. A duplicate request reads its original outcome without re-resolving mutable configuration. Global capacities default to two occupied and sixteen waiting runs (validated ranges 1–8 / 0–64). Cleanup/persistence holds consume occupied capacity; capacity reductions below existing counts are rejected. Native startup locks this data directory before database access, withholds interrupted runs and quarantines acquiring/leased sessions. It never resumes or deletes name-matched resources.

The worker consumes the captured snapshot, a transactionally reserved app session, native secret storage and real structured Docker operations. Durable ownership intents precede resource creation and returned IDs are recorded before later effects. The real adapter creates private internal IPv4 and separate egress networks, a restricted provider-only helper and an unprivileged bounded/read-only work container. It inspects ownership/configuration afresh, stages cache on private tmpfs through a separate stdin pipe, and feeds only the prompt to Codex. The container-local keeper retains tmpfs after the child exits. Writer-group confirmation precedes export. Current rotated cache must pass schema checks, native secure-store write/readback and positively owned cleanup before release; failures quarantine rather than reuse the seed. Bounded JSONL admits only structured successful final text and redacts staged/rotated token values. Raw stderr and transcripts are never retained.

Only the dedicated `signin` native window has device-prompt commands; main can open that surface and see metadata, not codes. Explicit acquisition uses the pinned CLI managed device flow in an empty isolated tmpfs home; no host grant import, custom refresh broker, auth fallback, API key or new host server. A native keyring adapter uses the app namespace with Secret Service on Linux and Keychain on macOS, without a file fallback.

## Supported source path and operational limits

Admission now has a conditional Linux success path: same-host root dockerd verified from Unix peer credentials/proc identity, no active host swap, no piped core collector, daemon core limits zero, and application core-disabled/non-dumpable controls. No swap/services are changed. Exact runtime image inspection and fresh owned resource checks remain mandatory. Actual cgroup-v2 swap=0 is required before login AND staging. Native app-namespaced keyring write/readback gates availability, never a file fallback. macOS, remote/VM/socket-proxy daemons and unsupported prerequisites fail closed.

Linux requires `/usr/bin/zenity` for a host-owned native question on each acquisition/run. The permission/label-restricted main command captures request/data-directory/revisions and selected session generation. Approval is consumed only by that call, not supplied by IPC Boolean, persistent setting or model. Decline/unavailability authorizes nothing. The user confirms trusted/private infrastructure and trusted private inputs, excluding public/open-source/untrusted workloads. Trusted OS/keyring/admins remain a boundary: no defense against intentional privileged memory capture, hibernation or snapshots is promised. Do not hibernate or snapshot memory while credentials are active.

Unclaimed Admitted stop requires no resource intents transactionally before releasing only the reservation/active marker. Running/uncertain records stay held. Pending setup ownership is installed before fallible login start; cancellation remains operable on failure and retryable after partial deletion. Explicit quarantined retirement requires positive owned resource/recorded-ID absence and deletion/readback absence of the exact old secure item. Errors retain quarantine. Only fresh provider authorization under a new slot may follow; no old grant is read/copied/reused. This is narrow explicit recovery, not general automatic reconciliation.

No real keyring, Docker, login, model, container or native dialog was exercised. Fake-boundary checks establish source contracts, not live provider or platform qualification. Pinned keeper/CLI/Docker findings must be settled before source is declared complete. Full native close/Quit UX remains later scope.

## Bounded concurrent FIFO (#34)

Schema 4 adds queue/settings companion tables without changing the six-column
run record. Migration preserves captures and held session ownership, and removes
only the old single-active index. Every accepted request already owns its immutable
snapshot and monotonic FIFO sequence. Waiting admission does not lease credentials;
full-queue rejection and reservation are transactional. Clean durable completion
validates the exact slot/generation receipt and reserves the oldest waiting request
once. Cache persistence/cleanup uncertainty retains the occupied slot.

The existing captured executor serves both isolation modes. At most eight native
worker permits are held through actual worker return; the database's configured
occupied limit remains authoritative. Release alone cannot reuse a live worker
permit, and worker return alone cannot release durable ownership. Reserved requests
require a new native confirmation bound to request/run, captured revisions/image,
and exact session owner/generation. No blanket approval or automatic work retry.
Declined confirmation retains a visible authorization-required reservation for
explicit Authorize or Stop. Waiting credentials never cause FIFO-head bypass.

The global queue projection is bounded and read-only, separate from selected-run
output-storage retries and immutable history. Queue settings/errors, credential
wait, authorization requirements and ownership holds are visible. Selection keeps
RunPanel and inspection bound to the selected run/workflow, not canvas edits.
Unclaimed queued Stop only dequeues that request. Startup withholds waiting records
without claiming external cleanup or automatically resuming them. Full cancellation,
shutdown and interrupted-resource recovery remain #35–#37, not this slice.

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
