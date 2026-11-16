use rusqlite::{params, Connection};
use serde_json::json;
use workflow_engine::{
    runs::{Admission, RunState, StartRun},
    Store,
};
const D: &str = "00000000-0000-0000-0000-000000000001";
const C: &str = "00000000-0000-0000-0000-000000000002";
fn prepared() -> (tempfile::TempDir, Store, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let c = Connection::open(dir.path().join("definitions.sqlite3")).unwrap();
    c.execute("INSERT INTO revisions VALUES(?1,1,?2)",params![D,json!({"schemaVersion":1,"id":D,"name":"Prepared","blocks":[{"id":"a","name":"A","prompt":"Hello","position":{"x":0,"y":0},"configurationId":C}],"inputs":[]}).to_string()]).unwrap();
    c.execute(
        "INSERT INTO configurations VALUES(?1,2,?2)",
        params![C, config().to_string()],
    )
    .unwrap();
    (dir, store, c)
}
fn config() -> serde_json::Value {
    json!({"schemaVersion":2,"kind":"directModel","id":C,"name":"API","provider":"openaiResponses","model":"gpt-5","maxOutputTokens":128,"credential":{"kind":"environment","variable":"OPENAI_API_KEY"}})
}
fn request() -> StartRun {
    StartRun {
        request_id: uuid::Uuid::new_v4().to_string(),
        definition_id: D.into(),
        expected_revision: 1,
        expected_configuration_revision: 2,
        expected_configuration_revisions: Default::default(),
        supplied: Default::default(),
    }
}
#[test]
fn direct_capture_reserves_fifo_capacity_without_sessions() {
    let (dir, store, c) = prepared();
    let result = store.admit_run(
        request(),
        Admission {
            image_id: String::new(),
            blocker: None,
        },
    );
    assert!(
        result.is_ok(),
        "direct capture must be admitted without Codex: {result:?}"
    );
    let run = result.unwrap();
    assert_eq!(run.state, RunState::AuthorizationRequired);
    let captured = workflow_engine::runs::history::HistoryReader::open(dir.path())
        .unwrap()
        .run_detail(D, &run.id)
        .unwrap();
    assert!(captured.image_id.is_empty());
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM managed_sessions", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(store.queue_settings().unwrap().active, 2);
}
// Independently seeded authority: no admission/worker journey.
fn seed(c: &Connection, state: &str) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let request = request();
    let definition: serde_json::Value = serde_json::from_str(
        &c.query_row("SELECT body FROM revisions", [], |r| r.get::<_, String>(0))
            .unwrap(),
    )
    .unwrap();
    let snapshot = json!({"captureVersion":2,"executionKind":"directModel","definition":{"definition":definition,"revision":1},"configuration":{"config":config(),"revision":2},"inputs":{"values":{},"prompts":{"a":"Hello"}},"imageId":"","plans":[{"block_id":"a","segments":serde_json::to_value(vec![workflow_engine::prompts::Segment::Literal("Hello".into())]).unwrap(),"configuration":{"config":config(),"revision":2}}]});
    c.execute("INSERT INTO runs VALUES(?1,?2,?3,?4,?5,1)",params![id,request.request_id,serde_json::to_string(&request).unwrap(),snapshot.to_string(),json!({"id":id,"requestId":request.request_id,"state":state,"finalResponse":null,"diagnostic":"","cleanupPending":false,"blocks":[]}).to_string()]).unwrap();
    c.execute(
        "INSERT INTO run_queue(run_id,phase,execution_kind) VALUES(?1,'occupied','directModel')",
        [&id],
    )
    .unwrap();
    id
}
#[test]
fn exact_direct_authorization_and_claim_reject_drift_freeze_and_reuse() {
    let (_dir, store, c) = prepared();
    let id = seed(&c, "authorizationRequired");
    let reservation = store.direct_reservation(&id).unwrap();
    assert!(
        reservation.is_some(),
        "direct occupied authority must exist without a lease"
    );
    let r = reservation.unwrap();
    let mut wrong = r.clone();
    wrong.sequence += 1;
    assert!(!store.authorize_direct(&wrong).unwrap());
    c.execute("UPDATE desktop_lifecycle SET frozen=1", [])
        .unwrap();
    assert!(!store.authorize_direct(&r).unwrap());
    c.execute("UPDATE desktop_lifecycle SET frozen=0", [])
        .unwrap();
    assert!(store.authorize_direct(&r).unwrap());
    assert!(!store.authorize_direct(&r).unwrap());
    assert!(store.claim_direct(&r).unwrap().is_some());
    assert!(store.claim_direct(&r).unwrap().is_none());
    assert_eq!(store.run_status(&id).unwrap().state, RunState::Running);
}
#[test]
fn prepared_direct_block_uses_durable_outputs_and_honors_cancel_freeze() {
    use workflow_engine::{runs::direct::DirectReservation, sandbox::Cancel};
    for mode in ["success", "cancel", "freeze", "pending"] {
        let (_dir, store, c) = prepared();
        let id = seed(&c, "running");
        let r = DirectReservation {
            run_id: id.clone(),
            sequence: c
                .query_row(
                    "SELECT sequence FROM run_queue WHERE run_id=?1",
                    [&id],
                    |r| r.get(0),
                )
                .unwrap(),
        };
        let cancel = Cancel::default();
        if mode == "cancel" {
            cancel.0.store(true, std::sync::atomic::Ordering::Release);
        }
        if mode == "freeze" {
            c.execute("UPDATE desktop_lifecycle SET frozen=1", [])
                .unwrap();
        }
        if mode == "pending" {
            c.execute("UPDATE runs SET status=json_set(status,'$.cleanupPending',json('true')) WHERE id=?1",[&id]).unwrap();
        }
        let mut invoked = false;
        let result = store.execute_next_block(&id, &r, &(), &cancel, |_, plan, prompt| {
            invoked = true;
            assert_eq!(plan.block_id, "a");
            assert_eq!(prompt, "Hello");
            Ok("literal result".into())
        });
        if mode == "success" {
            assert!(
                result.is_ok(),
                "prepared direct step must execute: {result:?}"
            );
            assert!(invoked);
            assert_eq!(
                store.run_status(&id).unwrap().blocks[0]
                    .final_response
                    .as_deref(),
                Some("literal result")
            );
        } else {
            assert!(result.is_err());
            assert!(!invoked);
        }
    }
}
#[test]
fn direct_settlement_is_exact_durable_storage_only_and_atomic() {
    use workflow_engine::runs::direct::{DirectReservation, DirectSettlement};
    for (state, text) in [
        ("running", "done".to_string()),
        ("cancelling", "done".to_string()),
        ("running", "a".repeat(262144)),
        ("running", format!("x{}", "\n".repeat(262143))),
        ("cancelling", "\"".repeat(262144)),
        ("running", "\\".repeat(262144)),
        (
            "running",
            format!("x{}{}", "\u{0001}".repeat(157200), "a".repeat(104943)),
        ),
    ] {
        let (dir, store, c) = prepared();
        let id = seed(&c, state);
        let wire = json!({"object":"response","status":"completed","error":null,"incomplete_details":null,"output":[{"type":"message","status":"completed","role":"assistant","content":[{"type":"output_text","text":text}]}]}).to_string();
        assert_eq!(
            workflow_engine::providers::parse_terminal(
                workflow_engine::DirectProviderId::OpenaiResponses,
                200,
                wire.as_bytes()
            )
            .unwrap(),
            text
        );
        c.execute("UPDATE runs SET status=json_set(status,'$.blocks',json(?1),'$.finalResponse',?2) WHERE id=?3",params![json!([{"blockId":"a","state":"succeeded","resolvedPrompt":"Hello","finalResponse":text}]).to_string(),text,id]).unwrap();
        let r = DirectReservation {
            run_id: id.clone(),
            sequence: c
                .query_row(
                    "SELECT sequence FROM run_queue WHERE run_id=?1",
                    [&id],
                    |r| r.get(0),
                )
                .unwrap(),
        };
        let f = DirectSettlement {
            reservation: r,
            text: Some(text.clone()),
            diagnostic: "\u{0001}".repeat(2048),
            success: true,
        };
        c.execute(
            "UPDATE runs SET status=json_set(status,'$.diagnostic',?1) WHERE id=?2",
            params![f.diagnostic, id],
        )
        .unwrap();
        let history = workflow_engine::runs::history::HistoryReader::open(dir.path()).unwrap();
        let queue = history
            .queue_status()
            .expect("valid direct status must remain visible in the read-only queue");
        assert_eq!(queue.occupied, 1);
        assert_eq!(queue.entries.len(), 1);
        assert_eq!(queue.entries[0].id, id);
        assert_eq!(queue.entries[0].definition_id, D);
        assert_eq!(queue.entries[0].diagnostic, f.diagnostic);
        assert!(!queue.entries[0].cleanup_pending);
        assert_eq!(
            queue.entries[0].state,
            if state == "running" {
                RunState::Running
            } else {
                RunState::Cancelling
            }
        );
        store.retain_direct_settlement(&f).unwrap();
        let retained = store.retained_direct_settlement(&id).unwrap();
        assert!(retained.is_some(), "chosen direct finality must be durable");
        assert!(retained.unwrap() == f, "literal payload must round trip");
        c.execute_batch("CREATE TRIGGER refuse_settle BEFORE UPDATE OF phase ON run_queue WHEN NEW.phase='settled' BEGIN SELECT RAISE(FAIL,'prepared'); END;").unwrap();
        assert!(store.settle_direct(&f).is_err());
        assert!(c
            .query_row("SELECT active FROM runs WHERE id=?1", [&id], |r| r
                .get::<_, bool>(0))
            .unwrap());
        c.execute_batch("DROP TRIGGER refuse_settle;").unwrap();
        // A new Store has no volatile worker/finality claim: recovery is storage only.
        let restarted = Store::open(dir.path()).unwrap();
        if text != "done" {
            restarted.withhold_unfinished_runs().unwrap();
            let recovered = restarted.retained_direct_settlement(&id).unwrap().unwrap();
            assert!(recovered == f);
            restarted.retain_direct_settlement(&recovered).unwrap();
            if state == "cancelling" {
                assert_eq!(restarted.stop_run(&id).unwrap().state, RunState::Stopped);
            } else {
                restarted.settle_direct(&recovered).unwrap();
            }
            let detail = restarted.run_detail(D, &id).unwrap();
            assert_eq!(detail.final_response.as_deref(), Some(text.as_str()));
            let history = workflow_engine::runs::history::HistoryReader::open(dir.path()).unwrap();
            assert_eq!(
                history
                    .run_status(Some(&id))
                    .unwrap()
                    .unwrap()
                    .final_response
                    .as_deref(),
                Some(text.as_str())
            );
            assert!(!history.list_run_history(D, None, 10).unwrap().entries[0].unavailable);
            assert_eq!(
                detail.state,
                if state == "cancelling" {
                    RunState::Stopped
                } else {
                    RunState::Succeeded
                }
            );
        }
        if state == "running" && text == "done" {
            assert_eq!(
                store.stop_run(&id).unwrap().state,
                RunState::Running,
                "last durable output wins over later Stop"
            );
        }
        store.settle_direct(&f).unwrap();
        let before = c.total_changes();
        store.settle_direct(&f).unwrap();
        assert_eq!(c.total_changes(), before);
        assert_eq!(
            store.run_status(&id).unwrap().state,
            if state == "running" {
                RunState::Succeeded
            } else {
                RunState::Stopped
            }
        );
        let mut conflict = f.clone();
        conflict.text = Some("changed".into());
        assert!(store.settle_direct(&conflict).is_err());
        assert_eq!(
            c.query_row("SELECT phase FROM run_queue WHERE run_id=?1", [&id], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
            "settled"
        );
    }
}
#[test]
fn direct_stop_and_repeated_startup_never_resend_or_require_cache_receipts() {
    for restart in [false, true] {
        let (_dir, store, c) = prepared();
        let id = seed(
            &c,
            if restart {
                "running"
            } else {
                "authorizationRequired"
            },
        );
        if restart {
            store.withhold_unfinished_runs().unwrap();
            store.withhold_unfinished_runs().unwrap();
            assert_eq!(store.run_status(&id).unwrap().state, RunState::Held);
            assert!(!store.run_status(&id).unwrap().cleanup_pending);
        }
        let result = store.stop_run(&id);
        assert!(
            result.is_ok(),
            "direct stop must not require a Codex lease: {result:?}"
        );
        assert_eq!(result.unwrap().state, RunState::Stopped);
        assert_eq!(
            c.query_row("SELECT COUNT(*) FROM managed_sessions", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            0
        );
        let signature = store.shutdown_snapshot().unwrap();
        store.freeze_shutdown(&signature).unwrap();
        assert!(store.shutdown_ready().unwrap());
    }
}
#[test]
fn prepared_successor_resolves_only_durable_succeeded_predecessor_literals() {
    use workflow_engine::{runs::direct::DirectReservation, sandbox::Cancel};
    for mode in ["success", "failed", "missing", "cancelled"] {
        let (_dir, store, c) = prepared();
        let id = seed(&c, "running");
        let mut snapshot: serde_json::Value = serde_json::from_str(
            &c.query_row("SELECT snapshot FROM runs WHERE id=?1", [&id], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
        )
        .unwrap();
        let mut block = snapshot["definition"]["definition"]["blocks"][0].clone();
        block["id"] = "b".into();
        block["name"] = "B".into();
        block["prompt"] = "{{output.a.finalResponse}}".into();
        snapshot["definition"]["definition"]["blocks"]
            .as_array_mut()
            .unwrap()
            .push(block);
        snapshot["definition"]["definition"]["connections"] = json!([{"source":"a","target":"b"}]);
        snapshot["inputs"]["prompts"]["b"] = "{{output.a.finalResponse}}".into();
        let mut plan = snapshot["plans"][0].clone();
        plan["block_id"] = "b".into();
        plan["segments"] = json!([{"Literal":""},{"Output":"a"},{"Literal":""}]);
        snapshot["plans"].as_array_mut().unwrap().push(plan);
        c.execute("UPDATE runs SET snapshot=?1,request=json_set(request,'$.expectedConfigurationRevisions',json(?2)),status=json_set(status,'$.blocks',json(?3),'$.state',?4) WHERE id=?5",params![snapshot.to_string(),json!({(C):2}).to_string(),json!([{"blockId":"a","state":if mode=="failed"{"failed"}else{"succeeded"},"resolvedPrompt":"Hello","finalResponse":if mode=="missing"{serde_json::Value::Null}else{json!("literal {{input.secret}}")}}]).to_string(),if mode=="cancelled"{"cancelling"}else{"running"},id]).unwrap();
        let r = DirectReservation {
            run_id: id.clone(),
            sequence: c
                .query_row(
                    "SELECT sequence FROM run_queue WHERE run_id=?1",
                    [&id],
                    |r| r.get(0),
                )
                .unwrap(),
        };
        let mut calls = 0;
        let result = store.execute_next_block(&id, &r, &(), &Cancel::default(), |_, p, prompt| {
            calls += 1;
            assert_eq!(p.block_id, "b");
            assert_eq!(prompt, "literal {{input.secret}}");
            Ok("next".into())
        });
        if mode == "success" {
            assert!(result.unwrap());
            assert_eq!(calls, 1);
        } else {
            assert!(result.is_err());
            assert_eq!(calls, 0);
        }
    }
}
#[test]
fn direct_fifo_keeps_credential_starved_codex_head_and_legacy_lease_authority() {
    let (_dir, store, c) = prepared();
    let first = seed(&c, "queued");
    let second = seed(&c, "queued");
    let codex = json!({"id":C,"name":"Legacy","model":"codex","reasoning":"high"});
    c.execute("UPDATE runs SET snapshot=json_set(snapshot,'$.executionKind','codex','$.imageId','real-image','$.configuration.config',json(?1),'$.plans[0].configuration.config',json(?1)) WHERE id=?2",params![codex.to_string(),first]).unwrap();
    c.execute("UPDATE run_queue SET phase='waiting',execution_kind=CASE WHEN run_id=?1 THEN 'codex' ELSE 'directModel' END",[&first]).unwrap();
    assert!(store.reserve_next_run().unwrap().is_none());
    assert!(store.direct_reservation(&second).unwrap().is_none());
    c.execute(
        "INSERT INTO managed_sessions VALUES('legacy-slot','available',7,NULL)",
        [],
    )
    .unwrap();
    assert_eq!(
        store.reserve_next_run().unwrap().as_deref(),
        Some(first.as_str())
    );
    assert!(store.execution_reservation(&first).unwrap().is_some());
    assert!(store.direct_reservation(&first).unwrap().is_none());
    assert_eq!(
        store.reserve_next_run().unwrap().as_deref(),
        Some(second.as_str())
    );
    assert!(store.direct_reservation(&second).unwrap().is_some());
}
#[test]
fn direct_capture_rejects_codex_mixtures_without_accepting_rows() {
    let (_dir, store, c) = prepared();
    let second = "00000000-0000-0000-0000-000000000008";
    c.execute(
        "INSERT INTO configurations VALUES(?1,1,?2)",
        params![
            second,
            json!({"id":second,"name":"Legacy","model":"codex","reasoning":"high"}).to_string()
        ],
    )
    .unwrap();
    c.execute("UPDATE revisions SET body=json_set(body,'$.blocks[1]',json(?1),'$.connections',json(?2))",params![json!({"id":"b","name":"B","prompt":"Next","position":{"x":0,"y":0},"configurationId":second}).to_string(),json!([{"source":"a","target":"b"}]).to_string()]).unwrap();
    let mut r = request();
    r.expected_configuration_revisions =
        std::collections::BTreeMap::from([(C.into(), 2), (second.into(), 1)]);
    assert!(store.request_execution_kind(&r).is_err());
    assert!(store
        .admit_run(
            r,
            Admission {
                image_id: String::new(),
                blocker: None
            }
        )
        .is_err());
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM runs", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        0
    );
}
#[test]
fn unclaimed_direct_stop_retries_failed_finality_without_execution() {
    for failure in ["retain", "settle"] {
        let (_dir, store, c) = prepared();
        let id = seed(&c, "authorizationRequired");
        if failure == "retain" {
            c.execute_batch("CREATE TRIGGER refuse_stop BEFORE INSERT ON direct_run_settlements BEGIN SELECT RAISE(FAIL,'prepared'); END;").unwrap();
        } else {
            c.execute_batch("CREATE TRIGGER refuse_stop BEFORE UPDATE OF phase ON run_queue WHEN NEW.phase='settled' BEGIN SELECT RAISE(FAIL,'prepared'); END;").unwrap();
        }
        assert!(store.stop_run(&id).is_err());
        assert_eq!(store.run_status(&id).unwrap().state, RunState::Cancelling);
        c.execute_batch("DROP TRIGGER refuse_stop;").unwrap();
        assert_eq!(
            store.stop_run(&id).unwrap().state,
            RunState::Stopped,
            "Stop storage retry must release only the never-claimed reservation"
        );
        assert_eq!(
            c.query_row("SELECT phase FROM run_queue WHERE run_id=?1", [&id], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
            "settled"
        );
    }
}
#[test]
fn retained_direct_finality_rejects_existing_corruption_without_replacement() {
    use workflow_engine::runs::direct::{DirectReservation, DirectSettlement};
    let (dir, store, c) = prepared();
    let id = seed(&c, "running");
    let history = workflow_engine::runs::history::HistoryReader::open(dir.path()).unwrap();
    let status: String = c
        .query_row("SELECT status FROM runs WHERE id=?1", [&id], |r| r.get(0))
        .unwrap();
    // Legacy projection retains the original 2 MiB ceiling; corruption is an
    // explicit error, never an empty queue or an allocation of the full body.
    c.execute(
        "UPDATE run_queue SET execution_kind='codex' WHERE run_id=?1",
        [&id],
    )
    .unwrap();
    assert_eq!(history.queue_status().unwrap().entries[0].id, id);
    let oversized_legacy = format!("{}{}", status, " ".repeat(2 * 1024 * 1024));
    c.execute(
        "UPDATE runs SET status=?1 WHERE id=?2",
        params![oversized_legacy, id],
    )
    .unwrap();
    assert!(history.queue_status().is_err());
    c.execute(
        "UPDATE run_queue SET execution_kind='directModel' WHERE run_id=?1",
        [&id],
    )
    .unwrap();
    assert_eq!(history.queue_status().unwrap().entries[0].id, id);
    c.execute("UPDATE runs SET status='{' WHERE id=?1", [&id])
        .unwrap();
    assert!(history.queue_status().is_err());
    c.execute("UPDATE runs SET status=?1 WHERE id=?2", params![status, id])
        .unwrap();
    assert!(store.retained_direct_settlement(&id).unwrap().is_none());
    let sequence = c
        .query_row(
            "SELECT sequence FROM run_queue WHERE run_id=?1",
            [&id],
            |r| r.get(0),
        )
        .unwrap();
    let f = DirectSettlement {
        reservation: DirectReservation {
            run_id: id.clone(),
            sequence,
        },
        text: Some("literal".into()),
        diagnostic: String::new(),
        success: false,
    };
    let valid = serde_json::to_value(&f).unwrap();
    let mut wrong_sequence = valid.clone();
    wrong_sequence["reservation"]["sequence"] = (sequence + 1).into();
    let mut wrong_id = valid.clone();
    wrong_id["reservation"]["run_id"] = D.into();
    let mut long_text = valid.clone();
    long_text["text"] = "x".repeat(262145).into();
    let mut long_diagnostic = valid.clone();
    long_diagnostic["diagnostic"] = "x".repeat(2049).into();
    for body in [
        "{".into(),
        " ".repeat(2 * 1024 * 1024),
        wrong_sequence.to_string(),
        wrong_id.to_string(),
        long_text.to_string(),
        long_diagnostic.to_string(),
    ] {
        c.execute(
            "INSERT OR REPLACE INTO direct_run_settlements VALUES(?1,?2,?3)",
            params![id, sequence, body],
        )
        .unwrap();
        assert!(
            store.retained_direct_settlement(&id).is_err(),
            "existing invalid finality must not look absent"
        );
        assert!(store.retain_direct_settlement(&f).is_err());
        assert_eq!(
            c.query_row(
                "SELECT body FROM direct_run_settlements WHERE run_id=?1",
                [&id],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            body
        );
    }
    c.execute("DELETE FROM direct_run_settlements", []).unwrap();
    // The storage contract supports every 256 KiB text, even the worst sixfold
    // JSON expansion (not a claim that this fits a provider's raw wire cap).
    let mut escaped = f.clone();
    escaped.text = Some("\u{0001}".repeat(262144));
    escaped.diagnostic = "\u{0001}".repeat(2048);
    store.retain_direct_settlement(&escaped).unwrap();
    assert!(store.retained_direct_settlement(&id).unwrap().unwrap() == escaped);
    c.execute_batch("CREATE TRIGGER reject_replay BEFORE INSERT ON direct_run_settlements BEGIN SELECT RAISE(FAIL,'must not write'); END;").unwrap();
    store.retain_direct_settlement(&escaped).unwrap();
    assert!(store.retain_direct_settlement(&f).is_err());
    c.execute_batch("DROP TRIGGER reject_replay; DELETE FROM direct_run_settlements;")
        .unwrap();
    let mut too_large = f.clone();
    too_large.text = Some("x".repeat(262145));
    assert!(store.retain_direct_settlement(&too_large).is_err());
    assert!(store.retained_direct_settlement(&id).unwrap().is_none());
}
#[test]
fn kind_preflight_resolves_exact_saved_revisions_before_native_probes() {
    let (_dir, store, c) = prepared();
    let mut r = request();
    assert_eq!(
        store.request_execution_kind(&r).unwrap(),
        workflow_engine::runs::ExecutionKind::DirectModel
    );
    r.expected_configuration_revision = 9;
    assert!(store.request_execution_kind(&r).is_err());
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM runs", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        0
    );
}
