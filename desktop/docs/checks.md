# Combined focused checks

From the repository root, install declared development dependencies and build the local assets before compiling the Tauri test target:

```sh
npm ci --ignore-scripts
npm ci --prefix desktop --ignore-scripts
npm --prefix desktop run build
node desktop/scripts/check-tests.mjs --inventory-only
node desktop/scripts/check-tests.mjs
```

The first command inventories without running test bodies. It parses direct literal Node registrations in retained `test/` and desktop `test/`, compiles all workspace test harnesses, and asks each Rust harness for `--list`. The combined unique-case count must be at most 200 **before** tests run. The second invocation runs those focused suites and reconciles their actual pass totals with discovery; skips/todos/ignored cases are refused. Generated Rust test targets are included. Keep test registrations direct and names literal; do not hide additional cases in wrappers or parameterization. Source review remains necessary to enforce the no-journey policy; an inventory gate cannot classify test semantics.

Optional `--output /absolute/path/receipt.json` saves the checkout, HEAD, exact inventory and completed counts. The script resolves the repository from its own location, not a workstation-specific path. `--inventory-only` receipts explicitly state that execution did not occur. The caller must keep source stable during verification and bind the receipt to the reviewed tree; a dirty-tree run is not final-commit evidence.

CI uses the same combined gate and retains the original five-minute job limit. The matrix declares Ubuntu 24.04 x86_64, macOS 15 arm64 and macOS 15 Intel. Native debug packages are compiled but never launched, uploaded or released. No signing purchase, authentication, Docker call or model work is performed. A workflow file is not evidence that those target jobs have passed; local-only branches have no remote CI receipt. If a cold native build exceeds the existing limit, report that incomplete gate rather than silently increasing it or weakening coverage.

The standalone `desktop` frontend test command does not include retained browser or Rust cases. Use the combined gate for delivery. There are no runnable Rust documentation examples at the foundation checkpoint; any future doctest or other test framework must be added to the aggregate discovery contract before adoption.
