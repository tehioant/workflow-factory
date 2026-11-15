# Direct model configurations

In **Configurations**, choose **New direct model configuration**. Select OpenAI Responses or Anthropic Messages, enter an explicit model identifier and a maximum output-token cap (1–65536), then save. Select that reusable configuration in the AI block inspector. Model availability is not checked or substituted. Changing provider clears the model and resets the cap and environment binding; enter the new model deliberately.

Direct calls send a prompt and return text. They do not run tools, shell commands, files, workspaces or containers. API usage is separately billed from chat subscriptions. Authoring and saving require no API key. A run must be **all-direct or all-Codex**: OpenAI and Anthropic direct blocks may mix, but a capture containing both direct blocks and legacy Codex agents is refused before acceptance.

Model identifiers are syntax-checked, not discovered: OpenAI accepts the `gpt-`, `chatgpt-`, `ft:gpt-` and `o` followed by a digit families; Anthropic accepts `claude-` identifiers without a colon. These accepted forms do not establish access to any particular model. Provider-side limits may be lower than the configurable output-token cap.

## Environment binding and readiness

The saved nonsecret binding is fixed: `OPENAI_API_KEY` for OpenAI Responses, `ANTHROPIC_API_KEY` for Anthropic Messages. Supply the selected variable in the native application's launch environment using your trusted local secret-management mechanism, then launch or relaunch the application from that environment. Desktop launcher environments may differ from your terminal. Do not paste keys into model, name, prompt or any other webview field, or retain them in command history. There is no key-entry form, key manager, `.env` discovery, ambient profile import or frontend credential access. Direct calls do not borrow Codex credentials.

In **Settings → Direct model readiness**, load the static provider descriptions, choose a provider and click **Check selected provider binding**. Mounting these controls does not inspect credentials. The native check reads only the selected fixed environment binding and checks its local shape, not balance, entitlement, validity or model access. Missing/invalid bindings require correcting the native launch environment. Readiness grants no execution consent and sends no model request.

## Execution, limits and recovery

Start the saved workflow through the existing run controls. Direct runs share FIFO queue capacity, worker permits, cancellation and safe-shutdown authority with legacy runs, but do not require a Codex managed session, container image or Docker preflight. A credential-starved Codex FIFO head is not bypassed by a later direct run.

Before sending, the host requests **native per-run approval for separately billed API use**. The dialog identifies the immutable captured workflow/configuration revisions, providers, destinations, models, output-token caps, key-variable names, prompt templates, declared inputs and referenced predecessor-output scope. Queue acceptance, saving and readiness are not paid-send approval. Declining or an unavailable native dialog authorizes no API send; use explicit Authorize or Stop for the retained reservation. Approval covers that captured chain once, not future runs. An oversized consent scope is refused rather than silently shortened.

The wired native transport makes nonstreaming HTTPS POSTs only to `https://api.openai.com/v1/responses` or `https://api.anthropic.com/v1/messages`. It verifies TLS, disables proxies, redirects and automatic retries, and uses a 10-second connection timeout and a 120-second request deadline. Raw response bytes are capped at 1 MiB and returned text at 256 KiB. Only supported completed text responses are accepted; malformed, incomplete, tool-bearing or oversized replies fail without fallback, model substitution or resend. Known selected API-key values are redacted from returned text before retention; this is not a general guarantee against sensitive content in prompts or responses.

Connected direct blocks consume only explicitly referenced, durably successful predecessor outputs. A failed or interrupted block does not authorize a successor. Output/finality storage failures retain the selected recovery state and withhold capacity until exact settlement; **Retry output storage** retries storage only, never HTTP, consent or execution. Memory-only retained payloads are volatile and may be lost on process exit. Durable chosen finality can be replayed without an in-memory claim; absent recovery authority remains held rather than reconstructing a result.

Stop cancels the owned local HTTP operation and waits for its return and durable settlement. It cannot undo or disprove remote processing or API charges after a request was sent. Startup withholds interrupted direct requests and never resends them automatically. Explicit Stop can finalize a coherent no-worker direct reservation; retained-output recovery remains storage-only. Safe shutdown freezes admission, signals owned workers and waits for actual worker return and pending storage/finality settlement. None of these controls promises remote cancellation or a refund.

## Compatibility and retained evidence

**New legacy Codex configuration** retains the original model/reasoning editor and unversioned JSON. Legacy agent runs continue to use their managed session, container and native-security requirements; direct execution does not waive those gates. Configuration IDs, revisions and compare-and-swap save envelopes are unchanged. Refused saves/deletes retain the selected draft; shutdown uses the same snapshot-before-clone draft-save hook.

Captured block summaries show the captured provider, model and token cap, not current configurations. Direct retained-history details do not claim a Docker image, isolation or workspace artifacts. Legacy captures remain readable and missing per-block settings are not reconstructed from current settings. Ambiguous start retries remain bound to the original immutable request, not a new execution request.

## Integration and verification boundary

The desktop controls, generated transport, provider descriptor/readiness IPC, guarded native HTTP caller, queue reservations and storage/finality recovery are now wired together in source. Frontend typechecking/build and focused mocked component, protocol, transport and prepared storage/dispatch checks provide local source evidence; they are not live service or native interaction qualification.

No live API/model request, real-key authentication, balance, entitlement, billing or remote cancellation has been qualified. Linux nonlaunch native compilation is not a native GUI/paid-send interaction check. The macOS-specific native approval path has not been compiled or exercised on macOS, and no macOS runtime qualification is claimed. Full/workspace suites, whole-worker/E2E journeys and publication remain outside this local verification scope. Independent final acceptance is separate from these implementation receipts.
