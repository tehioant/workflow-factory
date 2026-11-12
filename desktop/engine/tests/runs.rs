use rusqlite::{params, Connection};
use workflow_engine::{
    runs::{Admission, RunState, RunStatus, StartRun},
    sessions::SecureStore,
    Store,
};
const DEFINITION: &str = "00000000-0000-0000-0000-000000000001";
#[test]
fn p2_prepared_successor_invokes_once_with_captured_prompt_and_same_resources() {
    use workflow_engine::{runs::BlockResult, sandbox::Cancel, sessions::Lease};
    for scenario in [
        "success",
        "failed",
        "held",
        "cancelled",
        "duplicate",
        "storage",
        "result-storage",
        "execution-failed",
        "recovery-won",
        "recovery-conflict",
    ] {
        let (_dir, store, c) = prepared();
        let mut s = status();
        s.blocks = vec![BlockResult {
            block_id: "a".into(),
            state: if scenario == "failed" {
                RunState::Failed
            } else {
                RunState::Succeeded
            },
            resolved_prompt: "A own".into(),
            final_response: Some("A literal {{input.topic}} <img>".into()),
        }];
        if scenario == "held" {
            s.state = RunState::Held;
            s.cleanup_pending = true;
        }
        if scenario == "duplicate" {
            s.blocks.push(BlockResult {
                block_id: "b".into(),
                state: RunState::Running,
                resolved_prompt: "already reserved".into(),
                final_response: None,
            });
        }
        seed_run(&c, &s, true);
        let config = serde_json::json!({"config":{"id":CONFIG,"name":"Captured","model":"captured-model","reasoning":"high"},"revision":2});
        let snapshot = serde_json::json!({"definition":{"definition":{"schemaVersion":1,"id":DEFINITION,"name":"Captured","blocks":[{"id":"b","name":"B","prompt":"B own $ 雪 / {{output.a.finalResponse}}","position":{"x":0,"y":0},"configurationId":CONFIG},{"id":"a","name":"A","prompt":"A own","position":{"x":0,"y":0},"configurationId":CONFIG}],"connections":[{"source":"a","target":"b"}],"inputs":[]},"revision":1},"configuration":config,"inputs":{"values":{},"prompts":{}},"imageId":"captured-image","plans":[{"block_id":"a","segments":[{"Literal":"A own"}],"configuration":config},{"block_id":"b","segments":[{"Literal":"B own $ 雪 / "},{"Output":"a"}],"configuration":config}]});
        c.execute("UPDATE runs SET snapshot=?1", [snapshot.to_string()])
            .unwrap();
        c.execute("DELETE FROM configurations", []).unwrap();
        c.execute(
            "INSERT INTO managed_sessions VALUES('slot','leased',7,'owned')",
            [],
        )
        .unwrap();
        if scenario == "storage" {
            c.execute_batch("CREATE TRIGGER refuse_transition BEFORE UPDATE ON runs BEGIN SELECT RAISE(FAIL,'storage failure'); END;").unwrap();
        }
        let lease = Lease {
            slot: "slot".into(),
            owner: "owned".into(),
            generation: 7,
        };
        let resources = String::from("prepared-existing-run-workspace");
        let cancel = Cancel::default();
        if scenario == "cancelled" {
            cancel.0.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        let mut calls = 0;
        if scenario == "result-storage" {
            c.execute_batch("CREATE TRIGGER refuse_result BEFORE UPDATE ON runs WHEN NEW.status LIKE '%B [REDACTED]%' BEGIN SELECT RAISE(FAIL,'result storage failure'); END;").unwrap();
        }
        let result = store.execute_next_block(
            "owned",
            &lease,
            &resources,
            &cancel,
            |same, plan, prompt| {
                calls += 1;
                assert!(std::ptr::eq(same, &resources));
                assert_eq!(plan.block_id, "b");
                assert_eq!(plan.configuration.config.model, "captured-model");
                assert_eq!(prompt, "B own $ 雪 / A literal {{input.topic}} <img>");
                if scenario == "execution-failed" {
                    return Err(workflow_engine::Error::Validation {
                        message: "Prepared immediate adapter failure".into(),
                    });
                }
                if matches!(scenario, "recovery-won" | "recovery-conflict") {
                    store
                        .retain_block_output(
                            "owned",
                            "b",
                            if scenario == "recovery-won" {
                                "B [REDACTED]"
                            } else {
                                "Other immutable response"
                            },
                        )
                        .unwrap();
                }
                Ok("B [REDACTED]".into())
            },
        );
        assert_eq!(
            result.is_ok(),
            matches!(scenario, "success" | "recovery-won"),
            "{scenario}"
        );
        assert_eq!(
            calls,
            usize::from(matches!(
                scenario,
                "success"
                    | "result-storage"
                    | "execution-failed"
                    | "recovery-won"
                    | "recovery-conflict"
            ))
        );
        let after = store.run_status("owned").unwrap();
        assert_eq!(after.blocks[0].final_response, s.blocks[0].final_response);
        if scenario == "execution-failed" {
            assert_eq!(after.blocks[1].state, RunState::Failed);
            assert_eq!(after.final_response, s.final_response);
        }
        if scenario == "result-storage" {
            assert_eq!(after.blocks[1].state, RunState::Running);
            assert_eq!(after.blocks[1].final_response, None);
        }
        if matches!(scenario, "success" | "recovery-won") {
            assert_eq!(after.blocks.len(), 2);
            assert_eq!(after.blocks[1].state, RunState::Succeeded);
            assert_eq!(
                after.blocks[1].final_response.as_deref(),
                Some("B [REDACTED]")
            );
            assert_eq!(
                after.blocks[1].resolved_prompt,
                "B own $ 雪 / A literal {{input.topic}} <img>"
            );
        }
        if scenario == "recovery-conflict" {
            assert_eq!(after.blocks[1].state, RunState::Succeeded);
            assert_eq!(
                after.blocks[1].final_response.as_deref(),
                Some("Other immutable response")
            );
            assert_eq!(
                after.final_response.as_deref(),
                Some("Other immutable response")
            );
        }
        assert_eq!(
            c.query_row("SELECT state FROM managed_sessions", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "leased"
        );
        assert_eq!(
            c.query_row("SELECT COUNT(*) FROM run_resources", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            0
        );
    }
}
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
        expected_configuration_revisions: Default::default(),
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
        blocks: Vec::new(),
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
    c.execute(
        "UPDATE revisions SET body=json_set(body,'$.isolation','perBlock')",
        [],
    )
    .unwrap();
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
    assert_eq!(
        snapshot["definition"]["definition"]["isolation"],
        "perBlock"
    );
}
#[test]
fn c02_duplicate_request_reads_original_without_current_configuration() {
    let (_dir, store, c) = prepared();
    let s = status();
    seed_run(&c, &s, true);
    c.execute("DELETE FROM configurations", []).unwrap();
    assert_eq!(store.recorded_run(&request()).unwrap().unwrap().id, s.id);
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
struct FailingStore(bool);
impl SecureStore for FailingStore {
    fn write(&mut self, _: &str, _: &[u8]) -> Result<(), workflow_engine::Error> {
        if self.0 {
            Err(workflow_engine::Error::Storage {
                message: "simulated denial".into(),
            })
        } else {
            Ok(())
        }
    }
    fn read(&mut self, _: &str) -> Result<Vec<u8>, workflow_engine::Error> {
        Ok(br#"{"auth_mode":"chatgpt","tokens":{"login_owner_id":"synthetic","access_token":"synthetic-a","refresh_token":"synthetic-r","id_token":"a.b.c","account_id":"synthetic-account","oauth_provenance":{"issuer":"https://auth.openai.com","client_id":"synthetic-client"}}}"#.to_vec())
    }
}
#[test]
fn c05_writeback_failure_quarantines_and_never_destroys_tmpfs() {
    for write_fails in [true, false] {
        let (_dir, store, c) = prepared();
        seed_run(&c, &status(), true);
        store
            .retain_run_output("owned", "validated [REDACTED] response")
            .unwrap();
        c.execute(
            "INSERT INTO managed_sessions VALUES('slot','leased',7,'owned')",
            [],
        )
        .unwrap();
        let cache=br#"{"auth_mode":"chatgpt","tokens":{"login_owner_id":"synthetic","access_token":"synthetic-a","refresh_token":"synthetic-r","id_token":"a.b.c","account_id":"synthetic-account","oauth_provenance":{"issuer":"https://auth.openai.com","client_id":"synthetic-client"}}}"#;
        assert!(store
            .finish_session(
                "slot",
                "owned",
                7,
                cache,
                &mut FailingStore(write_fails),
                || {
                    assert!(!write_fails, "cleanup before persistence");
                    Err(workflow_engine::Error::Storage {
                        message: "prepared cleanup refusal".into(),
                    })
                }
            )
            .is_err());
        assert_eq!(
            c.query_row("SELECT state FROM managed_sessions", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "quarantined"
        );
        store
            .finish_run("owned", None, "Cleanup held", false, false)
            .unwrap();
        assert_eq!(
            store.run_status("owned").unwrap().final_response.as_deref(),
            Some("validated [REDACTED] response")
        );
        assert!(store.run_status("owned").unwrap().cleanup_pending);
    }
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
fn d10_live_stream_projects_bounded_durable_pages() {
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::fs::PermissionsExt;
    use workflow_engine::{
        output::{DiagnosticIdentity, LiveDecoder, Publication},
        runs::inspection::Journal,
    };
    let root = tempfile::tempdir().unwrap();
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let id = "00000000-0000-0000-0000-000000000001";
    let identity = DiagnosticIdentity {
        run_id: id.into(),
        block_id: "a".into(),
        epoch: "prepared-epoch".into(),
    };
    let mut live = LiveDecoder::default();
    let frames=concat!("{\"version\":1,\"type\":\"wf.diagnostic\",\"seq\":1,\"kind\":\"command\",\"bytes\":10,\"text\":\"WITHHELD\",\"incomplete\":false}\n",
        "{\"version\":1,\"type\":\"wf.diagnostic\",\"seq\":2,\"kind\":\"stopped\",\"bytes\":20,\"text\":\"WITHHELD\",\"incomplete\":false}\n");
    for byte in frames.bytes() {
        live.push(&[byte]).unwrap();
    }
    live.finish().unwrap();
    assert!(live.push(b"{}\n").is_err());
    assert!(LiveDecoder::default().push(&vec![b'a'; 1025]).is_err());
    let raw = serde_json::json!({"version":1,"identity":identity,"records":[{"kind":"command","command":"check.py","text":"check.py:42 expected café, got 3 prepared-token","status":"completed","exitCode":1}],"final":"safe final","incomplete":false,"completed":true,"exit":0});
    let publication = Publication::decode(
        &serde_json::to_vec(&raw).unwrap(),
        &identity,
        &["prepared-token".into()],
    )
    .unwrap();
    Journal::commit(root.path(), id, &publication).unwrap();
    let page = Journal::page(root.path(), id, None, 0).unwrap();
    assert!(page.resync);
    assert_eq!(page.high, 1);
    assert_eq!(page.next, 1);
    assert!(page.entries[0].text.contains("café"));
    assert!(!page.entries[0].text.contains("prepared-token"));
    let path = root.path().join("inspection").join(id).join("journal");
    assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
    let before = std::fs::read(&path).unwrap();
    let next = Journal::page(root.path(), id, Some(&page.generation), page.next).unwrap();
    assert!(!next.resync);
    assert!(next.entries.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(Journal::page(root.path(), "00000000-0000-0000-0000-000000000002", None, 0).is_err());
    let mut wrong = identity.clone();
    wrong.block_id = "other".into();
    assert!(Publication::decode(&serde_json::to_vec(&raw).unwrap(), &wrong, &[]).is_err());
    let mut bulk = raw.clone();
    bulk["records"][0]["text"] = serde_json::json!("x".repeat(16000));
    let bulk = Publication::decode(&serde_json::to_vec(&bulk).unwrap(), &identity, &[]).unwrap();
    for _ in 0..70 {
        Journal::commit(root.path(), id, &bulk).unwrap();
    }
    let retained = Journal::page(root.path(), id, Some(&page.generation), 0).unwrap();
    assert!(retained.resync);
    assert!(retained.floor > 0);
    assert!(retained.incomplete);
    assert!(std::fs::metadata(&path).unwrap().len() <= 1_048_576);
    assert!(serde_json::to_vec(&retained.entries).unwrap().len() <= 32768);
    std::fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(root.path().join("outside"), &path).unwrap();
    assert!(Journal::page(root.path(), id, None, 0).is_err());
    assert!(Journal::commit(root.path(), id, &publication).is_err());
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
    assert_eq!(store.run_status("owned").unwrap().state, RunState::Held);
    assert_eq!(s.state, RunState::Held);
    assert!(s.cleanup_pending);
    assert_eq!(s.final_response.as_deref(), Some("retained"));
    assert_eq!(
        c.query_row("SELECT active FROM runs", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        1
    );
}

#[test]
fn c12_unclaimed_stop_releases_only_reservation_without_any_resource_intent() {
    for (state, intent, safe) in [
        (RunState::Admitted, false, true),
        (RunState::Admitted, true, false),
        (RunState::Running, false, false),
        (RunState::Held, false, false),
    ] {
        let (_dir, store, c) = prepared();
        let mut s = status();
        s.state = state;
        seed_run(&c, &s, true);
        c.execute(
            "INSERT INTO managed_sessions VALUES('slot','leased',7,'owned')",
            [],
        )
        .unwrap();
        if intent {
            c.execute(
                "INSERT INTO run_resources VALUES('owned','work-container',NULL)",
                [],
            )
            .unwrap();
        }
        let stopped = store.stop_run("owned").unwrap();
        assert_eq!(
            stopped.state,
            if safe {
                RunState::Stopped
            } else {
                RunState::Held
            }
        );
        assert_eq!(stopped.cleanup_pending, !safe);
        let (active, session): (bool, String) = c
            .query_row(
                "SELECT active,(SELECT state FROM managed_sessions) FROM runs",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(active, !safe);
        assert_eq!(session, if safe { "available" } else { "leased" });
        assert!(store.claim_run("owned").unwrap().is_none());
    }
}
#[test]
fn c13_quarantine_retirement_requires_positive_resource_and_secure_item_absence() {
    struct RetireStore(bool);
    impl SecureStore for RetireStore {
        fn read(&mut self, _: &str) -> Result<Vec<u8>, workflow_engine::Error> {
            panic!("old grant must never be read")
        }
        fn write(&mut self, _: &str, _: &[u8]) -> Result<(), workflow_engine::Error> {
            panic!("old grant must never be copied")
        }
        fn retire(&mut self, _: &str) -> Result<(), workflow_engine::Error> {
            self.0 = true;
            Ok(())
        }
    }
    for absent in [false, true] {
        let (_dir, store, c) = prepared();
        c.execute(
            "INSERT INTO managed_sessions VALUES(?1,'quarantined',7,?1)",
            [DEFINITION],
        )
        .unwrap();
        c.execute(
            "INSERT INTO run_resources VALUES(?1,'work-container',NULL)",
            [DEFINITION],
        )
        .unwrap();
        let mut secure = RetireStore(false);
        let result = store.retire_quarantined(&mut secure, |owner, records| {
            assert_eq!(owner, DEFINITION);
            assert_eq!(records, &[("work-container".into(), None)]);
            if absent {
                Ok(())
            } else {
                Err(workflow_engine::Error::Validation {
                    message: "absence unconfirmed".into(),
                })
            }
        });
        assert_eq!(result.is_ok(), absent);
        assert_eq!(secure.0, absent);
        assert_eq!(
            c.query_row("SELECT COUNT(*) FROM managed_sessions", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            if absent { 0 } else { 1 }
        );
        if absent {
            let fresh = store.begin_acquisition().unwrap();
            assert_ne!(fresh.slot, DEFINITION);
            assert_eq!(fresh.owner, fresh.slot);
        }
    }
}

#[test]
fn fhistory_prepared_rows_project_scoped_bounded_immutable_capture() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let c = Connection::open(dir.path().join("definitions.sqlite3")).unwrap();
    let config = serde_json::json!({"config":{"id":CONFIG,"name":"Captured <img>","model":"captured-model","reasoning":"high"},"revision":2});
    let definition = serde_json::json!({"definition":{"schemaVersion":1,"id":DEFINITION,"name":"Captured workflow","blocks":[{"id":"a","name":"A","prompt":"Hello {{input.topic}}","position":{"x":0,"y":0},"configurationId":CONFIG}],"inputs":[{"name":"topic","required":true,"default":null}]},"revision":1});
    let mut request = request();
    request.supplied.insert("topic".into(), "雪".repeat(100));
    let inputs = serde_json::json!({"values":{"topic":"雪".repeat(100)},"prompts":{"a":format!("Hello {}","雪".repeat(100))}});
    let snapshot = serde_json::json!({"definition":definition,"configuration":config,"inputs":inputs,"imageId":"captured-image","credentialExtra":"DO_NOT_EXPOSE"});
    for n in 1..=4 {
        let id = format!("10000000-0000-0000-0000-{n:012}");
        request.request_id = format!("20000000-0000-0000-0000-{n:012}");
        let status = serde_json::json!({"id":id,"requestId":request.request_id,"state":"held","finalResponse":"durable <script>text</script>","diagnostic":"cleanup held","cleanupPending":true,"secretExtra":"DO_NOT_EXPOSE"});
        c.execute(
            "INSERT INTO runs VALUES(?1,?2,?3,?4,?5,0)",
            params![
                id,
                request.request_id,
                serde_json::to_string(&request).unwrap(),
                snapshot.to_string(),
                status.to_string()
            ],
        )
        .unwrap();
    }
    let before: Vec<String> = c
        .prepare("SELECT request||snapshot||status FROM runs ORDER BY rowid")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let page = store.list_run_history(DEFINITION, None, 2).unwrap();
    assert_eq!(page.entries.len(), 2);
    assert!(page.entries[0].id.ends_with('4'));
    assert!(page.entries[0].input_summary.len() <= 240);
    assert!(!page.entries[0].unavailable);
    let next = store
        .list_run_history(DEFINITION, page.next_cursor.as_deref(), 2)
        .unwrap();
    assert!(next.entries[0].id.ends_with('2'));
    assert!(next.next_cursor.is_none());
    let id = &page.entries[0].id;
    let detail = store.run_detail(DEFINITION, id).unwrap();
    assert!(detail.legacy);
    assert_eq!(
        detail.definition.definition.isolation,
        workflow_engine::IsolationMode::PerRun
    );
    assert!(detail.blocks[0].result.is_none());
    assert_eq!(
        detail.legacy_configuration.unwrap().config.model,
        "captured-model"
    );
    assert_eq!(
        detail.final_response.as_deref(),
        Some("durable <script>text</script>")
    );
    let serialized = serde_json::to_string(&store.run_detail(DEFINITION, id).unwrap()).unwrap();
    assert!(!serialized.contains("DO_NOT_EXPOSE"));
    assert!(!serialized.contains("requestId"));
    assert!(!serialized.contains("snapshot"));
    assert!(store.run_detail(CONFIG, id).is_err());
    assert!(store.list_run_history(CONFIG, Some(id), 2).is_err());
    assert!(store.list_run_history(DEFINITION, None, 51).is_err());
    let after: Vec<String> = c
        .prepare("SELECT request||snapshot||status FROM runs ORDER BY rowid")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(before, after);
    // Corrupt stored identities never suppress healthy rows or become cursors.
    for bad_id in [
        "corrupt-id".to_string(),
        "z".repeat(36),
        format!("{CONFIG}{}", "x".repeat(2 * 1024 * 1024)),
    ] {
        c.execute(
            "INSERT INTO runs VALUES(?1,?2,'{}',?3,'{}',0)",
            params![
                bad_id,
                format!("bad-{}", bad_id.len()),
                snapshot.to_string()
            ],
        )
        .unwrap();
        let mut cursor = None;
        let mut seen = Vec::new();
        loop {
            let page = store
                .list_run_history(DEFINITION, cursor.as_deref(), 1)
                .unwrap();
            assert!(page.unavailable_records);
            assert!(!page.unknown_scope_records);
            seen.extend(page.entries.iter().map(|entry| entry.id.clone()));
            assert!(seen.len() <= 4, "pagination did not advance");
            match page.next_cursor {
                Some(next) => {
                    assert_ne!(cursor.as_ref(), Some(&next));
                    cursor = Some(next);
                }
                None => break,
            }
        }
        assert_eq!(seen.len(), 4);
        assert_eq!(
            seen.iter().collect::<std::collections::BTreeSet<_>>().len(),
            4
        );
        assert!(store
            .list_run_history(DEFINITION, Some(&bad_id), 1)
            .is_err());
        assert!(store.run_detail(DEFINITION, &bad_id).is_err());
        c.execute("DELETE FROM runs WHERE id=?1", [bad_id]).unwrap();
    }
    // Unscopable oversized/malformed snapshots produce a global, not workflow-attributed warning.
    for raw in [
        "{".to_string(),
        format!("{}{}", snapshot, " ".repeat(2 * 1024 * 1024)),
        serde_json::json!({"definition":{"definition":{"id":CONFIG}},"padding":"x".repeat(2 * 1024 * 1024)}).to_string(),
    ] {
        c.execute(
            "INSERT INTO runs VALUES(?1,'unscopable','{}',?2,'{}',0)",
            params![CONFIG, raw],
        )
        .unwrap();
        let page = store.list_run_history(DEFINITION, None, 1).unwrap();
        assert!(page.unknown_scope_records);
        assert!(!page.unavailable_records);
        assert_eq!(page.entries.len(), 1);
        assert!(!page.entries[0].unavailable);
        assert!(store.list_run_history(DEFINITION, Some(CONFIG), 1).is_err());
        assert!(store.run_detail(DEFINITION, CONFIG).is_err());
        c.execute("DELETE FROM runs WHERE id=?1", [CONFIG]).unwrap();
    }
    // Same captured projection invariant, now with explicit multiblock evidence and refusals.
    let mut multi = snapshot.clone();
    multi["definition"]["definition"]["blocks"][0]["prompt"] = serde_json::json!("A own");
    let mut b = multi["definition"]["definition"]["blocks"][0].clone();
    b["id"] = serde_json::json!("b");
    b["prompt"] = serde_json::json!("B {{output.a.finalResponse}}");
    let mut second_config = config.clone();
    second_config["config"]["id"] = serde_json::json!("30000000-0000-0000-0000-000000000005");
    second_config["config"]["model"] = serde_json::json!("second-model");
    second_config["revision"] = serde_json::json!(3);
    b["configurationId"] = second_config["config"]["id"].clone();
    request
        .expected_configuration_revisions
        .insert("30000000-0000-0000-0000-000000000005".into(), 3);
    // This compatibility field need not match the first execution plan when the map is present.
    request.expected_configuration_revision = 3;
    multi["definition"]["definition"]["blocks"]
        .as_array_mut()
        .unwrap()
        .push(b);
    multi["definition"]["definition"]["connections"] =
        serde_json::json!([{"source":"a","target":"b"}]);
    multi["inputs"]["prompts"] =
        serde_json::json!({"a":"A own","b":"B {{output.a.finalResponse}}"});
    multi["plans"] = serde_json::json!([{"block_id":"a","segments":[{"Literal":"A own"}],"configuration":config},{"block_id":"b","segments":[{"Literal":"B "},{"Output":"a"},{"Literal":""}],"configuration":second_config}]);
    request
        .expected_configuration_revisions
        .insert(CONFIG.into(), 2);
    request.request_id = "20000000-0000-0000-0000-000000000004".into();
    let status = serde_json::json!({"id":id,"requestId":request.request_id,"state":"failed","finalResponse":"A durable","diagnostic":"B failed","cleanupPending":true,"blocks":[{"blockId":"a","state":"succeeded","resolvedPrompt":"A own","finalResponse":"A durable"}]});
    c.execute(
        "UPDATE runs SET request=?1,snapshot=?2,status=?3 WHERE id=?4",
        params![
            serde_json::to_string(&request).unwrap(),
            multi.to_string(),
            status.to_string(),
            id
        ],
    )
    .unwrap();
    let detail = store.run_detail(DEFINITION, id).unwrap();
    assert!(!detail.legacy);
    assert!(detail.blocks[1].result.is_none());
    assert_eq!(
        detail.blocks[1]
            .configuration
            .as_ref()
            .unwrap()
            .config
            .model,
        "second-model"
    );
    assert_eq!(
        detail.blocks[0]
            .result
            .as_ref()
            .unwrap()
            .final_response
            .as_deref(),
        Some("A durable")
    );
    let mut failed_status = status.clone();
    failed_status["blocks"].as_array_mut().unwrap().push(serde_json::json!({"blockId":"b","state":"failed","resolvedPrompt":"B A durable","finalResponse":null}));
    c.execute(
        "UPDATE runs SET status=?1 WHERE id=?2",
        params![failed_status.to_string(), id],
    )
    .unwrap();
    let failed = store.run_detail(DEFINITION, id).unwrap();
    assert_eq!(
        failed.blocks[1].result.as_ref().unwrap().resolved_prompt,
        "B A durable"
    );
    assert_eq!(
        failed.blocks[0]
            .result
            .as_ref()
            .unwrap()
            .final_response
            .as_deref(),
        Some("A durable")
    );
    for corruption in [
        "status-id",
        "request-id",
        "revision",
        "schema",
        "duplicate-block",
        "oversized",
    ] {
        let mut bad = multi.clone();
        let mut bad_status = status.clone();
        match corruption {
            "status-id" => bad_status["id"] = serde_json::json!(CONFIG),
            "request-id" => bad_status["requestId"] = serde_json::json!(CONFIG),
            "revision" => bad["definition"]["revision"] = serde_json::json!(0),
            "schema" => bad["definition"]["definition"]["schemaVersion"] = serde_json::json!(99),
            "duplicate-block" => {
                let duplicate = bad_status["blocks"][0].clone();
                bad_status["blocks"].as_array_mut().unwrap().push(duplicate);
            }
            _ => bad_status["diagnostic"] = serde_json::json!("x".repeat(2 * 1024 * 1024)),
        }
        c.execute(
            "UPDATE runs SET snapshot=?1,status=?2 WHERE id=?3",
            params![bad.to_string(), bad_status.to_string(), id],
        )
        .unwrap();
        assert!(store.run_detail(DEFINITION, id).is_err(), "{corruption}");
        let page = store.list_run_history(DEFINITION, None, 2).unwrap();
        assert!(page.entries[0].unavailable, "{corruption}");
        assert!(!page.entries[1].unavailable);
    }
}
