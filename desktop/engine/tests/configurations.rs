use workflow_engine::{CodexConfiguration, Reasoning, SaveConfiguration, Store};
#[test]
fn rejects_missing_configuration_reference_on_definition_save() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let d=serde_json::from_str(r#"{"schemaVersion":1,"id":"ecbca0a8-d457-4c42-89d4-6727261747bc","name":"Graph","blocks":[{"id":"a","name":"A","prompt":"","position":{"x":0,"y":0},"configurationId":"00000000-0000-0000-0000-000000000001"}]}"#).unwrap();
    assert!(
        matches!(store.save(workflow_engine::SaveDefinition{definition:d,expected_revision:None}),Err(workflow_engine::Error::Validation{message}) if message.contains("configuration"))
    );
}
#[test]
fn refuses_deleting_configuration_used_by_latest_definition() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let db = rusqlite::Connection::open(dir.path().join("definitions.sqlite3")).unwrap();
    db.execute_batch(r#"INSERT INTO configurations VALUES ('ecbca0a8-d457-4c42-89d4-6727261747bc',2,'{"id":"ecbca0a8-d457-4c42-89d4-6727261747bc","name":"Used","model":"model","reasoning":"high"}');
 INSERT INTO revisions VALUES ('00000000-0000-0000-0000-000000000001',1,'{"schemaVersion":1,"id":"00000000-0000-0000-0000-000000000001","name":"Prepared","blocks":[{"id":"a","name":"A","prompt":"","position":{"x":0,"y":0},"configurationId":"ecbca0a8-d457-4c42-89d4-6727261747bc"}]}');"#).unwrap();
    assert!(
        matches!(store.delete_configuration("ecbca0a8-d457-4c42-89d4-6727261747bc",2),Err(workflow_engine::Error::Validation{message}) if message.contains("referenced"))
    );
}
#[test]
fn readiness_never_confuses_host_credentials_with_supported_concurrent_auth() {
    let status = workflow_engine::auth_readiness();
    assert!(!status.execution_enabled);
    assert_eq!(status.credential_source, "notInspected");
    assert_eq!(status.status, "unsupportedConcurrentAuth");
    assert!(status.guidance.contains("refresh"));
}
#[test]
fn saves_nonsecret_configuration_and_reads_it_back() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let config = CodexConfiguration {
        id: "ecbca0a8-d457-4c42-89d4-6727261747bc".into(),
        name: "Careful".into(),
        model: "gpt-5.3-codex".into(),
        reasoning: Reasoning::High,
    };
    let saved = store
        .save_configuration(SaveConfiguration {
            config: config.clone(),
            expected_revision: None,
        })
        .unwrap();
    assert_eq!(saved.revision, 1);
    assert_eq!(store.list_configurations().unwrap()[0].config, config);
}
