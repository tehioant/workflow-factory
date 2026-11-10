//! Read-only local Docker readiness. No execution or automatic setup authority.
use serde::Serialize;
use ts_rs::TS;

pub const IMAGE_TAG: &str = "workflow-factory-runtime:codex-0.166.0-v1";
pub const CODEX_VERSION: &str = "0.166.0";
pub const OUTPUT_LIMIT: usize = 131_072;
/// Readiness accepts no stdin: it is always /dev/null, not an unbounded pipe.
pub const INPUT_LIMIT: usize = 0;
pub const PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

#[derive(Debug, Clone, Copy)]
pub enum Platform {
    Linux,
    MacOs,
}
#[derive(Debug, Clone)]
pub struct Docker {
    endpoint: String,
    platform: Platform,
}
#[derive(Debug, Clone, PartialEq)]
pub struct CommandSpec {
    program: String,
    argv: Vec<String>,
}
impl CommandSpec {
    /// Conversion only; does not spawn. No ambient Docker context, proxy, credential or plugin environment.
    pub fn process_command(&self) -> std::process::Command {
        let mut command = std::process::Command::new(&self.program);
        command
            .args(&self.argv)
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/local/bin")
            .env("HOME", "/var/empty")
            .env("DOCKER_CONFIG", "/var/empty/workflow-factory-no-config")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        command
    }
    pub fn program(&self) -> &str {
        &self.program
    }
    pub fn argv(&self) -> &[String] {
        &self.argv
    }
}
#[derive(Debug, Clone, PartialEq)]
pub enum ProcessError {
    Missing,
    Unavailable,
    Timeout,
    OutputLimit,
}
#[derive(Debug, Clone)]
pub struct ProcessOutput {
    pub code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}
/// Real read-only probe transport. No shell, worker threads, detached children or inherited stdin.
#[derive(Default)]
pub struct RealProcess;
#[cfg(unix)]
impl Process for RealProcess {
    fn run(&mut self, command: &CommandSpec) -> Result<ProcessOutput, ProcessError> {
        use std::io::{ErrorKind, Read};
        use std::os::{fd::OwnedFd, unix::net::UnixStream};
        use std::process::Stdio;
        let pair = || -> Result<_, ProcessError> {
            let (reader, writer) = UnixStream::pair().map_err(|_| ProcessError::Unavailable)?;
            reader
                .set_nonblocking(true)
                .map_err(|_| ProcessError::Unavailable)?;
            Ok((reader, Stdio::from(OwnedFd::from(writer))))
        };
        let (mut stdout, out) = pair()?;
        let (mut stderr, err) = pair()?;
        let mut child = command
            .process_command()
            .stdout(out)
            .stderr(err)
            .spawn()
            .map_err(|e| {
                if e.kind() == ErrorKind::NotFound {
                    ProcessError::Missing
                } else {
                    ProcessError::Unavailable
                }
            })?;
        // All post-spawn exits run through this closure and the unconditional reap below.
        let result = (|| {
            let start = std::time::Instant::now();
            let mut output = ProcessOutput {
                code: 1,
                stdout: vec![],
                stderr: vec![],
            };
            let mut closed = [false; 2];
            let mut status = None;
            loop {
                if start.elapsed() >= PROBE_TIMEOUT {
                    return Err(ProcessError::Timeout);
                }
                let mut bytes = [0u8; 8192];
                for (index, stream) in [&mut stdout, &mut stderr].into_iter().enumerate() {
                    // One bounded read per stream per tick; continuous writers cannot starve deadline checks.
                    match stream.read(&mut bytes) {
                        Ok(0) => closed[index] = true,
                        Ok(n) => {
                            if output.stdout.len() + output.stderr.len() + n > OUTPUT_LIMIT {
                                return Err(ProcessError::OutputLimit);
                            }
                            if index == 0 {
                                output.stdout.extend_from_slice(&bytes[..n]);
                            } else {
                                output.stderr.extend_from_slice(&bytes[..n]);
                            }
                        }
                        Err(e)
                            if e.kind() == ErrorKind::WouldBlock
                                || e.kind() == ErrorKind::Interrupted => {}
                        Err(_) => return Err(ProcessError::Unavailable),
                    }
                }
                if status.is_none() {
                    status = child.try_wait().map_err(|_| ProcessError::Unavailable)?;
                }
                if let Some(status) = status {
                    if closed == [true, true] {
                        output.code = status.code().unwrap_or(1);
                        return Ok(output);
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        })();
        // Killing a read-only CLI is safe here, but is NOT container termination evidence.
        if result.is_err() {
            let _ = child.kill();
        }
        let reaped = child.wait().map_err(|_| ProcessError::Unavailable);
        reaped?;
        result
    }
}
#[cfg(not(unix))]
impl Process for RealProcess {
    fn run(&mut self, _: &CommandSpec) -> Result<ProcessOutput, ProcessError> {
        Err(ProcessError::Unavailable)
    }
}
pub trait Process {
    fn run(&mut self, command: &CommandSpec) -> Result<ProcessOutput, ProcessError>;
}
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeStatus {
    Missing,
    Stopped,
    Unsupported,
    ImageMissing,
    ImageUnsupported,
    NetworkUnavailable,
    AuthUnsupported,
}
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum IsolationEvidence {
    NotChecked,
    ExpectedNotMeasured,
    MeasuredNetworkConfiguration,
}
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Readiness {
    pub status: RuntimeStatus,
    pub message: String,
    pub image_id: Option<String>,
    pub isolation: IsolationEvidence,
    pub execution_ready: bool,
}
fn image_id(id: &str) -> bool {
    id.strip_prefix("sha256:").is_some_and(|s| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
fn read_json(
    process: &mut impl Process,
    command: &CommandSpec,
) -> Result<serde_json::Value, ProcessError> {
    let out = process.run(command)?;
    if out.stdout.len().saturating_add(out.stderr.len()) > OUTPUT_LIMIT {
        return Err(ProcessError::OutputLimit);
    }
    if out.code != 0 {
        return Err(ProcessError::Unavailable);
    }
    serde_json::from_slice(&out.stdout).map_err(|_| ProcessError::Unavailable)
}
fn readiness(status: RuntimeStatus, message: &str) -> Readiness {
    Readiness {
        status,
        message: message.into(),
        image_id: None,
        isolation: IsolationEvidence::NotChecked,
        execution_ready: false,
    }
}
pub fn verify_private_network(
    value: &serde_json::Value,
    expected_id: &str,
) -> Result<IsolationEvidence, RuntimeStatus> {
    if expected_id.len() != 64
        || !expected_id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || value["Id"] != expected_id
        || value["Driver"] != "bridge"
        || value["Internal"] != true
        || value["EnableIPv6"] != false
        || value["Options"]["com.docker.network.bridge.gateway_mode_ipv4"] != "isolated"
    {
        return Err(RuntimeStatus::NetworkUnavailable);
    }
    Ok(IsolationEvidence::MeasuredNetworkConfiguration)
}
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SetupPlan {
    pub program: String,
    pub argv: Vec<String>,
    pub instructions: String,
}
impl Docker {
    pub fn setup_plan(&self) -> SetupPlan {
        let command = self.command(&[
            "build",
            "--file",
            "runtime/Dockerfile",
            "--tag",
            IMAGE_TAG,
            ".",
        ]);
        SetupPlan { program: command.program, argv: command.argv,
            instructions: "Only on explicit user request: review the existing runtime/Dockerfile and runtime/egress.mjs, then run this argv from that reviewed repository root. The build downloads the base image/npm package and uses disk/network; it is never run by readiness or startup. Verify labels and capture the resulting immutable image ID. No auth is configured by this build.".into() }
    }
    pub fn new(endpoint: &str, platform: Platform) -> Result<Self, ProcessError> {
        let path = endpoint
            .strip_prefix("unix://")
            .ok_or(ProcessError::Unavailable)?;
        if !path.starts_with('/')
            || path.starts_with("//")
            || path.len() > 1024
            || path.bytes().any(|b| b < 32 || b == 127)
            || std::path::Path::new(path).components().any(|c| {
                !matches!(
                    c,
                    std::path::Component::RootDir | std::path::Component::Normal(_)
                )
            })
        {
            return Err(ProcessError::Unavailable);
        }
        Ok(Self {
            endpoint: endpoint.into(),
            platform,
        })
    }
    fn command(&self, args: &[&str]) -> CommandSpec {
        CommandSpec {
            program: match self.platform {
                Platform::Linux => "/usr/bin/docker",
                Platform::MacOs => "/usr/local/bin/docker",
            }
            .into(),
            argv: [
                vec!["--host".into(), self.endpoint.clone()],
                args.iter().map(|s| (*s).into()).collect(),
            ]
            .concat(),
        }
    }
    /// Must be called freshly immediately before any future start, using a durable owned network ID.
    /// Configuration evidence is not a TLS-egress measurement or start authorization.
    pub fn inspect_private_network(
        &self,
        process: &mut impl Process,
        expected_id: &str,
    ) -> Result<IsolationEvidence, RuntimeStatus> {
        if expected_id.len() != 64
            || !expected_id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(RuntimeStatus::NetworkUnavailable);
        }
        let value = read_json(process, &self.command(&["network", "inspect", expected_id]))
            .map_err(|_| RuntimeStatus::NetworkUnavailable)?;
        let network = value
            .as_array()
            .filter(|rows| rows.len() == 1)
            .and_then(|rows| rows.first())
            .ok_or(RuntimeStatus::NetworkUnavailable)?;
        verify_private_network(network, expected_id)
    }
    pub fn info_command(&self) -> CommandSpec {
        self.command(&["info", "--format", "{{json .}}"])
    }
    pub fn readiness(&self, process: &mut impl Process, pinned_image: Option<&str>) -> Readiness {
        if pinned_image.is_some_and(|id| !image_id(id)) {
            return readiness(RuntimeStatus::ImageUnsupported, "Captured image identity must be lowercase sha256 plus 64 hex digits; malformed identity refused before Docker lookup.");
        }
        let info = match read_json(process, &self.info_command()) {
            Err(ProcessError::Missing) => return readiness(RuntimeStatus::Missing, "Install Docker Engine 28+ on Linux or Docker Desktop on macOS explicitly; editing remains available."),
            Err(_) => return readiness(RuntimeStatus::Stopped, "Start the selected local Linux Docker daemon and check socket permissions; probe failed or exceeded bounds. No TCP endpoint is supported."),
            Ok(info) => info,
        };
        let major = info["ServerVersion"]
            .as_str()
            .and_then(|s| s.split('.').next())
            .and_then(|s| s.parse::<u32>().ok());
        let rootful = info["SecurityOptions"].as_array().is_some_and(|options| {
            options
                .iter()
                .all(|s| s.as_str().is_some_and(|s| !s.contains("rootless")))
        });
        if info["OSType"] != "linux" || !major.is_some_and(|m| m >= 28) || !rootful {
            return readiness(RuntimeStatus::Unsupported, "Use a trusted rootful local Docker Engine 28+ Linux daemon with internal isolated IPv4 bridges. Rootless/remote/Windows engines are unsupported; on macOS select Docker Desktop's Linux VM.");
        }
        let image = match read_json(process, &self.command(&["image", "inspect", IMAGE_TAG])) {
            Ok(value) => value,
            Err(_) => return readiness(RuntimeStatus::ImageMissing, "Pinned local runtime image absent or inspection failed. Build reviewed runtime/Dockerfile explicitly, then recheck; no pull/build was attempted."),
        };
        let Some(rows) = image.as_array().filter(|rows| rows.len() == 1) else {
            return readiness(
                RuntimeStatus::ImageUnsupported,
                "Invalid image inspection; rebuild reviewed assets explicitly.",
            );
        };
        let image = &rows[0];
        let id = image["Id"].as_str().unwrap_or("");
        if !image_id(id)
            || image["Config"]["Labels"]["org.workflow-factory.runtime"] != "1"
            || image["Config"]["Labels"]["org.workflow-factory.codex"] != CODEX_VERSION
            || pinned_image.is_some_and(|expected| !image_id(expected) || expected != id)
        {
            return readiness(RuntimeStatus::ImageUnsupported, "Runtime labels, immutable sha256 image ID or captured identity mismatch. Review and explicitly rebuild/reselect; never substitute a moved tag for an admitted run.");
        }
        Readiness {
            status: RuntimeStatus::AuthUnsupported,
            message: "Docker version suggests isolated IPv4 bridge capability, NOT measured containment. Secure container subscription authentication and concurrent refresh are unsupported pending an approved mechanism. Execution is blocked. No credentials were read.".into(),
            image_id: Some(id.into()), isolation: IsolationEvidence::ExpectedNotMeasured, execution_ready: false,
        }
    }
}
