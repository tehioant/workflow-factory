// Prepared single cleanup operation. Serializer fixture matches block_environment.rs.
use serde_json::{json, Value};
use std::{cell::RefCell, collections::BTreeSet, rc::Rc};
use workflow_engine::{
    sandbox::{Cancel, CleanupContext, Pipe, Resources, Sandbox},
    sessions::{Lease, SecureStore},
    Error, Store,
};
const OWNER: &str = "00000000-0000-0000-0000-000000000003";
const CACHE:&[u8]=br#"{"auth_mode":"chatgpt","tokens":{"login_owner_id":"synthetic","access_token":"synthetic-current-a","refresh_token":"synthetic-current-r","id_token":"a.b.c","account_id":"synthetic-account","oauth_provenance":{"issuer":"https://auth.openai.com","client_id":"synthetic-client"}}}"#;
fn fail() -> Error {
    Error::Validation {
        message: "synthetic cleanup refusal".into(),
    }
}
struct Prepared {
    r: Resources,
    present: BTreeSet<String>,
    scenario: &'static str,
    events: Rc<RefCell<Vec<String>>>,
}
impl Prepared {
    fn inspection(&self, kind: &str, id: &str) -> Value {
        let r = &self.r;
        let next = "e".repeat(64);
        let label = json!({"org.workflow-factory.owner":if self.scenario=="owner"{"another-run"}else{OWNER}});
        if kind == "volume" {
            return json!({"Name":r.workspace,"Driver":if self.scenario=="disk"{"overlay2"}else{"local"},"Scope":"local","Labels":label,"Options":{"type":"tmpfs","device":"tmpfs","o":"rw,exec,nosuid,nodev,size=256m,uid=1000,gid=1000,mode=700"}});
        }
        let private = format!("wf-private-{OWNER}");
        let outer = format!("wf-outer-{OWNER}");
        if kind == "network" {
            let mut members = serde_json::Map::new();
            if self.present.contains(&r.helper) {
                members.insert(r.helper.clone(), json!({}));
            }
            if id == r.network {
                if self.present.contains(&r.container) {
                    members.insert(r.container.clone(), json!({}));
                }
                if false {
                    members.insert(next.clone(), json!({}));
                }
            }
            return json!({"Id":id,"Labels":label,"Driver":"bridge","Internal":id==r.network,"EnableIPv6":false,"Options":{"com.docker.network.bridge.gateway_mode_ipv4":"isolated"},"Containers":members});
        }
        let helper = id == r.helper;
        let mut tmpfs = json!({"/tmp":"rw,noexec,nosuid,nodev,uid=1000,mode=700"});
        if !helper {
            if r.workspace.is_none() {
                tmpfs["/workspace"] = json!("rw,exec,nosuid,nodev,uid=1000,mode=700");
            }
            tmpfs["/run/wf-codex"] = json!("rw,noexec,nosuid,nodev,uid=1000,mode=700");
        }
        let mount = json!({"Type":"volume","Name":r.workspace,"Driver":"local","Destination":if self.scenario=="mount"{"/other"}else{"/workspace"},"RW":true});
        let mut declared = json!({"Type":"volume","Source":r.workspace,"Target":"/workspace","VolumeOptions":{"NoCopy":true}});
        match self.scenario {
            "explicit-false" => declared["ReadOnly"] = json!(false),
            "readonly" => declared["ReadOnly"] = json!(true),
            "readonly-null" => declared["ReadOnly"] = Value::Null,
            "readonly-string" => declared["ReadOnly"] = json!("false"),
            "readonly-number" => declared["ReadOnly"] = json!(0),
            "readonly-array" => declared["ReadOnly"] = json!([]),
            "readonly-object" => declared["ReadOnly"] = json!({}),
            _ => {}
        }
        let memory = if helper { 134217728u64 } else { 536870912 };
        json!({"Id":id,"Image":r.image,"State":{"Running":id!=next||false,"StartedAt":if self.scenario=="restart"&&false{"changed-epoch"}else{"prepared-start-epoch"}},"RestartCount":0,"Config":{"Labels":label,"User":"1000:1000","Entrypoint":["node"],"Cmd":["/opt/workflow-factory/keeper.mjs"]},"HostConfig":{"ReadonlyRootfs":true,"Privileged":false,"Init":true,"Ulimits":[{"Name":"core","Soft":0,"Hard":0}],"Memory":memory,"MemorySwap":memory,"PidsLimit":if helper{32}else{128},"NanoCpus":1000000000u64,"LogConfig":{"Type":"none"},"CapDrop":["ALL"],"SecurityOpt":["no-new-privileges"],"Tmpfs":tmpfs,"Mounts":if helper{json!([])}else{if r.workspace.is_some(){json!([declared])}else{json!([])}}},"Mounts":if helper {if self.scenario=="helper" {json!([mount])}else{json!([])}}else{if r.workspace.is_some(){json!([mount])}else{json!([])}},"NetworkSettings":{"Networks":if helper{json!({private:{"NetworkID":r.network},outer:{"NetworkID":r.outer}})}else{json!({private:{"NetworkID":r.network}})}}})
    }
}
impl Pipe for Prepared {
    fn call(
        &mut self,
        args: &[String],
        input: &[u8],
        limit: usize,
        cancel: &Cancel,
    ) -> Result<(i32, Vec<u8>, Vec<u8>), Error> {
        cancel.check()?;
        assert!(input.is_empty());
        let a: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = match a.as_slice() {
            [kind, "inspect", id] if ["container", "network", "volume"].contains(kind) => {
                assert_eq!(limit, 131072);
                assert!(
                    [
                        self.r.container.as_str(),
                        self.r.helper.as_str(),
                        self.r.network.as_str(),
                        self.r.outer.as_str()
                    ]
                    .contains(id)
                        || self.r.workspace.as_deref() == Some(id)
                );
                if self.scenario == "inspect" {
                    return Err(fail());
                }
                serde_json::to_vec(&vec![self.inspection(kind, id)]).unwrap()
            }
            ["container", "ls", "-a", "--no-trunc", "--filter", filter, "--format", "{{.ID}}"] => {
                assert_eq!(limit, 131072);
                if self.scenario == "list" {
                    return Err(fail());
                }
                if let Some(id) = filter.strip_prefix("id=") {
                    if self.present.contains(id) {
                        id.as_bytes().to_vec()
                    } else {
                        vec![]
                    }
                } else if let Some(name) = filter.strip_prefix("volume=") {
                    assert_eq!(Some(name), self.r.workspace.as_deref());
                    vec![]
                } else {
                    panic!("unknown filter")
                }
            }
            ["network", "ls", "--no-trunc", "--filter", filter, "--format", "{{.ID}}"] => {
                assert_eq!(limit, 131072);
                let id = filter.strip_prefix("id=").unwrap();
                if self.present.contains(id) {
                    id.as_bytes().to_vec()
                } else {
                    vec![]
                }
            }
            ["volume", "ls", "--filter", filter, "--format", "{{.Name}}"] => {
                assert_eq!(limit, 131072);
                let id = filter
                    .strip_prefix("name=^")
                    .unwrap()
                    .strip_suffix('$')
                    .unwrap();
                if self.present.contains(id) {
                    id.as_bytes().to_vec()
                } else {
                    vec![]
                }
            }
            ["exec", id, "node", "/opt/workflow-factory/keeper.mjs", op]
                if ["stop-writers", "export"].contains(op) =>
            {
                assert_eq!(*id, self.r.container);
                self.events.borrow_mut().push(op.to_string());
                if self.scenario == "writer" {
                    return Err(fail());
                }
                assert_eq!(limit, if *op == "export" { 65536 } else { 131072 });
                if *op == "export" {
                    CACHE.to_vec()
                } else {
                    vec![]
                }
            }
            ["rm", "-f", id] | ["network", "rm", id] | ["volume", "rm", id] => {
                assert_eq!(limit, 131072);
                assert!(self.events.borrow().iter().any(|s| s == "readback"));
                self.events.borrow_mut().push(format!("rm:{id}"));
                assert!(self.present.remove(*id));
                if self.scenario == "lost-ack" && *id == self.r.container {
                    self.scenario = "success";
                    return Err(fail());
                }
                vec![]
            }
            _ => panic!("unknown cleanup argv {args:?}"),
        };
        Ok((0, out, vec![]))
    }
}
struct Secure {
    bytes: Vec<u8>,
    fail: bool,
    events: Rc<RefCell<Vec<String>>>,
    written: bool,
}
impl SecureStore for Secure {
    fn read(&mut self, slot: &str) -> Result<Vec<u8>, Error> {
        assert_eq!(slot, "slot");
        if self.written {
            self.events.borrow_mut().push("readback".into());
        }
        Ok(self.bytes.clone())
    }
    fn write(&mut self, slot: &str, bytes: &[u8]) -> Result<(), Error> {
        assert_eq!(slot, "slot");
        assert_eq!(bytes, CACHE);
        self.events.borrow_mut().push("write".into());
        if self.fail {
            return Err(fail());
        }
        self.bytes = bytes.into();
        self.written = true;
        Ok(())
    }
}
pub fn check() {
    for block in [false, true] {
        for scenario in [
            "success",
            "write",
            "lost-ack",
            "owner",
            "writer",
            "list",
            "inspect",
            "stale",
            "journal",
            "unknown-intent",
            "epoch",
            "foreign-id",
            "missing-proof",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let store = Store::open(dir.path()).unwrap();
            let c = rusqlite::Connection::open(dir.path().join("definitions.sqlite3")).unwrap();
            let status = json!({"id":OWNER,"requestId":"request","state":"held","cleanupPending":true,"diagnostic":"held","finalResponse":"retained","blocks":[]});
            c.execute("INSERT INTO runs(id,request_id,request,snapshot,status,active) VALUES(?1,'request','{}','{}',?2,1)",rusqlite::params![OWNER,status.to_string()]).unwrap();
            c.execute(
                "INSERT INTO managed_sessions VALUES('slot','quarantined',7,?1)",
                [OWNER],
            )
            .unwrap();
            c.execute("INSERT INTO run_queue(run_id,phase,slot,generation) VALUES(?1,'occupied','slot',7)",[OWNER]).unwrap();
            let lease = Lease {
                slot: "slot".into(),
                owner: OWNER.into(),
                generation: 7,
            };
            let r = Resources {
                owner: OWNER.into(),
                image: format!("sha256:{}", "f".repeat(64)),
                network: "a".repeat(64),
                outer: "b".repeat(64),
                helper: "c".repeat(64),
                container: "d".repeat(64),
                workspace: block.then(|| format!("wf-workspace-{OWNER}")),
                peer: None,
                started_at: Some("prepared-start-epoch".into()),
                retired: None,
            };
            for (kind, id) in [
                (
                    if block {
                        "work-container-0"
                    } else {
                        "work-container"
                    },
                    Some(&r.container),
                ),
                ("egress-helper", Some(&r.helper)),
                ("private-network", Some(&r.network)),
                ("egress-network", Some(&r.outer)),
                ("workspace-volume", r.workspace.as_ref()),
            ] {
                if let Some(id) = id {
                    store.resource_intent(OWNER, kind, None).unwrap();
                    store.resource_intent(OWNER, kind, Some(id)).unwrap();
                }
            }
            if scenario == "journal" {
                store
                    .resource_intent(OWNER, "unknown", Some("foreign"))
                    .ok();
                c.execute(
                    "INSERT INTO run_resources VALUES(?1,'unknown','foreign')",
                    [OWNER],
                )
                .unwrap();
            }
            if scenario == "unknown-intent" {
                store.resource_intent(OWNER, "agent-start-1", None).unwrap();
            }
            if scenario == "foreign-id" {
                c.execute(
                    "UPDATE run_resources SET resource_id=?1 WHERE kind='egress-helper'",
                    ["e".repeat(64)],
                )
                .unwrap();
            }
            if scenario == "stale" {
                c.execute("UPDATE managed_sessions SET generation=8", [])
                    .unwrap();
            }
            let events = Rc::new(RefCell::new(vec![]));
            let present = [
                r.container.clone(),
                r.helper.clone(),
                r.network.clone(),
                r.outer.clone(),
            ]
            .into_iter()
            .chain(r.workspace.clone())
            .collect();
            let mut sandbox = Sandbox {
                pipe: Prepared {
                    r: r.clone(),
                    present,
                    scenario,
                    events: events.clone(),
                },
            };
            let mut context = CleanupContext::new(
                if scenario == "epoch" {
                    Resources {
                        started_at: Some("changed-epoch".into()),
                        ..r.clone()
                    }
                } else {
                    r.clone()
                },
                lease.clone(),
            );
            if scenario == "missing-proof" {
                sandbox.pipe.present.remove(&r.container);
            }
            let mut secure = Secure {
                bytes: {
                    let mut seed: serde_json::Value = serde_json::from_slice(CACHE).unwrap();
                    seed["tokens"]["refresh_token"] = json!("synthetic-seed-r");
                    serde_json::to_vec(&seed).unwrap()
                },
                fail: scenario == "write",
                events: events.clone(),
                written: false,
            };
            let result = sandbox.finish_cleanup(&store, &lease, &mut context, &mut secure, false);
            if scenario == "success" {
                assert!(result.is_ok());
                assert!(sandbox.pipe.present.is_empty());
                let exports = events.borrow().iter().filter(|s| *s == "export").count();
                assert!(sandbox
                    .finish_cleanup(&store, &lease, &mut context, &mut secure, false)
                    .is_ok());
                assert_eq!(
                    events.borrow().iter().filter(|s| *s == "export").count(),
                    exports
                );
            } else {
                assert!(result.is_err(), "{scenario}");
                if scenario == "lost-ack" {
                    assert!(!sandbox.pipe.present.contains(&r.container));
                    assert!(sandbox
                        .finish_cleanup(&store, &lease, &mut context, &mut secure, false)
                        .is_ok());
                    assert_eq!(events.borrow().iter().filter(|s| *s == "export").count(), 1);
                    assert!(sandbox.pipe.present.is_empty());
                } else if scenario != "missing-proof" {
                    assert!(sandbox.pipe.present.contains(&r.container));
                }
            }
            if scenario == "write" {
                secure.fail = false;
                assert!(sandbox
                    .finish_cleanup(&store, &lease, &mut context, &mut secure, false)
                    .is_ok());
            }
            assert_eq!(
                c.query_row(
                    "SELECT state||':'||generation FROM managed_sessions",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
                if scenario == "stale" {
                    "quarantined:8"
                } else {
                    "quarantined:7"
                }
            );
        }
    }
}
