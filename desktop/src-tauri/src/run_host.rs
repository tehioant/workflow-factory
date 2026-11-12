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
pub fn admission() -> Admission {
    let reason = if cfg!(target_os = "linux") {
        match std::fs::read_to_string("/proc/swaps") {
            Ok(swaps) if swaps.lines().skip(1).any(|s|!s.trim().is_empty()) => "Native paging observation: active swap. Protected host/daemon paging and dump policy is not established.",
            Ok(_) => "Native paging observation: no active host swap. Docker daemon/VM paging, dumps, native secure-store roundtrip and provider-only containment remain unqualified.",
            Err(_) => "Native paging observation unavailable. Secure staging cannot be established.",
        }
    } else {
        "This native platform has no established protected paging/dump and secure-store qualification. Execution withheld."
    };
    Admission{image_id:crate::runtime_host::readiness_with(&mut workflow_engine::runtime::RealProcess).image_id.unwrap_or_default(),blocker:Some(format!("secureStagingUnavailable: {reason} Trusted-private workload authorization and an independently app-acquired managed session are also required. No login, model call or fallback was attempted."))}
}
