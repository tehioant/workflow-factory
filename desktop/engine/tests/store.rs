use workflow_engine::{Definition, SaveDefinition, Store};

#[test]
fn reports_storage_failure_without_a_saved_revision() {
    let dir = seeded();
    let store = Store::open(dir.path()).unwrap();
    rusqlite::Connection::open(dir.path().join("definitions.sqlite3")).unwrap().execute_batch("CREATE TRIGGER fail_save BEFORE INSERT ON revisions BEGIN SELECT RAISE(ABORT, 'disk failure'); END;").unwrap();
    let definition = Definition {
        schema_version: 1,
        id: "ecbca0a8-d457-4c42-89d4-6727261747bc".into(),
        name: "Keep my edits".into(),
        inputs: vec![],
        blocks: vec![],
        connections: vec![],
    };
    assert!(matches!(
        store.save(SaveDefinition {
            definition,
            expected_revision: Some(3)
        }),
        Err(workflow_engine::Error::Storage { .. })
    ));
    assert_eq!(
        store
            .load("ecbca0a8-d457-4c42-89d4-6727261747bc")
            .unwrap()
            .revision,
        3
    );
}

#[test]
fn creates_private_storage_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("desktop-identity");
    Store::open(&data).unwrap();
    assert_eq!(
        std::fs::metadata(&data).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        std::fs::metadata(data.join("definitions.sqlite3"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn rejects_a_stale_save_without_overwriting() {
    let dir = seeded();
    let store = Store::open(dir.path()).unwrap();
    let definition = Definition {
        schema_version: 1,
        id: "ecbca0a8-d457-4c42-89d4-6727261747bc".into(),
        name: "Stale".into(),
        inputs: vec![],
        blocks: vec![],
        connections: vec![],
    };
    assert!(matches!(
        store.save(SaveDefinition {
            definition,
            expected_revision: Some(2)
        }),
        Err(workflow_engine::Error::Conflict { .. })
    ));
    assert_eq!(
        store
            .load("ecbca0a8-d457-4c42-89d4-6727261747bc")
            .unwrap()
            .definition
            .name,
        "Saved"
    );
}

#[test]
fn updates_only_the_expected_revision() {
    let dir = seeded();
    let store = Store::open(dir.path()).unwrap();
    let definition = Definition {
        schema_version: 1,
        id: "ecbca0a8-d457-4c42-89d4-6727261747bc".into(),
        name: "Updated".into(),
        inputs: vec![],
        blocks: vec![],
        connections: vec![],
    };
    let saved = store
        .save(SaveDefinition {
            definition,
            expected_revision: Some(3),
        })
        .unwrap();
    assert_eq!(saved.revision, 4);
    assert_eq!(saved.definition.name, "Updated");
}

#[test]
fn refuses_and_backs_up_a_newer_database_without_changing_it() {
    let dir = seeded();
    rusqlite::Connection::open(dir.path().join("definitions.sqlite3"))
        .unwrap()
        .execute_batch("PRAGMA user_version=99")
        .unwrap();
    assert!(matches!(
        Store::open(dir.path()),
        Err(workflow_engine::Error::UnsupportedSchema { .. })
    ));
    assert!(std::fs::read_dir(dir.path()).unwrap().any(|p| p
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with("backup-")));
    let version: u32 = rusqlite::Connection::open(dir.path().join("definitions.sqlite3"))
        .unwrap()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 99);
}

#[test]
fn rejects_an_invalid_name_before_persistence() {
    let dir = seeded();
    let store = Store::open(dir.path()).unwrap();
    let mut definition = store
        .load("ecbca0a8-d457-4c42-89d4-6727261747bc")
        .unwrap()
        .definition;
    definition.name = " ".into();
    assert!(matches!(
        store.save(SaveDefinition {
            definition,
            expected_revision: Some(3)
        }),
        Err(workflow_engine::Error::Validation { .. })
    ));
}

#[test]
fn lists_only_latest_saved_definitions() {
    let dir = seeded();
    let records = Store::open(dir.path()).unwrap().list().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].revision, 3);
    assert_eq!(records[0].definition.name, "Saved");
}

fn seeded() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let db = rusqlite::Connection::open(dir.path().join("definitions.sqlite3")).unwrap();
    db.execute_batch(r#"CREATE TABLE revisions (id TEXT NOT NULL, revision INTEGER NOT NULL, body TEXT NOT NULL, PRIMARY KEY(id,revision));
    PRAGMA user_version=1;
    INSERT INTO revisions VALUES ('ecbca0a8-d457-4c42-89d4-6727261747bc', 3, '{"schemaVersion":1,"id":"ecbca0a8-d457-4c42-89d4-6727261747bc","name":"Saved"}');"#).unwrap();
    dir
}

#[test]
fn loads_the_latest_prepared_revision() {
    let dir = seeded();
    let saved = Store::open(dir.path())
        .unwrap()
        .load("ecbca0a8-d457-4c42-89d4-6727261747bc")
        .unwrap();
    assert_eq!(saved.revision, 3);
    assert_eq!(saved.definition.name, "Saved");
}

#[test]
fn saves_a_new_definition_at_revision_one() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let saved = store
        .save(SaveDefinition {
            definition: Definition {
                schema_version: 1,
                id: "ecbca0a8-d457-4c42-89d4-6727261747bc".into(),
                name: "Research".into(),
                inputs: vec![],
                blocks: vec![],
                connections: vec![],
            },
            expected_revision: None,
        })
        .unwrap();
    assert_eq!(saved.revision, 1);
    assert_eq!(saved.definition.name, "Research");
}
