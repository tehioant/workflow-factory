---
version: alpha
name: Workflow Factory — Canvas Workspace
description: A quiet charcoal workspace with compact AI blocks and restrained acid-lime actions.
colors:
  primary: "#D1F56A"
  canvas: "#141615"
  rail: "#101211"
  surface: "#1B1E1B"
  raised: "#242824"
  divider: "#343A33"
  control-border: "#79846F"
  text: "#EDF0E8"
  muted: "#A0A89D"
  secondary: "#8A9485"
  decorative: "#737D70"
  on-primary: "#1A230D"
  selection-fill: "#2B3521"
  reference-fill: "#2E3824"
  success: "#A4D5AA"
  warning: "#E4C17B"
  danger: "#D7A49B"
  accent: "#D1F56A"
  focus: "#D1F56A"
  light-canvas: "#F7F8F2"
  light-rail: "#EBEEE4"
  light-surface: "#FFFFFF"
  light-raised: "#EDF1E7"
  light-divider: "#CED5C5"
  light-control-border: "#6C7961"
  light-text: "#232B20"
  light-muted: "#52604C"
  light-secondary: "#53604D"
  light-accent: "#46620D"
  light-focus: "#46620D"
  light-hover: "#E1E7D9"
  light-grid: "#CED5C5"
  light-selection-fill: "#E3EDCF"
  light-reference-fill: "#E8F0D9"
  light-selection-halo: "#D9E7BD"
  light-success: "#35603D"
  light-warning: "#795400"
  light-danger: "#963F35"
  light-overlay: "#202B2066"
  light-panel-overlay: "#202B2055"
  light-shadow: "#202B2029"
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
    backgroundColor: "#DDF88E"
    textColor: "{colors.on-primary}"
  button-primary-pressed:
    backgroundColor: "#BDDE5C"
    textColor: "{colors.on-primary}"
  button-secondary:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.text}"
    rounded: "{rounded.button}"
    height: 38px
    padding: 12px
  button-secondary-hover:
    backgroundColor: "#30362E"
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

The three [reference images](docs/design/canvas-first/README.md) establish this design direction. This file defines shared visual tokens; the [behavior spec](docs/specs/canvas-first-redesign.md) defines interactions and acceptance. These are target specifications, not evidence of an implemented or live-ready interface.

## Colors

### Explicit light-theme extension

The user explicitly approved Light/Dark selection after the original dark-only
scope. Dark stays the default, including its original palette. Settings → Appearance
provides labelled Dark and Light choices, saved locally as `workflow-factory.theme`.
Only the exact `light` value opts in; missing, invalid or blocked storage uses Dark.
Storage write failure does not prevent switching in the current window. Both the
main workspace and separate SignIn mount restore this preference before mounting.

The `light-*` tokens above are the normative light overrides; omitted roles inherit
the dark value. Warm near-white canvas, white panels, green-tinted raised and selected
surfaces retain the hierarchy. Lime `primary` remains an action fill with dark
`on-primary` text; dark olive `light-accent`/`light-focus` is used for essential
icons, references, selected outlines and focus, never pale lime text on white.
Status colors retain textual labels. Grid/dividers/halos are decorative; meaningful
borders and edges use control-border. Hover, overlay and shadow have semantic roles.
Draft and captured Svelte Flow share the selected explicit color mode.

[Light CSS](docs/design/canvas-first/light-theme.css) and
[DTCG light tokens](docs/design/canvas-first/light-theme.tokens.json) are derived
exports of these light overrides. Original dark concept PNGs and historical dark
exports are unchanged and are not light-theme screenshots. Implementation tokens
live in `desktop/src/app.css`. Local measured light contrast and command receipts
are retained in `/home/ops/reports/workflow-factory-light-theme/`; browser/native
visual qualification remains a separate gate.


- Canvas, rail, surface and raised define a small warm-neutral hierarchy. Separate regions with subtle dividers rather than shadows or decorative rails.
- Acid lime identifies the main Run action, active navigation, selected-node outline, focus and prompt references. Keep the rest of a node neutral. Success is a separate muted green and always has a text label/icon.
- Use text for primary content, muted for supporting copy and secondary for essential low-emphasis text. Decorative is limited to grid dots or redundant, nonessential decoration; it is not permitted for readable small labels.
- This corrects a contrast weakness in the concepts: original faint `#737D70` text does not meet 4.5:1 on the dark surfaces. Secondary `#8A9485` does, including on raised surfaces. This is a deliberate accessibility correction, not a new theme.
- Divider is decorative only. Controls, graph handles and meaningful connection lines use control-border, which must maintain 3:1 against adjacent dark surfaces. Focus is a 2px primary outline with 3px offset. Selected graph nodes use a primary outline and restrained selection-fill halo, plus semantic selected state.
- Status is never color-only. Preserve unknown/unavailable states rather than projecting success from a green readiness dot.

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

- Preserve the compact dark-green/charcoal composition and clear typography across secondary destinations.
- Bind controls and statuses to actual command results; retain runtime/authentication failures and incomplete inspection states.
- Keep the three example workflows, prompts, counts, outputs, revision 12 and durations in design fixtures only. Omit unavailable duration fields in production: the inspected transport does not supply them.
- Preserve native text selection, keyboard operation, independent scrolling and clear focus.
- Avoid integration catalogs, branching diamonds, gradients, oversized tiles, invented telemetry, cost estimates and decorative dashboard metrics.
- Accessibility and truthful data binding take precedence over literal reproduction of mockup shortcomings.
