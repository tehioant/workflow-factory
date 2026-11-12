//! Native observations only. No IPC flag can establish platform qualification.
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use workflow_engine::runs::Admission;
fn outputs() -> &'static Mutex<BTreeMap<String, String>> {
    static OUTPUTS: OnceLock<Mutex<BTreeMap<String, String>>> = OnceLock::new();
    OUTPUTS.get_or_init(Mutex::default)
}
fn output_hold() -> workflow_engine::Error {
    workflow_engine::Error::Storage { message: "Completed redacted output retained in app memory; storage failed. Ownership remains held. Keep the app open and use Retry retained output storage; no execution retry.".into() }
}
struct PendingOutput(String);
fn retain_output(id: &str, text: String) -> Result<PendingOutput, workflow_engine::Error> {
    outputs()
        .lock()
        .map_err(|_| output_hold())?
        .insert(id.into(), text);
    Ok(PendingOutput(id.into()))
}
impl PendingOutput {
    fn settle(
        self,
        persisted: Result<bool, workflow_engine::Error>,
    ) -> Result<bool, workflow_engine::Error> {
        if matches!(persisted, Ok(true)) {
            outputs().lock().map_err(|_| output_hold())?.remove(&self.0);
        }
        persisted
    }
}
pub fn retry_outputs(
    store: &workflow_engine::Store,
    selected: &str,
) -> Result<(), workflow_engine::Error> {
    retry_selected_outputs(selected, |id, block, text| match block {
        Some(block) => store.retain_block_output(id, block, text).map(|_| ()),
        None => store.retain_run_output(id, text).map(|_| ()),
    })
}
fn retry_selected_outputs(
    selected: &str,
    mut persist: impl FnMut(&str, Option<&str>, &str) -> Result<(), workflow_engine::Error>,
) -> Result<(), workflow_engine::Error> {
    let mut pending = outputs().lock().map_err(|_| output_hold())?;
    let mut failed = false;
    pending.retain(|key, text| {
        let (id, block) = key
            .split_once('/')
            .map_or((key.as_str(), None), |(id, block)| (id, Some(block)));
        if id != selected {
            return true;
        }
        let keep = persist(id, block, text).is_err();
        failed |= keep;
        keep
    });
    if !failed {
        Ok(())
    } else {
        Err(output_hold())
    }
}

pub fn execute_confirmed(
    cancel: &workflow_engine::sandbox::Cancel,
    confirm: impl FnOnce() -> Result<(), workflow_engine::Error>,
    authorize: impl FnOnce() -> Result<bool, workflow_engine::Error>,
    execute: impl FnOnce(),
) -> Result<bool, workflow_engine::Error> {
    cancel.check()?;
    confirm()?;
    cancel.check()?;
    if !authorize()? {
        return Ok(false);
    }
    cancel.check()?;
    execute();
    Ok(true)
}

pub fn execute(
    directory: std::path::PathBuf,
    id: String,
    cancel: workflow_engine::sandbox::Cancel,
) {
    use workflow_engine::{
        sandbox::{NativePipe, Sandbox},
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
        let mut resources = sandbox.create_for_mode(
            &id,
            &snapshot.image_id,
            snapshot.definition.definition.isolation,
            &cancel,
            |kind, resource| store.resource_intent(&id, kind, resource),
        )?;
        native_prerequisites().map_err(|message| workflow_engine::Error::Validation { message })?;
        let seed = sandbox.stage_owned(&lease, &resources, &mut secure, &cancel)?;
        cancel.check()?;
        if snapshot.plans.is_empty() {
            return Err(workflow_engine::Error::Validation {
                message: "Legacy interrupted run cannot resume automatically.".into(),
            });
        }
        let mut latest_cache = seed.clone();
        let mut cache_ready = false;
        let mut secrets = Vec::new();
        let mut completed = 0;
        let mut capture_block = None;
        let final_result = loop {
            if completed > 0 && completed < snapshot.plans.len() {
                resources = sandbox.switch_block_environment(
                    &resources,
                    completed,
                    &cancel,
                    || store.checkpoint_session(&lease, &latest_cache, &mut secure, &cancel),
                    |kind, resource| store.resource_intent(&id, kind, resource),
                    || {
                        native_prerequisites()
                            .map_err(|message| workflow_engine::Error::Validation { message })
                    },
                )?;
            }
            let mut pending_output = None;
            let step = store.execute_next_block(
                &id,
                &lease,
                &resources,
                &cancel,
                |resources, plan, prompt| {
                    cache_ready = false;
                    let key = format!("{id}/{}", plan.block_id);
                    native_prerequisites()
                        .map_err(|message| workflow_engine::Error::Validation { message })?;
                    cancel.check()?;
                    let identity=workflow_engine::output::DiagnosticIdentity{run_id:id.clone(),block_id:plan.block_id.clone(),epoch:resources.started_at.clone().ok_or_else(||workflow_engine::Error::Validation{message:"Diagnostic epoch unavailable.".into()})?};
                    let mut live=workflow_engine::runs::inspection::LiveJournal::new(&directory,identity)?;
                    let execution = sandbox.execute(resources,prompt,&plan.configuration.config,&cancel,&plan.block_id,&mut |chunk|live.push(chunk,||crate::inspection_host::notify(&directory,&id)));
                    // Even on Codex error, export its CURRENT cache before stopping the keeper.
                    if execution.is_err() {
                        sandbox.stop_writers(resources)?;
                    }
                    let mut exported = None;
                    for _ in 0..10 {
                        match sandbox.export(resources) {
                            Ok(cache) => {
                                exported = Some(cache);
                                break;
                            }
                            Err(_) => std::thread::sleep(std::time::Duration::from_millis(100)),
                        }
                    }
                    let rotated = exported.ok_or_else(|| workflow_engine::Error::Validation {
                        message:
                            "Credential writers/export unconfirmed; keeper and session remain held."
                                .into(),
                    })?;
                    latest_cache = rotated.clone();
                    for bytes in [&seed, &rotated] {
                        let v: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| {
                            workflow_engine::Error::Validation {
                                message: "Managed cache invalid; session quarantined.".into(),
                            }
                        })?;
                        for key in ["access_token", "refresh_token", "id_token"] {
                            if let Some(token) = v["tokens"][key].as_str() {
                                secrets.push(token.to_string());
                            }
                        }
                    }
                    cache_ready = true;
                    capture_block = Some(plan.block_id.clone());
                    secrets.sort();secrets.dedup();
                    let final_result = (|| {
                        if serde_json::to_vec(&secrets).map_err(|_|workflow_engine::Error::Validation{message:"Known-secret set invalid.".into()})?.len()>196608{return Err(workflow_engine::Error::Validation{message:"Known-secret publication budget exceeded.".into()});}
                        let publication=sandbox.diagnostics(resources,&plan.block_id,&rotated,&secrets,false)?;
                        // Failure here must still reach mandatory cache writeback and cleanup below.
                        workflow_engine::runs::inspection::Journal::commit(&directory,&id,&publication).map_err(|_| workflow_engine::Error::Validation{message:"Sanitized diagnostics unavailable: private journal commit failed; no retained-byte claim.".into()})?;
                        crate::inspection_host::notify(&directory,&id);
                        sandbox.diagnostics(resources,&plan.block_id,&rotated,&secrets,true)?;
                        let code=execution?;live.finish()?;
                        if code!=0||!publication.completed||publication.incomplete{return Err(workflow_engine::Error::Validation{message:"Incomplete or failed diagnostic publication.".into()});}
                        publication.final_response.ok_or_else(||workflow_engine::Error::Validation{message:"Final response unavailable.".into()})
                    })().map_err(|_|workflow_engine::Error::Validation{message:"Diagnostic publication incomplete or unavailable; no uncommitted retained-byte claim. Mandatory credential recovery remains independent.".into()});
                    if let Ok(text) = &final_result {
                        pending_output = Some(retain_output(&key, text.clone())?);
                    }
                    final_result
                },
            );
            let step = match pending_output {
                Some(pending) => pending.settle(step),
                None => step,
            };
            match step {
                Ok(true) => {
                    completed += 1;
                }
                Ok(false) => break Ok(store.run_status(&id)?.final_response.unwrap_or_default()),
                Err(error @ workflow_engine::Error::Storage { .. }) => return Err(error),
                Err(error) => {
                    if !cache_ready {
                        return Err(error);
                    }
                    break Err(error);
                }
            }
        };
        // Capture once from the final stopped writer/current-cache checkpoint, before
        // secure writeback and final mount removal. Failure cannot revoke recovery.
        let artifact_result = if cache_ready {
            match capture_block.as_deref() {
                Some(block) => sandbox.capture_artifacts(
                    &resources,
                    block,
                    &latest_cache,
                    &secrets,
                    &directory,
                ),
                None => Err(workflow_engine::Error::Validation {
                    message: "Artifacts unavailable: no safe checkpoint.".into(),
                }),
            }
        } else {
            Err(workflow_engine::Error::Validation {
                message: "Artifacts unavailable: writers/cache unconfirmed.".into(),
            })
        };
        let artifact_result = artifact_result.and_then(|_| {
            let identity = workflow_engine::output::DiagnosticIdentity {
                run_id: id.clone(),
                block_id: capture_block.clone().ok_or_else(|| {
                    workflow_engine::Error::Validation {
                        message: "Capture identity unavailable.".into(),
                    }
                })?,
                epoch: resources.started_at.clone().ok_or_else(|| {
                    workflow_engine::Error::Validation {
                        message: "Capture epoch unavailable.".into(),
                    }
                })?,
            };
            workflow_engine::runs::inspection::Journal::capture_committed(
                &directory,
                &id,
                identity,
                || crate::inspection_host::notify(&directory, &id),
            )?;
            Ok(())
        });
        store.finish_owned_session(
            &lease,
            &latest_cache,
            &mut secure,
            final_result.is_ok(),
            || {
                if resources.workspace.is_some() {
                    sandbox.cleanup_block_resources(&resources, |kind, resource| {
                        store.resource_intent(&id, kind, resource)
                    })
                } else {
                    sandbox.cleanup(&resources)
                }
            },
        )?;
        let artifact_message = if artifact_result.is_ok() {
            "Completed; immutable sanitized artifacts captured; cache persisted and resources removed."
        } else {
            "Completed; artifacts unavailable or incomplete; cache persisted and resources removed."
        };
        match final_result {Ok(text)=>store.finish_run(&id,Some(text),artifact_message,true,true),Err(_)=>store.finish_run(&id,None,"Provider execution or output validation failed. Current cache persisted and resources removed; session quarantined. Explicit reauthentication required; no retry or failover.",false,false)}
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
#[cfg(any(target_os = "linux", test))]
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
#[cfg(target_os = "macos")]
fn native_prerequisites() -> Result<(), String> {
    crate::mac_security::qualify()
}
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn native_prerequisites() -> Result<(), String> {
    Err("secureStagingUnavailable: this operating system has no supported secure mode; Linux and macOS require native qualification.".into())
}
/// No IPC approval Boolean: the host owns the question, text and direct native
/// user response. Approval is consumed by this exact caller, never cached/reused.
pub fn confirm(scope: &str) -> Result<(), workflow_engine::Error> {
    static CONFIRMATION: Mutex<()> = Mutex::new(());
    let _dialog = CONFIRMATION
        .lock()
        .map_err(|_| workflow_engine::Error::Busy {
            message: "Native confirmation unavailable; no authorization granted.".into(),
        })?;
    #[cfg(target_os = "macos")]
    return crate::mac_security::confirm(scope);
    #[cfg(not(target_os = "macos"))]
    let result = std::process::Command::new("/usr/bin/zenity")
        .args(["--question","--no-markup","--default-cancel","--title=Workflow Factory · trusted private scope","--text", &format!("Authorize only this operation on trusted private infrastructure and private trusted inputs (not public/open-source or untrusted workloads). Credentials are accessible to the workload. Do not hibernate or take memory snapshots while active. This mode does not defend against privileged host administrators. No paid fallback or host grant import.\n\n{scope}")])
        .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
    #[cfg(not(target_os = "macos"))]
    if result.is_ok_and(|s| s.success()) {
        Ok(())
    } else {
        Err(workflow_engine::Error::Validation{message:"Native confirmation declined/unavailable. Install the supported Linux zenity dialog; no authorization was granted.".into()})
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn c16_redacted_output_storage_failure_retains_exact_retry_payload() {
        let id = "prepared-output-test";
        let text = "validated [REDACTED] response";
        let pending = super::retain_output(id, text.into()).unwrap();
        assert!(pending.settle(Err(super::output_hold())).is_err());
        assert_eq!(
            super::outputs().lock().unwrap().get(id).map(String::as_str),
            Some(text)
        );
        let pending = super::retain_output(id, text.into()).unwrap();
        assert!(pending.settle(Ok(true)).unwrap());
        assert!(!super::outputs().lock().unwrap().contains_key(id));
        for (key, text) in [
            ("selected/a", "A durable"),
            ("selected/b", "B durable"),
            ("unrelated/a", "unrelated pending"),
        ] {
            super::retain_output(key, text.into()).unwrap();
        }
        let mut calls = Vec::new();
        super::retry_selected_outputs("selected", |run, block, text| {
            calls.push((
                run.to_string(),
                block.unwrap().to_string(),
                text.to_string(),
            ));
            Ok(())
        })
        .unwrap();
        assert_eq!(
            calls,
            vec![
                ("selected".into(), "a".into(), "A durable".into()),
                ("selected".into(), "b".into(), "B durable".into())
            ]
        );
        assert!(super::outputs().lock().unwrap().contains_key("unrelated/a"));
        assert!(
            super::retry_selected_outputs("unrelated", |_, _, _| Err(super::output_hold()))
                .is_err()
        );
        super::outputs().lock().unwrap().remove("unrelated/a");
    }
    #[test]
    fn c11_native_security_policy_has_supported_positive_and_specific_refusals() {
        use std::cell::Cell;
        for outcome in ["approved", "declined", "reservation-changed", "cancelled"] {
            let cancel = workflow_engine::sandbox::Cancel::default();
            let confirmations = Cell::new(0);
            let authorizations = Cell::new(0);
            let executions = Cell::new(0);
            let result = super::execute_confirmed(
                &cancel,
                || {
                    confirmations.set(confirmations.get() + 1);
                    if outcome == "cancelled" {
                        cancel.request();
                    }
                    if outcome == "declined" {
                        return Err(super::output_hold());
                    }
                    Ok(())
                },
                || {
                    authorizations.set(authorizations.get() + 1);
                    Ok(outcome != "reservation-changed")
                },
                || executions.set(executions.get() + 1),
            );
            assert_eq!(confirmations.get(), 1);
            assert_eq!(executions.get(), u32::from(outcome == "approved"));
            assert_eq!(
                authorizations.get(),
                u32::from(matches!(outcome, "approved" | "reservation-changed"))
            );
            assert_eq!(
                result.is_ok(),
                matches!(outcome, "approved" | "reservation-changed")
            );
        }
        let docker = r#"{"OperatingSystem":"Docker Desktop","OSType":"linux"}"#;
        assert!(crate::mac_security::observations(
            "FileVault is On.\n",
            docker
        ));
        assert!(!crate::mac_security::observations(
            "FileVault is Off.",
            docker
        ));
        assert!(!crate::mac_security::observations("FileVault is On.", "{}"));
        assert!(crate::mac_security::confirmation_script()
            .contains("default button \"Cancel\" cancel button \"Cancel\""));
        assert!(crate::mac_security::confirmation_script().contains("item 1 of argv"));
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
