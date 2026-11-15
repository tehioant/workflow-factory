---
version: alpha
name: Workflow Factory — Canvas Workspace
description: A quiet cool blue-grey workspace with compact AI blocks and restrained blue actions.
colors:
  primary: "#8DAEFF"
  canvas: "#141824"
  rail: "#10141F"
  surface: "#1B2130"
  raised: "#252D3D"
  divider: "#394357"
  control-border: "#7F8FA8"
  text: "#EDF1F8"
  muted: "#A6B0C3"
  secondary: "#97A5BE"
  decorative: "#77859D"
  on-primary: "#101A2E"
  selection-fill: "#243653"
  reference-fill: "#263A5C"
  success: "#91B7F5"
  warning: "#E4C17B"
  danger: "#D7A49B"
  accent: "#8DAEFF"
  focus: "#8DAEFF"
  light-canvas: "#F7F7F7"
  light-rail: "#EEEEEE"
  light-surface: "#FFFFFF"
  light-raised: "#F0F0F0"
  light-divider: "#D2D2D2"
  light-control-border: "#737373"
  light-text: "#242424"
  light-muted: "#555555"
  light-secondary: "#595959"
  light-accent: "#2855C7"
  light-focus: "#2855C7"
  light-hover: "#E7E7E7"
  light-grid: "#D2D2D2"
  light-selection-fill: "#EAEAEA"
  light-reference-fill: "#F0F0F0"
  light-selection-halo: "#DDDDDD"
  light-success: "#285FA3"
  light-warning: "#795400"
  light-danger: "#963F35"
  light-overlay: "#20202066"
  light-panel-overlay: "#20202055"
  light-shadow: "#20202029"
  hover: "#303B50"
  grid: "#394357"
  selection-halo: "#8DAEFF22"
  primary-hover: "#A9C1FF"
  primary-pressed: "#7F9FEA"
  overlay: "#00000099"
  panel-overlay: "#00000088"
  shadow: "#00000088"
  light-primary: "#315DD9"
  light-primary-hover: "#274FBE"
  light-primary-pressed: "#2347AC"
  light-on-primary: "#FFFFFF"
typography:
  title:
    fontFamily: FreeSans
    fontSize: 24px
    fontWeight: 700
    lineHeight: 1.25
  section:
    fontFamily: FreeSans
    fontSize: 18px
    fontWeight: 700
    lineHeight: 1.4
  node:
    fontFamily: FreeSans
    fontSize: 19px
    fontWeight: 700
    lineHeight: 1.3
  body:
    fontFamily: FreeSans
    fontSize: 15px
    fontWeight: 400
    lineHeight: 1.5
  label:
    fontFamily: FreeSans
    fontSize: 14px
    fontWeight: 700
    lineHeight: 1.4
  caption:
    fontFamily: FreeSans
    fontSize: 13px
    fontWeight: 400
    lineHeight: 1.5
  eyebrow:
    fontFamily: FreeSans
    fontSize: 11px
    fontWeight: 700
    lineHeight: 1.4
    letterSpacing: 0.06em
  code:
    fontFamily: FreeMono
    fontSize: 13px
    fontWeight: 400
    lineHeight: 1.6
rounded:
  input: 6px
  button: 7px
  panel: 8px
  node: 10px
spacing:
  xs: 4px
  sm: 8px
  md: 12px
  lg: 16px
  xl: 24px
  xxl: 32px
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.on-primary}"
    rounded: "{rounded.button}"
    height: 38px
    padding: 12px
  button-primary-hover:
    backgroundColor: "{colors.primary-hover}"
    textColor: "{colors.on-primary}"
  button-primary-pressed:
    backgroundColor: "{colors.primary-pressed}"
    textColor: "{colors.on-primary}"
  button-secondary:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.text}"
    rounded: "{rounded.button}"
    height: 38px
    padding: 12px
  button-secondary-hover:
    backgroundColor: "{colors.hover}"
    textColor: "{colors.text}"
  input:
    backgroundColor: "{colors.canvas}"
    textColor: "{colors.text}"
    rounded: "{rounded.input}"
    height: 42px
    padding: 12px
  node:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.node}"
    width: 260px
    padding: 16px
  reference:
    backgroundColor: "{colors.reference-fill}"
    textColor: "{colors.primary}"
    rounded: "{rounded.input}"
    padding: 8px
  status-success:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.success}"
    rounded: "{rounded.input}"
  status-warning:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.warning}"
    rounded: "{rounded.input}"
  status-danger:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.danger}"
    rounded: "{rounded.input}"
  secondary-copy:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.secondary}"
---

## Overview

The product is a local desktop workspace for authoring and inspecting sequential AI workflows. Its primary surface is Command / Inspect: the canvas owns the center; editable properties and historical results appear contextually.

The three historical [reference images](docs/design/canvas-first/README.md) establish this design direction. This file defines shared visual tokens; the [behavior spec](docs/specs/canvas-first-redesign.md) defines interactions and acceptance. These are target specifications, not evidence of an implemented or live-ready interface.

## Colors

### Explicit blue themes

Settings → Appearance provides labelled Dark and Light choices, saved locally as
`workflow-factory.theme`. Dark remains the default; only the exact `light` value
opts in. Missing, invalid or blocked storage uses Dark. Storage write failure does
not prevent switching. Both workspace and SignIn restore before mounting.

Both themes use blue actions, references, focus, selection outlines and semantic
success. Dark uses cool blue-grey surfaces; Light keeps achromatic white/gray
ordinary surfaces, text, borders, overlays and selection fills. Light explicitly
overrides primary, hover, pressed and on-primary: filled blue buttons use white
text rather than inheriting Dark's dark text. Warning amber and danger red are
unchanged. Success and all other states retain textual labels, never color alone.
Typography, spacing, layout, saved preferences and application behavior are unchanged.

[Dark CSS](docs/design/canvas-first/theme.css), [dark DTCG tokens](docs/design/canvas-first/tokens.json),
[Light CSS](docs/design/canvas-first/light-theme.css) and [Light DTCG tokens](docs/design/canvas-first/light-theme.tokens.json)
are documentation derivatives of these normative colors, not runtime theme engines.
The original concept PNGs are historical green-palette illustrations and remain
unchanged; they are not screenshots of either current blue theme. Earlier reports
are historical evidence, not acceptance of this palette. Frontend previews do not
establish native macOS visual acceptance.

- Canvas, rail, surface and raised form a cool blue-grey hierarchy in Dark and an achromatic hierarchy in Light. Dividers/grid/halos are decorative.
- Blue primary identifies the main action; accent/focus marks essential navigation, references and selection. Keep node bodies neutral. Semantic success is blue with a text label/icon.
- Text, muted and secondary copy must maintain at least 4.5:1 against occupied surfaces. Decorative is only for redundant nonessential marks, never readable labels.
- Controls, handles, connections and focus use functional colors with at least 3:1 against adjacent surfaces. Focus is a 2px outline with 3px offset; the clipped full-bleed canvas uses -3px inset so it remains visible. Selection also retains semantic state.
- Primary default, hover and pressed fills must maintain at least 4.5:1 with their theme's on-primary text. Unknown/unavailable states must never be projected as successful.

## Typography

Use FreeSans regular/bold for product UI and FreeMono for template tokens, opaque identifiers and diagnostics. The concepts were rendered with these actual families. At implementation, bundle legally distributable font files and their license notices; document any required fallback as a visual deviation rather than loading a remote font service.

Use the declared type scale. Body copy is left-aligned; labels sit above fields. Eyebrows are short, sparing and nonessential. Do not shrink functional labels below 13px to fit. Truncate long node names with a discoverable full-name tooltip/accessibility name; allow headings, errors and output content to wrap. Full prompt text belongs in the inspector, not a node card.

## Layout

All dimensions are CSS pixels, not physical display pixels. The 1920 × 1200 reference composition has a 72px navigation rail, a 268px left panel, an 82px editor header and a 380px inspector. The center flexes. Inspector content has 24px horizontal padding. Use the spacing scale, with 16–24px between related form groups and 24–32px between sections.

The canvas stretches to the remaining window height. Its dotted grid uses 24px spacing and is decorative. A collapsed run strip is 44px high. The expanded run drawer targets 510px at the reference size, bounded to 45% of the editor's available height; its contents scroll independently. The production interface omits the reference images' 52px concept footer.

The behavior spec defines narrow-window composition. Never scale the whole desktop screenshot down to fit; collapse or replace panels while preserving readable controls and canvas space.

## Elevation & Depth

Surfaces communicate nesting through neutral fills and 1px dividers. A selected node has a slim outline and 4px halo. Popovers/dialogs can have a restrained dark shadow; the normal shell is flat. Avoid blur, glass, glossy gradients and continuously animated connections.

## Shapes

Use 6px input/chip corners, 7px buttons, 8px groups and 10px node cards. Handles are small circular visual marks inside larger interactive targets. Icons are consistent 18–20px line icons. Desktop controls target at least 32px hit areas; touch/coarse-pointer controls target at least 44 × 44px. Visible geometry can remain compact.

## Components

- **Navigation:** 48px active tile inside the rail; accessible label, tooltip and current-view semantics. Hide the concept's avatar because this scope has no account/profile feature. Keep the temporary F mark decorative; this spec does not adopt a new brand identity.
- **Buttons:** one main primary action per context. Run is primary in the editor; Save is secondary. Dialog confirmation may be primary within its own surface. Disabled, hovered, pressed, focused and busy states must be visually distinct; never reduce all parent opacity and thereby weaken text contrast.
- **Fields:** visible label, 42px single-line input, 12px inner padding, control-border outline, contextual error/help text. Required/default rules are expressed as text. Textareas use body typography, with monospace reserved for actual tokens/code.
- **AI node:** 260px reference width, approximately 160px high; icon, type label, name, optional derived prompt preview, then configuration summary. Content may grow modestly for accessibility. Source/target handles are on right/left. No status appears on an editable draft merely because an old run succeeded.
- **Connections:** directional curves, 2px control-border stroke; show selected/focused edge semantics and a named keyboard alternative. The canvas grid stays lower contrast than connections.
- **Inspector:** one contextual object at a time. Workflow properties and block editing are different modes; historical block details are explicitly read-only.
- **References:** the green chip in the concept is a visual explanation, not a requirement for a rich-text editor. Initial implementation uses a plain textarea and an accessible insertion menu that writes literal existing template syntax at the caret.
- **Run drawer:** compact run identity/state header; bounded block list at left, selected output/diagnostics/inputs/artifacts at right. Tab selection has a line and semantic state, not color alone.
- **Feedback:** loading, empty, unavailable, validation error, conflict, busy and hold states have meaningful copy close to the affected action. A toast is not the sole record of a save/admission failure.

Use 100–150ms color/outline transitions where useful; omit automatic graph travel, pulsing status and looping edge animation. Respect reduced motion. Tooltips and popovers work from keyboard focus, close with Escape and restore focus to their trigger.

## Do's and Don'ts

- Preserve the compact charcoal composition in Dark and neutral white/gray composition in Light, with clear typography across secondary destinations.
- Bind controls and statuses to actual command results; retain runtime/authentication failures and incomplete inspection states.
- Keep the three example workflows, prompts, counts, outputs, revision 12 and durations in design fixtures only. Omit unavailable duration fields in production: the inspected transport does not supply them.
- Preserve native text selection, keyboard operation, independent scrolling and clear focus.
- Avoid integration catalogs, branching diamonds, gradients, oversized tiles, invented telemetry, cost estimates and decorative dashboard metrics.
- Accessibility and truthful data binding take precedence over literal reproduction of mockup shortcomings.
