use std::sync::Mutex;
use workflow_engine::{
    sandbox::{NativePipe, Resources, Sandbox},
    sessions::{Lease, SignInStatus},
    Error, Store,
};
pub struct SignIn(pub Mutex<Option<Pending>>);
pub struct Pending {
    resources: Resources,
    lease: Lease,
}
fn status(state: &str, message: &str) -> SignInStatus {
    SignInStatus {
        state: state.into(),
        url: None,
        code: None,
        message: message.into(),
    }
}
pub fn begin(directory: &std::path::Path, state: &SignIn) -> Result<SignInStatus, Error> {
    let mut pending = state.0.try_lock().map_err(|_| Error::Busy {
        message: "Sign-in operation running.".into(),
    })?;
    if pending.is_some() {
        return Err(Error::Busy {
            message: "Sign-in already pending.".into(),
        });
    }
    let admission = crate::run_host::admission();
    if let Some(blocker) = admission.blocker {
        return Ok(status("blocked", &blocker));
    }
    crate::run_host::confirm(&format!(
        "Acquire one fresh app-owned session. Data: {}",
        directory.display()
    ))?;
    let store = Store::open(directory)?;
    let lease = store.begin_acquisition()?;
    let mut sandbox = Sandbox { pipe: NativePipe };
    let resources = match sandbox.create(
        &lease.owner,
        &admission.image_id,
        &workflow_engine::sandbox::Cancel::default(),
        |kind, resource| store.resource_intent(&lease.owner, kind, resource),
    ) {
        Ok(value) => value,
        Err(error) => {
            store.quarantine_session(&lease)?;
            return Err(error);
        }
    };
    // Install ownership BEFORE the fallible start so the surface can cancel it.
    *pending = Some(Pending { resources, lease });
    if sandbox
        .start_login(&pending.as_ref().expect("installed").resources)
        .is_err()
    {
        store.quarantine_session(&pending.as_ref().expect("installed").lease)?;
        return Ok(status("cleanupRequired", "Login start failed. Cancel setup to remove owned resources; then explicitly retire the quarantined slot before a fresh sign-in."));
    }
    Ok(status("pending","Complete the provider device sign-in yourself. Codes are transient in this dedicated native surface only."))
}
pub fn read(directory: &std::path::Path, state: &SignIn) -> Result<SignInStatus, Error> {
    let mut pending = state.0.try_lock().map_err(|_| Error::Busy {
        message: "Sign-in operation running.".into(),
    })?;
    let Some(p) = pending.as_ref() else {
        return Ok(status("idle", "No managed sign-in pending."));
    };
    let mut sandbox = Sandbox { pipe: NativePipe };
    let (exit, url, code) = sandbox.login_status(&p.resources)?;
    match exit {
  None=>Ok(SignInStatus{state:"pending".into(),url,code,message:"Only the official provider device flow is supported. Disabled device login requires account/workspace setup; no alternate auth or paid fallback.".into()}),
  Some(0)=>{
   let store=Store::open(directory)?;let cache=sandbox.export(&p.resources)?;
   store.finish_acquisition(&p.lease,&cache,&mut crate::sessions_host::NativeSecureStore,||sandbox.cleanup(&p.resources))?;
   *pending=None;Ok(status("available","App-owned managed session persisted and read back; setup cleanup confirmed. Native/live qualification is separate."))
  },
  Some(_)=>{let store=Store::open(directory)?;store.quarantine_session(&p.lease)?;sandbox.cleanup(&p.resources)?;*pending=None;Ok(status("failed","Provider sign-in failed. No usable session published. Raw login output withheld."))}
 }
}
pub fn retire(directory: &std::path::Path, state: &SignIn) -> Result<SignInStatus, Error> {
    let pending = state.0.try_lock().map_err(|_| Error::Busy {
        message: "Sign-in operation running.".into(),
    })?;
    if pending.is_some() {
        return Err(Error::Busy {
            message: "Cancel pending setup first.".into(),
        });
    }
    crate::run_host::confirm(&format!("Permanently retire the quarantined session and its owned resources. Its old grant will NEVER be reused. Fresh provider authorization is required. Data: {}",directory.display()))?;
    let store = Store::open(directory)?;
    store.retire_quarantined(
        &mut crate::sessions_host::NativeSecureStore,
        |owner, records| Sandbox { pipe: NativePipe }.retire_owned(owner, records),
    )?;
    Ok(status("retired","Old secure item and owned resources confirmed absent. Begin a fresh authorization; no grant was cloned."))
}
pub fn cancel(directory: &std::path::Path, state: &SignIn) -> Result<SignInStatus, Error> {
    let mut pending = state.0.try_lock().map_err(|_| Error::Busy {
        message: "Sign-in operation running.".into(),
    })?;
    if let Some(p) = pending.as_ref() {
        let store = Store::open(directory)?;
        store.quarantine_session(&p.lease)?;
        let mut sandbox = Sandbox { pipe: NativePipe };
        sandbox.retire_owned(
            &p.lease.owner,
            &[
                ("work-container".into(), Some(p.resources.container.clone())),
                ("egress-helper".into(), Some(p.resources.helper.clone())),
                ("private-network".into(), Some(p.resources.network.clone())),
                ("egress-network".into(), Some(p.resources.outer.clone())),
            ],
        )?;
    }
    *pending = None;
    Ok(status(
        "cancelled",
        "Setup cancelled. Any interrupted cache is quarantined, never reused.",
    ))
}
