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

#[test]
fn c14_inspection_accepts_inert_static_endpoints_but_requires_applied_ids_after_start() {
    use serde_json::{json, Value};
    struct Inspect {
        r: Resources,
        applied: bool,
    }
    impl Pipe for Inspect {
        fn call(
            &mut self,
            args: &[String],
            _: &[u8],
            _: usize,
            _: &Cancel,
        ) -> Result<(i32, Vec<u8>, Vec<u8>), Error> {
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
                json!({"Id":r.container,"Image":r.image,"Config":{"Labels":label,"User":"1000:1000"},"HostConfig":host(536870912,128),"Mounts":[],"NetworkSettings":{"Networks":{private:endpoint(&r.network)}}})
            };
            Ok((0, serde_json::to_vec(&vec![value]).unwrap(), vec![]))
        }
    }
    let r = Resources {
        owner: "00000000-0000-0000-0000-000000000003".into(),
        image: format!("sha256:{}", "f".repeat(64)),
        network: "a".repeat(64),
        outer: "b".repeat(64),
        helper: "c".repeat(64),
        container: "d".repeat(64),
    };
    assert!(Sandbox {
        pipe: Inspect {
            r: r.clone(),
            applied: false
        }
    }
    .inspect_phase(&r, false)
    .is_ok());
    assert!(Sandbox {
        pipe: Inspect {
            r: r.clone(),
            applied: false
        }
    }
    .inspect_work(&r)
    .is_err());
    assert!(Sandbox {
        pipe: Inspect {
            r: r.clone(),
            applied: true
        }
    }
    .inspect_work(&r)
    .is_ok());
}
