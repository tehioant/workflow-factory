//! Native observations only. No IPC flag can establish platform qualification.
use workflow_engine::runs::Admission;
pub fn execute(
    directory: std::path::PathBuf,
    id: String,
    cancel: workflow_engine::sandbox::Cancel,
) {
    use workflow_engine::{
        sandbox::{NativePipe, Sandbox},
        sessions::SecureStore,
        Store,
    };
    let Ok(store) = Store::open(&directory) else {
        return;
    };
    let Ok(Some((snapshot, lease))) = store.claim_run(&id) else {
        return;
    };
    let mut sandbox = Sandbox { pipe: NativePipe };
    let mut secure = crate::sessions_host::NativeSecureStore;
    let result = (|| {
        native_prerequisites().map_err(|message| workflow_engine::Error::Validation { message })?;
        cancel.check()?;
        let resources = sandbox.create(&id, &snapshot.image_id, |kind, resource| {
            store.resource_intent(&id, kind, resource)
        })?;
        let seed = secure.read(&lease.slot)?;
        cancel.check()?;
        sandbox.stage(&resources, &seed)?;
        cancel.check()?;
        let execution = sandbox.execute(
            &resources,
            &snapshot.inputs.prompts[&snapshot.definition.definition.blocks[0].id],
            &snapshot.configuration.config,
            &cancel,
        );
        // Even on Codex error, export its CURRENT cache before stopping the keeper.
        if execution.is_err() {
            sandbox.stop_writers(&resources)?;
        }
        let mut exported = None;
        for _ in 0..10 {
            match sandbox.export(&resources) {
                Ok(cache) => {
                    exported = Some(cache);
                    break;
                }
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(100)),
            }
        }
        let rotated = exported.ok_or_else(|| workflow_engine::Error::Validation {
            message: "Credential writers/export unconfirmed; keeper and session remain held."
                .into(),
        })?;
        let mut secrets = Vec::new();
        for bytes in [&seed, &rotated] {
            let v: serde_json::Value =
                serde_json::from_slice(bytes).map_err(|_| workflow_engine::Error::Validation {
                    message: "Managed cache invalid; session quarantined.".into(),
                })?;
            for key in ["access_token", "refresh_token", "id_token"] {
                if let Some(token) = v["tokens"][key].as_str() {
                    secrets.push(token.to_string());
                }
            }
        }
        let final_result = execution.and_then(|(code, output)| {
            workflow_engine::output::Decoder::decode(&output, code, &secrets)
        });
        store.finish_session(
            &lease.slot,
            &id,
            lease.generation,
            &rotated,
            &mut secure,
            || sandbox.cleanup(&resources),
        )?;
        match final_result {Ok(text)=>store.finish_run(&id,Some(text),"Completed; secure cache persisted and owned resources removed.",true,true),Err(_)=>store.finish_run(&id,None,"Codex output failed validation or was interrupted. Cache persisted and resources removed.",true,false)}
    })();
    if result.is_err() {
        let _ = store.quarantine_session(&lease);
        let _=store.finish_run(&id,None,"Execution or secure-cache cleanup failed. Ownership and session remain withheld. No automatic retry.",false,false);
    }
}
/// Supported secure mode: Linux, same-host rootful dockerd, no active swap,
/// no piped core collector, daemon core limit zero, app non-dumpable/core-disabled.
/// This is not protection from a privileged operator, hibernation or VM snapshots.
pub fn admission() -> Admission {
    let result = native_prerequisites();
    if let Err(message) = result {
        return Admission {
            image_id: String::new(),
            blocker: Some(message),
        };
    }
    let runtime = crate::runtime_host::readiness_with(&mut workflow_engine::runtime::RealProcess);
    Admission {
        image_id: runtime.image_id.clone().unwrap_or_default(),
        blocker: if runtime.image_id.is_some() {
            None
        } else {
            Some(runtime.message)
        },
    }
}
fn policy(swaps: &str, core_pattern: &str, limits: &str) -> bool {
    swaps
        .lines()
        .next()
        .is_some_and(|s| s.starts_with("Filename"))
        && !swaps.lines().skip(1).any(|s| !s.trim().is_empty())
        && !core_pattern.trim().is_empty()
        && !core_pattern.trim().starts_with('|')
        && limits
            .lines()
            .find_map(|s| s.strip_prefix("Max core file size"))
            .is_some_and(|s| s.split_whitespace().take(2).collect::<Vec<_>>() == ["0", "0"])
}
#[cfg(target_os = "linux")]
fn native_prerequisites() -> Result<(), String> {
    use std::os::{fd::AsRawFd, unix::net::UnixStream};
    let fail = || {
        "secureStagingUnavailable: requires same-host root dockerd with zero core limits, no host swap or piped core collector, and app dump controls. No host/service settings are changed.".to_string()
    };
    let socket = UnixStream::connect("/var/run/docker.sock").map_err(|_| fail())?;
    let mut peer: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            socket.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut peer as *mut libc::ucred).cast(),
            &mut len,
        )
    } != 0
        || peer.uid != 0
        || peer.pid <= 0
    {
        return Err(fail());
    }
    let proc = format!("/proc/{}", peer.pid);
    // SO_PEERCRED identifies the real local daemon in our PID namespace. comm
    // and limits are readable without ptrace access to a root process's exe/ns.
    // Root administrators remain explicitly trusted; proxies are unsupported.
    if std::fs::read_to_string(format!("{proc}/comm"))
        .ok()
        .as_deref()
        .map(str::trim)
        != Some("dockerd")
    {
        return Err(fail());
    }
    let swaps = std::fs::read_to_string("/proc/swaps").map_err(|_| fail())?;
    let pattern = std::fs::read_to_string("/proc/sys/kernel/core_pattern").map_err(|_| fail())?;
    let limits = std::fs::read_to_string(format!("{proc}/limits")).map_err(|_| fail())?;
    if !policy(&swaps, &pattern, &limits) {
        return Err(fail());
    }
    let zero = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    if unsafe { libc::setrlimit(libc::RLIMIT_CORE, &zero) } != 0
        || unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0) } != 0
        || unsafe { libc::prctl(libc::PR_GET_DUMPABLE) } != 0
    {
        return Err(fail());
    }
    Ok(())
}
#[cfg(not(target_os = "linux"))]
fn native_prerequisites() -> Result<(), String> {
    Err("secureStagingUnavailable: only qualified local Linux secure mode is implemented; macOS/VM execution unsupported.".into())
}
/// No IPC approval Boolean: the host owns the question, text and direct native
/// user response. Approval is consumed by this exact caller, never cached/reused.
pub fn confirm(scope: &str) -> Result<(), workflow_engine::Error> {
    let result = std::process::Command::new("/usr/bin/zenity")
        .args(["--question","--no-markup","--default-cancel","--title=Workflow Factory · trusted private scope","--text", &format!("Authorize only this operation on trusted private infrastructure and private trusted inputs (not public/open-source or untrusted workloads). Credentials are accessible to the workload. Do not hibernate or take memory snapshots while active. This mode does not defend against privileged host administrators. No paid fallback or host grant import.\n\n{scope}")])
        .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
    if result.is_ok_and(|s| s.success()) {
        Ok(())
    } else {
        Err(workflow_engine::Error::Validation{message:"Native confirmation declined/unavailable. Install the supported Linux zenity dialog; no authorization was granted.".into()})
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn c11_native_security_policy_has_supported_positive_and_specific_refusals() {
        let swaps = "Filename\tType\tSize\tUsed\tPriority\n";
        let limits = "Max core file size        0                    0                    bytes\n";
        assert!(super::policy(swaps, "core", limits));
        assert!(!super::policy(
            &format!("{swaps}/swap file 1 0 -2\n"),
            "core",
            limits
        ));
        assert!(!super::policy(
            swaps,
            "|/usr/lib/systemd/systemd-coredump",
            limits
        ));
        assert!(!super::policy(
            swaps,
            "core",
            "Max core file size        unlimited unlimited bytes"
        ));
    }
}
