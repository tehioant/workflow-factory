use workflow_engine::auth::{inspect, CredentialAvailability as A, CredentialSource};
struct Fake(Result<Vec<u8>, A>, usize);
impl CredentialSource for Fake {
    fn read(&mut self) -> Result<Vec<u8>, A> {
        self.1 += 1;
        self.0.clone()
    }
}
fn probe(body: &str) -> A {
    let mut source = Fake(Ok(body.as_bytes().to_vec()), 0);
    let result = inspect(&mut source);
    assert_eq!(source.1, 1);
    result
}
#[test]
fn source_failures_are_sanitized_without_fallback() {
    for outcome in [
        A::Missing,
        A::Unsupported,
        A::Malformed,
        A::Inaccessible,
        A::Unsafe,
    ] {
        let mut source = Fake(Err(outcome), 0);
        assert_eq!(inspect(&mut source), outcome);
        assert_eq!(source.1, 1);
    }
}
#[test]
fn malformed_and_unsupported_sources_fail_closed() {
    for (body, expected) in [
        ("{}", A::Malformed),
        ("not json", A::Malformed),
        (r#"{"auth_mode":"chatgptAuthTokens"}"#, A::Unsupported),
        (r#"{"OPENAI_API_KEY":"fake-api-key"}"#, A::Unsupported),
        (r#"{"auth_mode":"new-kind","tokens":{}}"#, A::Unsupported),
        (
            r#"{"tokens":{"id_token":"fake.id.sig","access_token":"fake","refresh_token":"fake","account_id":"fake","extra":"ignored?"}}"#,
            A::Malformed,
        ),
    ] {
        assert_eq!(probe(body), expected);
    }
    assert_eq!(probe(&" ".repeat(65_537)), A::Malformed);
}
#[test]
fn file_policy_requires_owned_private_bounded_regular_source() {
    use workflow_engine::auth_file::safe_metadata;
    assert!(safe_metadata(1000, 0o600, 1000, true, true, 1, 100));
    for (uid, mode, regular, links, size) in [
        (0, 0o600, true, 1, 100),
        (1000, 0o644, true, 1, 100),
        (1000, 0o600, false, 1, 100),
        (1000, 0o600, true, 2, 100),
        (1000, 0o600, true, 1, 65_537),
    ] {
        assert!(!safe_metadata(uid, mode, 1000, true, regular, links, size));
    }
    assert!(safe_metadata(0, 0o755, 1000, false, false, 1, 0));
    assert!(!safe_metadata(1000, 0o777, 1000, false, false, 1, 0));
}
#[test]
fn subscription_material_is_available_without_exposing_it() {
    let result = probe(
        r#"{"auth_mode":"chatgpt","OPENAI_API_KEY":null,"tokens":{"id_token":"fake.id.signature","access_token":"synthetic-access","refresh_token":"synthetic-refresh","account_id":"synthetic-account"},"last_refresh":"2026-11-12T00:00:00Z"}"#,
    );
    assert_eq!(result, A::Available);
    assert_eq!(
        probe(
            r#"{"auth_mode":"chatgpt","tokens":{"id_token":"fake.id.sig","access_token":"synthetic-access","refresh_token":"synthetic-refresh","account_id":"synthetic-account","oauth_provenance":{"issuer":"https://auth.openai.com","client_id":"synthetic-client"},"login_owner_id":"synthetic-owner"}}"#
        ),
        A::Available
    );
    let dto = workflow_engine::credential_readiness(result);
    assert!(!dto.execution_enabled);
    let json = serde_json::to_string(&dto).unwrap();
    assert!(!json.contains("synthetic-"));
    assert!(json.contains("unsupportedConcurrentAuth"));
    assert_eq!(serde_json::to_string(&result).unwrap(), "\"available\"");
}
