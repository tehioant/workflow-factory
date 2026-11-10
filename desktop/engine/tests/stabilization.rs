use workflow_engine::{SaveConfiguration, SaveDefinition, Store};
const ID: &str = "00000000-0000-0000-0000-000000000001";
fn definition() -> serde_json::Value {
    serde_json::json!({"schemaVersion":1,"id":ID,"name":"Prepared"})
}
fn config() -> serde_json::Value {
    serde_json::json!({"id":ID,"name":"Prepared","model":"codex","reasoning":"high"})
}
fn fixture() -> (tempfile::TempDir, Store, rusqlite::Connection) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let db = rusqlite::Connection::open(dir.path().join("definitions.sqlite3")).unwrap();
    (dir, store, db)
}
fn seed(db: &rusqlite::Connection, table: &str, id: &str, revision: i64, body: &serde_json::Value) {
    db.execute(
        &format!("INSERT INTO {table} VALUES (?1,?2,?3)"),
        rusqlite::params![id, revision, body.to_string()],
    )
    .unwrap();
}
#[test]
fn definition_capacity_is_checked_before_admission() {
    let (_dir, store, db) = fixture();
    for n in 2..=501 {
        let id = format!("00000000-0000-0000-0000-{n:012}");
        let mut d = definition();
        d["id"] = id.clone().into();
        seed(&db, "revisions", &id, 1, &d);
    }
    assert!(store
        .save(SaveDefinition {
            definition: serde_json::from_value(definition()).unwrap(),
            expected_revision: None
        })
        .is_err());
    assert_eq!(store.list().unwrap().len(), 500);
}
#[test]
fn configuration_capacity_is_checked_before_admission() {
    let (_dir, store, db) = fixture();
    for n in 2..=501 {
        let id = format!("00000000-0000-0000-0000-{n:012}");
        let mut c = config();
        c["id"] = id.clone().into();
        seed(&db, "configurations", &id, 1, &c);
    }
    assert!(store
        .save_configuration(SaveConfiguration {
            config: serde_json::from_value(config()).unwrap(),
            expected_revision: None
        })
        .is_err());
    assert_eq!(store.list_configurations().unwrap().len(), 500);
}
#[test]
fn corrupt_definition_rows_are_refused_at_every_read_boundary() {
    for (field, value, revision) in [
        (
            "id",
            serde_json::json!("00000000-0000-0000-0000-000000000002"),
            1,
        ),
        ("schemaVersion", serde_json::json!(2), 1),
        ("name", serde_json::json!(""), 1),
        (
            "blocks",
            serde_json::json!([{"id":"a","name":"A","prompt":"","position":{"x":1000001,"y":0}}]),
            1,
        ),
        ("name", serde_json::json!("Prepared"), 0),
        ("name", serde_json::json!("Prepared"), 4294967296),
    ] {
        for operation in ["load", "list", "save", "referenceScan"] {
            let (_dir, store, db) = fixture();
            let mut d = definition();
            d[field] = value.clone();
            seed(&db, "revisions", ID, revision, &d);
            seed(&db, "configurations", ID, 1, &config());
            let refused = match operation {
                "load" => store.load(ID).is_err(),
                "list" => store.list().is_err(),
                "save" => store
                    .save(SaveDefinition {
                        definition: serde_json::from_value(definition()).unwrap(),
                        expected_revision: Some(1),
                    })
                    .is_err(),
                _ => store.delete_configuration(ID, 1).is_err(),
            };
            assert!(refused, "{field} revision={revision} operation={operation}");
        }
    }
}
#[test]
fn corrupt_configuration_rows_are_refused_at_every_read_boundary() {
    for (field, value, revision) in [
        (
            "id",
            serde_json::json!("00000000-0000-0000-0000-000000000002"),
            1,
        ),
        ("model", serde_json::json!("bad model"), 1),
        ("name", serde_json::json!(""), 1),
        ("name", serde_json::json!("Prepared"), 0),
        ("name", serde_json::json!("Prepared"), 4294967296),
    ] {
        for operation in ["list", "save", "delete", "reference"] {
            let (_dir, store, db) = fixture();
            let mut c = config();
            c[field] = value.clone();
            seed(&db, "configurations", ID, revision, &c);
            let refused = match operation {
                "list" => store.list_configurations().is_err(),
                "save" => store
                    .save_configuration(SaveConfiguration {
                        config: serde_json::from_value(config()).unwrap(),
                        expected_revision: Some(1),
                    })
                    .is_err(),
                "delete" => store.delete_configuration(ID, 1).is_err(),
                _ => {
                    let mut d = definition();
                    d["blocks"] = serde_json::json!([{"id":"a","name":"A","prompt":"","position":{"x":0,"y":0},"configurationId":ID}]);
                    store
                        .save(SaveDefinition {
                            definition: serde_json::from_value(d).unwrap(),
                            expected_revision: None,
                        })
                        .is_err()
                }
            };
            assert!(refused, "{field} revision={revision} operation={operation}");
        }
    }
}
