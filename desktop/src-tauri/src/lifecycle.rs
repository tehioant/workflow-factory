use crate::{run_host, signin_host, ActiveRuns, Database};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use tauri::{Emitter, Manager};
use workflow_engine::{Error, Store};
#[derive(Default)]
pub struct Lifecycle {
    preview: Mutex<Option<(String, String)>>,
    pub approved: AtomicBool,
    pub frozen: AtomicBool,
}
fn ready_to_exit(
    workers: usize,
    setup: bool,
    retained: bool,
    durable: bool,
    exit: impl FnOnce(),
) -> Result<(), Error> {
    if workers != 0 || setup || retained || !durable {
        return Err(busy());
    }
    exit();
    Ok(())
}
fn activate(focus: impl FnOnce() -> bool) -> &'static [u8; 2] {
    if focus() {
        b"ok"
    } else {
        b"no"
    }
}
fn busy() -> Error {
    Error::Busy {
        message: "Shutdown state changed or is busy. Keep the app open and explicitly recheck."
            .into(),
    }
}
pub fn request(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show().and_then(|()| w.set_focus());
        let _ = w.emit("shutdown-requested", ());
    }
}
fn native(app: &tauri::AppHandle) -> Result<(Vec<String>, bool, bool), Error> {
    let active = app.state::<std::sync::Arc<ActiveRuns>>();
    let ids = active
        .0
        .try_lock()
        .map_err(|_| busy())?
        .keys()
        .cloned()
        .collect();
    let setup = signin_host::pending(&app.state::<std::sync::Arc<signin_host::SignIn>>())?;
    Ok((ids, setup, run_host::shutdown_pending()?))
}
#[tauri::command]
pub async fn shutdown_preview(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<workflow_engine::lifecycle::ShutdownPreview, Error> {
    if window.label() != "main" {
        return Err(busy());
    }
    let root = app.state::<Database>().directory.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let store = Store::open(&root)?;
        let engine = store.shutdown_snapshot()?;
        let state = native(&app)?;
        let signature = serde_json::to_string(&(engine, &state)).map_err(|_| busy())?;
        static TOKENS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let token = TOKENS.fetch_add(1, Ordering::Relaxed).to_string();
        *app.state::<Lifecycle>()
            .preview
            .try_lock()
            .map_err(|_| busy())? = Some((token.clone(), signature));
        let queue = workflow_engine::runs::history::HistoryReader::open(&root)?.queue_status()?;
        Ok(workflow_engine::lifecycle::ShutdownPreview {
            token,
            work: queue.entries.len() as u32,
            workers: state.0.len() as u32,
            setup: state.1,
            retained: state.2,
            frozen: app.state::<Lifecycle>().frozen.load(Ordering::Acquire),
        })
    })
    .await
    .map_err(|_| busy())?
}
#[tauri::command]
pub async fn shutdown_confirm(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    token: String,
    cancel_work: bool,
) -> Result<(), Error> {
    if window.label() != "main" || !cancel_work {
        return Err(busy());
    }
    let root = app.state::<Database>().directory.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let stored = app
            .state::<Lifecycle>()
            .preview
            .try_lock()
            .map_err(|_| busy())?
            .take()
            .ok_or_else(busy)?;
        if stored.0 != token {
            return Err(busy());
        }
        let store = Store::open(&root)?;
        let engine = store.shutdown_snapshot()?;
        if serde_json::to_string(&(engine.clone(), native(&app)?)).map_err(|_| busy())? != stored.1
        {
            return Err(busy());
        }
        let active = app.state::<std::sync::Arc<ActiveRuns>>();
        let registrations = active.0.lock().map_err(|_| busy())?;
        let ids = store.freeze_shutdown(&engine)?;
        active.1.store(true, Ordering::Release);
        drop(registrations);
        app.state::<Lifecycle>()
            .frozen
            .store(true, Ordering::Release);
        // Signal ALL selected workers even when any durable stop write fails.
        let mut failed = false;
        for id in &ids {
            failed |= app
                .state::<std::sync::Arc<ActiveRuns>>()
                .signal_stop(id)
                .is_err();
        }
        for id in &ids {
            failed |= store.stop_run(id).is_err();
        }
        if signin_host::pending(&app.state::<std::sync::Arc<signin_host::SignIn>>())? {
            failed |=
                signin_host::cancel(&root, &app.state::<std::sync::Arc<signin_host::SignIn>>())
                    .is_err();
        }
        if failed {
            return Err(busy());
        }
        Ok(())
    })
    .await
    .map_err(|_| busy())?
}
#[tauri::command]
pub async fn shutdown_finish(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<(), Error> {
    if window.label() != "main" {
        return Err(busy());
    }
    let root = app.state::<Database>().directory.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = native(&app)?;
        let active = app.state::<std::sync::Arc<ActiveRuns>>();
        let registrations = active.0.lock().map_err(|_| busy())?;
        ready_to_exit(
            registrations.len(),
            state.1,
            state.2,
            Store::open(&root)?.shutdown_ready()?,
            || {
                app.state::<Lifecycle>()
                    .approved
                    .store(true, Ordering::Release);
                app.exit(0);
            },
        )
    })
    .await
    .map_err(|_| busy())?
}

#[cfg(unix)]
pub fn own(
    app: &tauri::AppHandle,
    root: &std::path::Path,
) -> Result<bool, Box<dyn std::error::Error>> {
    use std::{
        io::{Read, Write},
        os::unix::{
            fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
            net::{UnixListener, UnixStream},
        },
        time::Duration,
    };
    let metadata = root.symlink_metadata()?;
    if !metadata.is_dir() || metadata.uid() != unsafe { libc::geteuid() } {
        return Err("Unsafe desktop data identity".into());
    }
    std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700))?;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(root.join("desktop.lock"))?;
    let socket = root.join("activation.sock");
    if file.try_lock().is_err() {
        let mut connection = UnixStream::connect(&socket)
            .map_err(|_| "Existing instance activation unavailable; no competing writer started")?;
        connection.set_read_timeout(Some(Duration::from_secs(2)))?;
        connection.set_write_timeout(Some(Duration::from_secs(2)))?;
        connection.write_all(b"activate")?;
        let mut ack = [0; 2];
        connection
            .read_exact(&mut ack)
            .map_err(|_| "Activation unconfirmed; no competing writer started")?;
        if &ack != b"ok" {
            return Err("Existing window focus unconfirmed".into());
        }
        return Ok(false);
    }
    if socket.symlink_metadata().is_ok() {
        std::fs::remove_file(&socket)?;
    }
    let listener = UnixListener::bind(socket)?;
    app.manage(crate::DataLock { _file: file });
    let app = app.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
            let mut command = [0; 8];
            if stream.read_exact(&mut command).is_err() || &command != b"activate" {
                continue;
            }
            let handle = app.clone();
            let _ = app.run_on_main_thread(move || {
                let ack = activate(|| {
                    handle
                        .get_webview_window("main")
                        .is_some_and(|w| w.show().and_then(|()| w.set_focus()).is_ok())
                });
                let _ = stream.write_all(ack);
            });
        }
    });
    Ok(true)
}

#[cfg(test)]
mod tests {
    #[test]
    fn d13_prepared_native_freeze_and_final_barrier() {
        use std::cell::Cell;
        let dir = std::env::temp_dir().join(format!("shutdown-prepared-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let store = workflow_engine::Store::open(&dir).unwrap();
        let c = rusqlite::Connection::open(dir.join("definitions.sqlite3")).unwrap();
        let run = "00000000-0000-0000-0000-000000000013";
        let status=serde_json::json!({"id":run,"requestId":"prepared","state":"queued","finalResponse":null,"diagnostic":"","cleanupPending":false,"blocks":[]}).to_string();
        c.execute(
            "INSERT INTO runs VALUES(?1,'prepared','{}','{}',?2,1)",
            rusqlite::params![run, status],
        )
        .unwrap();
        c.execute(
            "INSERT INTO run_queue(run_id,phase) VALUES(?1,'waiting')",
            [run],
        )
        .unwrap();
        c.execute(
            "INSERT INTO managed_sessions VALUES('slot','available',7,NULL)",
            [],
        )
        .unwrap();
        let stale = store.shutdown_snapshot().unwrap();
        c.execute(
            "UPDATE runs SET status=json_set(status,'$.diagnostic','changed')",
            [],
        )
        .unwrap();
        assert!(store.freeze_shutdown(&stale).is_err());
        let fresh = store.shutdown_snapshot().unwrap();
        assert_eq!(store.freeze_shutdown(&fresh).unwrap(), vec![run]);
        assert!(store.reserve_next_run().unwrap().is_none());
        assert!(store.begin_acquisition().is_err());
        assert!(!store
            .authorize_reserved_run(&workflow_engine::sessions::Lease {
                slot: "slot".into(),
                owner: run.into(),
                generation: 7
            })
            .unwrap());
        assert!(store
            .save_queue_settings(workflow_engine::runs::queue::QueueSettings {
                active: 2,
                waiting: 16
            })
            .is_ok());
        assert!(store.reserve_next_run().unwrap().is_none());
        assert!(!store.shutdown_ready().unwrap());
        let active = crate::ActiveRuns::default();
        let cancel = workflow_engine::sandbox::Cancel::default();
        active.0.lock().unwrap().insert(run.into(), cancel.clone());
        active.signal_stop(run).unwrap();
        assert!(cancel.check().is_err());
        store.stop_run(run).unwrap();
        assert!(store.shutdown_ready().unwrap());
        let exit = Cell::new(0);
        for (workers, setup, retained, durable) in [
            (1, false, false, true),
            (0, true, false, true),
            (0, false, true, true),
            (0, false, false, false),
        ] {
            assert!(
                super::ready_to_exit(workers, setup, retained, durable, || exit
                    .set(exit.get() + 1))
                .is_err()
            );
        }
        assert_eq!(exit.get(), 0);
        super::ready_to_exit(0, false, false, true, || exit.set(exit.get() + 1)).unwrap();
        assert_eq!(exit.get(), 1);
        assert_eq!(super::activate(|| true), b"ok");
        assert_eq!(super::activate(|| false), b"no");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
