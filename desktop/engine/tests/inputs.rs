use std::collections::BTreeMap;
use workflow_engine::{resolve_inputs, Definition};
fn definition() -> Definition {
    serde_json::from_str(r#"{"schemaVersion":1,"id":"ecbca0a8-d457-4c42-89d4-6727261747bc","name":"Inputs","blocks":[{"id":"a","name":"A","prompt":"Hello {{input.topic}} / {{input.note}}","position":{"x":0,"y":0}}],"inputs":[{"name":"topic","required":true,"default":null},{"name":"note","required":false,"default":"Default"}]}"#).unwrap()
}
#[test]
fn rejects_duplicate_input_names() {
    let mut d = definition();
    d.inputs[1].name = "topic".into();
    let dir = tempfile::tempdir().unwrap();
    let store = workflow_engine::Store::open(dir.path()).unwrap();
    assert!(
        matches!(store.save(workflow_engine::SaveDefinition{definition:d,expected_revision:None}),Err(workflow_engine::Error::Validation{message}) if message.contains("duplicate"))
    );
}
#[test]
fn rejects_dangling_references_on_save() {
    let mut d = definition();
    d.blocks[0].prompt = "{{input.missing}}".into();
    let dir = tempfile::tempdir().unwrap();
    let store = workflow_engine::Store::open(dir.path()).unwrap();
    assert!(
        matches!(store.save(workflow_engine::SaveDefinition{definition:d,expected_revision:None}),Err(workflow_engine::Error::Validation{message}) if message.contains("Dangling"))
    );
}
#[test]
fn missing_required_value_has_setup_guidance() {
    let d = definition();
    assert!(
        matches!(resolve_inputs(&d,BTreeMap::new()),Err(workflow_engine::Error::Validation{message}) if message.contains("topic"))
    );
}
#[test]
fn resolves_literal_values_once_without_changing_definition() {
    let d = definition();
    let original = d.clone();
    let values = BTreeMap::from([("topic".into(), "$1 {{input.note}}".into())]);
    let prepared = resolve_inputs(&d, values).unwrap();
    assert_eq!(prepared.prompts["a"], "Hello $1 {{input.note}} / Default");
    assert_eq!(d, original);
}
