# Codex subscription availability and separate managed-session authority

Settings reports app-owned managed-session metadata, not the ordinary host login. Conditional Linux and macOS execution paths are implemented in source; all native runtime/security/consent gates still apply. Live authentication and native qualification remain unperformed. See [execution](docs/single-block-run.md) for the current path. The legacy host-file inspector below never imports a login or authorizes execution.

## Settings → Run preparation

- **Check managed sessions** explicitly reads `managed_session_status`: available and withheld counts plus sanitized guidance. Opening Settings does not inspect host credential files, probe Docker, or start sign-in. Available metadata is not proof of current keyring access, provider validity, native security qualification or permission to execute.
- **Open native managed sign-in** opens the dedicated sign-in surface. Beginning acquisition is a separate explicit action with native prerequisites and scoped confirmation. Only that window can display transient provider URL/code information; Settings receives no credentials or device codes. Never send credentials or device codes in chat.
- **Check local Docker readiness** separately inspects Docker and the pinned image. A successful check is not an authentication or security check, and its `executionReady: false` is not a claim that the separate executor is unimplemented. See [runtime guidance](docs/runtime.md).
- Run preparation shows managed-session counts and selected saved revisions. No available slot? Open Settings to review sessions or use native sign-in. Withheld slots need explicit native recovery, not host-login import or a copied old cache. Queuing may wait for an available session; native execution requires a fresh exact reservation and single-use confirmation.

These actions never build an image, enable execution, change security settings or spend money automatically. Native refusal remains authoritative; do not bypass FileVault, swap/core, Keychain or containment checks.

## Explicit manual source support

The retained legacy read-only Tauri `auth_readiness` command runs only on an
explicit request; Settings no longer offers this inspector as an execution setup step.
Mounting/opening the app never invokes this host-file inspection.
On supported Unix hosts it checks exactly the host home directory's existing
`~/.codex/auth.json`. The location is resolved in Rust; JS cannot supply a path.
This is an explicit choice to inspect that file, not automatic selection of the
active Codex store. `CODEX_HOME`, config files, environment bearer values,
keyrings, ephemeral stores and Hermes credentials are not read or searched.
A missing file does not mean a keyring login is missing. Custom homes and other
stores are unsupported; do not convert/export an existing login for this app.
Non-Unix file inspection is unsupported.

Source pin: Codex **0.166.0**, commit
`6ff1e1ca3b30fc8b914720681fcf5fe62e95f856`:
[storage schema](https://github.com/openai/codex/blob/6ff1e1ca3b30fc8b914720681fcf5fe62e95f856/codex-rs/login/src/auth/storage.rs),
[token schema](https://github.com/openai/codex/blob/6ff1e1ca3b30fc8b914720681fcf5fe62e95f856/codex-rs/login/src/token_data.rs).
This inspector deliberately supports a conservative subset: an object with
`auth_mode` absent/null or `chatgpt`, `OPENAI_API_KEY` absent/null, a `tokens`
object containing `id_token`, `access_token`, `refresh_token`,
`account_id`, optional `login_owner_id` and optional `oauth_provenance` (an object
with exactly `issuer` and `client_id`); optional strings use the same 16,384-byte
nonempty ASCII graphic bound. There is optional absent/null/string `last_refresh` (1–64 ASCII graphic
bytes). All four token/account fields must be nonempty ASCII graphic strings
of at most 16,384 bytes; the ID token must have three nonempty base64url-shaped
segments. No JWT signature, claim, timestamp, validity, expiry, revocation,
entitlement or provider authentication is verified. Account identity is required by this conservative inspection subset; nullable
account identities are refused rather than inferred from claims. Unknown
fields/duplicate fields/wrong types/incomplete shapes are
malformed; other auth modes, API-key material and known other auth families
are unsupported, including mixed subscription/API-key files.

Backend reads are descriptor-relative from the root, with no-follow directory
and file opens and no symlink parents. Ancestors must be root/current-user owned
and not group/world writable; the `.codex` directory must be current-user owned
and private. The source must be an owned, private, nonexecutable regular file
with one link and at most 65,536 bytes. Special mode bits are refused. Opens are
nonblocking; reads are capped, with post-read size/change checks. No file or
parent is created, repaired, chmodded, copied or persisted. Unsupported safety
conditions fail closed. The DTO contains only typed source/availability/support
categories, false execution enablement and fixed guidance, never tokens, raw
JSON, account identity, raw paths, parse errors or underlying I/O errors.
Availability outcomes: `notInspected`, `available`, `missing`, `unsupported`,
`malformed`, `inaccessible`, `unsafe`. A failed manual request clears stale UI
results. Credentials remain confined to transient backend memory; this is not a
memory-forensics/secure-zeroization guarantee.

## Host-file inspection is not managed-session authority

There is a gated source execution/staging adapter and a separate app-owned native sign-in surface. No native platform is qualified yet. Every host-file availability result,
including `available`, preserves `unsupportedConcurrentAuth` and
`executionEnabled: false`. Host material availability is not successful
in-container authentication. `start_run` records an immutable captured workflow request or an explicit blocked outcome; it never derives managed-session authority from this host-file inspection. Conditional Linux and macOS admission use separately acquired app-owned sessions and native runtime/security/consent gates. Unknown or unsupported prerequisites fail closed.

Copied per-run refresh grants can race with each other and the ordinary host
CLI; moving a copied grant to a different store does not create an independent
session. An application mutex cannot coordinate unrelated CLI use. The external
token app-server bridge is explicitly internal-only/unstable and is not used.
No access-only snapshot route has been approved or implemented either.

A documented public building block does exist:
[Maintain Codex account auth in CI/CD](https://developers.openai.com/codex/auth/ci-cd-auth)
([first-party redirect](https://learn.chatgpt.com/docs/auth/ci-cd-auth)) describes
restoring a managed ChatGPT cache from secure storage, running ordinary Codex,
and persisting the **updated** cache. Each cache belongs to one machine or
exclusive serialized job stream; do not clone a grant across concurrent jobs
or reseed from an obsolete original. This route is for trusted private
infrastructure/workloads, not public/open-source repositories or untrusted work.
The [CLI-specific auth guide](https://developers.openai.com/codex/auth.md?surface=cli)
documents managed login and container cache consumption. No internal token
injection interface or host refresh API is needed.

Historical #29 design requirements (now implemented as gated source, not live-qualified): separately acquired app-owned managed sessions require secure
native storage, exclusive restore → run → persist-updated-cache → cleanup
leases, and private staging. Host-file availability does not create such a
session or authorize importing/copying the ordinary host login. Different
independently acquired sessions may serve different runs; neither docs nor
source guarantee unlimited same-account session independence/non-interference.
A failed write-back or lost staged cache must quarantine the slot for explicit
reauthentication rather than reuse a possibly obsolete seed. Secure native
store behavior, tmpfs/host/VM paging and diagnostic persistence constraints,
trusted-private scope, containment and cleanup remain mandatory backend admission
requirements. Concrete source operations now exist; live platform/provider verification
is still separate, **not absence of a public cache consumption route**.
The host-file inspector introduces no automatic login/status process, refresh,
network/model call, paid API-key fallback, host-login overwrite, plaintext
conversion or concurrency reduction. Use the separate native managed sign-in
surface for this app; do not export or convert your ordinary subscription login.

Named configuration/revision/reference behavior remains unchanged. SQLite
contains typed nonsecret settings, not credential DTOs or source content.
User-entered names/prompts are not automatically secret-detected or encrypted;
model syntax validation is not subscription entitlement.

## Independently acquired managed slots (#34)

The execution route now supports up to eight separately acquired app-owned slots,
with one pending native acquisition. Every acquisition still starts the existing
CLI sign-in in a fresh empty app-owned home. Each slot has exclusive owner/generation
leasing, independent native secure storage and current-cache writeback. There is no
cache cloning, host import, automatic login, stale-cache reseeding or slot failover.
Native-only bounded comparisons refuse known duplicate login ownership or refresh
material before publication. Same account alone is not a duplicate; separate client
logins do **not** guarantee independent provider token families or account capacity.

Publication comparison/write/readback is serialized, not provider execution. Failed
provider execution, uncertain cache persistence or cleanup retains quarantine and
requires explicit reauthentication. FIFO acceptance itself does not need a free
credential; execution requires the exact reserved session plus a distinct single-use
native confirmation. No credential/account/token fields are exposed in queue DTOs.

Provider concurrency/refresh, native login/keyring/container/consent and macOS
interaction remain **unrun**. Source/fake-boundary checks are not native qualification.
Historical blocker text above describes the host-file inspector and earlier slice;
it does not revoke this separate managed-session source route.

## Verification limits

Routine checks use synthetic source bytes/errors, synthetic metadata and a
single mounted component with an injected command. No real credential file,
keyring, CLI login, container, model or native webview interaction is exercised.
Native macOS/Ubuntu interaction and live secure-auth/concurrency acceptance
remain unperformed; source checks do not qualify execution.
