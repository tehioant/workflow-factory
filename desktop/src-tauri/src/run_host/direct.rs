//! Direct execution uses the same worker, output registry and per-run final claim.
use super::*;
use workflow_engine::{
    runs::{
        direct::{DirectReservation, DirectSettlement},
        BlockPlan,
    },
    sandbox::Cancel,
    Error, Store,
};
pub fn direct_scope(d: &workflow_engine::runs::history::RunDetail) -> Result<String, Error> {
    if d.execution_kind != workflow_engine::runs::ExecutionKind::DirectModel || d.legacy {
        return Err(output_hold());
    }
    let mut scope=format!("Direct model API execution, separately billed API use. Approval authorizes this captured chain once: no tools, workspace, container, managed Codex session, fallback or automatic retry. A sent request may already be processed and billed even after Stop.\nDefinition: {} revision {}\nTransmit captured prompt templates and declared input values below, plus only durably successful predecessor outputs referenced by those templates.\nDeclared inputs: {:?}\n",d.definition_id,d.definition.revision,d.inputs.values);
    for block in &d.blocks {
        let config = block.configuration.as_ref().ok_or_else(output_hold)?;
        let workflow_engine::Configuration::DirectModel(c) = &config.config else {
            return Err(output_hold());
        };
        let prompt = &d
            .definition
            .definition
            .blocks
            .iter()
            .find(|b| b.id == block.block_id)
            .ok_or_else(output_hold)?
            .prompt;
        let request = workflow_engine::providers::prepare(c, "").map_err(|_| output_hold())?;
        scope.push_str(&format!("Block {}: provider {:?}; destination {}; model {}; output token cap {}; key binding {}; configuration {} revision {}\nPrompt template: {}\n",block.block_id,c.provider,request.url,c.model,c.max_output_tokens,crate::direct_model_host::variable(c.provider),c.id,config.revision,prompt));
        if scope.len() > 60000 {
            return Err(Error::Validation{message:"Captured consent scope exceeds native dialog bounds; shorten the prompts/inputs before starting.".into()});
        }
    }
    Ok(scope)
}
pub fn confirm_direct(scope: &str) -> Result<(), Error> {
    let _dialog = CONFIRMATION.lock().map_err(|_| output_hold())?;
    #[cfg(target_os="macos")]
 let result=std::process::Command::new("/usr/bin/osascript").args(["-e","on run argv\ndisplay dialog (item 1 of argv) with title \"Workflow Factory · billed API request\" buttons {\"Cancel\", \"Authorize this operation\"} default button \"Cancel\" cancel button \"Cancel\"\nend run","--",scope]).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
    #[cfg(not(target_os = "macos"))]
    let result = std::process::Command::new("/usr/bin/zenity")
        .args([
            "--question",
            "--no-markup",
            "--default-cancel",
            "--title=Workflow Factory · billed API request",
            "--text",
            scope,
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    if result.is_ok_and(|s| s.success()) {
        Ok(())
    } else {
        Err(Error::Validation {
            message: "Native confirmation declined or unavailable; no API request authorized."
                .into(),
        })
    }
}
pub fn execute_direct(store: &Store, r: &DirectReservation, cancel: &Cancel) {
    let Ok(claim) = final_claim(&r.run_id) else {
        return;
    };
    let Ok(_serialized) = claim.lock() else {
        return;
    };
    let Ok(Some(snapshot)) = store.claim_direct(r) else {
        if let Ok(mut claims) = final_claims().lock() {
            claims.remove(&r.run_id);
        }
        return;
    };
    let mut secrets = Vec::new();
    let mut text = None;
    let mut success = false;
    for index in 0..snapshot.plans.len() {
        let mut produced = false;
        let step = direct_step(store, r, cancel, |plan, prompt| {
            let workflow_engine::Configuration::DirectModel(config) = &plan.configuration.config
            else {
                return Err(output_hold());
            };
            let value =
                crate::direct_model_host::infer(config, prompt, cancel, &mut secrets, || {
                    store.check_direct_send(r)
                })?;
            produced = true;
            text = Some(value.clone());
            Ok(value)
        });
        match step {
            Ok(true) => {
                if index + 1 == snapshot.plans.len() {
                    success = true;
                    break;
                }
            }
            Ok(false) => break,
            Err(_) => {
                success = produced && index + 1 == snapshot.plans.len();
                break;
            }
        }
    }
    let chosen=DirectSettlement{reservation:r.clone(),text:text.or_else(||store.run_status(&r.run_id).ok().and_then(|s|s.final_response)),diagnostic:if success{"Direct API response returned; output storage only. Separately billed API use; no execution retry."}else{"Direct execution stopped, failed or interrupted; no automatic retry. A sent request may already have been processed and billed; Stop does not undo remote work or charges."}.into(),success};
    let Ok(mut finality) = claim.direct.lock() else {
        return;
    };
    *finality = Some(chosen.clone());
    drop(finality);
    if persist_direct(store, &claim, &chosen).is_ok() {
        if let Ok(mut claims) = final_claims().lock() {
            claims.remove(&r.run_id);
        }
    }
}
fn direct_step(
    store: &Store,
    r: &DirectReservation,
    cancel: &Cancel,
    invoke: impl FnOnce(&BlockPlan, &str) -> Result<String, Error>,
) -> Result<bool, Error> {
    let id = &r.run_id;
    let mut pending = None;
    let result = store.execute_next_block(id, r, &(), cancel, |_, plan, prompt| {
        store.check_direct_send(r)?;
        cancel.check()?;
        let text = invoke(plan, prompt)?;
        pending = Some(retain_output(
            &format!("{id}/{}", plan.block_id),
            text.clone(),
        )?);
        Ok(text)
    });
    match pending {
        Some(p) => p.settle(result),
        None => result,
    }
}
pub(super) fn persist_direct(
    store: &Store,
    claim: &Claim,
    chosen: &DirectSettlement,
) -> Result<(), Error> {
    let id = &chosen.reservation.run_id;
    let result = store
        .retain_direct_settlement(chosen)
        .and_then(|()| {
            retry_selected_outputs(id, |id, block, text| match block {
                Some(block) => store.retain_block_output(id, block, text),
                None => store.retain_run_output(id, text),
            })
        })
        .and_then(|()| store.settle_direct(chosen));
    if result.is_err() {
        claim
            .persistence_failure
            .store(2, std::sync::atomic::Ordering::Release);
        let _ = store.hold_direct_persistence(chosen, PERSISTENCE_HOLD);
    } else {
        claim
            .persistence_failure
            .store(0, std::sync::atomic::Ordering::Release);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn fixture() -> (
        std::path::PathBuf,
        Store,
        rusqlite::Connection,
        DirectReservation,
    ) {
        let root = std::env::temp_dir().join(format!(
            "mp03b-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let store = Store::open(&root).unwrap();
        let c = rusqlite::Connection::open(root.join("definitions.sqlite3")).unwrap();
        let id = "00000000-0000-0000-0000-000000000041";
        let d = "00000000-0000-0000-0000-000000000042";
        let config_id = "00000000-0000-0000-0000-000000000043";
        let request = "00000000-0000-0000-0000-000000000044";
        let config = json!({"config":{"schemaVersion":2,"kind":"directModel","id":config_id,"name":"API","provider":"openaiResponses","model":"gpt-5","maxOutputTokens":128,"credential":{"kind":"environment","variable":"OPENAI_API_KEY"}},"revision":1});
        let definition = json!({"revision":1,"definition":{"schemaVersion":1,"id":d,"name":"Prepared","blocks":[{"id":"a","name":"A","prompt":"Hello","position":{"x":0,"y":0},"configurationId":config_id}],"inputs":[]}});
        let snapshot = json!({"captureVersion":2,"executionKind":"directModel","definition":definition,"configuration":config,"inputs":{"values":{},"prompts":{"a":"Hello"}},"imageId":"","plans":[{"block_id":"a","segments":[{"Literal":"Hello"}],"configuration":config}]});
        c.execute("INSERT INTO runs VALUES(?1,?2,?3,?4,?5,1)",rusqlite::params![id,request,json!({"requestId":request,"definitionId":d,"expectedRevision":1,"expectedConfigurationRevision":1,"supplied":{}}).to_string(),snapshot.to_string(),json!({"id":id,"requestId":request,"state":"running","finalResponse":null,"diagnostic":"","cleanupPending":false,"blocks":[]}).to_string()]).unwrap();
        c.execute("INSERT INTO run_queue(run_id,phase,execution_kind) VALUES(?1,'occupied','directModel')",[id]).unwrap();
        let r = DirectReservation {
            run_id: id.into(),
            sequence: 1,
        };
        (root, store, c, r)
    }
    #[test]
    fn native_direct_consent_scope_names_paid_destinations_and_capture() {
        let _serial = super::super::tests::FINALITY_TEST.lock().unwrap();
        let (root, _store, _c, r) = fixture();
        let reader = workflow_engine::runs::history::HistoryReader::open(&root).unwrap();
        let d = reader
            .run_detail("00000000-0000-0000-0000-000000000042", &r.run_id)
            .unwrap();
        let scope = direct_scope(&d);
        assert!(scope.is_ok());
        let scope = scope.unwrap();
        for expected in [
            "https://api.openai.com/v1/responses",
            "gpt-5",
            "128",
            "Hello",
            "OPENAI_API_KEY",
            "billed",
            "predecessor",
            "no tools",
        ] {
            assert!(scope.contains(expected), "missing {expected}");
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn prepared_direct_finality_failure_is_visible_and_replays_storage_only() {
        let _serial = super::super::tests::FINALITY_TEST.lock().unwrap();
        let (root, store, c, r) = fixture();
        c.execute("UPDATE runs SET status=json_set(status,'$.blocks',json(?1),'$.finalResponse','done')",[json!([{"blockId":"a","state":"succeeded","resolvedPrompt":"Hello","finalResponse":"done"}]).to_string()]).unwrap();
        let f = DirectSettlement {
            reservation: r.clone(),
            text: Some("done".into()),
            diagnostic: "HTTP returned. API charges may apply.".into(),
            success: true,
        };
        let claim = final_claim(&r.run_id).unwrap();
        *claim.direct.lock().unwrap() = Some(f.clone());
        c.execute_batch("CREATE TRIGGER refuse_final BEFORE INSERT ON direct_run_settlements BEGIN SELECT RAISE(FAIL,'prepared'); END; CREATE TRIGGER refuse_hold BEFORE UPDATE ON runs BEGIN SELECT RAISE(FAIL,'prepared'); END;").unwrap();
        assert!(persist_direct(&store, &claim, &f).is_err());
        let reader = workflow_engine::runs::history::HistoryReader::open(&root).unwrap();
        let status = live_status(&reader, Some(&r.run_id)).unwrap().unwrap();
        assert_eq!(status.state, workflow_engine::runs::RunState::Held);
        assert!(status.diagnostic.contains("volatile"));
        assert!(!status.cleanup_pending);
        assert!(shutdown_pending().unwrap());
        c.execute_batch("DROP TRIGGER refuse_final; DROP TRIGGER refuse_hold;")
            .unwrap();
        retry_outputs(&store, &r.run_id).unwrap();
        assert_eq!(
            store.run_status(&r.run_id).unwrap().state,
            workflow_engine::runs::RunState::Succeeded
        );
        assert!(!final_claims().lock().unwrap().contains_key(&r.run_id));
        // Prepared restart: only the durable chosen result and successful block
        // survive; no in-memory final claim or HTTP execution authority exists.
        c.execute(
            "UPDATE runs SET active=1,status=json_set(status,'$.state','held')",
            [],
        )
        .unwrap();
        c.execute("UPDATE run_queue SET phase='occupied'", [])
            .unwrap();
        retry_outputs(&store, &r.run_id).unwrap();
        assert_eq!(
            store.run_status(&r.run_id).unwrap().state,
            workflow_engine::runs::RunState::Succeeded
        );
        assert!(!final_claims().lock().unwrap().contains_key(&r.run_id));
        assert!(!shutdown_pending().unwrap());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn prepared_native_direct_step_retains_before_durable_success() {
        let _serial = super::super::tests::FINALITY_TEST.lock().unwrap();
        let (root, store, _c, r) = fixture();
        let mut calls = 0;
        let result = direct_step(&store, &r, &Cancel::default(), |p, prompt| {
            calls += 1;
            assert_eq!(p.block_id, "a");
            assert_eq!(prompt, "Hello");
            Ok("redacted result".into())
        });
        assert!(result.unwrap());
        assert_eq!(calls, 1);
        assert_eq!(
            store.run_status(&r.run_id).unwrap().blocks[0]
                .final_response
                .as_deref(),
            Some("redacted result")
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
