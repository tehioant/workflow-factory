# Subscription authentication: execution is blocked

Authoritative research: `/home/ops/reports/workflow-factory-desktop/auth-feasibility.md`.
Codex source pin: 0.166.0, commit `6ff1e1ca3b30fc8b914720681fcf5fe62e95f856`.

There is currently **no approved supported staging route** preserving the required
independent concurrent sandboxes. Therefore `auth_readiness` returns
`unsupportedConcurrentAuth`, `credentialSource: notInspected`, and
`executionEnabled: false`. It does not inspect credential files, keyrings, host
processes, environment secrets, or Hermes credentials. This is a support-status
report, not a successful login check or certification of missing credentials.
There is no execution command in the host and the Run control is disabled.

Copied per-run refresh grants may race against each other and the ordinary host
CLI. Moving a copied grant to a different store does not create an independent
session. A file/keyring presence check would not prove sandbox authentication.
The external-token app-server bridge is explicitly internal-only/unstable;
mechanical source support is not a public third-party integration contract.
Access-only staged snapshots likewise need an explicit support/product decision
and have hard expiry/revocation limitations. Neither has been implemented.

Before execution: choose an approved publicly supported route, establish truly
independent credential ownership and secure Linux/macOS storage, prove concurrent
refresh/session behavior, staging lifetime/cleanup and exact-host containment.
No automatic login, refresh, credential staging, host-auth writes, paid API-key
fallback, concurrency reduction, host daemon, or model/network calls are present.
Editing and non-secret configuration management remain available without login.

SQLite stores only typed configuration identities, names, model identifiers,
reasoning effort and revisions. Unknown transport fields are rejected; there are
no token/API-key/path fields or arbitrary provider options. This is not encryption
at rest or a claim that user-entered names/prompts cannot contain sensitive text.
Model identifiers are syntax-checked, not certified available for a subscription.
