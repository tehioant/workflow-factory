//! Owned nonstreaming provider calls. Secrets never implement Debug or leave native memory.
use std::{future::Future, pin::Pin, time::Duration};
use workflow_engine::{
    providers::{
        self, CredentialReadiness, DirectModelConfiguration, DirectProviderId, ProviderReadiness,
    },
    sandbox::Cancel,
    Error,
};
fn failure() -> Error {
    Error::Validation{message:"Direct provider request failed, cancelled, incomplete or unavailable. No automatic retry; a sent request may already have been processed and billed.".into()}
}
pub fn variable(provider: DirectProviderId) -> &'static str {
    match provider {
        DirectProviderId::OpenaiResponses => "OPENAI_API_KEY",
        DirectProviderId::AnthropicMessages => "ANTHROPIC_API_KEY",
    }
}
pub fn readiness_with(
    provider: DirectProviderId,
    get: impl FnOnce(&str) -> Option<String>,
) -> ProviderReadiness {
    let credential = match get(variable(provider)) {
        None => CredentialReadiness::Missing,
        Some(value) if valid_key(&value) => CredentialReadiness::Configured,
        Some(_) => CredentialReadiness::Invalid,
    };
    let message=match credential{CredentialReadiness::Configured=>"Selected environment key is configured. This does not verify entitlement, model availability or balance.",CredentialReadiness::Missing=>"Launch the native app with the selected provider's fixed API-key variable. No host profile or .env import is performed.",CredentialReadiness::Invalid=>"Selected environment binding has invalid key shape. No provider request was sent."}.into();
    ProviderReadiness {
        provider,
        credential,
        message,
    }
}
pub struct Reply {
    pub status: u16,
    pub body: Vec<u8>,
}
pub trait Transport {
    fn send<'a>(
        &'a mut self,
        request: reqwest::Request,
        cancel: &'a Cancel,
    ) -> Pin<Box<dyn Future<Output = Result<Reply, Error>> + 'a>>;
}
pub fn infer_with(
    config: &DirectModelConfiguration,
    prompt: &str,
    cancel: &Cancel,
    secrets: &mut Vec<String>,
    get: impl FnOnce(&str) -> Option<String>,
    guard: impl FnOnce() -> Result<(), Error>,
    transport: &mut impl Transport,
) -> Result<String, Error> {
    cancel.check()?;
    if prompt.len() > 2_097_152 {
        return Err(failure());
    }
    let prepared = providers::prepare(config, prompt).map_err(|_| failure())?;
    let key = get(variable(config.provider))
        .filter(|s| valid_key(s))
        .ok_or_else(failure)?;
    if !secrets.contains(&key) {
        secrets.push(key.clone());
    } // bounded by at most the captured block count
    if secrets.len() > 64 {
        return Err(failure());
    }
    let mut request = reqwest::Request::new(
        reqwest::Method::POST,
        prepared.url.parse().map_err(|_| failure())?,
    );
    for (name, value) in prepared.headers {
        request.headers_mut().insert(
            reqwest::header::HeaderName::from_static(name),
            reqwest::header::HeaderValue::from_static(value),
        );
    }
    let (name, value) = match config.provider {
        DirectProviderId::OpenaiResponses => ("authorization", format!("Bearer {key}")),
        DirectProviderId::AnthropicMessages => ("x-api-key", key),
    };
    let mut value = reqwest::header::HeaderValue::from_str(&value).map_err(|_| failure())?;
    value.set_sensitive(true);
    request
        .headers_mut()
        .insert(reqwest::header::HeaderName::from_static(name), value);
    *request.body_mut() = Some(
        serde_json::to_vec(&prepared.body)
            .map_err(|_| failure())?
            .into(),
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| failure())?;
    cancel.check()?;
    guard()?;
    cancel.check()?;
    let reply = runtime.block_on(owned(cancel, transport.send(request, cancel)))?;
    let mut text = providers::parse_terminal(config.provider, reply.status, &reply.body)
        .map_err(|_| failure())?;
    secrets.sort_by_key(|s| std::cmp::Reverse(s.len()));
    for secret in secrets.iter() {
        text = text.replace(secret, "[REDACTED]");
    }
    if text.len() > providers::MAX_OUTPUT_BYTES {
        return Err(failure());
    }
    Ok(text)
}
fn valid_key(value: &str) -> bool {
    (16..=4096).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_graphic())
}
async fn owned<T>(
    cancel: &Cancel,
    operation: impl Future<Output = Result<T, Error>>,
) -> Result<T, Error> {
    let mut operation = std::pin::pin!(operation);
    let deadline = tokio::time::sleep(Duration::from_secs(120));
    let mut deadline = std::pin::pin!(deadline);
    loop {
        cancel.check()?;
        tokio::select! {biased; _=&mut deadline=>return Err(failure()),result=&mut operation=>return result,_=tokio::time::sleep(Duration::from_millis(25))=>{}}
    }
    // Returning drops the operation in this worker. No spawned/detached request task.
}
pub struct NativeTransport(reqwest::Client);
impl NativeTransport {
    pub fn new() -> Result<Self, Error> {
        reqwest::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .retry(reqwest::retry::never())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(120))
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .build()
            .map(Self)
            .map_err(|_| failure())
    }
}
fn append(body: &mut Vec<u8>, chunk: &[u8]) -> Result<(), Error> {
    if body.len().saturating_add(chunk.len()) > providers::MAX_RESPONSE_BYTES {
        return Err(failure());
    }
    body.extend_from_slice(chunk);
    Ok(())
}
impl Transport for NativeTransport {
    fn send<'a>(
        &'a mut self,
        request: reqwest::Request,
        cancel: &'a Cancel,
    ) -> Pin<Box<dyn Future<Output = Result<Reply, Error>> + 'a>> {
        Box::pin(async move {
            cancel.check()?;
            let mut response = self.0.execute(request).await.map_err(|_| failure())?;
            let status = response.status().as_u16();
            if status != 200 {
                return Err(failure());
            }
            if response
                .content_length()
                .is_some_and(|n| n > providers::MAX_RESPONSE_BYTES as u64)
            {
                return Err(failure());
            }
            let mut body = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|_| failure())? {
                cancel.check()?;
                append(&mut body, &chunk)?;
            }
            Ok(Reply { status, body })
        })
    }
}
pub fn infer(
    config: &DirectModelConfiguration,
    prompt: &str,
    cancel: &Cancel,
    secrets: &mut Vec<String>,
    guard: impl FnOnce() -> Result<(), Error>,
) -> Result<String, Error> {
    infer_with(
        config,
        prompt,
        cancel,
        secrets,
        |name| std::env::var(name).ok(),
        guard,
        &mut NativeTransport::new()?,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    fn config(provider: DirectProviderId) -> DirectModelConfiguration {
        serde_json::from_value(serde_json::json!({"schemaVersion":2,"kind":"directModel","id":"00000000-0000-0000-0000-000000000002","name":"API","provider":provider,"model":if provider==DirectProviderId::OpenaiResponses{"gpt-5"}else{"claude-sonnet-4-5"},"maxOutputTokens":128,"credential":{"kind":"environment","variable":variable(provider)}})).unwrap()
    }
    struct Fake {
        provider: DirectProviderId,
        calls: usize,
    }
    impl Transport for Fake {
        fn send<'a>(
            &'a mut self,
            r: reqwest::Request,
            _: &'a Cancel,
        ) -> Pin<Box<dyn Future<Output = Result<Reply, Error>> + 'a>> {
            self.calls += 1;
            assert_eq!(r.method(), reqwest::Method::POST);
            let open = self.provider == DirectProviderId::OpenaiResponses;
            assert_eq!(
                r.url().as_str(),
                if open {
                    "https://api.openai.com/v1/responses"
                } else {
                    "https://api.anthropic.com/v1/messages"
                }
            );
            let body: serde_json::Value =
                serde_json::from_slice(r.body().unwrap().as_bytes().unwrap()).unwrap();
            assert_eq!(body["stream"], false);
            assert!(body.get("tools").is_none());
            if open {
                assert_eq!(body["store"], false);
                assert_eq!(body["input"], "literal prompt");
                assert_eq!(
                    r.headers()["authorization"],
                    "Bearer synthetic-bound-secret"
                );
            } else {
                assert_eq!(r.headers()["x-api-key"], "synthetic-bound-secret");
                assert_eq!(r.headers()["anthropic-version"], "2026-10-21");
            }
            let body = if open {
                serde_json::json!({"object":"response","status":"completed","error":null,"incomplete_details":null,"output":[{"type":"message","status":"completed","role":"assistant","content":[{"type":"output_text","text":"safe synthetic-bound-secret"}]}]})
            } else {
                serde_json::json!({"type":"message","role":"assistant","stop_reason":"end_turn","content":[{"type":"text","text":"safe synthetic-bound-secret"}]})
            };
            Box::pin(async move {
                Ok(Reply {
                    status: 200,
                    body: body.to_string().into_bytes(),
                })
            })
        }
    }
    #[test]
    fn native_caller_consumes_both_codecs_selected_key_and_redacts_before_return() {
        for provider in [
            DirectProviderId::OpenaiResponses,
            DirectProviderId::AnthropicMessages,
        ] {
            let mut t = Fake { provider, calls: 0 };
            let mut secrets = vec![];
            let value = infer_with(
                &config(provider),
                "literal prompt",
                &Cancel::default(),
                &mut secrets,
                |name| {
                    assert_eq!(name, variable(provider));
                    Some("synthetic-bound-secret".into())
                },
                || Ok(()),
                &mut t,
            );
            assert!(value.is_ok(), "native positive route must send: {value:?}");
            assert_eq!(value.unwrap(), "safe [REDACTED]");
            assert_eq!(t.calls, 1);
        }
    }
    #[test]
    fn native_refuses_guard_cancel_status_and_limits_without_retry() {
        struct Refusal {
            calls: usize,
            status: u16,
            body: Vec<u8>,
        }
        impl Transport for Refusal {
            fn send<'a>(
                &'a mut self,
                _: reqwest::Request,
                _: &'a Cancel,
            ) -> Pin<Box<dyn Future<Output = Result<Reply, Error>> + 'a>> {
                self.calls += 1;
                let body = self.body.clone();
                let status = self.status;
                Box::pin(async move { Ok(Reply { status, body }) })
            }
        }
        for mode in ["guard", "cancel", "status", "oversized", "malformed"] {
            let cancel = Cancel::default();
            if mode == "cancel" {
                cancel.request();
            }
            let mut t = Refusal {
                calls: 0,
                status: if mode == "status" { 429 } else { 200 },
                body: if mode == "oversized" {
                    vec![b'x'; providers::MAX_RESPONSE_BYTES + 1]
                } else {
                    b"malformed synthetic-bound-secret".to_vec()
                },
            };
            let mut keys = 0;
            let result = infer_with(
                &config(DirectProviderId::OpenaiResponses),
                "literal prompt",
                &cancel,
                &mut vec![],
                |_| {
                    keys += 1;
                    Some("synthetic-bound-secret".into())
                },
                || {
                    if mode == "guard" {
                        Err(failure())
                    } else {
                        Ok(())
                    }
                },
                &mut t,
            );
            assert!(result.is_err());
            assert_eq!(t.calls, usize::from(!matches!(mode, "guard" | "cancel")));
            if mode == "cancel" {
                assert_eq!(keys, 0);
            }
            assert!(!format!("{result:?}").contains("synthetic-bound-secret"));
        }
    }
    #[test]
    fn bounded_raw_chunks_and_owned_cancel_drop_the_operation() {
        let mut body = vec![0; providers::MAX_RESPONSE_BYTES - 1];
        append(&mut body, b"x").unwrap();
        assert!(append(&mut body, b"y").is_err());
        assert_eq!(body.len(), providers::MAX_RESPONSE_BYTES);
        let cancel = Cancel::default();
        let dropped = std::rc::Rc::new(std::cell::Cell::new(false));
        struct DropMark(std::rc::Rc<std::cell::Cell<bool>>);
        impl Drop for DropMark {
            fn drop(&mut self) {
                self.0.set(true)
            }
        }
        let mark = DropMark(dropped.clone());
        let signal = cancel.clone();
        let future = async move {
            let _mark = mark;
            signal.request();
            std::future::pending::<Result<(), Error>>().await
        };
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        assert!(rt.block_on(owned(&cancel, future)).is_err());
        assert!(dropped.get());
        // Construct the concrete TLS client without sending any request.
        let _client = NativeTransport::new().unwrap();
    }
    #[test]
    fn readiness_reads_only_selected_binding_and_never_claims_entitlement() {
        let r = readiness_with(DirectProviderId::AnthropicMessages, |name| {
            assert_eq!(name, "ANTHROPIC_API_KEY");
            Some("synthetic-bound-secret".into())
        });
        assert_eq!(r.credential, CredentialReadiness::Configured);
        assert!(!r.message.contains("synthetic-bound-secret"));
    }
}
