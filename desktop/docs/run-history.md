# Retained run inspection (#32 / D09)

Open a workflow, then use **Retained run history · read-only** below the active
run panel. History does not change the working copy, retarget Stop, or start,
retry, reconcile or clean up a run. Refresh reads only the retained page.
**Older runs** follows a workflow-scoped keyset; **Newest page** returns to the
beginning without accumulating a history buffer. Records are ordered by SQLite
retained row order, not an invented execution date.

## Data boundary

- `runs::history` reads existing v3 `runs` rows through a read-only connection and
  one deferred read transaction per command. No migration or new service.
- The list accepts 1–50 entries (UI: 20), reads one extra ID for pagination,
  and returns at most 240 UTF-8 bytes of input summary per entry.
- Run-ID cursors must exist under the requested captured workflow identity.
- Detailed stored request + snapshot + status JSON is capped at 2 MiB **before**
  Rust materialization. Request IDs and list IDs are bounded in SQL too. Input
  expansion is bounded before historical preparation/validation.
- The public DTO is a typed allowlist: captured definition, prepared inputs,
  image identity, non-secret settings, status, diagnostics and durable results.
  Raw request/snapshot JSON, session/resource records and unknown extra fields
  are never returned. This is not a credential scanner for user-authored text.
- Schema/domain/revision and row/request/workflow/block correlations are checked
  against captured data only. Invalid or oversized scoped records are explicitly
  unavailable and do not suppress other entries in the page.
- Legacy missing plans/blocks remain readable with legacy settings and final
  response; missing executed prompts/per-block settings are labelled unavailable.
  Modern plans use each block's own captured settings, not the first setting or
  today's configuration. Unexecuted successors have no invented actual prompt.
- An active retained row can still change as the existing executor writes it;
  inspection is read-only, not a promise that an active run is terminal.

## Committed criterion bindings

| Criterion | Production binding | Focused evidence |
| --- | --- | --- |
| C1: paginated distinct retained runs and captured detail | `engine/src/runs/history.rs` → two database-only host commands → `commands.ts` → `RunHistory.svelte`, mounted by stable workflow ID in `App.svelte` | `fhistory_prepared_rows_project_scoped_bounded_immutable_capture`; `fhistory mounted selected capture rejects stale identities and renders only escaped text` |
| C2: independent of live definition/configuration/runtime/auth/resources | Only captured `runs` rows read; no retry/persist/runtime/session calls or mutable joins | Direct SQL rows with absent mutable records, scoped cursor/detail refusals, unchanged stored request/snapshot/status bytes after reads |
| C3: separate read-only inspection, escaped text, no extra credential fields | Explicit DTO/exporter/permissions; text and `pre` rendering; independent list/detail generations plus workflow/page/run/alive checks | Unknown credential-extra exclusion; actual mounted deferred fake commands, stale success/error/finally, identity mismatch, current failure and unmount assertions; hostile strings remain text |
| C4: focused checks, no lifecycle journey | Exactly one prepared-row Rust case and one mounted selected-view case | No admission, executor, save/reopen, auth, containers, services or native window invoked by either new case |

The new module is a cohesive child of `runs`; `engine/src/lib.rs` is unchanged.
Generated command permissions and transport declarations are part of this seam.
Local raw argv/exit/log and byte/mode evidence is retained by the delivery owner
under `d09-build`, not represented as live/native UI qualification. Native runtime,
login, container-removal and platform UI qualification are deliberately separate.
