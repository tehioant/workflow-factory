# Combined focused checks

From the repository root, install declared development dependencies and build the local assets before compiling the Tauri test target:

```sh
npm ci --ignore-scripts
npm ci --prefix desktop --ignore-scripts
npm --prefix desktop run build
node desktop/scripts/check-tests.mjs --inventory-only
npm test
```

The first command inventories without running test bodies. It parses direct literal Node registrations in retained `test/` and desktop `test/`, compiles all workspace test harnesses, and asks each Rust harness for `--list`. The combined unique-case count must be at most 200 **before** tests run. The second invocation runs those focused suites and reconciles their actual pass totals with discovery; skips/todos/ignored cases are refused. Generated Rust test targets are included. Keep test registrations direct and names literal; do not hide additional cases in wrappers or parameterization. Source review remains necessary to enforce the no-journey policy; an inventory gate cannot classify test semantics.

Optional `--output /absolute/path/receipt.json` saves the checkout, HEAD, exact inventory and completed counts. The script resolves the repository from its own location, not a workstation-specific path. `--inventory-only` receipts explicitly state that execution did not occur. The caller must keep source stable during verification and bind the receipt to the reviewed tree; a dirty-tree run is not final-commit evidence.

CI uses the same root `npm test` combined gate and forwarded root check/build commands. Local execution remains restricted to admitted focused selections; `--inventory-only` compiles/runs native harness listing and is not a source-only preflight. Existing PR56-only remote CI is separately admitted, not a local full-suite exception. Ubuntu 24.04 x86_64 and macOS 15 arm64 retain five-minute job limits. The macOS 15 Intel job has an explicitly approved ten-minute limit for cold pinned-toolchain setup and native compilation; no other job or test limit is increased. All discovered focused checks, the 200-case cap and all three native targets remain required. Native debug packages are compiled but never launched, uploaded or released. No signing purchase, authentication, Docker call or model work is performed. A workflow file is not evidence that those target jobs have passed; local-only branches have no remote CI receipt. Report any exceeded budget honestly rather than silently increasing it or weakening coverage.

Only CI disables dev/test debug symbols and incremental compilation (`CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`) to reduce compilation and cache archive work. Debug assertions, all focused checks and native packaging remain enabled; local defaults, release profiles and toolchain pins are unchanged. The Rust cache key includes architecture and this profile policy so older symbol-heavy artifacts are not restored.

The dependency-only Rust cache retains registry archives/indexes, Git object databases, compiler fingerprints, build-script outputs and dependency libraries/metadata. It excludes extracted registry sources, incremental state, project/test executables and package bundles. Cargo regenerates those one-shot outputs; no test, build or validation command is removed. The `desktop-deps-v2` key separates this artifact selection from the old whole-target cache. This reduces post-job archive/upload work; exact-head and integrated CI still have to demonstrate completion within the unchanged limits.

The lockfile-specific key is preferred. On a lockfile change, a restore prefix may reuse dependency inputs only within the same `desktop-deps-v2`, OS, architecture, pinned Rust version and debug/incremental policy. Cargo still validates fingerprints and rebuilds changed inputs under the existing locked check/build commands; a cache match does not qualify source or skip a check. No broad cross-platform/profile fallback is allowed. Retained cache metadata can establish an eligible input, not predict native timing: exact-head and integrated runs must still finish within the existing five/five/ten-minute limits.

CI installs use `--no-audit --no-fund` so advisory requests do not stall installation on each native runner. Both lockfiles are still audited once on Ubuntu in a separate non-gating step, bounded to one minute with ten-second requests and no retries. Findings or an unavailable advisory endpoint remain visible in that step; they do not suppress required checks or extend the applicable job limit.

Desktop-only static source inventory is **111 registrations: 6 root helper/config checks, 36 frontend and 69 Rust**. This is not compiled native harness discovery or a completed full-suite result. Root checks retain three egress predicates (vetted DNS/address allowlist, TLS ClientHello parsing, CONNECT/SNI mismatch), two metrics predicates and one non-launching launch/config invariant. No browser suite remains; root helper checks are required and nonempty.

The standalone `desktop` frontend test command does not include root helpers or Rust cases. Use the combined gate for delivery. There are no runnable Rust documentation examples at the foundation checkpoint; any future doctest or other test framework must be added to the aggregate discovery contract before adoption.
