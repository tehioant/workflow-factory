use workflow_engine::{Definition, SaveDefinition, Store};
fn graph() -> Definition {
    serde_json::from_str(r#"{"schemaVersion":1,"id":"ecbca0a8-d457-4c42-89d4-6727261747bc","name":"Graph","blocks":[{"id":"a","name":"First","prompt":"Own\nprompt","position":{"x":12,"y":34}},{"id":"b","name":"Second","prompt":"Other","position":{"x":240,"y":34}}],"connections":[{"source":"a","target":"b"}]}"#).unwrap()
}

#[test]
fn rejects_invalid_block_identity_before_saving() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let mut d = graph();
    d.blocks[1].id = "a".into();
    let error = store
        .save(SaveDefinition {
            definition: d,
            expected_revision: None,
        })
        .unwrap_err();
    assert!(
        matches!(error, workflow_engine::Error::Validation{message} if message.contains("block"))
    );
}

#[test]
fn rejects_branch_graph() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let mut d = graph();
    d.blocks.push(d.blocks[1].clone());
    d.blocks[2].id = "c".into();
    d.connections.push(workflow_engine::GraphConnection {
        source: "a".into(),
        target: "c".into(),
    });
    assert!(
        matches!(store.save(SaveDefinition {definition:d,expected_revision:None}), Err(workflow_engine::Error::Validation{message}) if message.contains("branch"))
    );
}
#[test]
fn rejects_cycle_graph() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let mut d = graph();
    d.connections.push(workflow_engine::GraphConnection {
        source: "b".into(),
        target: "a".into(),
    });
    assert!(
        matches!(store.save(SaveDefinition {definition:d,expected_revision:None}), Err(workflow_engine::Error::Validation{message}) if message.contains("cyclic"))
    );
}
#[test]
fn rejects_disconnected_graph() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let mut d = graph();
    d.connections.clear();
    assert!(
        matches!(store.save(SaveDefinition {definition:d,expected_revision:None}), Err(workflow_engine::Error::Validation{message}) if message.contains("Disconnected"))
    );
}
#[test]
fn saves_and_reopens_graph_properties() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let definition = graph();
    store
        .save(SaveDefinition {
            definition: definition.clone(),
            expected_revision: None,
        })
        .unwrap();
    assert_eq!(store.load(&definition.id).unwrap().definition, definition);
}
