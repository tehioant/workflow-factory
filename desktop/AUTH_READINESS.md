# Codex subscription availability; execution remains blocked

## Explicit manual source support

Only **Check Codex credential availability** requests the read-only Tauri
`auth_readiness` command. Mounting/opening the app never inspects credentials.
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

## Container authentication and concurrent refresh blocker

There is **no implemented execution/staging route in this app** yet. Every availability result,
including `available`, preserves `unsupportedConcurrentAuth` and
`executionEnabled: false`. Host material availability is not successful
in-container authentication. There is no execution command; Run stays disabled.

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

#29 may implement separately acquired app-owned managed sessions with secure
native storage, exclusive restore → run → persist-updated-cache → cleanup
leases, and private staging. Host-file availability does not create such a
session or authorize importing/copying the ordinary host login. Different
independently acquired sessions may serve different runs; neither docs nor
source guarantee unlimited same-account session independence/non-interference.
A failed write-back or lost staged cache must quarantine the slot for explicit
reauthentication rather than reuse a possibly obsolete seed. Secure native
store behavior, tmpfs/host/VM paging and diagnostic persistence constraints,
trusted-private scope, containment and cleanup remain unmet implementation and
platform-verification prerequisites. The current blocker is those unimplemented
operations/unverified gates, **not absence of a public cache consumption route**.
#29 must enforce these gates at backend admission, not only in UI.
No automatic login/status process, refresh, network/model call, paid API-key
fallback, host-login overwrite, plaintext conversion or concurrency reduction is
introduced. Consult [Codex setup](https://developers.openai.com/codex/auth) for
your own subscription login; this app does not start or modify it.

Named configuration/revision/reference behavior remains unchanged. SQLite
contains typed nonsecret settings, not credential DTOs or source content.
User-entered names/prompts are not automatically secret-detected or encrypted;
model syntax validation is not subscription entitlement.

## Verification limits

Routine checks use synthetic source bytes/errors, synthetic metadata and a
single mounted component with an injected command. No real credential file,
keyring, CLI login, container, model or native webview interaction is exercised.
Native macOS/Ubuntu interaction and live secure-auth/concurrency acceptance
remain unperformed; source checks do not qualify execution.
