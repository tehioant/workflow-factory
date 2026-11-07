use workflow_engine::runtime::*;
#[test]
fn unsupported_daemon_is_refused() {
    let docker = Docker::new("unix:///var/run/docker.sock", Platform::Linux).unwrap();
    let mut process = Injected(Ok(ProcessOutput {
        code: 0,
        stdout: br#"{"OSType":"linux","ServerVersion":"27.5.1","SecurityOptions":[]}"#.to_vec(),
        stderr: vec![],
    }));
    assert_eq!(
        docker.readiness(&mut process, None).status,
        RuntimeStatus::Unsupported
    );
}
fn output(value: serde_json::Value) -> ProcessOutput {
    ProcessOutput {
        code: 0,
        stdout: serde_json::to_vec(&value).unwrap(),
        stderr: vec![],
    }
}
struct Replies(std::collections::VecDeque<ProcessOutput>);
impl Process for Replies {
    fn run(&mut self, _: &CommandSpec) -> Result<ProcessOutput, ProcessError> {
        Ok(self.0.pop_front().expect("unexpected probe"))
    }
}
fn daemon() -> ProcessOutput {
    output(
        serde_json::json!({"OSType":"linux","ServerVersion":"28.0.1","SecurityOptions":["name=seccomp,profile=builtin"]}),
    )
}
#[test]
fn image_identity_drift_is_refused() {
    let docker = Docker::new("unix:///var/run/docker.sock", Platform::Linux).unwrap();
    let image = output(
        serde_json::json!([{"Id":format!("sha256:{}", "a".repeat(64)),"Config":{"Labels":{"org.workflow-factory.runtime":"1","org.workflow-factory.codex":"0.166.0"}}}]),
    );
    let mut process = Replies([daemon(), image].into());
    assert_eq!(
        docker
            .readiness(&mut process, Some(&format!("sha256:{}", "b".repeat(64))))
            .status,
        RuntimeStatus::ImageUnsupported
    );
}
#[test]
fn available_prerequisites_do_not_authorize_execution() {
    let docker = Docker::new("unix:///var/run/docker.sock", Platform::Linux).unwrap();
    let id = format!("sha256:{}", "a".repeat(64));
    let image = output(
        serde_json::json!([{"Id":id,"Config":{"Labels":{"org.workflow-factory.runtime":"1","org.workflow-factory.codex":"0.166.0"}}}]),
    );
    let result = docker.readiness(&mut Replies([daemon(), image].into()), None);
    assert_eq!(result.status, RuntimeStatus::AuthUnsupported);
    assert_eq!(result.isolation, IsolationEvidence::ExpectedNotMeasured);
    assert_eq!(result.image_id.as_deref(), Some(id.as_str()));
    assert!(!result.execution_ready);
}
#[test]
fn ordinary_bridge_cannot_be_isolation_evidence() {
    let id = "a".repeat(64);
    let network = serde_json::json!({"Id":id,"Driver":"bridge","Internal":true,"EnableIPv6":false,"Options":{}});
    let docker = Docker::new("unix:///var/run/docker.sock", Platform::Linux).unwrap();
    let mut process = Injected(Ok(output(serde_json::json!([network]))));
    assert_eq!(
        docker.inspect_private_network(&mut process, &id),
        Err(RuntimeStatus::NetworkUnavailable)
    );
}
#[test]
fn setup_is_explicit_build_argv() {
    let docker = Docker::new("unix:///var/run/docker.sock", Platform::Linux).unwrap();
    let plan = docker.setup_plan();
    assert_eq!(plan.program, "/usr/bin/docker");
    assert_eq!(
        plan.argv,
        [
            "--host",
            "unix:///var/run/docker.sock",
            "build",
            "--file",
            "runtime/Dockerfile",
            "--tag",
            IMAGE_TAG,
            "."
        ]
    );
    assert!(plan.instructions.contains("reviewed"));
}
struct Injected(Result<ProcessOutput, ProcessError>);
impl Process for Injected {
    fn run(&mut self, _: &CommandSpec) -> Result<ProcessOutput, ProcessError> {
        self.0.clone()
    }
}
#[test]
fn missing_cli_is_actionable() {
    let docker = Docker::new("unix:///var/run/docker.sock", Platform::Linux).unwrap();
    let result = docker.readiness(&mut Injected(Err(ProcessError::Missing)), None);
    assert_eq!(result.status, RuntimeStatus::Missing);
    assert!(result.message.contains("Install"));
}

#[test]
fn local_endpoint_becomes_explicit_argv() {
    let docker = Docker::new("unix:///var/run/docker.sock", Platform::Linux).unwrap();
    let command = docker.info_command();
    let child = command.process_command();
    assert_eq!(child.get_program(), "/usr/bin/docker");
    assert_eq!(child.get_envs().filter(|(_, v)| v.is_some()).count(), 3);
    assert!(child
        .get_envs()
        .any(|(k, v)| k == "HOME" && v == Some(std::ffi::OsStr::new("/var/empty"))));
    assert_eq!(command.program(), "/usr/bin/docker");
    assert_eq!(
        command.argv(),
        [
            "--host",
            "unix:///var/run/docker.sock",
            "info",
            "--format",
            "{{json .}}"
        ]
    );
}
