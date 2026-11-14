# Canvas workspace: locally reviewed source

The desktop redesign for spec #55 is implemented and locally code-reviewed. This
is a source candidate, not a released binary, native interaction qualification or
proof of supported execution authentication. The [behavior spec](../../docs/specs/canvas-first-redesign.md)
and [DESIGN.md](../../DESIGN.md) remain the acceptance contract. The three design
concepts contain illustrative data, not application execution results.

## Available behavior

- Canvas-first shell, searchable workflow browser, contextual workflow/block
  inspector, and a collapsible run drawer. Explicit edits remain separate from
  saved definitions. Failed saves/loads retain the draft. Configuration drafts
  remain mounted across destinations and retain their shutdown-save authority.
- Run preparation chooses an exact saved revision. Changing that choice keeps
  only declared input keys while preserving matching values literally. Unknown
  admission outcomes retain the original immutable request for exact retry;
  opening Settings or history does not create another request. Native readiness
  remains authoritative.
- Reference insertion uses stable input/block IDs and the selected prompt range.
  Only connected predecessors are offered. Invalid syntax, unknown inputs,
  invalid output references and unavailable configurations have adjacent,
  semantically associated guidance. Cancel/Escape returns to the picker trigger;
  insertion returns to the textarea caret.
- Canvas keyboard handling excludes editable fields. Space-to-pan is scoped to
  canvas focus rather than the graph library's global shortcut. The existing
  keyboard movement/connection alternatives and guarded domain callbacks remain.
- Admitted runs, validated history selection and queue selection open the drawer.
  History never fabricates a request ID: App reads the exact selected RunStatus.
  Mismatched/unavailable status is an error; competing selections invalidate
  older reads.
- Captured-canvas inspection uses the run's retained definition, configuration
  summaries and resolved prompt. It is read-only and separate from the current
  draft. Return to editing restores the retained draft and block selection.
  Shutdown saving still targets retained editable drafts, not the snapshot.
- Output, Detail, Diagnostics, Inputs and Artifacts retain existing raw states,
  cleanup holds and explicit selected cancellation/authorization/recovery.
  Closing the drawer disposes view subscriptions without cancelling execution.
  No timing, token or price estimates are invented.
- Settings owns the queue-capacity editor; Runs retains read-only queue browsing.
  Configurations, readiness, native sign-in and safe shutdown remain reachable.
- Local FreeSans/FreeMono assets and complete notices are bundled; fonts do not
  require remote requests.

## User-approved light-theme continuation

Settings → Appearance now offers **Dark** (the unchanged default) and **Light**.
The explicit choice is saved locally when storage is available, restored before
App or SignIn mounts, and never mutates workflow/configuration/run records. Invalid
preferences fall back to Dark; blocked storage cannot stop rendering or switching.
The light palette covers the actual draft/captured Svelte Flow canvas, panels,
fields, states, reference menus, dialogs, focus and hover. Lime stays an action
fill with dark text; essential light-surface accents/focus use darker olive.

This user-approved extension supersedes only the original light-theme exclusion.
The original concept PNGs are untouched. Focused shell/canvas cases extend the
same presentation invariants without new registrations; original assertions remain.
Local RED/GREEN, contrast, static inventory, check/build and input pins are saved
under `/home/ops/reports/workflow-factory-light-theme/`. New light/dark built-bundle
visual acceptance, simplification and independent review belong to the parent;
prior dark-only review/captures below do not certify this continuation.

## Completed local verification

The stabilized implementation received independent **Standards PASS** and
**Spec source PASS**, after correcting all seven confirmed review findings.
One optional ordering-helper duplication suggestion remains nonblocking.

- **21 unique registered focused cases passed** on the stabilized source, with
  mocked external boundaries. Coverage includes actual App snapshot/return,
  immutable pending retry, saved-revision values, picker caret/dismissal,
  contextual guidance, Settings ownership, canvas callbacks and retained helpers.
  No overlapping rerun is added to this count.
- Separate prepared mounted probes verified drawer-close ownership of a late
  inspection subscription, late success/error/finalizer isolation after selection
  changes, and actual App adoption of a queued reply through the bound drawer.
  These are one-off verification evidence, **not additional canonical coverage**.
- Svelte check and the production frontend build passed. Check reports **0 errors
  and 2 Canvas accessibility warnings**; the warnings are not suppressed.
- Actual bundled App previews with explicit synthetic reads were inspected at
  1920×1200, 1440×900, 1280×800 and 960×720. No document overflow or preview errors
  were observed. The original fresh narrow captured-inspector focus issue was
  rechecked: focus enters the panel, Escape closes it, and its trigger regains
  focus. Browser component evidence is not native Tauri evidence.
- Static inventory remains **200 registrations**, net zero: 96 root Node,
  35 frontend and 69 Rust source registrations. Source inventory is not compiled
  native harness discovery. Original meaningful assertions remain retained.

## Remaining qualification and delivery gates

These results do **not** declare every A1–A9 predicate fully qualified. True
browser/native 200% zoom, native Linux/macOS keyboard/pointer/screen-reader
interaction, long-content/state variants and an exhaustive accessibility pass
remain unqualified. CSS zoom stress is not a substitute for native browser zoom.
The two Canvas warnings remain visible limitations of the current check result.
Real execution/authentication/runtime qualification is separate from this redesign.

Canonical full-suite execution and PR publication remain **HOLD**: inherited
multistage fixtures need a separately approved exception or conversion. The
canonical runner and inherited checks remain intact; focused local results are
not canonical/CI success. No held journey, native application launch, real
service/container/authentication/model call, implementation-branch publication,
main merge, issue closure or release is claimed by this local delivery.
