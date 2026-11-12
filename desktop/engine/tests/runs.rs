use rusqlite::{params, Connection};
use workflow_engine::{
    runs::{Admission, RunState, RunStatus, StartRun},
    sessions::SecureStore,
    Store,
};
const DEFINITION: &str = "00000000-0000-0000-0000-000000000001";

#[test]
fn d11_prepared_admission_bounds_snapshot_and_idempotency() {
    use workflow_engine::runs::queue::QueueSettings;
    for mode in ["perRun", "perBlock"] {
        let (dir, store, c) = prepared();
        c.execute(
            "UPDATE revisions SET body=json_set(body,'$.isolation',?1)",
            [mode],
        )
        .unwrap();
        let mut captures = Vec::new();
        for topic in ["first own prompt", "second own prompt"] {
            let mut request = request();
            request.request_id = uuid::Uuid::new_v4().to_string();
            request.supplied.insert("topic".into(), topic.into());
            let accepted = store
                .admit_run(
                    request.clone(),
                    Admission {
                        image_id: "captured-image".into(),
                        blocker: None,
                    },
                )
                .unwrap();
            // Seed observed results independently: no workflow execution chain.
            c.execute(
                "UPDATE runs SET status=json_set(status,'$.finalResponse',?1) WHERE id=?2",
                params![format!("{topic} result"), accepted.id],
            )
            .unwrap();
            let reader = workflow_engine::runs::history::HistoryReader::open(dir.path()).unwrap();
            let captured = reader.run_detail(DEFINITION, &accepted.id).unwrap();
            assert_eq!(captured.inputs.prompts["a"], format!("Hello {topic}"));
            assert_eq!(
                captured.final_response.as_deref(),
                Some(format!("{topic} result").as_str())
            );
            assert_eq!(
                reader
                    .run_status(Some(&accepted.id))
                    .unwrap()
                    .unwrap()
                    .request_id,
                request.request_id
            );
            captures.push((
                accepted.id.clone(),
                workflow_engine::sandbox::resource_names(&accepted.id),
            ));
        }
        assert_ne!(captures[0].0, captures[1].0);
        for i in 0..3 {
            assert_ne!(captures[0].1[i], captures[1].1[i]);
        }
        // Direct production reservation boundary, with independently prepared slots.
        for generation in [7, 11] {
            c.execute(
                "INSERT INTO managed_sessions VALUES(?1,'available',?2,NULL)",
                params![uuid::Uuid::new_v4().to_string(), generation],
            )
            .unwrap();
        }
        assert_eq!(
            store.reserve_next_run().unwrap().as_deref(),
            Some(captures[0].0.as_str())
        );
        assert_eq!(
            store.reserve_next_run().unwrap().as_deref(),
            Some(captures[1].0.as_str())
        );
        let first = store
            .execution_reservation(&captures[0].0)
            .unwrap()
            .unwrap();
        let second = store
            .execution_reservation(&captures[1].0)
            .unwrap()
            .unwrap();
        assert_eq!(first.owner, captures[0].0);
        assert_eq!(second.owner, captures[1].0);
        assert_ne!(first.slot, second.slot);
        // One stage operation per independently captured owner; never execute/settle a workflow.
        struct Seed {
            slot: String,
            bytes: Vec<u8>,
            reads: usize,
        }
        impl SecureStore for Seed {
            fn read(&mut self, slot: &str) -> Result<Vec<u8>, workflow_engine::Error> {
                assert_eq!(slot, self.slot);
                self.reads += 1;
                Ok(self.bytes.clone())
            }
            fn write(&mut self, _: &str, _: &[u8]) -> Result<(), workflow_engine::Error> {
                panic!("stage must not write secure store")
            }
        }
        struct Stage {
            r: workflow_engine::sandbox::Resources,
            bytes: Vec<u8>,
            staged: Vec<(String, Vec<u8>)>,
            observations: usize,
            unsafe_guest: bool,
            cancel_after_observation: bool,
        }
        impl workflow_engine::sandbox::Pipe for Stage {
            fn call(
                &mut self,
                args: &[String],
                input: &[u8],
                limit: usize,
                cancel: &workflow_engine::sandbox::Cancel,
            ) -> Result<(i32, Vec<u8>, Vec<u8>), workflow_engine::Error> {
                use serde_json::json;
                let r = &self.r;
                if args.last().map(String::as_str) == Some("stage") {
                    assert_eq!(
                        args,
                        &[
                            "exec",
                            "-i",
                            &r.container,
                            "node",
                            "/opt/workflow-factory/keeper.mjs",
                            "stage"
                        ]
                        .map(str::to_string)
                    );
                    assert_eq!(input, self.bytes);
                    self.staged.push((r.container.clone(), input.to_vec()));
                    return Ok((0, vec![], vec![]));
                }
                assert!(input.is_empty());
                if args[0] == "exec" {
                    let guest = ["exec", &r.container, "node", "-e", "const fs=require('node:fs');process.stdout.write(JSON.stringify({swaps:fs.readFileSync('/proc/swaps','utf8'),core:fs.readFileSync('/proc/sys/kernel/core_pattern','utf8')}))"].map(str::to_string);
                    let cgroup = ["exec", &r.container, "node", "-e", "process.stdout.write(require('node:fs').readFileSync('/sys/fs/cgroup/memory.swap.max','utf8'))"].map(str::to_string);
                    if args == guest {
                        assert_eq!(limit, 16384);
                        self.observations += 1;
                        assert_eq!(self.observations, 1);
                        if self.cancel_after_observation {
                            cancel.0.store(true, std::sync::atomic::Ordering::Release);
                        }
                        return Ok((0, serde_json::to_vec(&json!({
                            "swaps": "Filename\tType\tSize\tUsed\tPriority\n",
                            "core": if self.unsafe_guest { "|unsafe-handler\n" } else { "core\n" }
                        })).unwrap(), vec![]));
                    }
                    assert_eq!(args, cgroup, "unknown observation command");
                    assert_eq!(limit, 1024);
                    return Ok((0, b"0\n".to_vec(), vec![]));
                }
                assert_eq!(args[1], "inspect");
                let label = json!({"org.workflow-factory.owner":r.owner});
                let private = format!("wf-private-{}", r.owner);
                let outer = format!("wf-outer-{}", r.owner);
                let host = |memory: u64, pids: u32| json!({"ReadonlyRootfs":true,"Privileged":false,"Init":true,"Ulimits":[{"Name":"core","Soft":0,"Hard":0}],"Memory":memory,"MemorySwap":memory,"PidsLimit":pids,"NanoCpus":1000000000u64,"LogConfig":{"Type":"none"},"CapDrop":["ALL"],"SecurityOpt":["no-new-privileges"],"Tmpfs":{"/workspace":"rw,exec,nosuid,nodev,uid=1000,mode=700","/run/wf-codex":"rw,noexec,nosuid,nodev,uid=1000,mode=700","/tmp":"rw,noexec,nosuid,nodev,uid=1000,mode=700"}});
                let value = if args[0] == "volume" {
                    assert_eq!(Some(&args[2]), r.workspace.as_ref());
                    json!({"Name":args[2],"Driver":"local","Scope":"local","Labels":label,"Options":{"type":"tmpfs","device":"tmpfs","o":"rw,exec,nosuid,nodev,size=256m,uid=1000,gid=1000,mode=700"}})
                } else if args[0] == "network" {
                    assert!([&r.network, &r.outer].contains(&&args[2]));
                    json!({"Id":args[2],"Labels":label,"Driver":"bridge","Internal":args[2]==r.network,"EnableIPv6":false,"Options":{"com.docker.network.bridge.gateway_mode_ipv4":"isolated"},"Containers":if args[2]==r.network {json!({r.container.clone():{},r.helper.clone():{}})}else{json!({r.helper.clone():{}})}})
                } else if args[2] == r.helper {
                    {
                        let mut helper_host = host(134217728, 32);
                        helper_host["Tmpfs"] =
                            json!({"/tmp":"rw,noexec,nosuid,nodev,uid=1000,mode=700"});
                        json!({"Id":r.helper,"Image":r.image,"Config":{"Labels":label,"User":"1000:1000"},"HostConfig":helper_host,"Mounts":[],"NetworkSettings":{"Networks":{private:{"NetworkID":r.network},outer:{"NetworkID":r.outer}}}})
                    }
                } else {
                    assert_eq!(args[2], r.container);
                    let mut row = json!({"State":{"Running":true,"StartedAt":"prepared-stage-epoch"},"RestartCount":0,"Id":r.container,"Image":r.image,"Config":{"Labels":label,"User":"1000:1000","Entrypoint":["node"],"Cmd":["/opt/workflow-factory/keeper.mjs"]},"HostConfig":host(536870912,128),"Mounts":[],"NetworkSettings":{"Networks":{private:{"NetworkID":r.network}}}});
                    if let Some(name) = &r.workspace {
                        row["HostConfig"]["Tmpfs"]
                            .as_object_mut()
                            .unwrap()
                            .remove("/workspace");
                        row["HostConfig"]["Mounts"] = json!([{"Type":"volume","Source":name,"Target":"/workspace","VolumeOptions":{"NoCopy":true}}]);
                        row["Mounts"] = json!([{"Type":"volume","Name":name,"Driver":"local","Destination":"/workspace","RW":true}]);
                    }
                    row
                };
                Ok((0, serde_json::to_vec(&json!([value])).unwrap(), vec![]))
            }
        }
        let mut bindings = Vec::new();
        for (index, lease) in [&first, &second].into_iter().enumerate() {
            let captured = workflow_engine::runs::history::HistoryReader::open(dir.path())
                .unwrap()
                .run_detail(DEFINITION, &captures[index].0)
                .unwrap();
            assert_eq!(lease.owner, captured.id);
            let resources = workflow_engine::sandbox::Resources {
                owner: captured.id.clone(),
                image: format!("sha256:{}", "f".repeat(64)),
                network: format!("{:064x}", index * 4 + 1),
                outer: format!("{:064x}", index * 4 + 2),
                helper: format!("{:064x}", index * 4 + 3),
                container: format!("{:064x}", index * 4 + 4),
                workspace: (mode == "perBlock").then(|| format!("wf-workspace-{}", captured.id)),
                peer: None,
                started_at: Some("prepared-stage-epoch".into()),
                retired: None,
            };
            let cache=serde_json::to_vec(&serde_json::json!({"auth_mode":"chatgpt","tokens":{"login_owner_id":format!("synthetic-owner-{index}"),"access_token":format!("synthetic-access-{index}"),"refresh_token":format!("synthetic-refresh-{index}"),"id_token":"a.b.c","account_id":"synthetic","oauth_provenance":{"issuer":"https://auth.openai.com","client_id":"synthetic-client"}}})).unwrap();
            let mut secure = Seed {
                slot: lease.slot.clone(),
                bytes: cache.clone(),
                reads: 0,
            };
            let mut sandbox = workflow_engine::sandbox::Sandbox {
                pipe: Stage {
                    r: resources.clone(),
                    bytes: cache.clone(),
                    staged: Vec::new(),
                    observations: 0,
                    unsafe_guest: false,
                    cancel_after_observation: false,
                },
            };
            assert_eq!(
                sandbox
                    .stage_owned(
                        lease,
                        &resources,
                        &mut secure,
                        &workflow_engine::sandbox::Cancel::default()
                    )
                    .unwrap(),
                cache
            );
            assert_eq!(secure.reads, 1);
            assert_eq!(
                sandbox.pipe.staged,
                vec![(resources.container.clone(), cache.clone())]
            );
            assert_eq!(
                sandbox.pipe.observations,
                usize::from(cfg!(target_os = "macos"))
            );
            #[cfg(target_os = "macos")]
            for (unsafe_guest, cancel_after_observation) in [(true, false), (false, true)] {
                let mut denied = workflow_engine::sandbox::Sandbox {
                    pipe: Stage {
                        r: resources.clone(),
                        bytes: cache.clone(),
                        staged: Vec::new(),
                        observations: 0,
                        unsafe_guest,
                        cancel_after_observation,
                    },
                };
                let cancel = workflow_engine::sandbox::Cancel::default();
                let error = denied
                    .stage_owned(lease, &resources, &mut secure, &cancel)
                    .unwrap_err();
                let message = format!("{error:?}");
                assert!(
                    message.contains(if unsafe_guest {
                        "Docker Desktop guest swap/core policy unavailable"
                    } else {
                        "Scoped stop requested"
                    }),
                    "{message}"
                );
                assert_eq!(denied.pipe.observations, 1);
                assert!(denied.pipe.staged.is_empty());
                assert_eq!(
                    cancel.0.load(std::sync::atomic::Ordering::Acquire),
                    cancel_after_observation
                );
            }
            bindings.push((
                captured.id,
                lease.slot.clone(),
                lease.generation,
                resources.container,
                resources.workspace,
                cache,
            ));
        }
        assert_ne!(bindings[0].0, bindings[1].0);
        assert_ne!(bindings[0].1, bindings[1].1);
        assert_ne!(bindings[0].3, bindings[1].3);
        assert_ne!(bindings[0].5, bindings[1].5);
        if mode == "perBlock" {
            assert_ne!(bindings[0].4, bindings[1].4);
        }

        assert!(store.reserve_next_run().unwrap().is_none());
    }
    {
        let (dir, _store, c) = prepared();
        let held = status();
        seed_run(&c, &held, true);
        c.execute(
            "INSERT INTO managed_sessions VALUES('owned-slot','quarantined',7,'owned')",
            [],
        )
        .unwrap();
        let before: String = c
            .query_row("SELECT request||snapshot||status FROM runs", [], |r| {
                r.get(0)
            })
            .unwrap();
        c.execute_batch("DROP TABLE run_queue; DROP TABLE queue_settings; CREATE UNIQUE INDEX one_active_run ON runs(active) WHERE active=1; PRAGMA user_version=3;").unwrap();
        let migrated = Store::open(dir.path()).unwrap();
        assert_eq!(migrated.queue_settings().unwrap().active, 2);
        assert_eq!(
            c.query_row("SELECT request||snapshot||status FROM runs", [], |r| r
                .get::<_, String>(
                0
            ))
            .unwrap(),
            before
        );
        assert_eq!(
            c.query_row(
                "SELECT phase||':'||slot||':'||generation FROM run_queue WHERE run_id='owned'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "occupied:owned-slot:7"
        );
        assert_eq!(
            c.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name='one_active_run'",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            0
        );
    }
    let (_dir, store, c) = prepared();
    assert_eq!(
        (
            store.queue_settings().unwrap().active,
            store.queue_settings().unwrap().waiting
        ),
        (2, 16)
    );
    for (active, waiting) in [(0, 16), (9, 16), (2, 65)] {
        assert!(store
            .save_queue_settings(QueueSettings { active, waiting })
            .is_err());
    }
    store
        .save_queue_settings(QueueSettings {
            active: 8,
            waiting: 1,
        })
        .unwrap();
    let accepted = store
        .admit_run(
            request(),
            Admission {
                image_id: "captured-image".into(),
                blocker: None,
            },
        )
        .unwrap();
    assert_eq!(accepted.state, RunState::Queued);
    let frozen: String = c
        .query_row(
            "SELECT snapshot FROM runs WHERE id=?1",
            [&accepted.id],
            |r| r.get(0),
        )
        .unwrap();
    let snapshot: serde_json::Value = serde_json::from_str(&frozen).unwrap();
    assert_eq!(snapshot["inputs"]["values"]["topic"], "literal $HOME");
    assert_eq!(snapshot["plans"][0]["configuration"]["revision"], 2);
    c.execute("DELETE FROM configurations", []).unwrap();
    assert_eq!(
        store
            .admit_run(
                request(),
                Admission {
                    image_id: "changed".into(),
                    blocker: None
                }
            )
            .unwrap()
            .id,
        accepted.id
    );
    let mut changed = request();
    changed.request_id = uuid::Uuid::new_v4().to_string();
    assert!(store
        .admit_run(
            changed,
            Admission {
                image_id: "other".into(),
                blocker: None
            }
        )
        .is_err());
    assert!(store
        .save_queue_settings(QueueSettings {
            active: 1,
            waiting: 0
        })
        .is_err());
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM runs", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM managed_sessions", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM run_resources", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        c.query_row("SELECT snapshot FROM runs", [], |r| r.get::<_, String>(0))
            .unwrap(),
        frozen
    );
    let stopped = store.stop_run(&accepted.id).unwrap();
    assert_eq!(stopped.state, RunState::Stopped);
    assert_eq!(
        c.query_row("SELECT phase FROM run_queue", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "settled"
    );
}
#[test]
fn d11_prepared_release_reserves_oldest_once_and_retains_holds() {
    for scenario in [
        "clean",
        "hold",
        "no-session",
        "persistence-failure",
        "wrong-generation",
    ] {
        let (_dir, store, c) = prepared();
        let owner = uuid::Uuid::new_v4().to_string();
        let oldest = uuid::Uuid::new_v4().to_string();
        let later = uuid::Uuid::new_v4().to_string();
        let slot = uuid::Uuid::new_v4().to_string();
        let mut s = status();
        s.id = owner.clone();
        s.request_id = uuid::Uuid::new_v4().to_string();
        seed_run(&c, &s, true);
        c.execute(
            "INSERT INTO run_queue(run_id,phase,slot,generation) VALUES(?1,'occupied',?2,7)",
            params![owner, slot],
        )
        .unwrap();
        c.execute("UPDATE queue_settings SET active=1", []).unwrap();
        c.execute(
            "INSERT INTO managed_sessions VALUES(?1,?2,?3,?4)",
            params![
                slot,
                if scenario == "hold" {
                    "leased"
                } else if scenario == "no-session" {
                    "quarantined"
                } else {
                    "available"
                },
                if scenario == "wrong-generation" { 9 } else { 8 },
                if scenario == "hold" {
                    Some(owner.clone())
                } else {
                    None
                }
            ],
        )
        .unwrap();
        for id in [&oldest, &later] {
            let mut wait = status();
            wait.id = id.clone();
            wait.request_id = uuid::Uuid::new_v4().to_string();
            wait.state = RunState::Queued;
            wait.final_response = None;
            seed_run(&c, &wait, true);
            c.execute(
                "INSERT INTO run_queue(run_id,phase) VALUES(?1,'waiting')",
                [id],
            )
            .unwrap();
        }
        if scenario == "persistence-failure" {
            c.execute_batch("CREATE TRIGGER refuse_release BEFORE UPDATE ON run_queue BEGIN SELECT RAISE(FAIL,'prepared refusal'); END;").unwrap();
        }
        let before: String = c
            .query_row("SELECT status FROM runs WHERE id=?1", [&owner], |r| {
                r.get(0)
            })
            .unwrap();
        let result = store.finish_run(
            &owner,
            None,
            "prepared completion",
            scenario != "hold",
            true,
        );
        assert_eq!(
            result.is_ok(),
            matches!(scenario, "clean" | "hold"),
            "{scenario}"
        );
        assert_eq!(
            store.run_status(&oldest).unwrap().state,
            if scenario == "clean" {
                RunState::AuthorizationRequired
            } else {
                RunState::Queued
            }
        );
        assert_eq!(store.run_status(&later).unwrap().state, RunState::Queued);
        if scenario == "clean" {
            assert!(store
                .finish_run(&owner, None, "duplicate", true, true)
                .is_err());
            let lease = store.execution_reservation(&oldest).unwrap().unwrap();
            assert_eq!(lease.owner, oldest);
            assert_eq!(lease.slot, slot);
            assert_eq!(lease.generation, 8);
            assert!(store.execution_reservation(&later).unwrap().is_none());
            assert_eq!(
                c.query_row(
                    "SELECT COUNT(*) FROM run_queue WHERE phase='occupied'",
                    [],
                    |r| r.get::<_, u32>(0)
                )
                .unwrap(),
                1
            );
        } else if scenario == "hold" {
            assert_eq!(store.run_status(&owner).unwrap().state, RunState::Held);
            assert!(store.run_status(&owner).unwrap().cleanup_pending);
        } else {
            assert_eq!(
                c.query_row("SELECT status FROM runs WHERE id=?1", [&owner], |r| r
                    .get::<_, String>(0))
                    .unwrap(),
                before
            );
        }
    }
}

#[test]
fn p2_prepared_successor_invokes_once_with_captured_prompt_and_same_resources() {
    use workflow_engine::{runs::BlockResult, sandbox::Cancel, sessions::Lease};
    for scenario in [
        "success",
        "success-per-block",
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
        let mut snapshot = serde_json::json!({"definition":{"definition":{"schemaVersion":1,"id":DEFINITION,"name":"Captured","blocks":[{"id":"b","name":"B","prompt":"B own $ 雪 / {{output.a.finalResponse}}","position":{"x":0,"y":0},"configurationId":CONFIG},{"id":"a","name":"A","prompt":"A own","position":{"x":0,"y":0},"configurationId":CONFIG}],"connections":[{"source":"a","target":"b"}],"inputs":[]},"revision":1},"configuration":config,"inputs":{"values":{},"prompts":{}},"imageId":"captured-image","plans":[{"block_id":"a","segments":[{"Literal":"A own"}],"configuration":config},{"block_id":"b","segments":[{"Literal":"B own $ 雪 / "},{"Output":"a"}],"configuration":config}]});
        snapshot["definition"]["definition"]["isolation"] =
            serde_json::json!(if scenario == "success-per-block" {
                "perBlock"
            } else {
                "perRun"
            });
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
            matches!(scenario, "success" | "success-per-block" | "recovery-won"),
            "{scenario}"
        );
        assert_eq!(
            calls,
            usize::from(matches!(
                scenario,
                "success"
                    | "success-per-block"
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
        if matches!(scenario, "success" | "success-per-block" | "recovery-won") {
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
#[test]
fn d12_scoped_cancel_transition() {
    use workflow_engine::{runs::BlockResult, sandbox::Cancel, sessions::Lease};
    for mode in ["perRun", "perBlock"] {
        for complete in [false, true] {
            let (_dir, store, c) = prepared();
            let mut selected = status();
            selected.blocks = vec![BlockResult {
                block_id: "a".into(),
                state: RunState::Succeeded,
                resolved_prompt: "Own prompt".into(),
                final_response: Some("Completed A".into()),
            }];
            selected.final_response = Some("Completed A".into());
            seed_run(&c, &selected, true);
            c.execute("UPDATE runs SET snapshot=?1", [serde_json::json!({"definition":{"definition":{"isolation":mode}},"plans":if complete {serde_json::json!([{}])}else{serde_json::json!([{},{}])}}).to_string()]).unwrap();
            let mut sibling = status();
            sibling.id = "sibling".into();
            sibling.request_id = "sibling-request".into();
            seed_run(&c, &sibling, true);
            let before: String = c
                .query_row("SELECT status FROM runs WHERE id='sibling'", [], |r| {
                    r.get(0)
                })
                .unwrap();
            let stopped = store.stop_run("owned").unwrap();
            assert_eq!(
                stopped.state,
                if complete {
                    RunState::Running
                } else {
                    RunState::Cancelling
                }
            );
            assert_eq!(stopped.final_response.as_deref(), Some("Completed A"));
            assert_eq!(store.stop_run("owned").unwrap().state, stopped.state);
            assert_eq!(
                c.query_row("SELECT status FROM runs WHERE id='sibling'", [], |r| r
                    .get::<_, String>(
                    0
                ))
                .unwrap(),
                before
            );
            if !complete {
                // Guard must reject before any external invocation, even with a fresh token.
                let lease = Lease {
                    slot: "slot".into(),
                    owner: "owned".into(),
                    generation: 7,
                };
                c.execute(
                    "INSERT INTO managed_sessions VALUES('slot','leased',7,'owned')",
                    [],
                )
                .unwrap();
                let refused = store.execute_next_block(
                    "owned",
                    &lease,
                    &(),
                    &Cancel::default(),
                    |_, _, _| panic!("successor forbidden"),
                );
                assert!(
                    matches!(refused, Err(workflow_engine::Error::Validation{message}) if message.contains("Run is stopped, held"))
                );
            }
        }
        for (state, clean, expected) in [
            (RunState::Held, true, RunState::Failed),
            (RunState::Cancelling, true, RunState::Stopped),
            (RunState::Cancelling, false, RunState::Cancelling),
        ] {
            let (_dir, store, c) = prepared();
            let mut selected = status();
            selected.state = state;
            seed_run(&c, &selected, true);
            c.execute(
                "INSERT INTO managed_sessions VALUES('slot',?1,?2,?3)",
                params![
                    if clean { "available" } else { "quarantined" },
                    if clean { 8 } else { 7 },
                    if clean { None } else { Some("owned") }
                ],
            )
            .unwrap();
            c.execute("INSERT INTO run_queue(run_id,phase,slot,generation) VALUES('owned','occupied','slot',7)",[]).unwrap();
            store
                .finish_run("owned", None, "Prepared receipt", clean, false)
                .unwrap();
            let result = store.run_status("owned").unwrap();
            assert_eq!(result.state, expected);
            assert_eq!(result.cleanup_pending, !clean);
            assert_eq!(result.final_response.as_deref(), Some("retained"));
            if clean {
                assert!(store
                    .finish_run("owned", Some("late overwrite".into()), "late", true, true)
                    .is_err());
                assert_eq!(store.stop_run("owned").unwrap().state, expected);
            }
        }
    }
}
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
    for _ in 0..3 {
        store.withhold_unfinished_runs().unwrap();
        let held = store.run_status("owned").unwrap();
        assert_eq!(held.state, RunState::Held);
        assert!(held.cleanup_pending);
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
    // Prepared durable startup predicates only: no admission, launch or workflow.
    for (phase, state, queue_only) in [
        (Some("waiting"), RunState::Queued, true),
        (Some("withheld"), RunState::Held, true),
        (Some("occupied"), RunState::Held, false),
        (Some("settled"), RunState::Held, false),
        (None, RunState::Running, false),
        (None, RunState::Queued, false),
        (Some("occupied"), RunState::Queued, false),
        (Some("waiting"), RunState::Running, false),
    ] {
        let (_dir, store, c) = prepared();
        let mut seeded = status();
        seeded.state = state;
        seed_run(&c, &seeded, true);
        if let Some(phase) = phase {
            c.execute(
                "INSERT INTO run_queue(run_id,phase) VALUES('owned',?1)",
                [phase],
            )
            .unwrap();
        }
        if phase == Some("occupied") {
            c.execute(
                "UPDATE run_queue SET slot='held-slot',generation=7 WHERE run_id='owned'",
                [],
            )
            .unwrap();
            c.execute(
                "INSERT INTO managed_sessions VALUES('held-slot','leased',7,'owned')",
                [],
            )
            .unwrap();
        }
        c.execute(
            "INSERT INTO managed_sessions VALUES('unused','available',11,NULL)",
            [],
        )
        .unwrap();
        for startup in 1..=3 {
            store.withhold_unfinished_runs().unwrap();
            let held = store.run_status("owned").unwrap();
            assert_eq!(held.state, RunState::Held);
            assert_eq!(
                held.cleanup_pending, !queue_only,
                "phase={phase:?}, startup={startup}"
            );
            assert_eq!(held.final_response.as_deref(), Some("retained"));
            assert_eq!(
                c.query_row("SELECT active FROM runs", [], |r| r.get::<_, u32>(0))
                    .unwrap(),
                1
            );
            assert_eq!(
                c.query_row(
                    "SELECT state||':'||generation FROM managed_sessions WHERE id='unused' AND owner IS NULL",
                    [],
                    |r| r.get::<_, String>(0)
                ).unwrap(),
                "available:11"
            );
            let durable: Option<String> = c
                .query_row(
                    "SELECT (SELECT phase FROM run_queue WHERE run_id='owned')",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(
                durable.as_deref(),
                match phase {
                    Some("waiting") => Some("withheld"),
                    other => other,
                }
            );
            assert_eq!(
                c.query_row("SELECT COUNT(*) FROM run_queue WHERE slot IS NOT NULL OR generation IS NOT NULL", [], |r| r.get::<_, u32>(0)).unwrap(),
                u32::from(phase == Some("occupied"))
            );
            if phase == Some("occupied") {
                assert_eq!(c.query_row("SELECT state||':'||generation||':'||owner FROM managed_sessions WHERE id='held-slot'", [], |r| r.get::<_, String>(0)).unwrap(), "quarantined:7:owned");
                assert_eq!(
                    c.query_row(
                        "SELECT slot||':'||generation FROM run_queue WHERE run_id='owned'",
                        [],
                        |r| r.get::<_, String>(0)
                    )
                    .unwrap(),
                    "held-slot:7"
                );
            }
        }
    }
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
    let mut notices = 0;
    let mut adapter =
        workflow_engine::runs::inspection::LiveJournal::new(root.path(), identity.clone()).unwrap();
    let first=b"{\"version\":1,\"type\":\"wf.diagnostic\",\"seq\":1,\"kind\":\"command\",\"bytes\":10,\"text\":\"WITHHELD\",\"incomplete\":false}\n";
    adapter
        .push(first, || {
            notices += 1;
            let durable = Journal::page(root.path(), id, None, 0).unwrap();
            assert_eq!(durable.high, 1);
            assert_eq!(durable.entries[0].text, "WITHHELD seq=1 bytes=10");
            assert_eq!(durable.entries[0].block, "a");
        })
        .unwrap();
    assert_eq!(notices, 1, "visible before executor EOF/return");
    for seq in 2..=4096 {
        let frame = serde_json::json!({"version":1,"type":"wf.diagnostic","seq":seq,"kind":"command","bytes":10,"text":"WITHHELD","incomplete":false});
        adapter
            .push(format!("{frame}\n").as_bytes(), || notices += 1)
            .unwrap();
    }
    adapter.push(b"{\"version\":1,\"type\":\"wf.diagnostic\",\"seq\":4097,\"kind\":\"stopped\",\"bytes\":10,\"text\":\"WITHHELD\",\"incomplete\":false}\n", || notices+=1).unwrap();
    adapter.finish().unwrap();
    assert_eq!(notices, 2, "same-byte updates coalesce, stopped publishes");
    assert!(adapter
        .push(first, || panic!("invalid frame notified"))
        .is_err());
    std::fs::remove_dir_all(root.path().join("inspection")).unwrap();
    let mut bucketed =
        workflow_engine::runs::inspection::LiveJournal::new(root.path(), identity.clone()).unwrap();
    let mut count = 0;
    for (seq, bytes, kind, incomplete) in [
        (1, 1, "command", false),
        (2, 65536, "stderr", false),
        (3, 65537, "error", false),
        (4, 65537, "limit", true),
        (5, 65537, "stopped", true),
    ] {
        let frame = serde_json::json!({"version":1,"type":"wf.diagnostic","seq":seq,"kind":kind,"bytes":bytes,"text":"WITHHELD","incomplete":incomplete});
        bucketed
            .push(format!("{frame}\n").as_bytes(), || count += 1)
            .unwrap();
    }
    assert_eq!(count, 4);
    let mut invalid =
        workflow_engine::runs::inspection::LiveJournal::new(root.path(), identity.clone()).unwrap();
    assert!(invalid
        .push(&vec![b'a'; 1_048_577], || panic!("limit notified"))
        .is_err());
    assert!(invalid.push(b"{\"version\":1,\"type\":\"wf.diagnostic\",\"seq\":1,\"kind\":\"command\",\"bytes\":0,\"text\":\"provider raw\",\"incomplete\":false}\n",||panic!("raw notified")).is_err());
    let mut wrong_identity = identity.clone();
    wrong_identity.block_id = "../bad".into();
    assert!(
        workflow_engine::runs::inspection::LiveJournal::new(root.path(), wrong_identity).is_err()
    );
    std::fs::remove_dir_all(root.path().join("inspection")).unwrap();
    let mut unavailable =
        workflow_engine::runs::inspection::LiveJournal::new(root.path(), identity.clone()).unwrap();
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(unavailable
        .push(first, || panic!("uncommitted metadata notified"))
        .is_err());
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
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
    assert_eq!(
        store.run_status("owned").unwrap().state,
        RunState::Cancelling
    );
    assert_eq!(s.state, RunState::Cancelling);
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
                RunState::Cancelling
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
    // Prepared trusted ancestor alias, as with macOS /var -> /private/var.
    // The private root itself and its database must still never be symlinks.
    let ancestor = tempfile::tempdir().unwrap();
    let real = ancestor.path().join("real");
    std::fs::create_dir(&real).unwrap();
    let alias = ancestor.path().join("alias");
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    let dir = tempfile::tempdir_in(&alias).unwrap();
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
    // Differential proof: legacy SQLite read-only accepts the trusted alias,
    // whereas SQLite's NOFOLLOW rejects even an ancestor link.
    let db_path = dir.path().join("definitions.sqlite3");
    let legacy =
        Connection::open_with_flags(&db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    assert_eq!(
        legacy
            .query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        4
    );
    let rejected = Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .unwrap_err();
    assert_eq!(
        rejected.sqlite_error().unwrap().extended_code,
        rusqlite::ffi::SQLITE_CANTOPEN_SYMLINK
    );
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
    let before_read = std::fs::read(dir.path().join("definitions.sqlite3")).unwrap();
    workflow_engine::runs::inspection::view::scoped(dir.path(), DEFINITION, id).unwrap();
    assert!(workflow_engine::runs::inspection::view::scoped(dir.path(), CONFIG, id).is_err());
    let reader = workflow_engine::runs::history::HistoryReader::open(dir.path()).unwrap();
    assert_eq!(
        reader.run_detail(DEFINITION, id).unwrap().id.as_str(),
        id.as_str()
    );
    assert_eq!(
        before_read,
        std::fs::read(dir.path().join("definitions.sqlite3")).unwrap()
    );
    let selected = reader.run_status(Some(id)).unwrap().unwrap();
    assert_eq!(selected.id, *id);
    assert_eq!(selected.request_id, "20000000-0000-0000-0000-000000000004");
    assert_eq!(reader.run_status(None).unwrap().unwrap().id, *id);
    assert!(reader.run_status(Some(CONFIG)).is_err());
    use std::os::unix::fs::PermissionsExt;
    let db_path = dir.path().join("definitions.sqlite3");
    std::fs::set_permissions(&db_path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(
        reader.run_detail(DEFINITION, id).is_err(),
        "permission drift must refuse"
    );
    std::fs::set_permissions(&db_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let saved = dir.path().join("original.sqlite3");
    std::fs::rename(&db_path, &saved).unwrap();
    std::os::unix::fs::symlink(&saved, &db_path).unwrap();
    assert!(workflow_engine::runs::history::HistoryReader::open(dir.path()).is_err());
    assert!(reader.run_detail(DEFINITION, id).is_err());
    assert!(store.list_run_history(DEFINITION, None, 2).is_err());
    std::fs::remove_file(&db_path).unwrap();
    std::fs::rename(&saved, &db_path).unwrap();
    let saved_root = alias.join("held-root");
    std::fs::rename(dir.path(), &saved_root).unwrap();
    std::os::unix::fs::symlink(&saved_root, dir.path()).unwrap();
    assert!(workflow_engine::runs::history::HistoryReader::open(dir.path()).is_err());
    assert!(reader.run_detail(DEFINITION, id).is_err());
    assert!(store.list_run_history(DEFINITION, None, 2).is_err());
    std::fs::remove_file(dir.path()).unwrap();
    std::fs::rename(&saved_root, dir.path()).unwrap();
    std::fs::rename(&db_path, &saved).unwrap();
    std::fs::copy(&saved, &db_path).unwrap();
    assert!(
        reader.run_detail(DEFINITION, id).is_err(),
        "replacement must not rebind reader"
    );
    std::fs::remove_file(&db_path).unwrap();
    std::fs::rename(&saved, &db_path).unwrap();
    let link = dir.path().join("linked.sqlite3");
    std::fs::hard_link(&db_path, &link).unwrap();
    assert!(reader.workflow(id).is_err());
    assert!(store.list_run_history(DEFINITION, None, 2).is_err());
    std::fs::remove_file(link).unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(reader.list_run_history(DEFINITION, None, 2).is_err());
    assert!(store.run_detail(DEFINITION, id).is_err());
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(reader.run_detail(CONFIG, id).is_err());
    assert!(reader.list_run_history(DEFINITION, None, 51).is_err());
    assert_eq!(
        before_read,
        std::fs::read(&db_path).unwrap(),
        "read/refusal paths never change DB bytes"
    );
    assert_eq!(
        c.query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        4
    );
    let missing = tempfile::tempdir().unwrap();
    assert!(workflow_engine::runs::history::HistoryReader::open(missing.path()).is_err());
    assert!(!missing.path().join("definitions.sqlite3").exists());
    let unsupported = tempfile::tempdir().unwrap();
    let unsupported_path = unsupported.path().join("definitions.sqlite3");
    let unsupported_c = Connection::open(&unsupported_path).unwrap();
    unsupported_c
        .execute_batch("PRAGMA user_version=99")
        .unwrap();
    std::fs::set_permissions(&unsupported_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let unsupported_before = std::fs::read(&unsupported_path).unwrap();
    assert!(workflow_engine::runs::history::HistoryReader::open(unsupported.path()).is_err());
    assert_eq!(
        unsupported_before,
        std::fs::read(&unsupported_path).unwrap()
    );
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
        c.execute_batch("CREATE TRIGGER forbid_status_write BEFORE UPDATE ON runs BEGIN SELECT RAISE(FAIL,'unrelated pending output write failure'); END;").unwrap();
        let reader = workflow_engine::runs::history::HistoryReader::open(dir.path()).unwrap();
        let before_status = std::fs::read(&db_path).unwrap();
        assert_eq!(reader.run_status(Some(id)).unwrap().unwrap().id, *id);
        assert!(reader.run_status(None).is_err());
        assert_eq!(before_status, std::fs::read(&db_path).unwrap());
        c.execute_batch("DROP TRIGGER forbid_status_write").unwrap();
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
    let before_read = std::fs::read(dir.path().join("definitions.sqlite3")).unwrap();
    workflow_engine::runs::inspection::view::scoped(dir.path(), DEFINITION, id).unwrap();
    assert!(workflow_engine::runs::inspection::view::scoped(dir.path(), CONFIG, id).is_err());
    let reader = workflow_engine::runs::history::HistoryReader::open(dir.path()).unwrap();
    assert_eq!(
        reader.run_detail(DEFINITION, id).unwrap().id.as_str(),
        id.as_str()
    );
    assert_eq!(
        before_read,
        std::fs::read(dir.path().join("definitions.sqlite3")).unwrap()
    );
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
