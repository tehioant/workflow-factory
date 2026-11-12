use workflow_engine::{
    sandbox::{Cancel, Pipe, Resources, Sandbox},
    Error,
};
struct PreparedInspect;
impl Pipe for PreparedInspect {
    fn call(
        &mut self,
        args: &[String],
        input: &[u8],
        _: usize,
        _: &Cancel,
    ) -> Result<(i32, Vec<u8>, Vec<u8>), Error> {
        assert!(input.is_empty());
        assert_eq!(&args[..2], ["network", "inspect"]);
        Ok((0,serde_json::to_vec(&serde_json::json!([{"Id":"a".repeat(64),"Labels":{"org.workflow-factory.owner":"different-owner"}}])).unwrap(),vec![]))
    }
}
#[test]
fn c09_real_sandbox_mutation_refuses_nonowned_resource_before_any_effect() {
    let r = Resources {
        owner: "00000000-0000-0000-0000-000000000003".into(),
        image: format!("sha256:{}", "f".repeat(64)),
        network: "a".repeat(64),
        outer: "b".repeat(64),
        helper: "c".repeat(64),
        container: "d".repeat(64),
        workspace: None,
        peer: None,
        started_at: Some("prepared-start-epoch".into()),
        retired: None,
    };
    assert!(Sandbox {
        pipe: PreparedInspect
    }
    .inspect_work(&r)
    .is_err());
}
#[test]
fn c10_one_prepared_available_session_is_exclusively_leased() {
    let d = tempfile::tempdir().unwrap();
    let s = workflow_engine::Store::open(d.path()).unwrap();
    let c = rusqlite::Connection::open(d.path().join("definitions.sqlite3")).unwrap();
    c.execute("INSERT INTO managed_sessions VALUES('00000000-0000-0000-0000-000000000001','available',9,NULL)",[]).unwrap();
    let lease = s
        .lease_session("00000000-0000-0000-0000-000000000003")
        .unwrap();
    assert_eq!(lease.generation, 9);
    let row: (String, String) = c
        .query_row("SELECT state,owner FROM managed_sessions", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(row, ("leased".into(), lease.owner));
}

use serde_json::{json, Value};
struct Inspect {
    r: Resources,
    applied: bool,
    cancel_after_inspect: Option<Cancel>,
    executions: usize,
    guest_allowed: bool,
    guest_observations: usize,
    capture: Option<Vec<u8>>,
}
impl Pipe for Inspect {
    fn execution(
        &mut self,
        args: &[String],
        input: &[u8],
        cancel: &Cancel,
        consume: &mut dyn FnMut(&[u8]) -> Result<(), Error>,
    ) -> Result<i32, Error> {
        let (code, bytes, _) = self.call(args, input, 1_048_576, cancel)?;
        consume(&bytes)?;
        Ok(code)
    }
    fn call(
        &mut self,
        args: &[String],
        input: &[u8],
        _: usize,
        _: &Cancel,
    ) -> Result<(i32, Vec<u8>, Vec<u8>), Error> {
        if args.get(0..4)
            == Some(
                &[
                    "exec".into(),
                    self.r.container.clone(),
                    "node".into(),
                    "-e".into(),
                ][..],
            )
        {
            assert_eq!(args.len(), 5);
            assert_eq!(args[4], "const fs=require('node:fs');process.stdout.write(JSON.stringify({swaps:fs.readFileSync('/proc/swaps','utf8'),core:fs.readFileSync('/proc/sys/kernel/core_pattern','utf8')}))");
            assert!(input.is_empty());
            self.guest_observations += 1;
            let swaps = if self.guest_allowed {
                "Filename\tType\tSize\tUsed\tPriority\n"
            } else {
                "Filename\tType\tSize\tUsed\tPriority\n/swap file 1 0 -2\n"
            };
            return Ok((
                0,
                serde_json::to_vec(&json!({"swaps": swaps, "core": "core\n"})).unwrap(),
                vec![],
            ));
        }
        if args
            .last()
            .is_some_and(|s| s == "/opt/workflow-factory/capture-artifacts.mjs")
        {
            assert_eq!(
                args,
                &[
                    "exec",
                    "-i",
                    &self.r.container,
                    "node",
                    "/opt/workflow-factory/capture-artifacts.mjs"
                ]
                .map(str::to_string)
            );
            let request: Value = serde_json::from_slice(input).unwrap();
            assert_eq!(
                request["identity"],
                json!({"runId":self.r.owner,"blockId":"a","epoch":"prepared-start-epoch"})
            );
            assert!(request["cache"]
                .as_str()
                .unwrap()
                .contains("synthetic-refresh"));
            self.executions += 1;
            return Ok((0, self.capture.clone().expect("prepared capture"), vec![]));
        }
        if args[0] == "exec" {
            assert_eq!(
                args,
                &[
                    "exec",
                    "-i",
                    &self.r.container,
                    "node",
                    "/opt/workflow-factory/keeper.mjs",
                    "run",
                    "codex",
                    "high",
                    &self.r.owner,
                    "a",
                    "prepared-start-epoch"
                ]
                .map(str::to_string)
            );
            assert_eq!(input, b"literal $HOME; not shell");
            self.executions += 1;
            return Ok((0, b"prepared output".to_vec(), vec![]));
        }
        assert_eq!(args[1], "inspect");
        let r = &self.r;
        let label = json!({"org.workflow-factory.owner":r.owner});
        let private = format!("wf-private-{}", r.owner);
        let outer = format!("wf-outer-{}", r.owner);
        let endpoint = |id: &str| json!({"NetworkID":if self.applied{id}else{""}});
        let host = |memory: u64, pids: u32| json!({"ReadonlyRootfs":true,"Privileged":false,"Init":true,"Ulimits":[{"Name":"core","Soft":0,"Hard":0}],"Memory":memory,"MemorySwap":memory,"PidsLimit":pids,"NanoCpus":1000000000u64,"LogConfig":{"Type":"none"},"CapDrop":["ALL"],"SecurityOpt":["no-new-privileges"],"Tmpfs":{"/workspace":"rw,exec,nosuid,nodev,uid=1000,mode=700","/run/wf-codex":"rw,noexec,nosuid,nodev,uid=1000,mode=700","/tmp":"rw,noexec,nosuid,nodev,uid=1000,mode=700"}});
        let value: Value = if args[0] == "network" {
            json!({"Id":args[2],"Labels":label,"Driver":"bridge","Internal":args[2]==r.network,"EnableIPv6":false,"Options":{"com.docker.network.bridge.gateway_mode_ipv4":"isolated"},"Containers":if self.applied {if args[2]==r.network {json!({r.container.clone():{},r.helper.clone():{}})} else {json!({r.helper.clone():{}})}}else{json!({})}})
        } else if args[2] == r.helper {
            json!({"Id":r.helper,"Image":r.image,"Config":{"Labels":label,"User":"1000:1000"},"HostConfig":host(134217728,32),"NetworkSettings":{"Networks":{private:endpoint(&r.network),outer:endpoint(&r.outer)}}})
        } else {
            json!({"State":{"Running":true,"StartedAt":"prepared-start-epoch"},"RestartCount":0,"Id":r.container,"Image":r.image,"Config":{"Labels":label,"User":"1000:1000"},"HostConfig":host(536870912,128),"Mounts":[],"NetworkSettings":{"Networks":{private:endpoint(&r.network)}}})
        };
        if args[2] == r.outer {
            if let Some(cancel) = &self.cancel_after_inspect {
                cancel.request();
            }
        }
        Ok((0, serde_json::to_vec(&vec![value]).unwrap(), vec![]))
    }
}

#[test]
fn c14_inspection_accepts_inert_static_endpoints_but_requires_applied_ids_after_start() {
    let r = Resources {
        owner: "00000000-0000-0000-0000-000000000003".into(),
        image: format!("sha256:{}", "f".repeat(64)),
        network: "a".repeat(64),
        outer: "b".repeat(64),
        helper: "c".repeat(64),
        container: "d".repeat(64),
        workspace: None,
        peer: None,
        started_at: Some("prepared-start-epoch".into()),
        retired: None,
    };
    assert!(Sandbox {
        pipe: Inspect {
            r: r.clone(),
            applied: false,
            cancel_after_inspect: None,
            guest_allowed: true,
            guest_observations: 0,
            capture: None,
            executions: 0
        }
    }
    .inspect_phase(&r, false)
    .is_ok());
    assert!(Sandbox {
        pipe: Inspect {
            r: r.clone(),
            applied: false,
            cancel_after_inspect: None,
            guest_allowed: true,
            guest_observations: 0,
            capture: None,
            executions: 0
        }
    }
    .inspect_work(&r)
    .is_err());
    assert!(Sandbox {
        pipe: Inspect {
            r: r.clone(),
            applied: true,
            cancel_after_inspect: None,
            guest_allowed: true,
            guest_observations: 0,
            capture: None,
            executions: 0
        }
    }
    .inspect_work(&r)
    .is_ok());
    for (interrupted, guest_allowed) in [(false, true), (true, true)]
        .into_iter()
        .chain(cfg!(target_os = "macos").then_some((false, false)))
    {
        let cancel = Cancel::default();
        let mut sandbox = Sandbox {
            pipe: Inspect {
                r: r.clone(),
                applied: true,
                cancel_after_inspect: interrupted.then(|| cancel.clone()),
                guest_allowed,
                guest_observations: 0,
                capture: None,
                executions: 0,
            },
        };
        let config: workflow_engine::CodexConfiguration = serde_json::from_value(serde_json::json!({"id":"00000000-0000-0000-0000-000000000001","name":"Prepared","model":"codex","reasoning":"high"})).unwrap();
        assert_eq!(
            sandbox
                .execute(
                    &r,
                    "literal $HOME; not shell",
                    &config,
                    &cancel,
                    "a",
                    &mut |_| Ok(())
                )
                .is_ok(),
            !interrupted && guest_allowed
        );
        assert_eq!(
            sandbox.pipe.executions,
            usize::from(!interrupted && guest_allowed)
        );
        assert_eq!(
            sandbox.pipe.guest_observations,
            usize::from(cfg!(target_os = "macos") && !interrupted)
        );
    }
}
#[test]
fn c15_cancelled_create_never_mutates_even_when_cancel_arrives_in_intent() {
    struct NoWork;
    impl Pipe for NoWork {
        fn call(
            &mut self,
            _: &[String],
            _: &[u8],
            _: usize,
            _: &Cancel,
        ) -> Result<(i32, Vec<u8>, Vec<u8>), Error> {
            panic!("cancelled work transport invoked")
        }
    }
    let cancel = Cancel::default();
    let mut sandbox = Sandbox { pipe: NoWork };
    assert!(sandbox
        .create(
            "00000000-0000-0000-0000-000000000003",
            &format!("sha256:{}", "f".repeat(64)),
            &cancel,
            |_, _| {
                cancel.request();
                Ok(())
            }
        )
        .is_err());
    assert!(cancel.check().is_err()); // Native pre-spawn boundary uses the retained refusing-process fake probe.
}
use workflow_engine::{output::DiagnosticIdentity, runs::inspection::artifacts::Artifacts};
#[test]
fn d10_owned_workspace_capture_is_immutable_and_scoped() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let d = tempfile::tempdir().unwrap();
    std::fs::set_permissions(d.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let identity = DiagnosticIdentity {
        run_id: "00000000-0000-0000-0000-000000000003".into(),
        block_id: "a".into(),
        epoch: "prepared-start-epoch".into(),
    };
    let mut guest = serde_json::json!({"version":1,"identity":identity,"complete":true,"files":[{"name":"results/雪.txt","text":"immutable <script> café [REDACTED]"}]});
    let bytes = serde_json::to_vec(&guest).unwrap();
    let r = Resources {
        owner: identity.run_id.clone(),
        image: format!("sha256:{}", "f".repeat(64)),
        network: "a".repeat(64),
        outer: "b".repeat(64),
        helper: "c".repeat(64),
        container: "d".repeat(64),
        workspace: None,
        peer: None,
        started_at: Some(identity.epoch.clone()),
        retired: None,
    };
    let cache=br#"{"tokens":{"access_token":"synthetic-access","refresh_token":"synthetic-refresh","id_token":"a.b.c","login_owner_id":"synthetic","account_id":"synthetic-account","oauth_provenance":{"issuer":"https://auth.openai.com","client_id":"synthetic-client"}},"auth_mode":"chatgpt"}"#;
    let mut sandbox = Sandbox {
        pipe: Inspect {
            r: r.clone(),
            applied: true,
            cancel_after_inspect: None,
            executions: 0,
            guest_allowed: true,
            guest_observations: 0,
            capture: Some(bytes.clone()),
        },
    };
    sandbox
        .capture_artifacts(&r, "a", cache, &["synthetic-secret".into()], d.path())
        .unwrap();
    assert_eq!(sandbox.pipe.executions, 1);
    use workflow_engine::{output::Publication, runs::inspection::Journal};
    Journal::commit(
        d.path(),
        &identity.run_id,
        &Publication::acknowledged(identity.clone()),
    )
    .unwrap();
    let before = Journal::page(d.path(), &identity.run_id, None, 0).unwrap();
    let mut notified = false;
    Journal::capture_committed(d.path(), &identity.run_id, identity.clone(), || {
        assert!(Artifacts::read(d.path(), &identity.run_id).is_ok());
        let durable = Journal::page(
            d.path(),
            &identity.run_id,
            Some(&before.generation),
            before.high,
        )
        .unwrap();
        assert_eq!(durable.high, before.high + 1);
        assert_eq!(durable.entries[0].kind, "capture.committed");
        assert_eq!(durable.entries[0].text, "ARTIFACT_CAPTURE_COMMITTED");
        notified = true;
    })
    .unwrap();
    assert!(notified);
    let original = Artifacts::read(d.path(), &identity.run_id).unwrap();
    assert_eq!(original.files[0].text, "immutable <script> café [REDACTED]");
    assert!(uuid::Uuid::parse_str(&original.files[0].id).is_ok());
    guest["files"][0]["text"] = serde_json::json!("changed source");
    assert!(Artifacts::commit(
        d.path(),
        &identity,
        &serde_json::to_vec(&guest).unwrap(),
        &[]
    )
    .is_err());
    assert_eq!(
        Artifacts::read(d.path(), &identity.run_id).unwrap().files[0].text,
        original.files[0].text
    );
    for (name, text) in [
        ("../escape", "safe"),
        ("/absolute", "safe"),
        ("a//b", "safe"),
        ("safe", "synthetic-secret"),
    ] {
        let other = tempfile::tempdir().unwrap();
        std::fs::set_permissions(other.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        guest["files"][0] = serde_json::json!({"name":name,"text":text});
        assert!(Artifacts::commit(
            other.path(),
            &identity,
            &serde_json::to_vec(&guest).unwrap(),
            &["synthetic-secret".into()]
        )
        .is_err());
    }
    let mut wrong = identity.clone();
    wrong.run_id = "00000000-0000-0000-0000-000000000004".into();
    assert!(Artifacts::commit(d.path(), &wrong, &bytes, &[]).is_err());
    assert!(Artifacts::read(d.path(), &wrong.run_id).is_err());
    let file = d
        .path()
        .join("inspection")
        .join(&identity.run_id)
        .join("artifacts");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(Artifacts::read(d.path(), &identity.run_id).is_err());
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    let old = file.with_extension("old");
    std::fs::rename(&file, &old).unwrap();
    symlink(&old, &file).unwrap();
    assert!(Artifacts::read(d.path(), &identity.run_id).is_err());
    std::fs::remove_file(&file).unwrap();
    std::fs::hard_link(&old, &file).unwrap();
    assert!(Artifacts::read(d.path(), &identity.run_id).is_err());
}
