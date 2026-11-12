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
