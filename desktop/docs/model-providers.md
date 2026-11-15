# Direct model configurations

In **Configurations**, choose **New direct model configuration**. Select OpenAI Responses or Anthropic Messages, enter an explicit model identifier and a maximum output-token cap (1–65536), then save. Select that reusable configuration in the AI block inspector. Model availability is not checked or substituted. Changing provider clears the model and resets the cap and environment binding; enter the new model deliberately.

Direct calls send a prompt and return text. They do not run tools, shell commands, files, workspaces or containers. API usage is separately billed from chat subscriptions. Authoring and saving require no API key. A run may mix OpenAI and Anthropic direct blocks, but this first slice refuses mixtures of direct blocks and legacy Codex agents.

## Environment binding and readiness

The saved nonsecret binding is fixed: `OPENAI_API_KEY` for OpenAI Responses, `ANTHROPIC_API_KEY` for Anthropic Messages. Supply the selected variable in the native application's launch environment. Desktop launcher environments may differ from your terminal. Do not paste keys into model, name, prompt or any other webview field. There is no key-entry form, key manager, `.env` discovery or frontend credential access.

In **Settings → Direct model readiness**, load the static provider descriptions, choose a provider and click **Check selected provider binding**. Mounting these controls does not inspect credentials. The native check is selected-provider-only and local, not a model request or proof of balance, entitlement or model access. Missing/invalid bindings require correcting the native launch environment. Execution still uses the existing start/authorize commands and native per-run scope/charge confirmation; readiness grants no execution consent.

## Compatibility and retained evidence

**New legacy Codex configuration** retains the original model/reasoning editor and unversioned JSON. Legacy agent runs continue to use their managed session and container requirements. Configuration IDs, revisions and compare-and-swap save envelopes are unchanged. Refused saves/deletes retain the selected draft; shutdown uses the same snapshot-before-clone draft-save hook.

Captured block summaries show the captured provider, model and token cap, not current configurations. Direct retained-history details do not claim a Docker image, isolation or workspace artifacts. Legacy captures remain readable and missing per-block settings are not reconstructed from current settings. Ambiguous start retries remain bound to the original immutable request.

## Integration and verification boundary

This UI checkpoint targets the frozen core wire union and provider read commands. Generated transport exports (`Configuration`, `DirectProviderId`, `ProviderDescriptor`, `ProviderReadiness`) and native `provider_descriptors` / `provider_readiness` arrive from the core lane. Until integrated, standalone typecheck is intentionally incomplete; the generated transport must not be patched in the UI lane. Focused mounted checks use a fake native invoke boundary. No live API, authentication, balance, billing, native-platform interaction or model qualification is established by these checks.
