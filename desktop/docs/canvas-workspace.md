# Canvas workspace: local integrated source status

This describes the desktop source candidate for spec #55, not a release, native
interaction qualification or proof of supported execution authentication. The
[behavior spec](../../docs/specs/canvas-first-redesign.md) and
[DESIGN.md](../../DESIGN.md) remain the acceptance target. Concept images are
illustrative data, not screenshots of this implementation.

## Reachable source paths

- App mounts the canvas-first shell, searchable workflow browser, contextual
  properties/block inspector and real RunWorkspace. Draft edits remain explicit;
  load/save failure retains them. Configurations stays mounted when hidden, so
  configuration drafts and shutdown save/identity authority survive navigation.
- Run preparation selects an exact saved definition, validates its inputs and
  captures configuration revisions. An ambiguous admission keeps the original
  request and only permits an exact retry; Settings/history navigation does not
  clear it. Native readiness remains authoritative.
- Admission, queue selection and validated history selection open the run drawer.
  History supplies captured detail, not a fabricated requestId: App reads the
  exact selected RunStatus before adopting it. Unavailable/mismatched status is
  an error, not invented acceptance. Competing selections invalidate older reads.
- Inspect on captured canvas renders the selected run's captured definition and
  configuration summaries, with CapturedRunBlock's retained resolved prompt.
  It never loads/replaces the editor draft or uses current configuration data.
  Editing is disabled and guarded. Return to editing clears only snapshot view
  state, retaining the exact in-memory draft and block selection. Shutdown Save
  still acts on the retained workflow/configuration drafts, not the snapshot.
- The drawer provides Output, Detail, Diagnostics, Inputs and Artifacts, explicit
  selected cancellation/authorization/recovery and raw-state distinctions plus
  cleanup holds. Closing disposes view subscriptions without stopping execution.
  No duration, price or token estimates are manufactured.
- Local FreeSans/FreeMono and complete notices are bundled; no remote font fetch.

## Verification boundary and remaining acceptance

The integration includes a registered mounted App snapshot/return regression,
retained shell-navigation check and focused pending-admission/helper checks.
The App check uses a prepared editor, fake command/native boundaries and a graph
adapter recorder; it mounts the real run workspace and captured-block inspector.
It checks a view-only snapshot/return transition, not a full execution journey or
physical graph-pointer delivery. Test registrations remain within the aggregate
200-case source inventory; source counts are not native harness discovery.

These checks do **not** complete A1–A9. Combined simplification and independent
review remain pending. Exact remaining predicates include:

- A1/A9: inspect integrated shell/drawer geometry and contained overflow at
  1920×1200, 1440×900, 1280×800, 960×720 and true 200% zoom; check long holds/output.
- A2: one failed workflow load retains draft/selection; Save-and-continue load
  occurs only after confirmed persistence, with conflict/failure retaining edits.
- A3/A4: mounted actual inspector Add/Move/Connect/Remove preserves unrelated
  content; typing never triggers graph shortcuts; reference insertion replaces
  only the prepared range and restores the caret/focus using stable-ID syntax.
- A5: chosen saved inputs differ from dirty declarations; failed save/unready
  state performs no admission; an admitted reply opens the real drawer without
  altering a later pending request. Exact pending retry has focused coverage.
- A6/A8: mounted validated history/queue callbacks select only exact real status
  and open the drawer; mismatched/late/unmounted responses have no effect. Seed
  these callbacks independently rather than executing preparation/admission.
- A7: seed all backend states, terminal result plus hold, unavailable/incomplete/
  truncated/empty data; verify selected action/poll exclusion and late-response,
  clipboard-failure, artifact-generation and close/tab subscription disposal.
- A9: focus traps/restoration, complete tab labels/relationships, keyboard-only
  alternatives, measured text/control contrast, reduced motion and native
  Linux/macOS interaction remain separately unqualified.

Canonical full-suite execution and PR publication remain **HOLD**: inherited
multistage fixtures need a separately approved exception or conversion. No held
journeys were run for this integration. The existing canonical runner and all
inherited checks are retained; a focused local pass is not canonical/CI success.
No push, PR, main merge, issue closure, service/container/auth/model call or native
application launch is part of this local checkpoint.
