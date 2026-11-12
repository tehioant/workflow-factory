use rusqlite::{params, Connection};
use workflow_engine::{
    runs::{Admission, RunState, RunStatus, StartRun},
    sessions::SecureStore,
    Store,
};
const DEFINITION: &str = "00000000-0000-0000-0000-000000000001";
const CONFIG: &str = "00000000-0000-0000-0000-000000000002";
fn prepared() -> (tempfile::TempDir, Store, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let c = Connection::open(dir.path().join("definitions.sqlite3")).unwrap();
    c.execute("INSERT INTO revisions VALUES(?1,1,?2)",params![DEFINITION,serde_json::json!({"schemaVersion":1,"id":DEFINITION,"name":"Prepared","blocks":[{"id":"a","name":"A","prompt":"Hello {{input.topic}}","position":{"x":0,"y":0},"configurationId":CONFIG}],"inputs":[{"name":"topic","required":true,"default":null}]}).to_string()]).unwrap();
    c.execute(
        "INSERT INTO configurations VALUES(?1,2,?2)",
        params![
            CONFIG,
            serde_json::json!({"id":CONFIG,"name":"Prepared","model":"codex","reasoning":"high"})
                .to_string()
        ],
    )
    .unwrap();
    (dir, store, c)
}
fn request() -> StartRun {
    StartRun {
        request_id: "00000000-0000-0000-0000-000000000003".into(),
        definition_id: DEFINITION.into(),
        expected_revision: 1,
        expected_configuration_revision: 2,
        supplied: std::collections::BTreeMap::from([("topic".into(), "literal $HOME".into())]),
    }
}
fn observation() -> Admission {
    Admission {
        image_id: format!("sha256:{}", "a".repeat(64)),
        blocker: Some("Native qualification missing".into()),
    }
}
fn status() -> RunStatus {
    RunStatus {
        id: "owned".into(),
        request_id: request().request_id,
        state: RunState::Running,
        final_response: Some("retained".into()),
        diagnostic: String::new(),
        cleanup_pending: false,
    }
}
fn seed_run(c: &Connection, s: &RunStatus, active: bool) {
    c.execute(
        "INSERT INTO runs VALUES(?1,?2,?3,'{}',?4,?5)",
        params![
            s.id,
            s.request_id,
            serde_json::to_string(&request()).unwrap(),
            serde_json::to_string(s).unwrap(),
            active
        ],
    )
    .unwrap();
}
#[test]
fn c01_admission_captures_saved_inputs_configuration_and_native_blocker() {
    let (_dir, store, c) = prepared();
    let result = store.admit_run(request(), observation()).unwrap();
    assert_eq!(result.state, RunState::Blocked);
    let body: String = c
        .query_row("SELECT snapshot FROM runs WHERE id=?1", [result.id], |r| {
            r.get(0)
        })
        .unwrap();
    let snapshot: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(snapshot["inputs"]["prompts"]["a"], "Hello literal $HOME");
    assert_eq!(snapshot["configuration"]["revision"], 2);
    assert_eq!(snapshot["definition"]["revision"], 1);
}
#[test]
fn c02_duplicate_request_reads_original_without_current_configuration() {
    let (_dir, store, c) = prepared();
    let s = status();
    seed_run(&c, &s, true);
    c.execute("DELETE FROM configurations", []).unwrap();
    assert_eq!(store.admit_run(request(), observation()).unwrap().id, s.id);
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM runs", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        1
    );
}
#[test]
fn c03_request_identity_cannot_bind_changed_payload() {
    let (_dir, store, c) = prepared();
    seed_run(&c, &status(), true);
    let mut r = request();
    r.supplied.insert("topic".into(), "different".into());
    assert!(store.admit_run(r, observation()).is_err());
}
#[test]
fn c04_restart_withholds_active_run_and_session_without_releasing() {
    let (_dir, store, c) = prepared();
    seed_run(&c, &status(), true);
    c.execute(
        "INSERT INTO managed_sessions VALUES('slot','leased',7,'owned')",
        [],
    )
    .unwrap();
    store.withhold_unfinished_runs().unwrap();
    assert_eq!(store.run_status("owned").unwrap().state, RunState::Held);
    assert_eq!(
        c.query_row("SELECT state FROM managed_sessions", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "quarantined"
    );
    assert_eq!(
        c.query_row("SELECT active FROM runs", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        1
    );
}
struct FailingStore;
impl SecureStore for FailingStore {
    fn write(&mut self, _: &str, _: &[u8]) -> Result<(), workflow_engine::Error> {
        Err(workflow_engine::Error::Storage {
            message: "simulated denial".into(),
        })
    }
    fn read(&mut self, _: &str) -> Result<Vec<u8>, workflow_engine::Error> {
        panic!("readback after failed write")
    }
}
#[test]
fn c05_writeback_failure_quarantines_and_never_destroys_tmpfs() {
    let (_dir, store, c) = prepared();
    c.execute(
        "INSERT INTO managed_sessions VALUES('slot','leased',7,'owned')",
        [],
    )
    .unwrap();
    let cache=br#"{"auth_mode":"chatgpt","login_owner_id":"synthetic","tokens":{"access_token":"synthetic-a","refresh_token":"synthetic-r","id_token":"synthetic-i"}}"#;
    assert!(store
        .finish_session("slot", "owned", 7, cache, &mut FailingStore, || panic!(
            "cleanup before persistence"
        ))
        .is_err());
    assert_eq!(
        c.query_row("SELECT state FROM managed_sessions", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "quarantined"
    );
}
#[test]
fn c06_structured_terminal_output_redacts_staged_and_rotated_secrets() {
    let bytes=b"{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"seed rotated\"}}\n{\"type\":\"turn.completed\"}\n";
    assert_eq!(
        workflow_engine::output::Decoder::decode(bytes, 0, &["seed".into(), "rotated".into()])
            .unwrap(),
        "[REDACTED] [REDACTED]"
    );
}
#[test]
fn c07_bounded_decoder_refuses_oversized_frame() {
    assert!(workflow_engine::output::Decoder::decode(&vec![b' '; 65537], 0, &[]).is_err());
}
#[test]
fn c08_scoped_stop_keeps_output_and_ownership_hold() {
    let (_dir, store, c) = prepared();
    seed_run(&c, &status(), true);
    let s = store.stop_run("owned").unwrap();
    assert_eq!(s.state, RunState::Held);
    assert!(s.cleanup_pending);
    assert_eq!(s.final_response.as_deref(), Some("retained"));
    assert_eq!(
        c.query_row("SELECT active FROM runs", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        1
    );
}
