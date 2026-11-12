//! Structured local Docker mutations. No shell, ambient credentials, bind mounts or name-only deletion.
use crate::{runtime::verify_private_network, sessions::CACHE_LIMIT, validation, Error};
use serde_json::Value;
mod block;
pub fn resource_names(owner: &str) -> [String; 3] {
    [
        format!("wf-private-{owner}"),
        format!("wf-outer-{owner}"),
        format!("wf-workspace-{owner}"),
    ]
}
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
#[derive(Clone)]
pub struct Cancel(pub Arc<AtomicBool>);
impl Default for Cancel {
    fn default() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }
}
impl Cancel {
    pub fn check(&self) -> Result<(), Error> {
        if self.0.load(Ordering::Acquire) {
            Err(validation(
                "Scoped stop requested; no further work authorized.",
            ))
        } else {
            Ok(())
        }
    }
    pub fn request(&self) {
        self.0.store(true, Ordering::Release);
    }
}
pub trait Pipe {
    /// Only workload execution may enter the incremental diagnostic transport.
    fn execution(
        &mut self,
        _args: &[String],
        _input: &[u8],
        _cancel: &Cancel,
        _consume: &mut dyn FnMut(&[u8]) -> Result<(), Error>,
    ) -> Result<i32, Error> {
        Err(validation("Incremental execution transport unsupported."))
    }
    fn call(
        &mut self,
        args: &[String],
        input: &[u8],
        limit: usize,
        cancel: &Cancel,
    ) -> Result<(i32, Vec<u8>, Vec<u8>), Error>;
}
pub struct NativePipe;
type ExecutionSink<'a> = &'a mut dyn FnMut(&[u8]) -> Result<(), Error>;
impl Pipe for NativePipe {
    fn execution(
        &mut self,
        args: &[String],
        input: &[u8],
        cancel: &Cancel,
        consume: &mut dyn FnMut(&[u8]) -> Result<(), Error>,
    ) -> Result<i32, Error> {
        self.transport(args, input, 1_048_576, cancel, Some(consume))
            .map(|v| v.0)
    }
    fn call(
        &mut self,
        args: &[String],
        input: &[u8],
        limit: usize,
        cancel: &Cancel,
    ) -> Result<(i32, Vec<u8>, Vec<u8>), Error> {
        self.transport(args, input, limit, cancel, None)
    }
}
impl NativePipe {
    fn transport(
        &mut self,
        args: &[String],
        input: &[u8],
        limit: usize,
        cancel: &Cancel,
        mut consume: Option<ExecutionSink<'_>>,
    ) -> Result<(i32, Vec<u8>, Vec<u8>), Error> {
        use std::io::{ErrorKind, Read, Write};
        use std::os::{fd::OwnedFd, unix::net::UnixStream};
        use std::process::Stdio;
        if input.len() > 1_048_576 {
            return Err(validation("Sandbox input limit exceeded."));
        }
        let (mut out, out_writer) = UnixStream::pair().map_err(crate::storage)?;
        let (mut err, err_writer) = UnixStream::pair().map_err(crate::storage)?;
        let (mut writer, reader) = UnixStream::pair().map_err(crate::storage)?;
        for stream in [&out, &err, &writer] {
            stream.set_nonblocking(true).map_err(crate::storage)?;
        }
        cancel.check()?;
        let mut command = std::process::Command::new("docker");
        command
            .args(["--host", "unix:///var/run/docker.sock"])
            .args(args)
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/local/bin")
            .env("HOME", "/var/empty")
            .env("DOCKER_CONFIG", "/var/empty/workflow-factory-no-config")
            .stdin(Stdio::from(OwnedFd::from(reader)))
            .stdout(Stdio::from(OwnedFd::from(out_writer)))
            .stderr(Stdio::from(OwnedFd::from(err_writer)));
        cancel.check()?;
        let mut child = command
            .spawn()
            .map_err(|_| validation("Local sandbox transport unavailable."))?;
        let result = (|| {
            let start = std::time::Instant::now();
            let mut stdout = vec![];
            let mut stderr = vec![];
            let mut received: usize = 0;
            let mut sent = 0;
            let mut closed = [false; 2];
            let mut input_closed = false;
            let mut status = None;
            loop {
                if cancel.0.load(Ordering::Acquire)
                    || start.elapsed() > std::time::Duration::from_secs(120)
                {
                    return Err(validation(
                        "Sandbox operation interrupted; resource ownership remains held.",
                    ));
                }
                if !input_closed {
                    if sent == input.len() {
                        writer
                            .shutdown(std::net::Shutdown::Write)
                            .map_err(crate::storage)?;
                        input_closed = true;
                    } else {
                        match writer.write(&input[sent..]) {
                            Ok(0) => return Err(validation("Sandbox stdin closed.")),
                            Ok(n) => sent += n,
                            Err(e)
                                if matches!(
                                    e.kind(),
                                    ErrorKind::WouldBlock | ErrorKind::Interrupted
                                ) => {}
                            Err(_) => return Err(validation("Sandbox input failed.")),
                        }
                    }
                }
                let mut buf = [0u8; 8192];
                for (i, stream) in [&mut out, &mut err].into_iter().enumerate() {
                    match stream.read(&mut buf) {
                        Ok(0) => closed[i] = true,
                        Ok(n) => {
                            received = received.saturating_add(n);
                            if received > limit {
                                return Err(validation(
                                    "Sandbox output limit exceeded; ownership held.",
                                ));
                            }
                            if let Some(consume) = consume.as_deref_mut() {
                                if i == 0 {
                                    consume(&buf[..n])?;
                                }
                                // Untrusted exec stderr is drained but never logged or retained.
                            } else if i == 0 {
                                stdout.extend_from_slice(&buf[..n]);
                            } else {
                                stderr.extend_from_slice(&buf[..n]);
                            }
                        }
                        Err(e)
                            if matches!(
                                e.kind(),
                                ErrorKind::WouldBlock | ErrorKind::Interrupted
                            ) => {}
                        Err(_) => return Err(validation("Sandbox output unavailable.")),
                    }
                }
                if status.is_none() {
                    status = child.try_wait().map_err(crate::storage)?;
                }
                if let Some(status) = status {
                    if closed == [true, true] {
                        return Ok((status.code().unwrap_or(1), stdout, stderr));
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        })();
        if result.is_err() {
            let _ = child.kill();
        }
        child.wait().map_err(crate::storage)?;
        result
    }
}
fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| s.to_string()).collect()
}
fn opaque(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}
fn image(id: &str) -> bool {
    id.strip_prefix("sha256:").is_some_and(opaque)
}
pub type LoginObservation = (Option<i32>, Option<String>, Option<String>);
#[derive(Clone)]
pub struct Resources {
    pub owner: String,
    pub image: String,
    pub network: String,
    pub outer: String,
    pub helper: String,
    pub container: String,
    pub workspace: Option<String>,
    pub peer: Option<String>,
    pub started_at: Option<String>,
    pub retired: Option<String>,
}
pub struct Sandbox<P: Pipe> {
    pub pipe: P,
}
impl<P: Pipe> Sandbox<P> {
    pub fn create(
        &mut self,
        owner: &str,
        image_id: &str,
        cancel: &Cancel,
        receipt: impl FnMut(&str, Option<&str>) -> Result<(), Error>,
    ) -> Result<Resources, Error> {
        self.create_for_mode(
            owner,
            image_id,
            crate::IsolationMode::PerRun,
            cancel,
            receipt,
        )
    }
    pub fn create_for_mode(
        &mut self,
        owner: &str,
        image_id: &str,
        mode: crate::IsolationMode,
        cancel: &Cancel,
        mut receipt: impl FnMut(&str, Option<&str>) -> Result<(), Error>,
    ) -> Result<Resources, Error> {
        if uuid::Uuid::parse_str(owner).is_err() || !image(image_id) {
            return Err(validation("Invalid immutable run identity."));
        }
        let workspace = if mode == crate::IsolationMode::PerBlock {
            Some(self.create_workspace(owner, cancel, &mut receipt)?)
        } else {
            None
        };
        let label = format!("org.workflow-factory.owner={owner}");
        let mut create = |kind: &str, args: Vec<String>| -> Result<String, Error> {
            cancel.check()?;
            receipt(kind, None)?;
            if kind.starts_with("work-container") {
                if let Some(name) = &workspace {
                    self.inspect_workspace(name, owner)?;
                }
            }
            let bytes = self.work_success(&args, &[], 131072, cancel)?;
            let id = std::str::from_utf8(&bytes)
                .map_err(|_| validation("Resource ID unavailable; ownership held."))?
                .trim()
                .to_string();
            if !opaque(&id) {
                return Err(validation("Resource ID unavailable; ownership held."));
            }
            receipt(kind, Some(&id))?;
            Ok(id)
        };
        let network = create(
            "private-network",
            strings(&[
                "network",
                "create",
                "--internal",
                "--driver",
                "bridge",
                "--ipv6=false",
                "--opt",
                "com.docker.network.bridge.gateway_mode_ipv4=isolated",
                "--label",
                &label,
                &resource_names(owner)[0],
            ]),
        )?;
        let outer = create(
            "egress-network",
            strings(&[
                "network",
                "create",
                "--driver",
                "bridge",
                "--ipv6=false",
                "--label",
                &label,
                &resource_names(owner)[1],
            ]),
        )?;
        let base = |network: &str, memory: &str, pids: &str| {
            strings(&[
                "create",
                "--label",
                &label,
                "--network",
                network,
                "--read-only",
                "--init",
                "--user",
                "1000:1000",
                "--cap-drop",
                "ALL",
                "--security-opt",
                "no-new-privileges",
                "--memory",
                memory,
                "--memory-swap",
                memory,
                "--cpus",
                "1",
                "--pids-limit",
                pids,
                "--log-driver",
                "none",
                "--ulimit",
                "core=0:0",
                "--tmpfs",
                "/tmp:rw,noexec,nosuid,nodev,size=64m,uid=1000,gid=1000,mode=700",
            ])
        };
        let mut args = base(&outer, "128m", "32");
        args.extend(strings(&[
            image_id,
            "node",
            "/opt/workflow-factory/egress.mjs",
        ]));
        let helper = create("egress-helper", args)?;
        let mut args = base(&network, "512m", "128");
        args.extend(strings(&[
            "--dns",
            "127.0.0.1",
            "--tmpfs",
            "/run/wf-codex:rw,noexec,nosuid,nodev,size=16m,uid=1000,gid=1000,mode=700",
            "--tmpfs",
            "/workspace:rw,exec,nosuid,nodev,size=256m,uid=1000,gid=1000,mode=700",
            "--env",
            "HOME=/run/wf-codex",
            "--env",
            "CODEX_HOME=/run/wf-codex",
            "--env",
            "HTTPS_PROXY=http://wf-egress:3128",
            "--env",
            "HTTP_PROXY=http://wf-egress:3128",
            "--env",
            "NO_PROXY=",
            image_id,
            "node",
            "/opt/workflow-factory/keeper.mjs",
        ]));
        if let Some(name) = &workspace {
            let index = args
                .iter()
                .position(|s| s.starts_with("/workspace:"))
                .unwrap();
            args.splice(
                index - 1..=index,
                strings(&[
                    "--mount",
                    &format!("type=volume,src={name},dst=/workspace,volume-nocopy"),
                ]),
            );
            let end = args.len();
            args.splice(
                end - 3..end,
                strings(&[
                    "--entrypoint",
                    "node",
                    image_id,
                    "/opt/workflow-factory/keeper.mjs",
                ]),
            );
        }
        let container = create(
            if workspace.is_some() {
                "work-container-0"
            } else {
                "work-container"
            },
            args,
        )?;
        let mut r = Resources {
            owner: owner.into(),
            image: image_id.into(),
            network,
            outer,
            helper,
            container,
            workspace,
            peer: None,
            started_at: None,
            retired: None,
        };
        self.owned("container", &r.helper, owner)?;
        self.owned("network", &r.network, owner)?;
        self.work_success(
            &strings(&[
                "network",
                "connect",
                "--alias",
                "wf-egress",
                &r.network,
                &r.helper,
            ]),
            &[],
            131072,
            cancel,
        )?;
        self.inspect_phase(&r, false)?;
        self.work_success(&strings(&["start", &r.helper]), &[], 131072, cancel)?;
        if r.workspace.is_some() {
            cancel.check()?;
            receipt("agent-start-0", None)?;
        }
        self.work_success(&strings(&["start", &r.container]), &[], 131072, cancel)?;
        if r.workspace.is_some() {
            r.started_at = Some(self.live_epoch(&r)?);
        }
        self.inspect_work(&r)?;
        if r.workspace.is_some() {
            cancel.check()?;
            receipt("agent-start-0", Some(&r.container))?;
        }
        Ok(r)
    }

    #[cfg(target_os = "macos")]
    fn check_guest(&mut self, r: &Resources, cancel: &Cancel) -> Result<(), Error> {
        #[cfg(target_os = "macos")]
        {
            let guest = self.work_success(&strings(&["exec", &r.container, "node", "-e", "const fs=require('node:fs');process.stdout.write(JSON.stringify({swaps:fs.readFileSync('/proc/swaps','utf8'),core:fs.readFileSync('/proc/sys/kernel/core_pattern','utf8')}))"]), &[], 16384, cancel)?;
            let v: Value = serde_json::from_slice(&guest).map_err(crate::storage)?;
            if !v["swaps"].as_str().is_some_and(|s| {
                s.lines().next().is_some_and(|h| h.starts_with("Filename"))
                    && !s.lines().skip(1).any(|s| !s.trim().is_empty())
            }) || !v["core"]
                .as_str()
                .is_some_and(|s| !s.trim().is_empty() && !s.trim().starts_with('|'))
            {
                return Err(validation(
                    "Docker Desktop guest swap/core policy unavailable; no credentials staged.",
                ));
            }
        }
        Ok(())
    }
    fn success(&mut self, args: &[String], input: &[u8], limit: usize) -> Result<Vec<u8>, Error> {
        self.work_success(args, input, limit, &Cancel::default())
    }
    fn work_success(
        &mut self,
        args: &[String],
        input: &[u8],
        limit: usize,
        cancel: &Cancel,
    ) -> Result<Vec<u8>, Error> {
        cancel.check()?;
        let (code, out, _) = self.pipe.call(args, input, limit, cancel)?;
        if code != 0 {
            return Err(validation(
                "Sandbox operation failed. Raw diagnostics withheld; ownership retained.",
            ));
        }
        Ok(out)
    }
    fn json(&mut self, args: &[&str]) -> Result<Value, Error> {
        serde_json::from_slice(&self.success(&strings(args), &[], 131072)?)
            .map_err(|_| validation("Sandbox inspection invalid."))
    }
    pub fn owned(&mut self, kind: &str, id: &str, owner: &str) -> Result<Value, Error> {
        if !opaque(id) || uuid::Uuid::parse_str(owner).is_err() {
            return Err(validation("Sandbox ownership identity invalid."));
        }
        let value = self.json(&[kind, "inspect", id])?;
        let row = value
            .get(0)
            .ok_or_else(|| validation("Sandbox inspection missing."))?;
        let label = if kind == "network" {
            &row["Labels"]["org.workflow-factory.owner"]
        } else {
            &row["Config"]["Labels"]["org.workflow-factory.owner"]
        };
        if row["Id"] != id || label != owner {
            return Err(validation(
                "Sandbox ownership changed; no mutation authorized.",
            ));
        }
        Ok(row.clone())
    }
    pub fn inspect_work(&mut self, r: &Resources) -> Result<(), Error> {
        self.inspect_phase(r, true)
    }
    pub fn inspect_phase(&mut self, r: &Resources, started: bool) -> Result<(), Error> {
        if !image(&r.image) {
            return Err(validation("Immutable runtime image required."));
        }
        let network = self.owned("network", &r.network, &r.owner)?;
        verify_private_network(&network, &r.network)
            .map_err(|_| validation("Sandbox network is not isolated."))?;
        let c = self.owned("container", &r.container, &r.owner)?;
        let helper = self.owned("container", &r.helper, &r.owner)?;
        let outer = self.owned("network", &r.outer, &r.owner)?;
        if r.workspace.is_some() {
            self.helper_mounts(&helper)?;
            self.inspect_workspace_mount(r, &c, started)?;
        }
        let helper_networks = helper["NetworkSettings"]["Networks"]
            .as_object()
            .ok_or_else(|| validation("Helper network inspection missing."))?;
        let helper_config = &helper["HostConfig"];
        if outer["Driver"] != "bridge"
            || outer["Internal"] != false
            || outer["EnableIPv6"] != false
            || helper["Image"] != r.image
            || helper["Config"]["User"] != "1000:1000"
            || helper_config["ReadonlyRootfs"] != true
            || helper_config["Privileged"] != false
            || helper_config["LogConfig"]["Type"] != "none"
            || helper_config["Memory"] != 134217728u64
            || helper_config["MemorySwap"] != 134217728u64
            || helper_config["PidsLimit"] != 32
            || helper_networks.len() != 2
            || (started
                && (!helper_networks
                    .values()
                    .any(|n| n["NetworkID"] == r.network)
                    || !helper_networks.values().any(|n| n["NetworkID"] == r.outer)))
            || (!started
                && (!helper_networks.contains_key(&format!("wf-private-{}", r.owner))
                    || !helper_networks.contains_key(&format!("wf-outer-{}", r.owner))))
        {
            return Err(validation("Applied provider helper containment changed."));
        }
        let mut agents = vec![r.container.as_str(), r.helper.as_str()];
        if let Some(peer) = &r.peer {
            agents.push(peer);
        }
        for (network, allowed) in [(&network, agents), (&outer, vec![r.helper.as_str()])] {
            if network["Containers"].as_object().is_none_or(|containers| {
                containers.keys().any(|id| !allowed.contains(&id.as_str()))
                    || (started && containers.len() != allowed.len())
            }) {
                return Err(validation("Unexpected network member; execution withheld."));
            }
        }
        let h = &c["HostConfig"];
        let networks = c["NetworkSettings"]["Networks"]
            .as_object()
            .ok_or_else(|| validation("Sandbox network inspection missing."))?;
        for path in ["/run/wf-codex", "/workspace", "/tmp"] {
            if path == "/workspace" && r.workspace.is_some() {
                continue;
            }
            if h["Tmpfs"][path].as_str().is_none_or(|s| {
                !s.contains("uid=1000")
                    || !s.contains("mode=700")
                    || !s.contains("nosuid")
                    || !s.contains("nodev")
            }) {
                return Err(validation("Private tmpfs configuration changed."));
            }
        }
        for host in [h, helper_config] {
            if !host["Ulimits"].as_array().is_some_and(|a| {
                a.iter()
                    .any(|v| v["Name"] == "core" && v["Soft"] == 0 && v["Hard"] == 0)
            }) {
                return Err(validation("Container core dumps not disabled."));
            }
            for field in [
                "Binds",
                "Devices",
                "DeviceRequests",
                "CapAdd",
                "VolumesFrom",
            ] {
                if !host[field].is_null() && host[field].as_array().is_none_or(|a| !a.is_empty()) {
                    return Err(validation("Unexpected sandbox host access."));
                }
            }
            if host["Init"] != true {
                return Err(validation("Sandbox orphan reaper unavailable."));
            }
            if host["PublishAllPorts"] == true
                || host["PortBindings"]
                    .as_object()
                    .is_some_and(|ports| !ports.is_empty())
                || host["PidMode"] == "host"
                || host["IpcMode"] == "host"
            {
                return Err(validation(
                    "Unexpected sandbox host namespace or port access.",
                ));
            }
        }
        let exactly_private = networks.len() == 1
            && if started {
                networks.values().all(|n| n["NetworkID"] == r.network)
            } else {
                networks.contains_key(&format!("wf-private-{}", r.owner))
            };
        for (path, executable) in [
            ("/workspace", true),
            ("/run/wf-codex", false),
            ("/tmp", false),
        ] {
            if path == "/workspace" && r.workspace.is_some() {
                continue;
            }
            let options = h["Tmpfs"][path]
                .as_str()
                .unwrap_or("")
                .split(',')
                .collect::<Vec<_>>();
            if executable && (!options.contains(&"exec") || options.contains(&"noexec"))
                || !executable && !options.contains(&"noexec")
            {
                return Err(validation("Tmpfs execution policy changed."));
            }
        }
        if c["Image"] != r.image
            || c["Config"]["User"] != "1000:1000"
            || h["ReadonlyRootfs"] != true
            || h["Privileged"] != false
            || h["Memory"] != 536870912u64
            || h["MemorySwap"] != 536870912u64
            || h["PidsLimit"] != 128
            || h["NanoCpus"] != 1_000_000_000u64
            || h["LogConfig"]["Type"] != "none"
            || !h["CapDrop"]
                .as_array()
                .is_some_and(|a| a.iter().any(|v| v == "ALL"))
            || !h["SecurityOpt"].as_array().is_some_and(|a| {
                a.iter()
                    .any(|v| v == "no-new-privileges" || v == "no-new-privileges:true")
            })
            || !exactly_private
            || c["Mounts"].as_array().is_none_or(|a| {
                a.iter().any(|v| {
                    v["Type"] != "tmpfs"
                        && !(r.workspace.is_some()
                            && v["Type"] == "volume"
                            && v["Destination"] == "/workspace")
                })
            })
        {
            return Err(validation(
                "Applied sandbox configuration differs; execution withheld.",
            ));
        }
        Ok(())
    }
    pub fn start_login(&mut self, r: &Resources) -> Result<(), Error> {
        self.inspect_work(r)?;
        #[cfg(target_os = "macos")]
        self.check_guest(r, &Cancel::default())?;
        self.check_no_swap(r)?;
        self.success(
            &strings(&[
                "exec",
                "-d",
                &r.container,
                "node",
                "/opt/workflow-factory/keeper.mjs",
                "login",
            ]),
            &[],
            131072,
        )?;
        Ok(())
    }
    pub fn login_status(&mut self, r: &Resources) -> Result<LoginObservation, Error> {
        self.inspect_work(r)?;
        let bytes = self.success(
            &strings(&[
                "exec",
                &r.container,
                "node",
                "/opt/workflow-factory/keeper.mjs",
                "login-status",
            ]),
            &[],
            16384,
        )?;
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|_| validation("Invalid sign-in status."))?;
        let url = value["url"]
            .as_str()
            .filter(|s| *s == "https://auth.openai.com/codex/device")
            .map(str::to_string);
        let code = value["code"]
            .as_str()
            .filter(|s| {
                s.len() == 9
                    && s.as_bytes()[4] == b'-'
                    && s.bytes()
                        .enumerate()
                        .all(|(i, c)| i == 4 || c.is_ascii_uppercase() || c.is_ascii_digit())
            })
            .map(str::to_string);
        if url.is_some() != code.is_some() {
            return Err(validation("Unsupported device prompt."));
        }
        Ok((
            value["exit"].as_i64().and_then(|v| i32::try_from(v).ok()),
            url,
            code,
        ))
    }
    pub fn stop_writers(&mut self, r: &Resources) -> Result<(), Error> {
        self.inspect_work(r)?;
        self.success(
            &strings(&[
                "exec",
                &r.container,
                "node",
                "/opt/workflow-factory/keeper.mjs",
                "stop-writers",
            ]),
            &[],
            131072,
        )?;
        Ok(())
    }
    fn check_no_swap(&mut self, r: &Resources) -> Result<(), Error> {
        let swap=self.success(&strings(&["exec",&r.container,"node","-e","process.stdout.write(require('node:fs').readFileSync('/sys/fs/cgroup/memory.swap.max','utf8'))"]),&[],1024)?;
        if swap != b"0\n" && swap != b"0" {
            return Err(validation("Applied container no-swap policy unavailable."));
        }
        Ok(())
    }
    pub fn stage(&mut self, r: &Resources, cache: &[u8], cancel: &Cancel) -> Result<(), Error> {
        crate::sessions::validate_cache(cache)?;
        self.inspect_work(r)?;
        #[cfg(target_os = "macos")]
        self.check_guest(r, cancel)?;
        self.check_no_swap(r)?;
        if let Some(id) = &r.retired {
            self.absent_container(id, cancel)?;
        }
        self.work_success(
            &strings(&[
                "exec",
                "-i",
                &r.container,
                "node",
                "/opt/workflow-factory/keeper.mjs",
                "stage",
            ]),
            cache,
            CACHE_LIMIT,
            cancel,
        )?;
        Ok(())
    }
    pub fn execute(
        &mut self,
        r: &Resources,
        prompt: &str,
        config: &crate::CodexConfiguration,
        cancel: &Cancel,
        block: &str,
        consume: &mut dyn FnMut(&[u8]) -> Result<(), Error>,
    ) -> Result<i32, Error> {
        cancel.check()?;
        self.inspect_work(r)?;
        #[cfg(target_os = "macos")]
        self.check_guest(r, cancel)?;
        if r.workspace.is_some() {
            self.check_no_swap(r)?;
        }
        if let Some(id) = &r.retired {
            self.absent_container(id, cancel)?;
        }
        cancel.check()?;
        let code = self.pipe.execution(
            &strings(&[
                "exec",
                "-i",
                &r.container,
                "node",
                "/opt/workflow-factory/keeper.mjs",
                "run",
                &config.model,
                serde_json::to_string(&config.reasoning)
                    .map_err(crate::storage)?
                    .trim_matches('"'),
                &r.owner,
                block,
                r.started_at
                    .as_deref()
                    .ok_or_else(|| validation("Execution epoch unavailable."))?,
            ]),
            prompt.as_bytes(),
            cancel,
            consume,
        )?;
        Ok(code)
    }
    pub fn diagnostics(
        &mut self,
        r: &Resources,
        block: &str,
        cache: &[u8],
        secrets: &[String],
        discard: bool,
    ) -> Result<crate::output::Publication, Error> {
        self.inspect_work(r)?;
        crate::sessions::validate_cache(cache)?;
        let identity = crate::output::DiagnosticIdentity {
            run_id: r.owner.clone(),
            block_id: block.into(),
            epoch: r
                .started_at
                .clone()
                .ok_or_else(|| validation("Diagnostic epoch unavailable."))?,
        };
        let request=serde_json::to_vec(&serde_json::json!({"identity":identity,"cache":std::str::from_utf8(cache).map_err(crate::storage)?,"secrets":secrets})).map_err(crate::storage)?;
        if request.len() > 262144 {
            return Err(validation("Known-secret publication budget exceeded."));
        }
        let bytes = self.success(
            &strings(&[
                "exec",
                "-i",
                &r.container,
                "node",
                "/opt/workflow-factory/keeper.mjs",
                if discard {
                    "discard-diagnostics"
                } else {
                    "publish-diagnostics"
                },
            ]),
            &request,
            1_048_576,
        )?;
        if discard {
            return Ok(crate::output::Publication::acknowledged(identity));
        }
        crate::output::Publication::decode(&bytes, &identity, secrets)
    }
    pub fn capture_artifacts(
        &mut self,
        r: &Resources,
        block: &str,
        cache: &[u8],
        secrets: &[String],
        root: &std::path::Path,
    ) -> Result<(), Error> {
        self.inspect_work(r)?;
        if self.live_epoch(r)?
            != r.started_at
                .clone()
                .ok_or_else(|| validation("Artifact epoch unavailable."))?
        {
            return Err(validation("Artifact epoch changed."));
        }
        crate::sessions::validate_cache(cache)?;
        let identity = crate::output::DiagnosticIdentity {
            run_id: r.owner.clone(),
            block_id: block.into(),
            epoch: r.started_at.clone().unwrap(),
        };
        let request=serde_json::to_vec(&serde_json::json!({"identity":identity,"cache":std::str::from_utf8(cache).map_err(crate::storage)?,"secrets":secrets})).map_err(crate::storage)?;
        if request.len() > 262144 {
            return Err(validation("Artifact request limit exceeded."));
        }
        let bytes = self.success(
            &strings(&[
                "exec",
                "-i",
                &r.container,
                "node",
                "/opt/workflow-factory/capture-artifacts.mjs",
            ]),
            &request,
            1_048_576,
        )?;
        self.inspect_work(r)?;
        crate::runs::inspection::artifacts::Artifacts::commit(root, &identity, &bytes, secrets)
    }
    pub fn export(&mut self, r: &Resources) -> Result<Vec<u8>, Error> {
        self.inspect_work(r)?;
        let cache = self.success(
            &strings(&[
                "exec",
                &r.container,
                "node",
                "/opt/workflow-factory/keeper.mjs",
                "export",
            ]),
            &[],
            CACHE_LIMIT,
        )?;
        crate::sessions::validate_cache(&cache)?;
        Ok(cache)
    }
    /// Dedicated explicit retirement only, with no worker allowed to remain active.
    /// Failed inspect is never absence. Discover by owner label, verify exact IDs,
    /// remove only positively owned objects, then require successful empty lists.
    pub fn retire_owned(
        &mut self,
        owner: &str,
        records: &[(String, Option<String>)],
    ) -> Result<(), Error> {
        crate::sessions::valid_slot(owner)?;
        for kind in ["container", "volume", "network"] {
            let mut args = strings(&[
                kind,
                "ls",
                "--no-trunc",
                "--filter",
                &format!("label=org.workflow-factory.owner={owner}"),
                "--format",
                "{{.ID}}",
            ]);
            if kind == "volume" {
                args.retain(|a| a != "--no-trunc");
                let n = args.len();
                args[n - 1] = "{{.Name}}".into();
            }
            if kind == "container" {
                args.push("-a".into());
            }
            let bytes = self.success(&args, &[], 131072)?;
            let text = std::str::from_utf8(&bytes).map_err(crate::storage)?;
            for id in text.split_whitespace() {
                if kind == "volume" {
                    self.inspect_workspace(id, owner)?;
                } else {
                    self.owned(kind, id, owner)?;
                }
                let rm = if kind == "container" {
                    strings(&["rm", "-f", id])
                } else {
                    strings(&[kind, "rm", id])
                };
                self.success(&rm, &[], 131072)?;
            }
            if self
                .success(&args, &[], 131072)?
                .iter()
                .any(|b| !b.is_ascii_whitespace())
            {
                return Err(validation("Owned retirement absence unconfirmed."));
            }
            for (record_kind, id) in records {
                let expected = match record_kind.as_str() {
                    "workspace-volume" | "workspace-remove" => "volume",
                    "private-network"
                    | "egress-network"
                    | "private-network-remove"
                    | "egress-network-remove" => "network",
                    "work-container" | "egress-helper" | "final-agent-remove" | "helper-remove" => {
                        "container"
                    }
                    s if [
                        "work-container-",
                        "agent-start-",
                        "agent-overlap-",
                        "agent-remove-",
                    ]
                    .iter()
                    .any(|prefix| {
                        s.strip_prefix(prefix)
                            .is_some_and(|n| n.parse::<usize>().is_ok_and(|n| n < 100))
                    }) =>
                    {
                        "container"
                    }
                    _ => return Err(validation("Unknown resource kind; retirement held.")),
                };
                if expected != kind {
                    continue;
                }
                if let Some(id) = id {
                    if (kind == "volume" && id != &format!("wf-workspace-{owner}"))
                        || (kind != "volume" && !opaque(id))
                    {
                        return Err(validation("Invalid recorded resource."));
                    }
                    let mut exact = strings(&[
                        kind,
                        "ls",
                        "--no-trunc",
                        "--filter",
                        &format!("id={id}"),
                        "--format",
                        "{{.ID}}",
                    ]);
                    if kind == "volume" {
                        exact.retain(|a| a != "--no-trunc");
                        let n = exact.len();
                        exact[n - 1] = "{{.Name}}".into();
                        let i = exact.iter().position(|a| a.starts_with("id=")).unwrap();
                        exact[i] = format!("name=^{id}$");
                    }
                    if kind == "container" {
                        exact.push("-a".into());
                    }
                    if self
                        .success(&exact, &[], 131072)?
                        .iter()
                        .any(|b| !b.is_ascii_whitespace())
                    {
                        return Err(validation(
                            "Recorded resource remains; retirement withheld.",
                        ));
                    }
                }
            }
        }
        Ok(())
    }
    pub fn cleanup(&mut self, r: &Resources) -> Result<(), Error> {
        for id in [&r.container, &r.helper] {
            self.owned("container", id, &r.owner)?;
            self.success(&strings(&["rm", "-f", id]), &[], 131072)?;
            // A failed inspect is not absence evidence. List the exact ID and require an empty result.
            let found = self.success(
                &strings(&[
                    "container",
                    "ls",
                    "-a",
                    "--no-trunc",
                    "--filter",
                    &format!("id={id}"),
                    "--format",
                    "{{.ID}}",
                ]),
                &[],
                131072,
            )?;
            if found.iter().any(|c| !c.is_ascii_whitespace()) {
                return Err(validation("Owned container removal unconfirmed."));
            }
        }
        for id in [&r.network, &r.outer] {
            let n = self.owned("network", id, &r.owner)?;
            if n["Containers"].as_object().is_none_or(|a| !a.is_empty()) {
                return Err(validation("Network still has resources; cleanup held."));
            }
            self.success(&strings(&["network", "rm", id]), &[], 131072)?;
        }
        Ok(())
    }
}
