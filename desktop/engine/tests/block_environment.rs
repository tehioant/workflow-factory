use serde_json::{json, Value};
use std::{cell::RefCell, rc::Rc};
use workflow_engine::{
    sandbox::{Cancel, Pipe, Resources, Sandbox},
    sessions::{Lease, SecureStore},
    Error, Store,
};
const OWNER: &str = "00000000-0000-0000-0000-000000000003";
const SLOT: &str = "00000000-0000-0000-0000-000000000004";
const CACHE:&[u8]=br#"{"auth_mode":"chatgpt","tokens":{"login_owner_id":"synthetic","access_token":"synthetic-a","refresh_token":"synthetic-r","id_token":"a.b.c","account_id":"synthetic-account","oauth_provenance":{"issuer":"https://auth.openai.com","client_id":"synthetic-client"}}}"#;
fn fail() -> Error {
    Error::Validation {
        message: "prepared boundary refusal".into(),
    }
}
type Events = Rc<RefCell<Vec<String>>>;
struct Secure {
    cache: Vec<u8>,
    events: Events,
    db: std::path::PathBuf,
    scenario: &'static str,
    written: bool,
}
impl SecureStore for Secure {
    fn read(&mut self, _: &str) -> Result<Vec<u8>, Error> {
        self.events.borrow_mut().push("secure-read".into());
        if self.written && self.scenario == "readback" {
            return Ok(vec![]);
        }
        Ok(self.cache.clone())
    }
    fn write(&mut self, _: &str, cache: &[u8]) -> Result<(), Error> {
        let c = rusqlite::Connection::open(&self.db).unwrap();
        let state: String = c
            .query_row("SELECT state FROM managed_sessions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(state, "quarantined");
        self.events.borrow_mut().push("secure-write".into());
        if self.scenario == "write" {
            return Err(fail());
        }
        self.cache = cache.into();
        self.written = true;
        Ok(())
    }
}
struct Prepared {
    r: Resources,
    events: Events,
    created: bool,
    started: bool,
    removed: bool,
    scenario: &'static str,
    cancel: Cancel,
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
            members.insert(r.helper.clone(), json!({}));
            if id == r.network {
                if !self.removed {
                    members.insert(r.container.clone(), json!({}));
                }
                if self.started {
                    members.insert(next.clone(), json!({}));
                }
            }
            return json!({"Id":id,"Labels":label,"Driver":"bridge","Internal":id==r.network,"EnableIPv6":false,"Options":{"com.docker.network.bridge.gateway_mode_ipv4":"isolated"},"Containers":members});
        }
        let helper = id == r.helper;
        let mut tmpfs = json!({"/tmp":"rw,noexec,nosuid,nodev,uid=1000,mode=700"});
        if !helper {
            tmpfs["/run/wf-codex"] = json!("rw,noexec,nosuid,nodev,uid=1000,mode=700");
        }
        let mount = json!({"Type":"volume","Name":r.workspace,"Driver":"local","Destination":if self.scenario=="mount"{"/other"}else{"/workspace"},"RW":true});
        let declared = json!({"Type":"volume","Source":r.workspace,"Target":"/workspace","ReadOnly":false,"VolumeOptions":{"NoCopy":true}});
        let memory = if helper { 134217728u64 } else { 536870912 };
        json!({"Id":id,"Image":r.image,"State":{"Running":id!=next||self.started},"Config":{"Labels":label,"User":"1000:1000","Entrypoint":["node"],"Cmd":["/opt/workflow-factory/keeper.mjs"]},"HostConfig":{"ReadonlyRootfs":true,"Privileged":false,"Init":true,"Ulimits":[{"Name":"core","Soft":0,"Hard":0}],"Memory":memory,"MemorySwap":memory,"PidsLimit":if helper{32}else{128},"NanoCpus":1000000000u64,"LogConfig":{"Type":"none"},"CapDrop":["ALL"],"SecurityOpt":["no-new-privileges"],"Tmpfs":tmpfs,"Mounts":if helper{json!([])}else{json!([declared])}},"Mounts":if helper {if self.scenario=="helper" {json!([mount])}else{json!([])}}else{json!([mount])},"NetworkSettings":{"Networks":if helper{json!({private:{"NetworkID":r.network},outer:{"NetworkID":r.outer}})}else{json!({private:{"NetworkID":r.network}})}}})
    }
}
impl Pipe for Prepared {
    fn call(
        &mut self,
        args: &[String],
        input: &[u8],
        _: usize,
        _: &Cancel,
    ) -> Result<(i32, Vec<u8>, Vec<u8>), Error> {
        let mut out = vec![];
        if args.get(1).map(String::as_str) == Some("inspect") {
            self.events
                .borrow_mut()
                .push(format!("inspect:{}", args[2]));
            if self.scenario == "inspect" && self.created {
                return Err(fail());
            }
            out = serde_json::to_vec(&vec![self.inspection(&args[0], &args[2])]).unwrap();
        } else if args[0] == "create" {
            assert!(self.events.borrow().iter().any(|s| s == "secure-write"));
            assert!(!self.created);
            assert!(!self.removed);
            assert_eq!(
                &args[args.len() - 4..],
                [
                    "--entrypoint",
                    "node",
                    &self.r.image,
                    "/opt/workflow-factory/keeper.mjs"
                ]
            );
            assert!(args.windows(2).any(|w| w
                == [
                    "--mount",
                    &format!(
                        "type=volume,src={},dst=/workspace,volume-nocopy",
                        self.r.workspace.as_deref().unwrap()
                    )
                ]));
            assert!(!args.iter().any(|s| s.contains("volumes-from")));
            self.created = true;
            self.events.borrow_mut().push("create".into());
            out = "e".repeat(64).into_bytes();
        } else if args[0] == "start" {
            assert!(self.created && !self.removed);
            assert_eq!(args[1], "e".repeat(64));
            self.started = self.scenario != "continuity";
            self.events.borrow_mut().push("start".into());
        } else if args[0] == "rm" {
            assert!(self.started);
            assert_eq!(args[2], self.r.container);
            assert!(self
                .events
                .borrow()
                .iter()
                .any(|s| s == "receipt:agent-overlap-1"));
            self.events.borrow_mut().push("remove".into());
            if self.scenario == "remove" {
                return Err(fail());
            }
            self.removed = true;
        } else if args[0] == "container" && args[1] == "ls" {
            self.events.borrow_mut().push("absence".into());
            if self.scenario == "list" {
                return Err(fail());
            }
            if !self.removed || self.scenario == "present" {
                out = self.r.container.as_bytes().to_vec();
            }
        } else if args[0] == "exec" {
            if args.last().map(String::as_str) == Some("stage") {
                assert!(self.removed);
                assert!(self
                    .events
                    .borrow()
                    .iter()
                    .any(|s| s == "receipt:agent-remove-0"));
                assert!(self.events.borrow().iter().any(|s| s == "qualify"));
                assert_eq!(input, CACHE);
                self.events.borrow_mut().push("stage".into());
            } else if args.last().is_some_and(|s| s.contains("memory.swap.max")) {
                out = b"0\n".to_vec();
            } else if args.last().is_some_and(|s| s.contains("swaps:fs")) {
                out = serde_json::to_vec(
                    &json!({"swaps":"Filename Type Size Used Priority\n","core":"core"}),
                )
                .unwrap();
            } else {
                assert!(args.last().unwrap().contains("readdirSync"));
                if self.scenario == "credentials" {
                    return Err(fail());
                }
            }
        } else {
            panic!("unexpected operation {args:?}")
        }
        if self.scenario == "cancel" && self.started {
            self.cancel.request();
        }
        Ok((0, out, vec![]))
    }
}
#[test]
fn f1_prepared_environment_switch_checkpoints_same_lease_before_verified_handoff() {
    for scenario in [
        "success",
        "per-run",
        "write",
        "readback",
        "owner",
        "disk",
        "mount",
        "helper",
        "inspect",
        "continuity",
        "credentials",
        "remove",
        "list",
        "present",
        "receipt",
        "cancel",
        "qualify",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        let db = dir.path().join("definitions.sqlite3");
        let c = rusqlite::Connection::open(&db).unwrap();
        c.execute(
            "INSERT INTO managed_sessions VALUES(?1,'leased',7,?2)",
            rusqlite::params![SLOT, OWNER],
        )
        .unwrap();
        let lease = Lease {
            slot: SLOT.into(),
            owner: OWNER.into(),
            generation: 7,
        };
        let events = Events::default();
        let cancel = Cancel::default();
        let r = Resources {
            owner: OWNER.into(),
            image: format!("sha256:{}", "f".repeat(64)),
            network: "a".repeat(64),
            outer: "b".repeat(64),
            helper: "c".repeat(64),
            container: "d".repeat(64),
            workspace: if scenario == "per-run" {
                None
            } else {
                Some(format!("wf-workspace-{OWNER}"))
            },
            peer: None,
        };
        let mut sandbox = Sandbox {
            pipe: Prepared {
                r: r.clone(),
                events: events.clone(),
                created: false,
                started: false,
                removed: false,
                scenario,
                cancel: cancel.clone(),
            },
        };
        let mut secure = Secure {
            cache: CACHE.into(),
            events: events.clone(),
            db,
            scenario,
            written: false,
        };
        let result = sandbox.switch_block_environment(
            &r,
            1,
            &cancel,
            || store.checkpoint_session(&lease, CACHE, &mut secure, &cancel),
            |kind, id| {
                events.borrow_mut().push(format!(
                    "{}:{kind}",
                    if id.is_some() { "receipt" } else { "intent" }
                ));
                if scenario == "receipt" && kind == "agent-remove-0" && id.is_some() {
                    return Err(fail());
                }
                store.resource_intent(OWNER, kind, id)
            },
            || {
                events.borrow_mut().push("qualify".into());
                if scenario == "qualify" {
                    Err(fail())
                } else {
                    Ok(())
                }
            },
        );
        let success = matches!(scenario, "success" | "per-run");
        assert_eq!(result.is_ok(), success, "{scenario}: {:?}", events.borrow());
        assert_eq!(
            events
                .borrow()
                .iter()
                .filter(|s| s.as_str() == "stage")
                .count(),
            usize::from(scenario == "success"),
            "{scenario}"
        );
        if let Ok(next) = result {
            assert_eq!(
                next.container,
                if scenario == "per-run" {
                    r.container.clone()
                } else {
                    "e".repeat(64)
                }
            );
            assert_eq!(next.workspace, r.workspace);
            assert_eq!(next.helper, r.helper);
            assert!(next.peer.is_none());
        }
        let state: (String, String, u32) = c
            .query_row(
                "SELECT state,owner,generation FROM managed_sessions",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!((state.1, state.2), (OWNER.into(), 7));
        assert_eq!(
            state.0,
            if matches!(scenario, "write" | "readback") {
                "quarantined"
            } else {
                "leased"
            }
        );
        if scenario == "per-run" {
            assert!(events.borrow().is_empty());
        }
    }
}
