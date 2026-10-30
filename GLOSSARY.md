# Domain glossary

- **Definition**: durable editable workflow, identified by an ID and schema version.
- **AI block**: the only supported typed block; has a name, prompt and canvas position.
- **Connection**: directed edge establishing a sequential chain. Only a successful predecessor admits its successor; it does not forward a whole conversation.
- **Canvas**: scrollable visual editor with movable blocks and directional connections.
- **Store**: private atomic versioned JSON persistence; filesystem operations are its boundary.
- **Workflow input**: named text declaration, with required/default rules; values are supplied separately for each run.
- **Codex configuration**: reusable nonsecret model/reasoning settings referenced by AI blocks. It is not a login or credential record.
- **Run**: accepted execution instance with an immutable captured definition, resolved inputs/prompts/settings, and evolving statuses/results. Queued instances already own their snapshot.
- **Run history**: durable captured read model, independent of current workflow/configuration edits. Interrupted work never resumes automatically.
- **Workspace**: one private run's shared files, separate from credential staging and from every other run.
- **Isolation mode**: workflow-level choice of one agent per run or separate sequential agents per block. Both retain that run's workspace and captured outputs.
- **Egress helper**: trusted per-run network infrastructure admitting only allowed TLS destinations; additional to the agent container count, not an independent execution service.
- **Cleanup/persistence hold**: terminal work whose safe cleanup or durable save is not yet confirmed. Recovery is explicit and scoped; a queued storage-only retry needs no Docker.
- **Session token**: per-server random value required by the local API; never persisted.
