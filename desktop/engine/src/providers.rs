//! Pure direct-model settings and protocol contracts. No credential or I/O access.
use crate::{validation, Error};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
pub(crate) fn validate(c: &DirectModelConfiguration) -> Result<(), Error> {
    let matching = match c.provider {
        DirectProviderId::OpenaiResponses => {
            c.credential.variable == CredentialVariable::Openai
                && (c.model.starts_with("gpt-")
                    || c.model.starts_with("chatgpt-")
                    || c.model.starts_with("ft:gpt-")
                    || c.model
                        .strip_prefix('o')
                        .is_some_and(|s| s.starts_with(|ch: char| ch.is_ascii_digit())))
        }
        DirectProviderId::AnthropicMessages => {
            c.credential.variable == CredentialVariable::Anthropic
                && c.model.starts_with("claude-")
                && !c.model.contains(':')
        }
    };
    if !(1..=65536).contains(&c.max_output_tokens) || !matching {
        return Err(validation(
            "Invalid direct model settings or provider credential binding.",
        ));
    }
    Ok(())
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SchemaVersion2;
impl Serialize for SchemaVersion2 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(2)
    }
}
impl<'de> Deserialize<'de> for SchemaVersion2 {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if u8::deserialize(deserializer)? != 2 {
            return Err(serde::de::Error::custom(
                "Unsupported direct configuration version.",
            ));
        }
        Ok(Self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum DirectProviderId {
    OpenaiResponses,
    AnthropicMessages,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub enum CredentialVariable {
    #[serde(rename = "OPENAI_API_KEY")]
    Openai,
    #[serde(rename = "ANTHROPIC_API_KEY")]
    Anthropic,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum EnvironmentKind {
    Environment,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentCredential {
    pub kind: EnvironmentKind,
    pub variable: CredentialVariable,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum DirectModelKind {
    DirectModel,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DirectModelConfiguration {
    #[ts(type = "2")]
    pub schema_version: SchemaVersion2,
    pub kind: DirectModelKind,
    pub id: String,
    pub name: String,
    pub provider: DirectProviderId,
    pub model: String,
    pub max_output_tokens: u32,
    pub credential: EnvironmentCredential,
}
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ProviderAuth {
    ApiKey,
}
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ProviderSetting {
    MaxOutputTokens,
}
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDescriptor {
    pub id: DirectProviderId,
    pub label: String,
    pub auth: ProviderAuth,
    pub credential_variable: CredentialVariable,
    #[ts(type = "[\"maxOutputTokens\"]")]
    pub settings: [ProviderSetting; 1],
}
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum CredentialReadiness {
    Configured,
    Missing,
    Invalid,
}
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProviderReadiness {
    pub provider: DirectProviderId,
    pub credential: CredentialReadiness,
    pub message: String,
}
pub fn descriptors() -> Vec<ProviderDescriptor> {
    [
        (
            DirectProviderId::OpenaiResponses,
            "OpenAI Responses",
            CredentialVariable::Openai,
        ),
        (
            DirectProviderId::AnthropicMessages,
            "Anthropic Messages",
            CredentialVariable::Anthropic,
        ),
    ]
    .into_iter()
    .map(|(id, label, credential_variable)| ProviderDescriptor {
        id,
        label: label.into(),
        auth: ProviderAuth::ApiKey,
        credential_variable,
        settings: [ProviderSetting::MaxOutputTokens],
    })
    .collect()
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    InvalidConfiguration,
    InvalidResponse,
    HttpStatus,
    TooLarge,
}
#[derive(Debug, PartialEq)]
/// An HTTP POST request without authorization. Native transport must add the bound secret.
pub struct PreparedRequest {
    pub url: &'static str,
    pub headers: Vec<(&'static str, &'static str)>,
    pub body: serde_json::Value,
}
/// Protocols: OpenAI /v1/responses and Anthropic /v1/messages (2026-10-21).
/// Model names are syntax-checked only; no availability or entitlement is established.
pub fn prepare(
    config: &DirectModelConfiguration,
    prompt: &str,
) -> Result<PreparedRequest, ProviderError> {
    crate::validate_configuration(&crate::Configuration::DirectModel(config.clone()))
        .map_err(|_| ProviderError::InvalidConfiguration)?;
    let mut headers = vec![("content-type", "application/json")];
    let (url, body) = match config.provider {
        DirectProviderId::OpenaiResponses => (
            "https://api.openai.com/v1/responses",
            serde_json::json!({"model":config.model,"input":prompt,"max_output_tokens":config.max_output_tokens,"store":false,"stream":false}),
        ),
        DirectProviderId::AnthropicMessages => {
            headers.push(("anthropic-version", "2026-10-21"));
            (
                "https://api.anthropic.com/v1/messages",
                serde_json::json!({"model":config.model,"max_tokens":config.max_output_tokens,"messages":[{"role":"user","content":prompt}],"stream":false}),
            )
        }
    };
    Ok(PreparedRequest { url, headers, body })
}
pub const MAX_RESPONSE_BYTES: usize = 1_048_576;
pub const MAX_OUTPUT_BYTES: usize = 262_144;
pub fn parse_terminal(
    provider: DirectProviderId,
    status: u16,
    body: &[u8],
) -> Result<String, ProviderError> {
    if status != 200 {
        return Err(ProviderError::HttpStatus);
    }
    if body.len() > MAX_RESPONSE_BYTES {
        return Err(ProviderError::TooLarge);
    }
    let value: serde_json::Value =
        serde_json::from_slice(body).map_err(|_| ProviderError::InvalidResponse)?;
    let mut text = String::new();
    let blocks = match provider {
        DirectProviderId::OpenaiResponses => {
            if value["object"] != "response"
                || value["status"] != "completed"
                || !value.get("error").is_some_and(serde_json::Value::is_null)
                || !value
                    .get("incomplete_details")
                    .is_some_and(serde_json::Value::is_null)
            {
                return Err(ProviderError::InvalidResponse);
            }
            let mut blocks = Vec::new();
            for item in value["output"]
                .as_array()
                .ok_or(ProviderError::InvalidResponse)?
            {
                if item["type"] == "reasoning" && item["summary"].is_array() {
                    continue;
                }
                if item["type"] != "message"
                    || item["status"] != "completed"
                    || item["role"] != "assistant"
                {
                    return Err(ProviderError::InvalidResponse);
                }
                blocks.extend(
                    item["content"]
                        .as_array()
                        .ok_or(ProviderError::InvalidResponse)?,
                );
            }
            blocks
        }
        DirectProviderId::AnthropicMessages => {
            if value["type"] != "message"
                || value["role"] != "assistant"
                || value["stop_reason"] != "end_turn"
            {
                return Err(ProviderError::InvalidResponse);
            }
            value["content"]
                .as_array()
                .ok_or(ProviderError::InvalidResponse)?
                .iter()
                .collect()
        }
    };
    for block in blocks {
        let kind = match provider {
            DirectProviderId::OpenaiResponses => "output_text",
            DirectProviderId::AnthropicMessages => "text",
        };
        if block["type"] != kind {
            return Err(ProviderError::InvalidResponse);
        }
        let part = block["text"]
            .as_str()
            .ok_or(ProviderError::InvalidResponse)?;
        if text.len().saturating_add(part.len()) > MAX_OUTPUT_BYTES {
            return Err(ProviderError::TooLarge);
        }
        text.push_str(part);
    }
    if text.trim().is_empty() {
        return Err(ProviderError::InvalidResponse);
    }
    Ok(text)
}
