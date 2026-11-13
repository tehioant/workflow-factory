# Canvas-first desktop redesign

## Problem Statement

Workflow authors need to build and inspect sequential AI workflows without navigating an all-in-one form. The desktop currently exposes workflow properties, configuration management, run controls and runtime setup around a constrained canvas. This makes the graph and the selected task harder to understand.

The three supplied design concepts establish a compact charcoal workspace, restrained acid-lime actions, a central canvas, a contextual inspector and a collapsible run drawer. Their static sample results are not evidence of successful execution or runtime readiness.

## Solution

Recompose the existing desktop UI around the author's current task: choose a Definition, connect AI blocks, edit one block, explicitly save, supply run inputs and inspect the immutable Run independently of the current draft.

Use the existing Tauri + Rust, Svelte + TypeScript, Svelte Flow, selected shadcn-svelte controls, Tailwind CSS and Vite stack. Preserve the domain, execution and security contracts in [desktop specification #23](https://github.com/tehioant/workflow-factory/issues/23). This is a presentation and interaction redesign, not a second engine rewrite.

Visual values are maintained in [DESIGN.md](../../DESIGN.md). The [reference index](../design/canvas-first/README.md) pins all three original PNGs and their checksums. Where prose intentionally corrects inaccessible contrast, illustrative data or unsupported mockup controls, the prose takes precedence over the pictures. Existing backend safety contracts take precedence over both.

## User Stories

1. As a workflow author, I want a spacious canvas and a searchable workflow browser, so that I can understand and select the workflow I am editing.
2. As a workflow author, I want compact connected AI blocks and a contextual inspector, so that I can edit prompts, configurations and references without cluttering the graph.
3. As a workflow author, I want clear workflow properties, save status and run preparation, so that my saved definition and accepted execution always use the version I intended.
4. As a local user, I want a collapsible run inspector with immutable snapshots, outputs and diagnostics, so that I can understand execution without confusing it with current edits.
5. As a local user, I want configurations, runtime setup and queue/history access in predictable destinations, so that secondary controls remain available without dominating the editor.
6. As a keyboard user, I want labelled controls, visible focus, non-drag graph actions and usable narrow-window layouts, so that the editor remains operable without precise pointer gestures.

## Implementation Decisions

### Scope, authority and existing seams

- Retain the Definition, AI block, Connection, Workflow input, Codex configuration, Run, Run history, Workspace and Isolation mode vocabulary. A Definition revision is not its schema version.
- Compose the existing editor state/actions, command transport, graph mapping, inspector, configuration, readiness, queue and history surfaces. Keep one authoritative draft and one explicit selected-run identity. View state must not duplicate domain ownership.
- Rust remains authoritative for validation, revision conflicts, persistence, admission, credentials, cancellation and recovery. This spec requires no Definition/Run schema migration or expanded desktop capability. UI selection, tab and panel state need no new database/API framework.
- Preserve explicit saving, idempotent start requests, bounded FIFO admission, immutable snapshots, scoped cancellation, cleanup/persistence holds and safe native close/Quit. Surface the existing behavior; never weaken it to match a clean screenshot.
- Preserve backend credential redaction, withheld diagnostics, approved authentication, enforced sandbox/egress restrictions and bounded artifact/log reads. Relocating controls does not authorize setup, image pulls, model calls or a paid fallback.
- No new package is required solely for the shell, prompt editor or panel behavior. Continue using the existing flow engine and maintained controls rather than building a custom canvas or rich editor.

### 1. Shell and navigation

At the 1920 × 1200 reference viewport:

- Navigation rail: 72px. Left workflow browser: 268px. Editor header: 82px. Right inspector: 380px. Canvas takes the remaining width and height. Match the shared spacing and type hierarchy, not the fixture's exact graph coordinates.
- Rail destinations: Workflows, Runs, Configurations, Settings. Workflows owns the editor. Runs reuses the global queue and definition-scoped retained-history reads; select a workflow to browse its history rather than inventing unsupported cross-workflow archive search. Configurations hosts existing nonsecret Codex configuration management. Settings hosts runtime/authentication readiness and existing queue-limit settings.
- These destinations are in-window views, not a requirement for a new routing framework. Navigation has accessible names, focus feedback, tooltips and active semantics. The mockup's avatar is omitted; no account/profile or collaboration feature is added.
- The left panel contains a workflow search, New workflow, the saved workflow list, the sole supported AI block action and local-storage context. Search filters saved workflow names locally; distinguish no workflows, no search matches, loading and load failure. Retain a visible retry/refresh action on failure.
- Switching workflows protects dirty edits. Switching destinations keeps the same draft in memory; if a transition would replace/discard it, use the same unsaved-change decision. A failed workflow load retains the previous draft.
- Panels and lists scroll independently with fixed section headers. Flex/grid children permit contained overflow. No ordinary editor operation creates page-level horizontal overflow.

### 2. Definition header, saving and workflow properties

- Show the current workflow name, explicit revision/save status, Save and Run workflow. The title opens workflow properties; the workflow name is editable there. Save remains visible whenever editing.
- Save states: new/unsaved, dirty, saving, saved with actual revision, failed and revision conflict. Disable duplicate saves while busy; only report Saved after confirmed persistence. Save failure or conflict retains the draft and a persistent contextual message. Conflict resolution never forces an overwrite.
- Selecting Workflow properties opens its inspector mode. It contains workflow name, named text inputs, required/default controls, isolation mode and connection management. Re-selecting a block replaces this panel with block properties. Closing the inspector expands the canvas without clearing the draft.
- Both existing isolation choices remain: One agent container per run and Separate agent container per block. Explain that both share only that run's private workspace and captured outputs; no claim of filesystem isolation between those blocks.
- Input declarations remain distinct from per-run values. Validate the existing naming/reference rules, show errors beside affected fields and protect against dangling prompt references on rename/removal. Do not silently rewrite templates without explicit author action.
- Manage connections provides labelled From/To controls and a connection list with individually named removal actions. Keep it reachable without drag gestures.
- On a dirty-draft replacement, provide Stay, Save and continue, and Discard and continue. A failed save keeps the author in place. Never silently save or discard through navigation.

### 3. Canvas and block interactions

- Canvas-first composition, dotted 24px grid, compact left-to-right nodes, directional curves, small selection/pan toolbar and unobtrusive zoom/fit controls. The surrounding UI is fixed; graph pan/zoom affects only graph content.
- AI block is the only palette type. Clicking Add block adds one block at an appropriate visible position and selects it. A drag interaction may supplement, not replace, this action. Do not show decorative integrations or branching controls.
- At reference scale, a node is approximately 260 × 160px with icon, AI block label, name, optional short derived prompt preview and saved-configuration name. There is no new description field: the preview derives from the existing prompt or is omitted. Missing configuration is visible as Not configured, not a sample default.
- Node ordinals, when shown, derive from the valid sequential connection order; incomplete/invalid graphs do not receive a misleading execution order. Names and configuration summaries come from the draft, not historical Run records.
- Selected node uses a lime outline/quiet halo and semantic selection. Full prompts stay in the inspector. Support long names with constrained wrapping/truncation and access to the complete name.
- Ports: target left, source right. Connections are directed, keyboard-operable through the connection manager and visibly selectable/removable. Reuse existing validation; self-edges, duplicate/unknown endpoints, cycles and unsupported executable shapes are refused at the applicable save/admission boundary with actionable feedback. Preserve authoring incomplete drafts where the existing model permits it.
- Moving a node marks the draft dirty. Support existing keyboard movement with arrow keys on a focused graph movement handle. Typing arrows/Space in fields never moves/pans the graph. Space-to-pan applies only with canvas focus; zoom/fit have accessible named buttons.
- Removing a block identifies affected connections/references and uses an explicit confirmation when destructive dependencies exist. Do not leave invisible dangling references or silently remove unrelated blocks. Removal retains existing domain validation.
- Draft canvas contains no historical completion badge. To display statuses on the graph, explicitly enter a read-only Run snapshot canvas using that Run's captured definition/positions, not the current draft.

### 4. Block inspector and prompt references

- Selected AI block exposes name, Codex configuration, prompt and Insert reference. Advanced properties contains numeric X/Y position controls. Removal is a separated destructive action, not the main inspector CTA.
- Selecting a configuration changes the draft reference only. Model/reasoning remain managed in reusable configurations; a configuration is not a credential. Missing/deleted references are visibly unavailable. Do not hard-code Default Codex, model entitlement or execution readiness from the concept.
- Start with an accessible plain textarea. Insert reference opens a keyboard-operable menu grouping named inputs and eligible connected predecessor final responses. Insert the actual existing literal syntax at the caret or replace the selected range, then return focus to the textarea. Display labels may say Read changes · final response; stored references use stable block IDs, not names.
- Reference resolution remains literal and run-scoped. Only eligible predecessor output is available; a connection alone never forwards the whole conversation. Existing missing/future-reference validation remains authoritative.
- Keep textarea selection/focus stable through edits and unrelated run updates. Input errors, missing configuration and invalid reference guidance stay close to the associated control.

### 5. Run preparation and readiness

- Run workflow opens a focused preparation dialog for actual input values and saved-revision choice; opening it performs no admission. Show the definition/revision being requested and preserve entered values after an error.
- Clean draft: collect inputs, then explicitly start the saved revision. Dirty draft: require Save and run, Run last saved revision (only if one exists), or Stay. Running the saved revision leaves the unsaved draft intact; saving failures prevent admission. Validate values against the exact chosen saved definition, not a conflicting dirty input declaration.
- Capture expected definition/configuration revisions and supplied values through the existing start request. Disable duplicate submissions while pending. Unknown admission outcome exposes Retry same request with the original immutable request ID and payload; never generate a new execution as an error-recovery shortcut.
- Missing/unsupported runtime or authentication produces an actionable explanation and link to Settings. Do not disable authoring or retained-history access. An unavailable Run control needs an accessible reason; grey styling alone is insufficient.
- The inspected backend reports unsupported authentication/readiness states. The active green Run button in the image is illustrative, not authorization to override these gates. Ready can only be shown when the real command contract confirms it.
- Accepted runs open/select the run drawer, with queued versus active state taken from backend data. Later draft/configuration edits never modify accepted Runs.

### 6. Run drawer, snapshot and state projection

- Default editor has a collapsed 44px Run details strip with No run selected or the selected identity/state. Opening it does not start or retry work. Closing it releases view subscriptions; it never cancels execution.
- Expanded drawer: header with run ID, definition revision, explicit state and known block-completion count; ordered block list at left; Output, Diagnostics, Inputs and Artifacts tabs at right. A read-only selected-block snapshot inspector shows captured configuration and resolved prompt. Keep artifacts available even though the third concept only illustrates the first three tabs.
- The reference drawer is 510px high at 1920 × 1200, bounded to 45% of available editor height. Retain meaningful canvas space. Panel overflow scrolls inside the drawer. Narrow mode may use a dedicated run surface rather than an unreadably short drawer.
- Run-snapshot mode is explicitly labelled with run identity and revision. Return to editing restores the in-memory draft and its selection. Editing controls are unavailable in snapshot mode. A selected historical block must not accidentally edit a same-ID block in the draft.
- Preserve raw backend state distinctions. User-facing mapping: queued → Queued; authorizationRequired → Authorization required; admitted → Admitted; running → Running; cancelling → Cancelling; succeeded → Completed; failed → Failed; stopped → Stopped; blocked → Blocked; held → Held. Show diagnostic reasons and cleanupPending as a separate persistent hold notice, even beside a terminal model result. Unknown values are Unknown status, never Completed.
- A Run being admitted is not proof its first block is running. An absent block result means Not started/result unavailable according to known state, not success. A block failure does not imply later blocks ran. Interrupted recovery follows the backend's actual state/diagnostic projection; do not add a nonexistent frontend enum or resumable control.
- Durations 8s/12s/5s and 25s in the image are fixture data. The inspected transport has no duration/timestamp field. Omit those fields in production unless a separately approved backend extension supplies reliable data; do not estimate with frontend wall clocks. No cost/token telemetry is introduced.
- Output is retained final response, separate from diagnostics, rendered as text or sanitized Markdown. Copy reports success only after clipboard completion and exposes failure accessibly. Untrusted content is never executable HTML.
- Diagnostics/artifacts are bounded retained reads. Respect available, incomplete/truncated, resync, cursor and generation state. Unavailable is not empty, stale content is not current, and withheld text is not fabricated. Preserve redaction limitations, pagination, identity checks and subscription disposal; never expose raw provider streams or arbitrary host paths.
- Cancel run applies only to the selected run through existing scoped commands, with Cancelling versus settled/held outcomes visible. Existing explicit cleanup/output persistence recovery remains available where the backend permits it, without re-running the model. Readiness blockers do not hide retained results.

### 7. Responsive and accessible behavior

These narrow-window rules complete the desktop design; they are specified behavior, not additional supplied image mockups.

- At widths ≥1600px, allow the workflow browser and inspector simultaneously with the reference rail. The canvas absorbs remaining space.
- At 1200–1599px, use a 64px rail and a 360px inspector. Opening the inspector collapses the workflow browser; its trigger remains visible. Closing it can restore the browser. Never retain two wide side panels around an unusable canvas.
- Below 1200px, use one dismissible side-panel overlay at a time, width up to 360px and never wider than the available viewport. Preserve graph/draft state beneath it. Inspector/workflow browser overlays trap focus, close with Escape and restore focus. The header wraps actions into a compact second row rather than overflowing.
- At short heights, keep the drawer collapsed or open run inspection as its own in-window surface. Keep save/run controls visible, and scroll inspector content without losing its title/close action. At 200% UI zoom, use the same narrow layout rules rather than clipping text.
- Reference checks: 1920 × 1200, 1440 × 900, 1280 × 800, 960 × 720, plus 200% zoom. Graph pan is intentional; shell/document overflow is not. This is a desktop-window adaptation, not a new mobile app.
- All rail icons, graph actions, tabs, overflow menus, fields and recovery actions have accessible names. Focus is visible, selection is semantic, errors are text-associated, and status changes use restrained live announcements. Avoid announcing every streamed diagnostic line.
- Provide non-drag add/connect/move actions. Ordinary Delete/Backspace editing in a prompt must never delete a graph object. Use labelled confirmation for consequential deletion. Normal tab order follows navigation → editor toolbar → canvas/inspector → drawer, with contextual dialogs temporarily owning focus.
- Measure normal text at ≥4.5:1 and meaningful non-text affordances at ≥3:1. Use the accessible secondary/control-border tokens rather than reproducing the mockup's faint essential labels. Respect reduced motion.

## Testing Decisions

### Retained approach

The already-approved primary seams remain the engine's public interface and small focused UI/desktop-command checks. This redesign primarily needs prepared-state UI components with mocked Tauri commands; it does not reopen the engine architecture or require a testing interview.

Assert visible behavior from one action or state transition per check. Reuse the existing editor, reference-insertion, command, run-projection and inspection tests. Group useful assertions in focused cases; do not duplicate engine validation or test Svelte Flow internals. Preserve the existing repository-wide ceiling of 200 discovered Rust/frontend cases. Replace redundant coverage rather than raising the ceiling, wrapping many journeys in one test, increasing timeouts or adding sharding.

No automated E2E tests, full controller/workflow journeys, real services, containers, live authentication or model calls. For this documentation-only delivery, validate files, references, token structure and contrast, not the application suite. Native Linux/macOS interaction checks remain separately recorded manual evidence; rendered concepts are not that evidence.

### Acceptance criteria

- **A1 — Composition:** At reference width, rail/browser/header/inspector and collapsed/expanded drawer follow the declared geometry. Canvas fills the remaining space. Narrow states preserve readable controls with contained scrolling and no document horizontal overflow.
- **A2 — Definition integrity:** Prepared dirty/saving/saved/failure/conflict states expose correct copy/actions. A save failure or workflow-load failure retains the original draft. Destination changes do not discard it.
- **A3 — Graph and inspector:** One add/select/move/connect/remove action updates the correct draft object, shows selection and preserves unrelated content. Pointer-free connection and movement are available; prompt keyboard input does not trigger graph shortcuts.
- **A4 — Prompt references:** One insertion at a prepared caret/selection writes the existing literal stable-ID/input syntax, returns focus and changes no other prompt text. Missing configuration and invalid reference states are visible.
- **A5 — Admission:** Prepared clean/dirty/no-saved/invalid-input/unready/failed-save states expose only valid run choices. A pending/unknown admission retains the same request identity; no error creates a second Run.
- **A6 — Historical truth:** A seeded selected Run renders its captured definition, prompt, configuration and output even if the current draft differs. Snapshot is read-only; returning to editing restores that draft. Closing the drawer only disposes its view resources.
- **A7 — Status and inspection:** Seed each declared backend run-state projection directly, including terminal result plus cleanup hold. Unavailable/incomplete/truncated/empty records stay distinct. Ignore mismatched/late run and artifact-generation responses. Do not show fixture durations or invented success.
- **A8 — Secondary controls:** Configurations, readiness/settings, queue, definition-scoped history, cancel and permitted hold recovery remain reachable after relocation. Their rendered states use existing command data and preserve actionable failures.
- **A9 — Accessibility and fidelity:** Confirm focus, labels, semantic selection, keyboard alternatives, measured contrast, 200% zoom and long content. Compare component previews of the three concepts using deterministic sample fixtures; fixture names/results and concept footer do not leak into production.

The acceptance list is not a required one-test-per-bullet inventory. Choose the smallest meaningful focused coverage; visual captures establish composition only, not execution or persistence success.

## Out of Scope

- Engine rewrite, persistence migration, transport schema redesign, custom graph editor, new rich prompt editor or framework replacement.
- Branching/routers/loops, non-AI block types, integrations marketplace, parallel graph execution or additional providers.
- New account/profile, avatars, team collaboration, cloud sync, paid features, token/cost estimates or inferred execution telemetry.
- Automatic saving, automatic execution retries/resume, silent installation/image pulls, relaxed authentication/sandbox gates or provider/billing fallbacks.
- New branding approval, light theme, polished marketing pages, new binary releases, deployments, signing purchases or public-visibility changes.
- Implementation, automatic worker launch, merging, automated E2E or live platform/model qualification from publishing these specs.

## Further Notes

- Source baseline inspected: main commit `0d9e8a09be9efea5e9568a01521355d02cef7d50`. The generated transport confirms current data availability and unsupported readiness, not live execution qualification.
- This focused spec supplements #23 and supersedes its broad layout guidance only where this document explicitly refines presentation/interaction. The parent architecture/security/testing contracts remain in force; older browser startup prose is not this desktop's interaction contract.
- The three PNGs are the selected direction for specification, not implemented screenshots or approval of every decorative detail. The temporary mark, avatar, prompt chip rendering, short faint labels and duration examples are constrained/corrected above.
- Secondary destinations retain the same design system and existing functionality; no additional finished mockups of those destinations are claimed.
- Suggested implementation slices: shell and destinations; canvas/properties/block inspector; saved-revision run preparation; run drawer/snapshot and secondary-state polish. Each slice retains existing behavior and focused regressions.
- Publication/readiness does not start implementation. No unresolved product question blocks this documentation; native interaction quality and runtime readiness remain implementation/qualification work, not assumed successes.
