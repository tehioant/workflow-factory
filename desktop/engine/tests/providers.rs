// Synthetic fixtures shaped from the official HTTP references, not live inference:
// https://developers.openai.com/api/reference/resources/responses/methods/create
// https://platform.claude.com/docs/en/api/http/messages/create
// https://platform.claude.com/docs/en/api/versioning (2026-10-21)
use serde_json::json;
use workflow_engine::providers::*;
fn config(provider: &str, variable: &str, model: &str) -> DirectModelConfiguration {
    serde_json::from_value(json!({"schemaVersion":2,"kind":"directModel","id":"ecbca0a8-d457-4c42-89d4-6727261747bc","name":"Direct","provider":provider,"model":model,"maxOutputTokens":128,"credential":{"kind":"environment","variable":variable}})).unwrap()
}
fn openai(text: &str) -> serde_json::Value {
    json!({"id":"resp_synthetic","object":"response","status":"completed","error":null,"incomplete_details":null,"output":[{"type":"reasoning","summary":[]},{"id":"msg_synthetic","type":"message","status":"completed","role":"assistant","content":[{"type":"output_text","text":text,"annotations":[],"logprobs":[]}]}]})
}
fn anthropic(text: &str) -> serde_json::Value {
    json!({"id":"msg_synthetic","type":"message","role":"assistant","model":"claude-sonnet-4-5","stop_reason":"end_turn","stop_sequence":null,"content":[{"type":"text","text":text,"citations":null}],"usage":{"input_tokens":4,"output_tokens":3}})
}
#[test]
fn extracts_only_completed_terminal_text_in_order() {
    for (provider, body) in [
        (DirectProviderId::OpenaiResponses, openai(" Hello 🌍 ")),
        (DirectProviderId::AnthropicMessages, anthropic(" Hello 🌍 ")),
    ] {
        assert_eq!(
            parse_terminal(provider, 200, body.to_string().as_bytes()),
            Ok(" Hello 🌍 ".into())
        );
    }
    let mut body = openai("one");
    body["output"][1]["content"]
        .as_array_mut()
        .unwrap()
        .push(json!({"type":"output_text","text":"two"}));
    assert_eq!(
        parse_terminal(
            DirectProviderId::OpenaiResponses,
            200,
            body.to_string().as_bytes()
        ),
        Ok("onetwo".into())
    );
    let mut body = anthropic("one");
    body["content"]
        .as_array_mut()
        .unwrap()
        .push(json!({"type":"text","text":"two"}));
    assert_eq!(
        parse_terminal(
            DirectProviderId::AnthropicMessages,
            200,
            body.to_string().as_bytes()
        ),
        Ok("onetwo".into())
    );
}
#[test]
fn refuses_nonterminal_nontext_malformed_and_oversized_responses() {
    for (provider, good) in [
        (DirectProviderId::OpenaiResponses, openai("safe")),
        (DirectProviderId::AnthropicMessages, anthropic("safe")),
    ] {
        for status in [0, 201, 302, 400, 401, 429, 500] {
            assert_eq!(
                parse_terminal(provider, status, good.to_string().as_bytes()),
                Err(ProviderError::HttpStatus)
            );
        }
        for raw in [
            b"not json secret".as_slice(),
            b"null",
            b"[]",
            b"{}",
            b"{\"error\":\"secret\"}",
            b"{\"content\":",
        ] {
            assert_eq!(
                parse_terminal(provider, 200, raw),
                Err(ProviderError::InvalidResponse)
            );
        }
        let edits = if provider == DirectProviderId::OpenaiResponses {
            vec![
                ("/status", json!("incomplete")),
                ("/status", json!("failed")),
                ("/error", json!({"message":"secret"})),
                ("/incomplete_details", json!({"reason":"max_output_tokens"})),
                ("/object", json!("wrong")),
                ("/output/1/role", json!("user")),
                ("/output/1/status", json!("incomplete")),
                ("/output/1/type", json!("function_call")),
                ("/output/1/content/0/type", json!("refusal")),
                ("/output/1/content/0/text", json!(3)),
                ("/output", json!([])),
            ]
        } else {
            vec![
                ("/stop_reason", json!("max_tokens")),
                ("/stop_reason", json!("tool_use")),
                ("/stop_reason", json!("refusal")),
                ("/type", json!("error")),
                ("/role", json!("user")),
                ("/content/0/type", json!("tool_use")),
                ("/content/0/type", json!("thinking")),
                ("/content/0/text", json!(3)),
                ("/content", json!([])),
            ]
        };
        for (path, value) in edits {
            let mut body = good.clone();
            *body.pointer_mut(path).unwrap() = value;
            assert_eq!(
                parse_terminal(provider, 200, body.to_string().as_bytes()),
                Err(ProviderError::InvalidResponse),
                "{provider:?} {path}"
            );
        }
        for text in ["", " \n\t"] {
            let body = if provider == DirectProviderId::OpenaiResponses {
                openai(text)
            } else {
                anthropic(text)
            };
            assert_eq!(
                parse_terminal(provider, 200, body.to_string().as_bytes()),
                Err(ProviderError::InvalidResponse)
            );
        }
        let large = "🌍".repeat(65536);
        let body = if provider == DirectProviderId::OpenaiResponses {
            openai(&large)
        } else {
            anthropic(&large)
        };
        assert_eq!(
            parse_terminal(provider, 200, body.to_string().as_bytes()),
            Ok(large)
        );
        let too_large = "x".repeat(262145);
        let body = if provider == DirectProviderId::OpenaiResponses {
            openai(&too_large)
        } else {
            anthropic(&too_large)
        };
        assert_eq!(
            parse_terminal(provider, 200, body.to_string().as_bytes()),
            Err(ProviderError::TooLarge)
        );
        let mut body = good.clone();
        body["unretained_metadata"] = json!("x".repeat(1048576));
        assert_eq!(
            parse_terminal(provider, 200, body.to_string().as_bytes()),
            Err(ProviderError::TooLarge)
        );
    }
}
#[test]
fn descriptors_expose_only_two_bundled_nonsecret_bindings() {
    assert_eq!(
        serde_json::to_value(descriptors()).unwrap(),
        json!([
            {"id":"openaiResponses","label":"OpenAI Responses","auth":"apiKey","credentialVariable":"OPENAI_API_KEY","settings":["maxOutputTokens"]},
            {"id":"anthropicMessages","label":"Anthropic Messages","auth":"apiKey","credentialVariable":"ANTHROPIC_API_KEY","settings":["maxOutputTokens"]}
        ])
    );
}
#[test]
fn prepares_fixed_nonsecret_text_only_requests() {
    let prompt = "literal \n {{input.x}} \"quoted\"";
    let openai = prepare(
        &config("openaiResponses", "OPENAI_API_KEY", "gpt-5"),
        prompt,
    );
    assert!(
        openai.is_ok(),
        "OpenAI request should prepare without credentials"
    );
    let openai = openai.unwrap();
    assert_eq!(openai.url, "https://api.openai.com/v1/responses");
    assert_eq!(openai.headers, vec![("content-type", "application/json")]);
    assert_eq!(
        openai.body,
        json!({"model":"gpt-5","input":prompt,"max_output_tokens":128,"store":false,"stream":false})
    );
    let anthropic = prepare(
        &config(
            "anthropicMessages",
            "ANTHROPIC_API_KEY",
            "claude-sonnet-4-5",
        ),
        prompt,
    )
    .unwrap();
    assert_eq!(anthropic.url, "https://api.anthropic.com/v1/messages");
    assert_eq!(
        anthropic.headers,
        vec![
            ("content-type", "application/json"),
            ("anthropic-version", "2026-10-21")
        ]
    );
    assert_eq!(
        anthropic.body,
        json!({"model":"claude-sonnet-4-5","max_tokens":128,"messages":[{"role":"user","content":prompt}],"stream":false})
    );
    let mut invalid = config("openaiResponses", "OPENAI_API_KEY", "gpt-5");
    invalid.max_output_tokens = 0;
    assert_eq!(
        prepare(&invalid, prompt),
        Err(ProviderError::InvalidConfiguration)
    );
    invalid.max_output_tokens = 128;
    invalid.id = "bad".into();
    assert_eq!(
        prepare(&invalid, prompt),
        Err(ProviderError::InvalidConfiguration)
    );
}
