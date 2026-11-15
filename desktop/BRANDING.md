# Counterform app logo

The approved Workflow Factory identity is the **Counterform** symbol: two opposing cut blocks in white on cobalt `#244DF4`. Keep its geometry and colours unchanged. The interface uses the current blue Light/Dark palette in DESIGN.md independently of the unchanged logo.

- `src-tauri/icons/source.svg`: approved 1024-unit vector master.
- `src-tauri/icons/icon.png`: original approved 1024 × 1024 PNG export.
- `public/app-icon.svg`: byte-identical vector for the navigation rail and favicon.
- `src-tauri/icons/{32x32.png,128x128.png,128x128@2x.png,icon.icns}`: native derivatives generated with the pinned Tauri CLI.

From `desktop/`, regenerate derivatives into a temporary output directory:

```sh
npm exec -- tauri icon src-tauri/icons/source.svg --output "$TMPDIR/workflow-factory-icons"
```

Copy only the four native derivative files listed above into `src-tauri/icons/`. Keep `icon.png` as the approved original export and keep `public/app-icon.svg` identical to `source.svg`. The generator also produces files for platforms this application does not target; do not add those files.

These assets came from the original Workflow Factory identity study selected by Antoine. The concept presentation is not shipped. No third-party logo or new font is included. This implementation does not claim trademark clearance.

A rebuilt/reinstalled native application is needed for the OS icon to change; editing source does not update an existing installation.
