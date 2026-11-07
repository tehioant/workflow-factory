use workflow_engine::runtime::{Docker, Platform, Process, Readiness, SetupPlan};
fn docker() -> Docker {
    Docker::new(
        "unix:///var/run/docker.sock",
        if cfg!(target_os = "macos") {
            Platform::MacOs
        } else {
            Platform::Linux
        },
    )
    .expect("fixed local endpoint")
}
pub fn readiness_with(process: &mut impl Process) -> Readiness {
    docker().readiness(process, None)
}
pub fn setup_plan() -> SetupPlan {
    docker().setup_plan()
}
#[cfg(test)]
mod tests {
    use super::*;
    use workflow_engine::runtime::RuntimeStatus;
    use workflow_engine::runtime::{CommandSpec, ProcessError, ProcessOutput};
    struct Missing;
    impl Process for Missing {
        fn run(&mut self, command: &CommandSpec) -> Result<ProcessOutput, ProcessError> {
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
            Err(ProcessError::Missing)
        }
    }
    #[test]
    fn manual_host_probe_uses_fixed_readonly_process_without_storage() {
        let result = readiness_with(&mut Missing);
        assert_eq!(result.status, RuntimeStatus::Missing);
        assert!(!result.execution_ready);
    }
    #[test]
    fn host_setup_returns_reviewable_argv_only() {
        let plan = setup_plan();
        assert_eq!(
            &plan.argv[..3],
            ["--host", "unix:///var/run/docker.sock", "build"]
        );
        assert!(plan.instructions.contains("never run"));
    }
}
