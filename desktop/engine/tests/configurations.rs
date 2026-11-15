use workflow_engine::{CodexConfiguration, Reasoning, SaveConfiguration, Store};

fn direct(provider: &str, variable: &str) -> serde_json::Value {
    serde_json::json!({"schemaVersion":2,"kind":"directModel","id":"ecbca0a8-d457-4c42-89d4-6727261747bc","name":"Direct","provider":provider,"model":if provider == "openaiResponses" {"gpt-5.3"} else {"claude-sonnet-4-5"},"maxOutputTokens":1024,"credential":{"kind":"environment","variable":variable}})
}

#[test]
fn direct_configurations_round_trip_with_revision_conflicts() {
    for (provider, variable) in [
        ("openaiResponses", "OPENAI_API_KEY"),
        ("anthropicMessages", "ANTHROPIC_API_KEY"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        let body = direct(provider, variable);
        let request = serde_json::from_value::<SaveConfiguration>(
            serde_json::json!({"config":body,"expectedRevision":null}),
        );
        assert!(
            request.is_ok(),
            "strict direct configuration must be accepted"
        );
        let saved = store.save_configuration(request.unwrap()).unwrap();
        assert_eq!(saved.revision, 1);
        assert_eq!(
            serde_json::to_value(&store.list_configurations().unwrap()[0].config).unwrap(),
            body
        );
        let stale =
            serde_json::from_value(serde_json::json!({"config":body,"expectedRevision":null}))
                .unwrap();
        assert!(matches!(
            store.save_configuration(stale),
            Err(workflow_engine::Error::Conflict { .. })
        ));
        let update =
            serde_json::from_value(serde_json::json!({"config":body,"expectedRevision":1}))
                .unwrap();
        assert_eq!(store.save_configuration(update).unwrap().revision, 2);
        assert_eq!(
            Store::open(dir.path())
                .unwrap()
                .list_configurations()
                .unwrap()[0]
                .revision,
            2
        );
    }
}
#[test]
fn direct_configuration_validation_is_strict_and_non_normalizing() {
    let good = direct("openaiResponses", "OPENAI_API_KEY");
    for (field, value) in [
        ("schemaVersion", serde_json::json!(3)),
        ("schemaVersion", serde_json::Value::Null),
        ("kind", serde_json::json!("codex")),
        ("kind", serde_json::Value::Null),
        ("maxOutputTokens", serde_json::json!(0)),
        ("maxOutputTokens", serde_json::json!(65537)),
        ("maxOutputTokens", serde_json::json!(1.5)),
        ("maxOutputTokens", serde_json::json!(-1)),
        ("model", serde_json::json!(" gpt-5")),
        ("model", serde_json::json!("claude-sonnet-4")),
        ("model", serde_json::json!("gpt/5")),
        ("model", serde_json::json!("é")),
        ("model", serde_json::json!("g".repeat(101))),
        (
            "credential",
            serde_json::json!({"kind":"environment","variable":"ANTHROPIC_API_KEY"}),
        ),
        (
            "credential",
            serde_json::json!({"kind":"environment","variable":"OPENAI_API_KEY","value":"never echo this"}),
        ),
        ("baseUrl", serde_json::json!("https://invalid.example")),
        ("reasoning", serde_json::json!("high")),
        ("tools", serde_json::json!([])),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        let mut body = good.clone();
        body[field] = value;
        let parsed =
            serde_json::from_value::<SaveConfiguration>(serde_json::json!({"config":body}));
        let refused = parsed
            .map(|request| store.save_configuration(request).is_err())
            .unwrap_or(true);
        assert!(refused, "must refuse {field}: {body}");
        assert!(store.list_configurations().unwrap().is_empty());
    }
    for (provider, variable, models) in [
        (
            "openaiResponses",
            "OPENAI_API_KEY",
            vec![
                "gpt-5",
                "o3",
                "chatgpt-4o-latest",
                "ft:gpt-4.1:org:custom:123",
            ],
        ),
        (
            "anthropicMessages",
            "ANTHROPIC_API_KEY",
            vec!["claude-sonnet-4-5", "claude-opus-4-1-20250805"],
        ),
    ] {
        for model in models {
            let dir = tempfile::tempdir().unwrap();
            let store = Store::open(dir.path()).unwrap();
            let mut body = direct(provider, variable);
            body["model"] = model.into();
            body["maxOutputTokens"] = 65536.into();
            let saved = store
                .save_configuration(
                    serde_json::from_value(serde_json::json!({"config":body})).unwrap(),
                )
                .unwrap();
            assert_eq!(serde_json::to_value(saved.config).unwrap(), body);
        }
    }
    let legacy =
        serde_json::json!({"id":good["id"],"name":"Legacy","model":"codex","reasoning":"high"});
    for discriminator in ["kind", "schemaVersion"] {
        let mut body = legacy.clone();
        body[discriminator] = serde_json::Value::Null;
        assert!(
            serde_json::from_value::<SaveConfiguration>(serde_json::json!({"config":body}))
                .is_err()
        );
    }
}

#[test]
fn direct_stored_configuration_requires_valid_body_identity_and_revision() {
    for (field, value, revision) in [
        (
            "id",
            serde_json::json!("00000000-0000-0000-0000-000000000001"),
            1,
        ),
        ("schemaVersion", serde_json::json!(3), 1),
        ("maxOutputTokens", serde_json::json!(0), 1),
        ("name", serde_json::json!("Direct"), 0),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        let mut body = direct("anthropicMessages", "ANTHROPIC_API_KEY");
        body[field] = value;
        let db = rusqlite::Connection::open(dir.path().join("definitions.sqlite3")).unwrap();
        db.execute(
            "INSERT INTO configurations VALUES (?1,?2,?3)",
            rusqlite::params![
                "ecbca0a8-d457-4c42-89d4-6727261747bc",
                revision,
                body.to_string()
            ],
        )
        .unwrap();
        assert!(store.list_configurations().is_err());
        assert!(store
            .delete_configuration("ecbca0a8-d457-4c42-89d4-6727261747bc", revision)
            .is_err());
    }
}

#[test]
fn discriminated_configs_refuse_duplicate_fields_and_unsupported_standalone_versions() {
    let body = direct("openaiResponses", "OPENAI_API_KEY").to_string();
    for duplicate in [
        "\"schemaVersion\":3,",
        "\"kind\":\"unknown\",",
        "\"model\":\"other\",",
    ] {
        let raw = format!("{{{duplicate}{}", &body[1..]);
        assert!(
            serde_json::from_str::<workflow_engine::Configuration>(&raw).is_err(),
            "must refuse duplicate configuration fields"
        );
    }
    let raw = body.replace(
        "\"variable\":\"OPENAI_API_KEY\"",
        "\"variable\":\"ANTHROPIC_API_KEY\",\"variable\":\"OPENAI_API_KEY\"",
    );
    assert!(serde_json::from_str::<workflow_engine::Configuration>(&raw).is_err());
    let mut invalid = direct("openaiResponses", "OPENAI_API_KEY");
    invalid["schemaVersion"] = 3.into();
    assert!(serde_json::from_value::<workflow_engine::DirectModelConfiguration>(invalid).is_err());
}
#[test]
fn direct_execution_admission_refuses_agent_image_without_writes() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let db = rusqlite::Connection::open(dir.path().join("definitions.sqlite3")).unwrap();
    let id = "ecbca0a8-d457-4c42-89d4-6727261747bc";
    let definition = serde_json::json!({"schemaVersion":1,"id":id,"name":"Prepared","blocks":[{"id":"a","name":"A","prompt":"hello","position":{"x":0,"y":0},"configurationId":id}]});
    db.execute(
        "INSERT INTO configurations VALUES (?1,1,?2)",
        rusqlite::params![id, direct("openaiResponses", "OPENAI_API_KEY").to_string()],
    )
    .unwrap();
    db.execute(
        "INSERT INTO revisions VALUES (?1,1,?2)",
        rusqlite::params![id, definition.to_string()],
    )
    .unwrap();
    let request=serde_json::from_value(serde_json::json!({"requestId":"00000000-0000-4000-8000-000000000001","definitionId":id,"expectedRevision":1,"expectedConfigurationRevision":1,"supplied":{}})).unwrap();
    assert!(
        matches!(store.admit_run(request, workflow_engine::runs::Admission { image_id:"synthetic".into(),blocker:None }), Err(workflow_engine::Error::Validation { message }) if message.contains("Capture kind"))
    );
    for table in ["runs", "run_queue", "managed_sessions"] {
        assert_eq!(
            db.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row
                .get::<_, u32>(0))
                .unwrap(),
            0
        );
    }
}
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
    assert_eq!(
        status.credential_source,
        workflow_engine::CredentialSourceKind::NotInspected
    );
    assert_eq!(
        status.status,
        workflow_engine::ContainerAuthSupport::UnsupportedConcurrentAuth
    );
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
            config: config.clone().into(),
            expected_revision: None,
        })
        .unwrap();
    assert_eq!(saved.revision, 1);
    let expected = serde_json::json!({"id":"ecbca0a8-d457-4c42-89d4-6727261747bc","name":"Careful","model":"gpt-5.3-codex","reasoning":"high"});
    assert_eq!(serde_json::to_value(&saved.config).unwrap(), expected);
    let db = rusqlite::Connection::open(dir.path().join("definitions.sqlite3")).unwrap();
    let stored: String = db
        .query_row("SELECT body FROM configurations", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&stored).unwrap(),
        expected
    );
    assert_eq!(
        store.list_configurations().unwrap()[0].config,
        config.into()
    );
}
