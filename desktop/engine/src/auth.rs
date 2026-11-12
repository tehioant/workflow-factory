//! Read-only availability, never authentication or credential transport.
use serde::{Deserialize, Serialize};
use ts_rs::TS;
pub const MAX_AUTH_BYTES: usize = 65_536;
#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum CredentialAvailability {
    NotInspected,
    Available,
    Missing,
    Unsupported,
    Malformed,
    Inaccessible,
    Unsafe,
}
pub trait CredentialSource {
    fn read(&mut self) -> Result<Vec<u8>, CredentialAvailability>;
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthFile {
    auth_mode: Option<String>,
    #[serde(rename = "OPENAI_API_KEY")]
    api_key: Option<String>,
    tokens: Option<Tokens>,
    last_refresh: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Tokens {
    id_token: String,
    access_token: String,
    refresh_token: String,
    account_id: String,
    oauth_provenance: Option<Provenance>,
    login_owner_id: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Provenance {
    issuer: String,
    client_id: String,
}
fn bounded_text(s: &str) -> bool {
    !s.is_empty() && s.len() <= 16_384 && s.bytes().all(|c| c.is_ascii_graphic())
}
fn classify(bytes: &[u8]) -> CredentialAvailability {
    use CredentialAvailability as A;
    if bytes.len() > MAX_AUTH_BYTES {
        return A::Malformed;
    }
    // Unknown modes/families are unsupported, not guessed from token-shaped fields.
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return A::Malformed;
    };
    if value
        .get("auth_mode")
        .is_some_and(|mode| !mode.is_null() && mode != "chatgpt")
        || value
            .get("OPENAI_API_KEY")
            .is_some_and(|key| !key.is_null())
        || ["agent_identity", "personal_access_token", "bedrock"]
            .iter()
            .any(|key| value.get(key).is_some())
    {
        return A::Unsupported;
    }
    let Ok(file) = serde_json::from_slice::<AuthFile>(bytes) else {
        return A::Malformed;
    };
    let Some(tokens) = file.tokens else {
        return A::Malformed;
    };
    if file.api_key.is_some()
        || file
            .auth_mode
            .as_deref()
            .is_some_and(|mode| mode != "chatgpt")
    {
        return A::Unsupported;
    }
    if tokens
        .login_owner_id
        .as_deref()
        .is_some_and(|s| !bounded_text(s))
        || tokens
            .oauth_provenance
            .as_ref()
            .is_some_and(|p| !bounded_text(&p.issuer) || !bounded_text(&p.client_id))
    {
        return A::Malformed;
    }
    if [
        &tokens.id_token,
        &tokens.access_token,
        &tokens.refresh_token,
        &tokens.account_id,
    ]
    .iter()
    .any(|s| !bounded_text(s))
        || tokens.id_token.split('.').count() != 3
        || tokens.id_token.split('.').any(|s| {
            s.is_empty()
                || !s
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
        })
        || file
            .last_refresh
            .is_some_and(|s| s.len() > 64 || !bounded_text(&s))
    {
        return A::Malformed;
    }
    A::Available
}
pub fn inspect(source: &mut impl CredentialSource) -> CredentialAvailability {
    match source.read() {
        Ok(mut bytes) => {
            let result = classify(&bytes);
            bytes.fill(0);
            result
        }
        Err(outcome) => outcome,
    }
}
