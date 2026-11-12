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

#[test]
fn p1_original_prompt_resolves_only_predecessor_final_response_literally() {
    use workflow_engine::prompts::{compile, resolve, Segment};
    let literal = "$ {{input.note}} {{output.b.finalResponse}} 雪";
    let segments = vec![
        Segment::Literal("Own ".into()),
        Segment::Literal(literal.into()),
        Segment::Output("a".into()),
    ];
    assert_eq!(
        resolve(&segments, &BTreeMap::from([("a".into(), literal.into())])).unwrap(),
        format!("Own {literal}{literal}")
    );
    let mut d = definition();
    d.blocks.push(
        serde_json::from_value(
            serde_json::json!({"id":"b","name":"B","prompt":"","position":{"x":0,"y":0}}),
        )
        .unwrap(),
    );
    d.connections.push(workflow_engine::GraphConnection {
        source: "a".into(),
        target: "b".into(),
    });
    for (token, accepted) in [
        ("{{output.a.finalResponse}}", true),
        ("{{output.b.finalResponse}}", false),
        ("{{output.missing.finalResponse}}", false),
        ("{{output.a.transcript}}", false),
    ] {
        d.blocks[1].prompt = token.into();
        let original = d.clone();
        let result = compile(&d, "b", &BTreeMap::new());
        assert_eq!(result.is_ok(), accepted, "{token}");
        assert_eq!(d, original);
    }
    d.blocks[1].prompt = "Own {{input.topic}} / {{output.a.finalResponse}}".into();
    let compiled = compile(&d, "b", &BTreeMap::from([("topic".into(), literal.into())])).unwrap();
    assert_eq!(
        resolve(&compiled, &BTreeMap::from([("a".into(), literal.into())])).unwrap(),
        format!("Own {literal} / {literal}")
    );
}
