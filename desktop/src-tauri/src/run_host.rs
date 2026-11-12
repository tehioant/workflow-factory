//! Native observations only. No IPC flag can establish platform qualification.
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};
use workflow_engine::runs::Admission;
struct Claim {
    finality: Mutex<Option<workflow_engine::runs::settlement::FinalSettlement>>,
    // 0: no failure; 1: cleanup unconfirmed; 2: positive cleanup receipt.
    persistence_failure: std::sync::atomic::AtomicU8,
}
impl std::ops::Deref for Claim {
    type Target = Mutex<Option<workflow_engine::runs::settlement::FinalSettlement>>;
    fn deref(&self) -> &Self::Target {
        &self.finality
    }
}
type FinalClaim = Arc<Claim>;
const PERSISTENCE_HOLD: &str = "Final persistence failed; chosen result retained in volatile app memory only. Keep the app open and use Retry retained output storage; no execution retry.";
fn persist_final(
    store: &workflow_engine::Store,
    id: &str,
    claim: &Claim,
    chosen: &workflow_engine::runs::settlement::FinalSettlement,
) -> Result<(), workflow_engine::Error> {
    let result = store
        .retain_final_settlement(chosen)
        .and_then(|()| {
            retry_selected_outputs(id, |id, block, text| match block {
                Some(block) => store.retain_block_output(id, block, text),
                None => store.retain_run_output(id, text),
            })
        })
        .and_then(|()| store.settle_run(chosen));
    if result.is_err() {
        claim.persistence_failure.store(
            if chosen.receipt.is_some() { 2 } else { 1 },
            std::sync::atomic::Ordering::Release,
        );
        let _ = store.hold_final_persistence(chosen, PERSISTENCE_HOLD);
    } else {
        claim
            .persistence_failure
            .store(0, std::sync::atomic::Ordering::Release);
    }
    result
}
fn final_claims() -> &'static Mutex<BTreeMap<String, FinalClaim>> {
    static CLAIMS: OnceLock<Mutex<BTreeMap<String, FinalClaim>>> = OnceLock::new();
    CLAIMS.get_or_init(Mutex::default)
}
fn final_claim(id: &str) -> Result<FinalClaim, workflow_engine::Error> {
    let mut claims = final_claims().lock().map_err(|_| output_hold())?;
    if let Some(claim) = claims.get(id) {
        return Ok(claim.clone());
    }
    if claims.len() >= 8 {
        return Err(output_hold());
    }
    let claim = Arc::new(Claim {
        finality: Mutex::new(None),
        persistence_failure: std::sync::atomic::AtomicU8::new(0),
    });
    claims.insert(id.into(), claim.clone());
    Ok(claim)
}
fn recoveries() -> &'static Mutex<BTreeMap<String, workflow_engine::sandbox::CleanupContext>> {
    static RECOVERY: OnceLock<Mutex<BTreeMap<String, workflow_engine::sandbox::CleanupContext>>> =
        OnceLock::new();
    RECOVERY.get_or_init(Mutex::default)
}
fn cleanup_hold() -> workflow_engine::Error {
    workflow_engine::Error::Validation {message:"Selected current-cache cleanup context unavailable or uncertain. Ownership remains held; no execution, resource adoption or secure-cache seed fallback is authorized. Keep the app open.".into()}
}
pub fn retry_cleanup(
    store: &workflow_engine::Store,
    selected: &str,
) -> Result<(), workflow_engine::Error> {
    retry_cleanup_with(
        store,
        selected,
        &mut workflow_engine::sandbox::Sandbox {
            pipe: workflow_engine::sandbox::NativePipe,
        },
        &mut crate::sessions_host::NativeSecureStore,
    )
}
fn retry_cleanup_with<P: workflow_engine::sandbox::Pipe>(
    store: &workflow_engine::Store,
    selected: &str,
    sandbox: &mut workflow_engine::sandbox::Sandbox<P>,
    secure: &mut impl workflow_engine::sessions::SecureStore,
) -> Result<(), workflow_engine::Error> {
    let existing = final_claims()
        .lock()
        .map_err(|_| cleanup_hold())?
        .get(selected)
        .cloned();
    if existing.is_none() && store.retained_final_settlement(selected)?.is_none() {
        store.recovery_lease(selected)?;
    }
    let claim = match existing {
        Some(claim) => claim,
        None => final_claim(selected)?,
    };
    let mut finality = claim.try_lock().map_err(|_| workflow_engine::Error::Busy {
        message: "Selected final settlement or cleanup retry is active.".into(),
    })?;
    if finality.is_none() {
        if let Some(chosen) = store.retained_final_settlement(selected)? {
            *finality = Some(chosen);
        } else {
            let lease = store.recovery_lease(selected)?;
            let status = store.run_status(selected)?;
            *finality = Some(workflow_engine::runs::settlement::FinalSettlement {
                lease,
                text: status.final_response.or_else(|| status.blocks.iter().rev().find_map(|b| b.final_response.clone())),
                diagnostic: "Interrupted execution reconciled; durable completed results retained. Uncommitted volatile output is unavailable. Session quarantined; no execution resumed.".into(),
                success: false,
                receipt: None,
            });
        }
    }
    let chosen = finality.as_mut().ok_or_else(cleanup_hold)?;
    if chosen.receipt.is_none() {
        // Best effort only: journal refusal must not block mandatory cache cleanup.
        let _ = store.retain_final_settlement(chosen);
        if let Some(receipt) = sandbox.finish_unstaged_cleanup(store, &chosen.lease)? {
            chosen.receipt = Some(receipt);
        } else {
            let mut context = recoveries()
                .lock()
                .map_err(|_| cleanup_hold())?
                .remove(selected)
                .map(Ok)
                .unwrap_or_else(|| {
                    store
                        .interrupted_recovery(selected)
                        .map(|(lease, resources)| {
                            workflow_engine::sandbox::CleanupContext::new(resources, lease)
                        })
                })?;
            let result =
                sandbox.finish_cleanup(store, &chosen.lease, &mut context, secure, chosen.success);
            recoveries()
                .lock()
                .map_err(|_| cleanup_hold())?
                .insert(selected.into(), context);
            chosen.receipt = Some(result?);
        }
    }
    persist_final(store, selected, &claim, chosen)?;
    *finality = None;
    recoveries()
        .lock()
        .map_err(|_| cleanup_hold())?
        .remove(selected);
    final_claims()
        .lock()
        .map_err(|_| cleanup_hold())?
        .remove(selected);
    Ok(())
}
fn outputs() -> &'static Mutex<BTreeMap<String, String>> {
    static OUTPUTS: OnceLock<Mutex<BTreeMap<String, String>>> = OnceLock::new();
    OUTPUTS.get_or_init(Mutex::default)
}
fn output_hold() -> workflow_engine::Error {
    workflow_engine::Error::Storage { message: "Completed redacted output retained in app memory; storage failed. Ownership remains held. Keep the app open and use Retry retained output storage; no execution retry.".into() }
}
struct PendingOutput(String);
fn retain_output(id: &str, text: String) -> Result<PendingOutput, workflow_engine::Error> {
    if text.len() > 262144 {
        return Err(output_hold());
    }
    let mut pending = outputs().lock().map_err(|_| output_hold())?;
    let run = id.split('/').next().unwrap_or(id);
    if !pending.keys().any(|key| key.split('/').next() == Some(run))
        && pending
            .keys()
            .map(|key| key.split('/').next())
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            >= 8
    {
        return Err(output_hold());
    }
    if pending.get(id).is_some_and(|previous| previous != &text) {
        return Err(output_hold());
    }
    pending.insert(id.into(), text);
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
/// Ordinary live-status observer: read-only, with no store bootstrap or recovery.
pub fn live_status(
    reader: &workflow_engine::runs::history::HistoryReader,
    id: Option<&str>,
) -> Result<Option<workflow_engine::runs::RunStatus>, workflow_engine::Error> {
    let mut status = reader.run_status(id)?;
    if let Some(status) = status.as_mut() {
        overlay_pending(
            &status.id,
            &mut status.state,
            &mut status.diagnostic,
            &mut status.cleanup_pending,
        )?;
    }
    Ok(status)
}
fn overlay_pending(
    id: &str,
    state: &mut workflow_engine::runs::RunState,
    diagnostic: &mut String,
    cleanup: &mut bool,
) -> Result<(), workflow_engine::Error> {
    use workflow_engine::runs::RunState;
    // Never lock the per-run finality: secure-store/Docker work may hold it.
    let pending = final_claims()
        .lock()
        .map_err(|_| output_hold())?
        .get(id)
        .map_or(0, |claim| {
            claim
                .persistence_failure
                .load(std::sync::atomic::Ordering::Acquire)
        });
    if pending != 0
        && !matches!(
            state,
            RunState::Succeeded | RunState::Failed | RunState::Stopped | RunState::Blocked
        )
    {
        if *state != RunState::Cancelling {
            *state = RunState::Held;
        }
        *diagnostic = PERSISTENCE_HOLD.into();
        *cleanup = pending == 1;
    }
    Ok(())
}
pub fn live_queue(
    reader: &workflow_engine::runs::history::HistoryReader,
) -> Result<workflow_engine::runs::queue::QueueStatus, workflow_engine::Error> {
    let mut queue = reader.queue_status()?;
    for entry in &mut queue.entries {
        overlay_pending(
            &entry.id,
            &mut entry.state,
            &mut entry.diagnostic,
            &mut entry.cleanup_pending,
        )?;
    }
    Ok(queue)
}
pub fn retry_outputs(
    store: &workflow_engine::Store,
    selected: &str,
) -> Result<(), workflow_engine::Error> {
    let claim = final_claims()
        .lock()
        .map_err(|_| output_hold())?
        .get(selected)
        .cloned();
    let Some(claim) = claim else {
        retry_selected_outputs(selected, |id, block, text| match block {
            Some(block) => store.retain_block_output(id, block, text),
            None => store.retain_run_output(id, text),
        })?;
        if let Some(chosen) = store.retained_final_settlement(selected)? {
            return store.settle_run(&chosen);
        }
        let status = store.run_status(selected)?;
        if status.cleanup_pending || status.state == workflow_engine::runs::RunState::Held {
            return Err(cleanup_hold());
        }
        return Ok(());
    };
    let mut finality = claim.lock().map_err(|_| output_hold())?;
    if let Some(receipt) = finality.as_ref() {
        persist_final(store, selected, &claim, receipt)?;
        *finality = None;
        recoveries()
            .lock()
            .map_err(|_| output_hold())?
            .remove(selected);
        final_claims()
            .lock()
            .map_err(|_| output_hold())?
            .remove(selected);
    } else {
        retry_selected_outputs(selected, |id, block, text| match block {
            Some(block) => store.retain_block_output(id, block, text),
            None => store.retain_run_output(id, text),
        })?;
    }
    Ok(())
}
fn retry_selected_outputs(
    selected: &str,
    mut persist: impl FnMut(&str, Option<&str>, &str) -> Result<(), workflow_engine::Error>,
) -> Result<(), workflow_engine::Error> {
    let pending: Vec<_> = outputs()
        .lock()
        .map_err(|_| output_hold())?
        .iter()
        .filter(|(key, _)| key.split('/').next() == Some(selected))
        .map(|(key, text)| (key.clone(), text.clone()))
        .collect();
    let mut failed = false;
    for (key, text) in pending {
        let (id, block) = key
            .split_once('/')
            .map_or((key.as_str(), None), |(id, block)| (id, Some(block)));
        if id != selected {
            continue;
        }
        let keep = persist(id, block, &text).is_err();
        failed |= keep;
        if !keep {
            let mut outputs = outputs().lock().map_err(|_| output_hold())?;
            if outputs.get(&key) == Some(&text) {
                outputs.remove(&key);
            }
        }
    }
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
    let Ok(claim) = final_claim(&id) else {
        return;
    };
    let mut sandbox = Sandbox { pipe: NativePipe };
    let mut secure = crate::sessions_host::NativeSecureStore;
    let mut recovery = None;
    let mut resource_attempted = false;
    let mut staging_attempted = false;
    let result = (|| {
        native_prerequisites().map_err(|message| workflow_engine::Error::Validation { message })?;
        cancel.check()?;
        store.retain_recovery_resources(&lease, None)?;
        let mut resources = sandbox.create_for_mode(
            &id,
            &snapshot.image_id,
            snapshot.definition.definition.isolation,
            &cancel,
            |kind, resource| {
                resource_attempted = true;
                store.resource_intent(&id, kind, resource)
            },
        )?;
        store.retain_recovery_resources(&lease, Some(&resources))?;
        recovery = Some(workflow_engine::sandbox::CleanupContext::new(
            resources.clone(),
            lease.clone(),
        ));
        native_prerequisites().map_err(|message| workflow_engine::Error::Validation { message })?;
        store.recovery_phase(&lease, "staging")?;
        staging_attempted = true;
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
        let mut last_text = None;
        let mut storage_interrupted = false;
        let final_result = loop {
            if completed > 0 && completed < snapshot.plans.len() {
                store.recovery_phase(&lease, "handoff")?;
                resources = sandbox.switch_block_environment_recoverable(
                    &store,
                    &lease,
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
                store.retain_recovery_resources(&lease, Some(&resources))?;
                recovery = Some(workflow_engine::sandbox::CleanupContext::new(
                    resources.clone(),
                    lease.clone(),
                ));
            }
            let mut pending_output = None;
            let mut current_valid_output = false;
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
                        match sandbox.checkpoint_cleanup(recovery.as_mut().ok_or_else(output_hold)?) {
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
                        current_valid_output = true;
                        last_text = Some(text.clone());
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
                    if completed == snapshot.plans.len() {
                        break Ok(last_text.clone().unwrap_or_default());
                    }
                }
                Ok(false) => break Ok(last_text.clone().unwrap_or_default()),
                Err(error @ workflow_engine::Error::Storage { .. }) => {
                    if !cache_ready {
                        return Err(error);
                    }
                    storage_interrupted = true;
                    if completed + 1 == snapshot.plans.len() && current_valid_output {
                        break Ok(last_text.clone().unwrap());
                    }
                    break Err(error);
                }
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
        // The per-run claim serializes the producer with explicit storage replay.
        // No global map lock is held across secure-store, cleanup or SQLite I/O.
        let mut finality = claim.lock().map_err(|_| output_hold())?;
        let success = final_result.is_ok();
        let diagnostic = if success {
            if artifact_result.is_ok() {
                "Completed; immutable sanitized artifacts captured; cache persisted and resources removed."
            } else {
                "Completed; artifacts unavailable or incomplete; cache persisted and resources removed."
            }
        } else if storage_interrupted {
            "Block output storage interrupted the pipeline; no successor executed. Current cache persisted and resources removed; session quarantined."
        } else {
            "Provider execution or output validation failed. Current cache persisted and resources removed; session quarantined. Explicit reauthentication required; no retry or failover."
        };
        *finality = Some(workflow_engine::runs::settlement::FinalSettlement {
            lease: lease.clone(),
            text: last_text.clone(),
            diagnostic: diagnostic.into(),
            success,
            receipt: None,
        });
        // Failure to journal the choice must not prevent mandatory credential cleanup.
        let _ = store.retain_final_settlement(finality.as_ref().unwrap());
        let receipt = sandbox.finish_cleanup(
            &store,
            &lease,
            recovery.as_mut().ok_or_else(output_hold)?,
            &mut secure,
            success,
        )?;
        finality.as_mut().unwrap().receipt = Some(receipt);
        // Replay pending text first. This grants storage authority only, never execution.
        persist_final(&store, &id, &claim, finality.as_ref().unwrap())?;
        *finality = None;
        final_claims()
            .lock()
            .map_err(|_| output_hold())?
            .remove(&id);
        Ok(())
    })();
    if result.is_err() {
        // Preserve only actual current native context, never the secure-store seed.
        if let Ok(mut finality) = claim.lock() {
            if finality.is_none() {
                *finality = Some(workflow_engine::runs::settlement::FinalSettlement {
                    lease: lease.clone(),
                    text: store.run_status(&id).ok().and_then(|s| s.final_response),
                    diagnostic:
                        "Execution failed; current-cache recovery completed; session quarantined."
                            .into(),
                    success: false,
                    receipt: None,
                });
            }
            if let Some(context) = recovery {
                if let Ok(mut contexts) = recoveries().lock() {
                    contexts.insert(id.clone(), context);
                }
            }
        }
        if claim
            .lock()
            .is_ok_and(|finality| finality.as_ref().is_some_and(|f| f.receipt.is_some()))
        {
            return;
        }
        let _ = store.quarantine_session(&lease);
        let scope = if staging_attempted {
            "Credential staging attempted; current cache/writers must be positively recovered. Ownership remains held until verified cleanup. No execution retry."
        } else if resource_attempted {
            "No credential staging or provider execution attempted. Resource creation or handoff uncertain; unrecorded resources will not be adopted. Ownership held."
        } else {
            "No resource creation, credential staging or provider execution attempted. Current-cache cleanup context unavailable; ownership held without seed fallback."
        };
        let _ = store.finish_run(&id, None, scope, false, false);
        // Mandatory bounded recovery is independent of the execution Stop token.
        // The same chosen settlement and private context are used by explicit retry.
        let _ = retry_cleanup(&store, &id);
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
pub fn shutdown_pending() -> Result<bool, workflow_engine::Error> {
    // These registries retain failed chosen settlements even if hold storage failed.
    Ok(!outputs().try_lock().map_err(|_| output_hold())?.is_empty()
        || !final_claims()
            .try_lock()
            .map_err(|_| output_hold())?
            .is_empty()
        || !recoveries()
            .try_lock()
            .map_err(|_| cleanup_hold())?
            .is_empty())
}

#[cfg(test)]
mod tests {
    static FINALITY_TEST: std::sync::Mutex<()> = std::sync::Mutex::new(());
    #[test]
    fn d14_selected_interrupted_native_cleanup_uses_exact_durable_provenance() {
        let _serial = FINALITY_TEST.lock().unwrap();
        use workflow_engine::{
            sandbox::{Cancel, Pipe, Sandbox},
            sessions::SecureStore,
        };
        struct Absent;
        impl Pipe for Absent {
            fn call(
                &mut self,
                a: &[String],
                input: &[u8],
                limit: usize,
                _: &Cancel,
            ) -> Result<(i32, Vec<u8>, Vec<u8>), workflow_engine::Error> {
                assert!(input.is_empty());
                assert_eq!(limit, 131072);
                let a: Vec<_> = a.iter().map(String::as_str).collect();
                match a.as_slice() {
                    ["container", "ls", "-a", "--no-trunc", "--filter", f, "--format", "{{.ID}}"]
                    | ["network", "ls", "--no-trunc", "--filter", f, "--format", "{{.ID}}"] => {
                        let id = f.strip_prefix("id=").unwrap();
                        assert!([
                            "a".repeat(64),
                            "b".repeat(64),
                            "c".repeat(64),
                            "d".repeat(64)
                        ]
                        .iter()
                        .any(|known| known == id));
                    }
                    _ => panic!("unexpected native recovery process {a:?}"),
                };
                Ok((0, vec![], vec![]))
            }
        }
        struct Secure(Vec<u8>, usize, usize);
        impl SecureStore for Secure {
            fn read(&mut self, slot: &str) -> Result<Vec<u8>, workflow_engine::Error> {
                assert_eq!(slot, "selected-slot");
                self.1 += 1;
                Ok(self.0.clone())
            }
            fn write(&mut self, slot: &str, b: &[u8]) -> Result<(), workflow_engine::Error> {
                assert_eq!(slot, "selected-slot");
                assert_eq!(b, self.0);
                self.2 += 1;
                Ok(())
            }
        }
        let root = std::env::temp_dir().join(format!("d14-selected-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let store = workflow_engine::Store::open(&root).unwrap();
        let c = rusqlite::Connection::open(root.join("definitions.sqlite3")).unwrap();
        let id = "00000000-0000-4000-8000-000000000137";
        let cache=br#"{"auth_mode":"chatgpt","tokens":{"login_owner_id":"synthetic","access_token":"synthetic-current-a","refresh_token":"synthetic-current-r","id_token":"a.b.c","account_id":"synthetic-account","oauth_provenance":{"issuer":"https://auth.openai.com","client_id":"synthetic-client"}}}"#.to_vec();
        let lease = workflow_engine::sessions::Lease {
            owner: id.into(),
            slot: "selected-slot".into(),
            generation: 7,
        };
        let r = workflow_engine::sandbox::Resources {
            owner: id.into(),
            image: format!("sha256:{}", "f".repeat(64)),
            network: "a".repeat(64),
            outer: "b".repeat(64),
            helper: "c".repeat(64),
            container: "d".repeat(64),
            workspace: None,
            peer: None,
            started_at: Some("prepared-epoch".into()),
            retired: None,
        };
        let status = serde_json::json!({"id":id,"requestId":"selected","state":"held","cleanupPending":true,"diagnostic":"interrupted","finalResponse":"durable output","blocks":[]});
        c.execute(
            "INSERT INTO runs VALUES(?1,'selected','{}','{}',?2,1)",
            rusqlite::params![id, status.to_string()],
        )
        .unwrap();
        c.execute(
            "INSERT INTO managed_sessions VALUES('selected-slot','quarantined',7,?1)",
            [id],
        )
        .unwrap();
        c.execute("INSERT INTO run_queue(run_id,phase,slot,generation) VALUES(?1,'occupied','selected-slot',7)",[id]).unwrap();
        for (kind, value) in [
            ("work-container", &r.container),
            ("egress-helper", &r.helper),
            ("private-network", &r.network),
            ("egress-network", &r.outer),
        ] {
            c.execute(
                "INSERT INTO run_resources VALUES(?1,?2,?3)",
                rusqlite::params![id, kind, value],
            )
            .unwrap();
        }
        store.retain_recovery_resources(&lease, Some(&r)).unwrap();
        let chosen = workflow_engine::runs::settlement::FinalSettlement {
            lease: lease.clone(),
            text: Some("durable output".into()),
            diagnostic: "prepared interrupted finality".into(),
            success: false,
            receipt: None,
        };
        store.retain_final_settlement(&chosen).unwrap();
        let mut metadata = serde_json::to_value(&r).unwrap();
        metadata["phase"] = "cache-intent".into();
        metadata["cacheDigest"] =
            "39c34db2142972e8192a347f77393932311e6957f3561727fe812291451aa2c4".into();
        c.execute(
            "UPDATE run_recovery SET resources=?1 WHERE run_id=?2",
            rusqlite::params![metadata.to_string(), id],
        )
        .unwrap();
        let mut sandbox = Sandbox { pipe: Absent };
        let mut secure = Secure(cache, 0, 0);
        super::retry_cleanup_with(&store, id, &mut sandbox, &mut secure).unwrap();
        assert_eq!(
            c.query_row("SELECT active FROM runs WHERE id=?1", [id], |r| r
                .get::<_, i32>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            c.query_row(
                "SELECT state FROM managed_sessions WHERE id='selected-slot'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "quarantined"
        );
        struct ForbiddenSecure;
        impl SecureStore for ForbiddenSecure {
            fn read(&mut self, _: &str) -> Result<Vec<u8>, workflow_engine::Error> {
                panic!("storage replay read secure cache")
            }
            fn write(&mut self, _: &str, _: &[u8]) -> Result<(), workflow_engine::Error> {
                panic!("storage replay wrote secure cache")
            }
        }
        struct Forbidden;
        impl Pipe for Forbidden {
            fn call(
                &mut self,
                _: &[String],
                _: &[u8],
                _: usize,
                _: &Cancel,
            ) -> Result<(i32, Vec<u8>, Vec<u8>), workflow_engine::Error> {
                panic!("storage replay touched native process")
            }
        }
        super::retry_cleanup_with(
            &store,
            id,
            &mut Sandbox { pipe: Forbidden },
            &mut ForbiddenSecure,
        )
        .unwrap();
        // Independent prepared publication windows: no execute/admit/stage precursor.
        for retain_memory in [true, false] {
            c.execute(
                "UPDATE runs SET status=?2,active=1 WHERE id=?1",
                rusqlite::params![id, status.to_string()],
            )
            .unwrap();
            c.execute("UPDATE managed_sessions SET state='quarantined',owner=?1,generation=7 WHERE id='selected-slot'", [id]).unwrap();
            c.execute(
                "UPDATE run_queue SET phase='occupied' WHERE run_id=?1",
                [id],
            )
            .unwrap();
            c.execute("DELETE FROM run_resources WHERE run_id=?1 AND kind NOT IN ('work-container','egress-helper','private-network','egress-network')", [id]).unwrap();
            c.execute(
                "UPDATE run_recovery SET resources=?2,finality=NULL WHERE run_id=?1",
                rusqlite::params![id, metadata.to_string()],
            )
            .unwrap();
            let chosen = workflow_engine::runs::settlement::FinalSettlement {
                lease: lease.clone(),
                text: Some("durable output".into()),
                diagnostic: "chosen same-process success".into(),
                success: true,
                receipt: None,
            };
            c.execute_batch("CREATE TRIGGER refuse_choice BEFORE UPDATE OF finality ON run_recovery BEGIN SELECT RAISE(FAIL,'synthetic chosen journal refusal'); END;").unwrap();
            assert!(store.retain_final_settlement(&chosen).is_err());
            let writes = secure.2;
            let reads = secure.1;
            let result = sandbox.finish_cleanup(
                &store,
                &lease,
                &mut workflow_engine::sandbox::CleanupContext::new(r.clone(), lease.clone()),
                &mut secure,
                true,
            );
            assert!(
                secure.2 > writes && secure.1 >= reads + 3,
                "mandatory current-cache write/readback must run"
            );
            let removals: i64 = c.query_row("SELECT count(*) FROM run_resources WHERE run_id=?1 AND kind IN ('final-agent-remove','helper-remove','private-network-remove','egress-network-remove') AND resource_id IS NOT NULL", [id], |row| row.get(0)).unwrap();
            assert_eq!(
                removals, 4,
                "positive exact-ID cleanup receipts must survive journal refusal"
            );
            assert!(
                result.is_err(),
                "missing durable chosen finality must forbid reusable publication after cleanup"
            );
            assert!(store.retained_final_settlement(id).unwrap().is_none());
            let held: (String, String, u32) = c
                .query_row(
                    "SELECT state,owner,generation FROM managed_sessions WHERE id='selected-slot'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap();
            assert_eq!(held, ("quarantined".into(), id.into(), 7));
            assert_eq!(
                c.query_row("SELECT phase FROM run_queue WHERE run_id=?1", [id], |row| {
                    row.get::<_, String>(0)
                })
                .unwrap(),
                "occupied"
            );
            assert_eq!(
                c.query_row(
                    "SELECT resources FROM run_recovery WHERE run_id=?1",
                    [id],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
                metadata.to_string()
            );
            c.execute_batch("DROP TRIGGER refuse_choice;").unwrap();
            if retain_memory {
                *super::final_claim(id).unwrap().lock().unwrap() = Some(chosen.clone());
            }
            super::retry_cleanup_with(&store, id, &mut sandbox, &mut secure).unwrap();
            let settled = store.retained_final_settlement(id).unwrap().unwrap();
            assert_eq!(settled.success, retain_memory);
            assert_eq!(settled.text.as_deref(), Some("durable output"));
            assert!(settled.receipt.is_some());
            if retain_memory {
                assert_eq!(settled.diagnostic, chosen.diagnostic);
            } else {
                assert!(settled
                    .diagnostic
                    .contains("Uncommitted volatile output is unavailable"));
            }
            let disposition: (String, u32) = c
                .query_row(
                    "SELECT state,generation FROM managed_sessions WHERE id='selected-slot'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(
                disposition,
                if retain_memory {
                    ("available".into(), 8)
                } else {
                    ("quarantined".into(), 7)
                }
            );
            assert_eq!(
                c.query_row("SELECT phase FROM run_queue WHERE run_id=?1", [id], |row| {
                    row.get::<_, String>(0)
                })
                .unwrap(),
                "settled"
            );
            super::retry_cleanup_with(
                &store,
                id,
                &mut Sandbox { pipe: Forbidden },
                &mut ForbiddenSecure,
            )
            .unwrap();
            super::retry_outputs(&store, id).unwrap();
        }
        let partial = "00000000-0000-4000-8000-000000000138";
        c.execute("INSERT INTO runs VALUES(?1,'partial','{}','{}',?2,1)",rusqlite::params![partial,serde_json::json!({"id":partial,"requestId":"partial","state":"held","cleanupPending":true,"diagnostic":"partial","finalResponse":null,"blocks":[]}).to_string()]).unwrap();
        c.execute(
            "INSERT INTO managed_sessions VALUES('partial-slot','quarantined',8,?1)",
            [partial],
        )
        .unwrap();
        c.execute("INSERT INTO run_queue(run_id,phase,slot,generation) VALUES(?1,'occupied','partial-slot',8)",[partial]).unwrap();
        let partial_lease = workflow_engine::sessions::Lease {
            owner: partial.into(),
            slot: "partial-slot".into(),
            generation: 8,
        };
        store
            .retain_recovery_resources(&partial_lease, None)
            .unwrap();
        c.execute(
            "INSERT INTO run_resources VALUES(?1,'private-network',?2)",
            rusqlite::params![partial, "a".repeat(64)],
        )
        .unwrap();
        c.execute(
            "INSERT INTO run_resources VALUES(?1,'egress-network',NULL)",
            [partial],
        )
        .unwrap();
        assert!(super::retry_cleanup_with(&store, partial, &mut sandbox, &mut secure).is_err());
        c.execute(
            "DELETE FROM run_resources WHERE run_id=?1 AND kind='egress-network'",
            [partial],
        )
        .unwrap();
        super::retry_cleanup_with(&store, partial, &mut sandbox, &mut secure).unwrap();
        assert_eq!(
            c.query_row("SELECT active FROM runs WHERE id=?1", [partial], |r| r
                .get::<_, i32>(0))
                .unwrap(),
            0
        );
        c.execute(
            "UPDATE run_recovery SET finality=?1 WHERE run_id=?2",
            rusqlite::params!["x".repeat(270337), id],
        )
        .unwrap();
        assert!(store.retained_final_settlement(id).is_err());
        drop(c);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn c16_redacted_output_storage_failure_retains_exact_retry_payload() {
        let _serial = FINALITY_TEST.lock().unwrap();
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
        // Prepared persistence boundary through the existing explicit-retry consumer.
        // NativeSecureStore, Docker, confirmation and preflight are not constructed.
        let root = std::env::temp_dir().join(format!("settlement-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let store = workflow_engine::Store::open(&root).unwrap();
        let db = rusqlite::Connection::open(root.join("definitions.sqlite3")).unwrap();
        let status = serde_json::json!({"id":"00000000-0000-4000-8000-000000000035","requestId":"prepared","state":"running","finalResponse":null,"diagnostic":"","cleanupPending":false,"blocks":[{"blockId":"a","state":"running","resolvedPrompt":"prepared literal","finalResponse":null}]});
        let request = serde_json::json!({"requestId":"00000000-0000-4000-8000-000000000037","definitionId":"00000000-0000-4000-8000-000000000036","expectedRevision":1,"expectedConfigurationRevision":1,"supplied":{}});
        let definition: workflow_engine::Definition = serde_json::from_value(serde_json::json!({"schemaVersion":1,"id":"00000000-0000-4000-8000-000000000036","name":"Prepared","blocks":[{"id":"a","name":"A","prompt":"prepared literal","position":{"x":0,"y":0},"configurationId":"00000000-0000-4000-8000-000000000038"}]})).unwrap();
        let configuration: workflow_engine::SavedConfiguration = serde_json::from_value(serde_json::json!({"revision":1,"config":{"id":"00000000-0000-4000-8000-000000000038","name":"Prepared","model":"codex","reasoning":"high"}})).unwrap();
        let inputs = workflow_engine::resolve_inputs(&definition, Default::default()).unwrap();
        let segments = workflow_engine::prompts::compile(&definition, "a", &inputs.values).unwrap();
        let snapshot = serde_json::json!({"definition":{"revision":1,"definition":definition},"configuration":configuration,"inputs":inputs,"imageId":"prepared-image","plans":[{"block_id":"a","segments":segments,"configuration":configuration}]});
        let mut status = status;
        status["requestId"] = request["requestId"].clone();
        db.execute(
            "INSERT INTO runs VALUES('00000000-0000-4000-8000-000000000035',?1,?2,?3,?4,1)",
            rusqlite::params![
                request["requestId"].as_str().unwrap(),
                request.to_string(),
                snapshot.to_string(),
                status.to_string()
            ],
        )
        .unwrap();
        db.execute(
            "INSERT INTO managed_sessions VALUES('native-slot','leased',7,'00000000-0000-4000-8000-000000000035')",
            [],
        )
        .unwrap();
        db.execute("INSERT INTO run_queue(run_id,phase,slot,generation) VALUES('00000000-0000-4000-8000-000000000035','occupied','native-slot',7)",[]).unwrap();
        let lease = workflow_engine::sessions::Lease {
            slot: "native-slot".into(),
            owner: "00000000-0000-4000-8000-000000000035".into(),
            generation: 7,
        };
        let cache=br#"{"auth_mode":"chatgpt","tokens":{"login_owner_id":"synthetic","access_token":"synthetic-a","refresh_token":"synthetic-r","id_token":"a.b.c","account_id":"synthetic-account","oauth_provenance":{"issuer":"https://auth.openai.com","client_id":"synthetic-client"}}}"#.to_vec();
        struct Secure(Vec<u8>, usize);
        impl workflow_engine::sessions::SecureStore for Secure {
            fn read(&mut self, slot: &str) -> Result<Vec<u8>, workflow_engine::Error> {
                assert_eq!(slot, "native-slot");
                self.1 += 1;
                Ok(self.0.clone())
            }
            fn write(&mut self, slot: &str, bytes: &[u8]) -> Result<(), workflow_engine::Error> {
                assert_eq!(slot, "native-slot");
                self.0 = bytes.to_vec();
                Ok(())
            }
        }
        let mut secure = Secure(cache.clone(), 0);
        store
            .retain_final_settlement(&workflow_engine::runs::settlement::FinalSettlement {
                lease: lease.clone(),
                text: Some(text.into()),
                diagnostic: "Prepared completed outcome".into(),
                success: true,
                receipt: None,
            })
            .unwrap();
        let receipt = store
            .finish_owned_session_receipt(&lease, &cache, &mut secure, true, || Ok(()))
            .unwrap();
        assert_eq!(secure.1, 2, "previous cache plus exact writeback readback");
        let saved_receipt = receipt.clone();
        super::retain_output("00000000-0000-4000-8000-000000000035/a", text.into()).unwrap();
        super::retain_output("native-sibling/a", "sibling untouched".into()).unwrap();
        let claim = super::final_claim("00000000-0000-4000-8000-000000000035").unwrap();
        {
            let _held = claim.lock().unwrap();
            assert!(super::retry_cleanup(&store, "00000000-0000-4000-8000-000000000035").is_err());
        }
        assert!(super::retry_cleanup(&store, "native-missing-context").is_err());
        *claim.lock().unwrap() = Some(workflow_engine::runs::settlement::FinalSettlement {
            lease,
            text: Some(text.into()),
            diagnostic: "Prepared completed outcome".into(),
            success: true,
            receipt: Some(receipt),
        });
        db.execute_batch("CREATE TRIGGER deny_final BEFORE UPDATE ON runs WHEN NEW.active=0 BEGIN SELECT RAISE(FAIL,'synthetic final database failure'); END;").unwrap();
        db.execute_batch("CREATE TRIGGER deny_hold BEFORE UPDATE ON runs WHEN json_extract(NEW.status,'$.state')='held' BEGIN SELECT RAISE(FAIL,'synthetic hold failure'); END;").unwrap();
        assert!(super::retry_outputs(&store, "00000000-0000-4000-8000-000000000035").is_err());
        assert!(claim.lock().unwrap().as_ref().unwrap().receipt.is_some());
        assert_eq!(
            store
                .run_status("00000000-0000-4000-8000-000000000035")
                .unwrap()
                .final_response
                .as_deref(),
            Some(text)
        );
        let reader = workflow_engine::runs::history::HistoryReader::open(&root).unwrap();
        let observed = super::live_status(&reader, Some("00000000-0000-4000-8000-000000000035"))
            .unwrap()
            .unwrap();
        assert_eq!(
            observed.state,
            workflow_engine::runs::RunState::Held,
            "SPEC-35-01 ordinary production live observer must disclose final persistence failure"
        );
        assert!(
            !observed.cleanup_pending,
            "positive receipt must not become uncertain cleanup"
        );
        assert!(observed.diagnostic.contains("volatile"));
        assert!(observed.diagnostic.contains("Keep the app open"));
        assert_eq!(observed.request_id, request["requestId"].as_str().unwrap());
        let raw = || {
            db.query_row(
                "SELECT status,active FROM runs WHERE id='00000000-0000-4000-8000-000000000035'",
                [],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?)),
            )
            .unwrap()
        };
        let before = raw();
        assert_eq!(
            store.run_status(&observed.id).unwrap().state,
            workflow_engine::runs::RunState::Running
        );
        let claims_before = super::final_claims().lock().unwrap().len();
        // Polls cannot erase failure or wait behind finality/native work.
        let chosen = claim.lock().unwrap();
        let chosen_bytes = serde_json::json!({"text":chosen.as_ref().unwrap().text,"diagnostic":chosen.as_ref().unwrap().diagnostic,"success":chosen.as_ref().unwrap().success}).to_string();
        for _ in 0..3 {
            assert_eq!(
                super::live_status(&reader, None).unwrap().unwrap().state,
                workflow_engine::runs::RunState::Held
            );
            let queue = super::live_queue(&reader).unwrap();
            let entry = queue.entries.iter().find(|e| e.id == observed.id).unwrap();
            assert_eq!(entry.state, workflow_engine::runs::RunState::Held);
            assert!(!entry.cleanup_pending);
            assert_eq!(entry.diagnostic, observed.diagnostic);
            assert_eq!(queue.occupied, 1);
        }
        assert_eq!(before, raw(), "ordinary observations write nothing");
        assert_eq!(claims_before, super::final_claims().lock().unwrap().len());
        drop(chosen);
        db.execute_batch("DROP TRIGGER deny_hold").unwrap();
        assert!(super::retry_outputs(&store, &observed.id).is_err());
        assert_eq!(
            store.run_status(&observed.id).unwrap().state,
            workflow_engine::runs::RunState::Held,
            "best-effort durable hold"
        );
        let after = claim.lock().unwrap();
        assert_eq!(chosen_bytes, serde_json::json!({"text":after.as_ref().unwrap().text,"diagnostic":after.as_ref().unwrap().diagnostic,"success":after.as_ref().unwrap().success}).to_string());
        drop(after);
        db.execute(
            "UPDATE runs SET status=json_set(status,'$.state','cancelling') WHERE id=?1",
            [&observed.id],
        )
        .unwrap();
        assert_eq!(
            super::live_status(&reader, Some(&observed.id))
                .unwrap()
                .unwrap()
                .state,
            workflow_engine::runs::RunState::Cancelling
        );
        db.execute(
            "UPDATE runs SET status=json_set(status,'$.state','held') WHERE id=?1",
            [&observed.id],
        )
        .unwrap();
        db.execute_batch("DROP TRIGGER deny_final").unwrap();
        super::retry_outputs(&store, &observed.id).unwrap();
        assert_eq!(
            claim
                .persistence_failure
                .load(std::sync::atomic::Ordering::Acquire),
            0
        );
        assert_eq!(
            super::live_status(&reader, Some(&observed.id))
                .unwrap()
                .unwrap()
                .diagnostic,
            "Prepared completed outcome"
        );
        let retained = store
            .retained_final_settlement(&observed.id)
            .unwrap()
            .unwrap();
        assert_eq!(retained.text.as_deref(), Some(text));
        assert!(retained.receipt.is_some());
        // No volatile final claim: exact durable storage replay needs no native adapter.
        super::retry_outputs(&store, &observed.id).unwrap();
        assert_eq!(
            store
                .run_status("00000000-0000-4000-8000-000000000035")
                .unwrap()
                .state,
            workflow_engine::runs::RunState::Succeeded
        );
        assert!(!super::final_claims()
            .lock()
            .unwrap()
            .contains_key("00000000-0000-4000-8000-000000000035"));
        // Exact lost-ack replay retains the durable winner and clears only its own hold.
        let replay = super::final_claim(&observed.id).unwrap();
        *replay.lock().unwrap() = Some(workflow_engine::runs::settlement::FinalSettlement {
            lease: workflow_engine::sessions::Lease {
                slot: "native-slot".into(),
                owner: observed.id.clone(),
                generation: 7,
            },
            text: Some(text.into()),
            diagnostic: "Prepared completed outcome".into(),
            success: true,
            receipt: None,
        });
        // Use the positive receipt retained before successful replay, not reconstructed proof.
        replay.lock().unwrap().as_mut().unwrap().receipt = Some(saved_receipt);
        replay
            .persistence_failure
            .store(2, std::sync::atomic::Ordering::Release);
        assert_eq!(
            super::live_status(&reader, None).unwrap().unwrap().state,
            workflow_engine::runs::RunState::Succeeded
        );
        super::retry_outputs(&store, &observed.id).unwrap();
        assert_eq!(
            replay
                .persistence_failure
                .load(std::sync::atomic::Ordering::Acquire),
            0
        );
        assert!(super::outputs()
            .lock()
            .unwrap()
            .contains_key("native-sibling/a"));
        super::outputs().lock().unwrap().remove("native-sibling/a");
        // Text alone cannot manufacture the positive cleanup receipt.
        let claim = super::final_claim("00000000-0000-4000-8000-000000000035").unwrap();
        *claim.lock().unwrap() = Some(workflow_engine::runs::settlement::FinalSettlement {
            lease: workflow_engine::sessions::Lease {
                slot: "native-slot".into(),
                owner: "00000000-0000-4000-8000-000000000035".into(),
                generation: 7,
            },
            text: Some(text.into()),
            diagnostic: "uncertain".into(),
            success: false,
            receipt: None,
        });
        assert!(super::retry_outputs(&store, "00000000-0000-4000-8000-000000000035").is_err());
        assert_eq!(
            super::live_status(&reader, None).unwrap().unwrap().state,
            workflow_engine::runs::RunState::Succeeded,
            "durable terminal winner cannot be downgraded"
        );
        assert_ne!(
            claim
                .persistence_failure
                .load(std::sync::atomic::Ordering::Acquire),
            0,
            "failed non-exact replay must not clear hold"
        );
        super::final_claims()
            .lock()
            .unwrap()
            .remove("00000000-0000-4000-8000-000000000035");
        db.execute("UPDATE runs SET snapshot='{}'", []).unwrap();
        assert!(
            super::live_status(&reader, None).is_err(),
            "history failure is not an empty status"
        );
        drop(reader);
        drop(db);
        drop(store);
        std::fs::remove_dir_all(root).unwrap();
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
