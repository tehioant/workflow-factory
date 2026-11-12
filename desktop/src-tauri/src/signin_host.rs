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
    let store = Store::open(directory)?;
    let lease = store.begin_acquisition()?;
    let mut sandbox = Sandbox { pipe: NativePipe };
    let resources = match sandbox.create(&lease.owner, &admission.image_id, |kind, resource| {
        store.resource_intent(&lease.owner, kind, resource)
    }) {
        Ok(value) => value,
        Err(error) => {
            store.quarantine_session(&lease)?;
            return Err(error);
        }
    };
    sandbox.start_login(&resources)?;
    *pending = Some(Pending { resources, lease });
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
pub fn cancel(directory: &std::path::Path, state: &SignIn) -> Result<SignInStatus, Error> {
    let mut pending = state.0.try_lock().map_err(|_| Error::Busy {
        message: "Sign-in operation running.".into(),
    })?;
    if let Some(p) = pending.as_ref() {
        let store = Store::open(directory)?;
        store.quarantine_session(&p.lease)?;
        let mut sandbox = Sandbox { pipe: NativePipe };
        sandbox.cleanup(&p.resources)?;
    }
    *pending = None;
    Ok(status(
        "cancelled",
        "Setup cancelled. Any interrupted cache is quarantined, never reused.",
    ))
}
