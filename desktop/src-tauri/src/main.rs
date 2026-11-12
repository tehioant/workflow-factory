#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod inspection_commands;
mod inspection_host;
#[cfg(any(target_os = "macos", test))]
mod mac_security;
mod run_host;
mod runtime_host;
mod sessions_host;
mod signin_host;
use std::{path::PathBuf, sync::Arc};
use tauri::Manager;
use workflow_engine::{
    resolve_inputs, Definition, Error, PreparedInputs, SaveDefinition, SavedDefinition, Store,
};

#[derive(Default)]
struct ActiveRuns(
    std::sync::Mutex<std::collections::BTreeMap<String, workflow_engine::sandbox::Cancel>>,
);
impl ActiveRuns {
    fn register(
        &self,
        id: &str,
        permits: Arc<tokio::sync::Semaphore>,
    ) -> Result<
        Option<(
            workflow_engine::sandbox::Cancel,
            tokio::sync::OwnedSemaphorePermit,
        )>,
        Error,
    > {
        let mut runs = self.0.lock().map_err(|_| Error::Busy {
            message: "Worker registration unavailable; reservation remains held.".into(),
        })?;
        if runs.contains_key(id) {
            return Ok(None);
        }
        let Ok(permit) = permits.try_acquire_owned() else {
            return Ok(None);
        };
        let cancel = workflow_engine::sandbox::Cancel::default();
        runs.insert(id.into(), cancel.clone());
        Ok(Some((cancel, permit)))
    }

    fn worker_return(
        &self,
        id: &str,
        permit: tokio::sync::OwnedSemaphorePermit,
        wake: impl FnOnce(),
    ) {
        if let Ok(mut runs) = self.0.lock() {
            runs.remove(id);
        }
        drop(permit);
        wake();
    }
    fn signal_stop(&self, id: &str) -> Result<(), Error> {
        let runs = self.0.lock().map_err(|_| Error::Busy {
            message: "Worker cancellation ownership unavailable; retry Stop.".into(),
        })?;
        if let Some(cancel) = runs.get(id) {
            cancel.request();
        }
        Ok(())
    }
}
#[cfg(test)]
mod stop_tests {
    #[test]
    fn c17_stop_signals_only_exact_worker_despite_storage_refusal() {
        let active = super::ActiveRuns::default();
        let selected = workflow_engine::sandbox::Cancel::default();
        let other = workflow_engine::sandbox::Cancel::default();
        active.0.lock().unwrap().extend([
            ("selected".into(), selected.clone()),
            ("other".into(), other.clone()),
        ]);
        let stop = || -> Result<(), workflow_engine::Error> {
            active.signal_stop("selected")?;
            Err(workflow_engine::Error::Storage {
                message: "prepared database refusal; ownership held".into(),
            })
        };
        assert!(stop().is_err());
        assert!(selected.check().is_err());
        assert!(other.check().is_ok());
        assert_eq!(active.0.lock().unwrap().len(), 2);
        let permits = std::sync::Arc::new(tokio::sync::Semaphore::new(2));
        let bounded = super::ActiveRuns::default();
        let (first, first_permit) = bounded.register("first", permits.clone()).unwrap().unwrap();
        assert!(bounded
            .register("first", permits.clone())
            .unwrap()
            .is_none());
        let (second, second_permit) = bounded
            .register("second", permits.clone())
            .unwrap()
            .unwrap();
        assert!(bounded
            .register("waiting", permits.clone())
            .unwrap()
            .is_none());
        bounded.signal_stop("first").unwrap();
        assert!(first.check().is_err());
        assert!(second.check().is_ok());
        // A stop/database disposition alone does not release the live worker permit.
        assert_eq!(permits.available_permits(), 0);
        bounded.worker_return("first", first_permit, || {
            assert!(!bounded.0.lock().unwrap().contains_key("first"));
            assert_eq!(permits.available_permits(), 1);
            let (_, next) = bounded
                .register("waiting", permits.clone())
                .unwrap()
                .unwrap();
            assert_eq!(permits.available_permits(), 0);
            drop(next);
        });
        drop(second_permit);
        use std::cell::{Cell, RefCell};
        for ready_before_dispatch in [true, false] {
            let ready = Cell::new(ready_before_dispatch);
            let queued = Cell::new(true);
            let events = RefCell::new(Vec::new());
            let dispatch = || {
                super::reservation_dispatch(
                    || {
                        events.borrow_mut().push("reservation");
                        Ok(if ready.get() && queued.replace(false) {
                            Some("prepared-waiter".into())
                        } else {
                            None
                        })
                    },
                    || {
                        events
                            .borrow_mut()
                            .push(if queued.get() { "waiting" } else { "dispatch" });
                        Ok(())
                    },
                )
            };
            dispatch().unwrap();
            if !ready_before_dispatch {
                assert_eq!(*events.borrow(), vec!["reservation", "waiting"]);
                ready.set(true); // independently prepared cleanup/sign-in wakeup observation
                dispatch().unwrap();
            }
            assert!(!queued.get());
            assert_eq!(events.borrow().last(), Some(&"dispatch"));
            assert_eq!(
                events.borrow().iter().filter(|&&e| e == "dispatch").count(),
                1
            );
        }
    }
}
struct Database {
    directory: PathBuf,
    gate: Arc<tokio::sync::Semaphore>,
}

async fn database<T: Send + 'static>(
    state: &Database,
    action: impl FnOnce(Store) -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    let permit = state
        .gate
        .clone()
        .try_acquire_owned()
        .map_err(|_| Error::Busy {
            message: "Another storage action is running. Retry shortly.".into(),
        })?;
    let directory = state.directory.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        action(Store::open(&directory)?)
    })
    .await
    .map_err(|_| Error::Storage {
        message: "Storage worker interrupted. Your edits are retained; retry.".into(),
    })?
}

#[tauri::command]
async fn list_definitions(
    state: tauri::State<'_, Database>,
) -> Result<Vec<SavedDefinition>, Error> {
    database(&state, |store| store.list()).await
}
#[tauri::command]
async fn load_definition(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<SavedDefinition, Error> {
    database(&state, move |store| store.load(&id)).await
}
#[tauri::command]
async fn save_definition(
    state: tauri::State<'_, Database>,
    request: SaveDefinition,
) -> Result<SavedDefinition, Error> {
    database(&state, move |store| store.save(request)).await
}

#[tauri::command]
async fn prepare_inputs(
    definition: Definition,
    supplied: std::collections::BTreeMap<String, String>,
) -> Result<PreparedInputs, Error> {
    tauri::async_runtime::spawn_blocking(move || resolve_inputs(&definition, supplied))
        .await
        .map_err(|_| Error::Busy {
            message: "Input preparation interrupted. Retry.".into(),
        })?
}
#[tauri::command]
async fn list_configurations(
    state: tauri::State<'_, Database>,
) -> Result<Vec<workflow_engine::SavedConfiguration>, Error> {
    database(&state, |s| s.list_configurations()).await
}
#[tauri::command]
async fn save_configuration(
    state: tauri::State<'_, Database>,
    request: workflow_engine::SaveConfiguration,
) -> Result<workflow_engine::SavedConfiguration, Error> {
    database(&state, move |s| s.save_configuration(request)).await
}
#[tauri::command]
async fn delete_configuration(
    state: tauri::State<'_, Database>,
    id: String,
    expected_revision: u32,
) -> Result<(), Error> {
    database(&state, move |s| {
        s.delete_configuration(&id, expected_revision)
    })
    .await
}
#[tauri::command]
async fn auth_readiness(app: tauri::AppHandle) -> Result<workflow_engine::AuthReadiness, Error> {
    let home = app.path().home_dir().map_err(|_| Error::Busy {
        message: "Codex source location unavailable. No inspection performed.".into(),
    })?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut source = workflow_engine::auth_file::CodexFile { home };
        workflow_engine::credential_readiness(workflow_engine::auth::inspect(&mut source))
    })
    .await
    .map_err(|_| Error::Busy {
        message: "Credential inspection interrupted. Retry manually.".into(),
    })
}

struct WorkerPermits(Arc<tokio::sync::Semaphore>);
fn reservation_dispatch<T>(
    mut reserve: impl FnMut() -> Result<Option<String>, Error>,
    dispatch: impl FnOnce() -> Result<T, Error>,
) -> Result<T, Error> {
    for _ in 0..8 {
        if reserve()?.is_none() {
            break;
        }
    }
    dispatch()
}
fn dispatch_reserved(
    app: &tauri::AppHandle,
    directory: PathBuf,
    selected: Option<String>,
) -> Result<(), Error> {
    let store = Store::open(&directory)?;
    let entries = reservation_dispatch(
        || store.reserve_next_run(),
        || {
            Ok(
                workflow_engine::runs::history::HistoryReader::open(&directory)?
                    .queue_status()?
                    .entries,
            )
        },
    )?;
    let active = app.state::<Arc<ActiveRuns>>().inner().clone();
    for entry in entries {
        if entry.state != workflow_engine::runs::RunState::AuthorizationRequired {
            continue;
        }
        if let Some(ref id) = selected {
            if id != &entry.id {
                continue;
            }
        } else if entry.diagnostic.starts_with("Native confirmation declined") {
            continue;
        }
        let Some((cancel, permit)) =
            active.register(&entry.id, app.state::<WorkerPermits>().0.clone())?
        else {
            continue;
        };
        let app = app.clone();
        let directory = directory.clone();
        let active = active.clone();
        let id = entry.id;
        tauri::async_runtime::spawn_blocking(move || {
            let _permit = permit;
            let _ = (|| -> Result<(), Error> {
                let store = Store::open(&directory)?;
                let Some(lease) = store.execution_reservation(&id)? else {
                    return Ok(());
                };
                let request = store.run_status(&id)?.request_id;
                let reader = workflow_engine::runs::history::HistoryReader::open(&directory)?;
                let workflow = reader.workflow(&id)?;
                let captured = reader.run_detail(&workflow, &id)?;
                let scope = format!(
                    "Definition: {} revision {}\nIsolation: {:?}\nImage: {}\nConfigurations: {:?}",
                    workflow,
                    captured.definition.revision,
                    captured.definition.definition.isolation,
                    captured.image_id,
                    captured
                        .blocks
                        .iter()
                        .filter_map(|b| b
                            .configuration
                            .as_ref()
                            .map(|c| (&c.config.id, c.revision)))
                        .collect::<Vec<_>>()
                );
                let result = run_host::execute_confirmed(
                    &cancel,
                    || {
                        run_host::confirm(&format!("Data: {}\nRequest: {request}\nRun: {id}\nSession: {} generation {}\n{scope}\nOne execution only; no failover or automatic retry.",directory.display(),lease.slot,lease.generation))
                    },
                    || store.authorize_reserved_run(&lease),
                    || run_host::execute(directory.clone(), id.clone(), cancel.clone()),
                );
                if result.is_err() {
                    store.defer_run_authorization(&lease)?;
                }
                Ok(())
            })();
            // Worker return releases only the in-app permit. SQLite completion alone
            // controls whether another execution reservation exists.
            active.worker_return(&id, _permit, || {
                let clean_completion = Store::open(&directory)
                    .and_then(|s| s.run_status(&id))
                    .is_ok_and(|s| {
                        !s.cleanup_pending
                            && matches!(
                                s.state,
                                workflow_engine::runs::RunState::Succeeded
                                    | workflow_engine::runs::RunState::Failed
                                    | workflow_engine::runs::RunState::Stopped
                            )
                    });
                if clean_completion {
                    let _ = dispatch_reserved(&app, directory, None);
                }
            });
        });
    }
    Ok(())
}
#[tauri::command]
async fn authorize_run(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<(), Error> {
    if window.label() != "main" {
        return Err(Error::Validation {
            message: "Use the local main window.".into(),
        });
    }
    let directory = state.directory.clone();
    tauri::async_runtime::spawn_blocking(move || dispatch_reserved(&app, directory, Some(id)))
        .await
        .map_err(|_| Error::Busy {
            message: "Authorization interrupted; reservation remains held.".into(),
        })?
}
#[tauri::command]
async fn queue_status(
    state: tauri::State<'_, Database>,
) -> Result<workflow_engine::runs::queue::QueueStatus, Error> {
    history_database(&state, |s| s.queue_status()).await
}
#[tauri::command]
async fn queue_settings(
    state: tauri::State<'_, Database>,
) -> Result<workflow_engine::runs::queue::QueueSettings, Error> {
    database(&state, |s| s.queue_settings()).await
}
#[tauri::command]
async fn save_queue_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, Database>,
    settings: workflow_engine::runs::queue::QueueSettings,
) -> Result<workflow_engine::runs::queue::QueueSettings, Error> {
    let value = database(&state, move |s| s.save_queue_settings(settings)).await?;
    let directory = state.directory.clone();
    tauri::async_runtime::spawn_blocking(move || dispatch_reserved(&app, directory, None))
        .await
        .map_err(|_| Error::Busy {
            message: "Dispatch interrupted; settings saved, reservations held.".into(),
        })??;
    Ok(value)
}

struct RuntimeGate(Arc<tokio::sync::Semaphore>);
#[tauri::command]
async fn runtime_readiness(
    state: tauri::State<'_, RuntimeGate>,
) -> Result<workflow_engine::runtime::Readiness, Error> {
    let permit = state
        .0
        .clone()
        .try_acquire_owned()
        .map_err(|_| Error::Busy {
            message: "Runtime check already running. Retry shortly.".into(),
        })?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        runtime_host::readiness_with(&mut workflow_engine::runtime::RealProcess)
    })
    .await
    .map_err(|_| Error::Busy {
        message: "Runtime check interrupted. Retry.".into(),
    })
}
#[tauri::command]
async fn runtime_setup_plan() -> Result<workflow_engine::runtime::SetupPlan, Error> {
    tauri::async_runtime::spawn_blocking(runtime_host::setup_plan)
        .await
        .map_err(|_| Error::Busy {
            message: "Setup plan interrupted. Retry.".into(),
        })
}
#[tauri::command]
async fn start_run(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: tauri::State<'_, Database>,
    request: workflow_engine::runs::StartRun,
) -> Result<workflow_engine::runs::RunStatus, Error> {
    if window.label() != "main" {
        return Err(Error::Validation {
            message: "Use the local main window.".into(),
        });
    }
    let retry = request.clone();
    if let Some(original) = database(&state, move |s| s.recorded_run(&retry)).await? {
        return Ok(original);
    }
    let admission = tauri::async_runtime::spawn_blocking(run_host::admission)
        .await
        .map_err(|_| Error::Busy {
            message: "Native preflight interrupted; retry same request.".into(),
        })?;
    let status = database(&state, move |s| s.admit_run(request, admission)).await?;
    let directory = state.directory.clone();
    tauri::async_runtime::spawn_blocking(move || dispatch_reserved(&app, directory, None))
        .await
        .map_err(|_| Error::Busy {
            message: "Run accepted; dispatch uncertain. Retry the same request.".into(),
        })??;
    Ok(status)
}
#[tauri::command]
async fn run_status(
    state: tauri::State<'_, Database>,
    id: Option<String>,
) -> Result<Option<workflow_engine::runs::RunStatus>, Error> {
    history_database(&state, move |reader| reader.run_status(id.as_deref())).await
}
#[tauri::command]
async fn retry_run_output(
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<workflow_engine::runs::RunStatus, Error> {
    let selected = id.clone();
    history_database(&state, move |reader| reader.run_status(Some(&selected)))
        .await?
        .ok_or_else(|| Error::Validation {
            message: "Captured run unavailable.".into(),
        })?;
    let directory = state.directory.clone();
    database(&state, move |store| {
        run_host::retry_outputs(&store, &id)?;
        workflow_engine::runs::history::HistoryReader::open(&directory)?
            .run_status(Some(&id))?
            .ok_or_else(|| Error::Validation {
                message: "Captured run unavailable.".into(),
            })
    })
    .await
}
async fn history_database<T: Send + 'static>(
    state: &Database,
    action: impl FnOnce(workflow_engine::runs::history::HistoryReader) -> Result<T, Error>
        + Send
        + 'static,
) -> Result<T, Error> {
    let root = state.directory.clone();
    tauri::async_runtime::spawn_blocking(move || {
        action(workflow_engine::runs::history::HistoryReader::open(&root)?)
    })
    .await
    .map_err(|_| Error::Storage {
        message: "Captured history unavailable.".into(),
    })?
}
#[tauri::command]
async fn list_run_history(
    state: tauri::State<'_, Database>,
    definition_id: String,
    cursor: Option<String>,
    limit: u32,
) -> Result<workflow_engine::runs::history::HistoryPage, Error> {
    history_database(&state, move |s| {
        s.list_run_history(&definition_id, cursor.as_deref(), limit)
    })
    .await
}
#[tauri::command]
async fn run_detail(
    state: tauri::State<'_, Database>,
    definition_id: String,
    id: String,
) -> Result<workflow_engine::runs::history::RunDetail, Error> {
    history_database(&state, move |s| s.run_detail(&definition_id, &id)).await
}
#[tauri::command]
async fn stop_run(
    app: tauri::AppHandle,
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<workflow_engine::runs::RunStatus, Error> {
    let active = app.state::<Arc<ActiveRuns>>();
    active.signal_stop(&id)?;
    let status=database(&state,move|s|s.stop_run(&id)).await.map_err(|_| Error::Storage {
        message:"Selected worker cancellation requested, but stop storage failed. Ownership and cleanup remain held; retry Stop/status. No new work authorized.".into(),
    })?;
    if status.state == workflow_engine::runs::RunState::Stopped {
        let directory = state.directory.clone();
        tauri::async_runtime::spawn_blocking(move || dispatch_reserved(&app, directory, None))
            .await
            .map_err(|_| Error::Busy {
                message: "Selected run stopped; next reservation requires recheck.".into(),
            })??;
    }
    Ok(status)
}
#[tauri::command]
async fn managed_session_status(
    state: tauri::State<'_, Database>,
) -> Result<workflow_engine::sessions::ManagedSessionStatus, Error> {
    database(&state, move |s| s.managed_session_status()).await
}
#[tauri::command]
async fn open_session_sign_in(app: tauri::AppHandle) -> Result<(), Error> {
    if let Some(window) = app.get_webview_window("signin") {
        window.set_focus().map_err(|_| Error::Busy {
            message: "Sign-in window unavailable.".into(),
        })?;
        return Ok(());
    }
    tauri::WebviewWindowBuilder::new(
        &app,
        "signin",
        tauri::WebviewUrl::App("index.html?signin=1".into()),
    )
    .title("Managed Codex sign-in · private native surface")
    .inner_size(600.0, 480.0)
    .on_navigation(|url| {
        url.host_str() == Some("localhost") && url.scheme() == "tauri"
            || url.host_str() == Some("tauri.localhost") && url.scheme() == "http"
    })
    .build()
    .map_err(|_| Error::Busy {
        message: "Cannot open native sign-in surface.".into(),
    })?;
    Ok(())
}
fn signin_only(window: &tauri::WebviewWindow) -> Result<(), Error> {
    if window.label() != "signin" {
        return Err(Error::Validation {
            message: "Use the dedicated native sign-in surface.".into(),
        });
    }
    Ok(())
}
#[tauri::command]
async fn begin_session_sign_in(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: tauri::State<'_, Database>,
) -> Result<workflow_engine::sessions::SignInStatus, Error> {
    signin_only(&window)?;
    let directory = state.directory.clone();
    let signin = app.state::<Arc<signin_host::SignIn>>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || signin_host::begin(&directory, &signin))
        .await
        .map_err(|_| Error::Busy {
            message: "Native sign-in interrupted; session remains withheld.".into(),
        })?
}
#[tauri::command]
async fn read_session_sign_in(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: tauri::State<'_, Database>,
) -> Result<workflow_engine::sessions::SignInStatus, Error> {
    signin_only(&window)?;
    let directory = state.directory.clone();
    let signin = app.state::<Arc<signin_host::SignIn>>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let status = signin_host::read(&directory, &signin)?;
        if status.state == "available" {
            dispatch_reserved(&app, directory, None)?;
        }
        Ok(status)
    })
    .await
    .map_err(|_| Error::Busy {
        message: "Native sign-in interrupted; session remains withheld.".into(),
    })?
}
#[tauri::command]
async fn cancel_session_sign_in(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: tauri::State<'_, Database>,
) -> Result<workflow_engine::sessions::SignInStatus, Error> {
    signin_only(&window)?;
    let directory = state.directory.clone();
    let signin = app.state::<Arc<signin_host::SignIn>>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || signin_host::cancel(&directory, &signin))
        .await
        .map_err(|_| Error::Busy {
            message: "Native sign-in cancellation unconfirmed; ownership remains held.".into(),
        })?
}
#[tauri::command]
async fn retire_session_sign_in(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: tauri::State<'_, Database>,
) -> Result<workflow_engine::sessions::SignInStatus, Error> {
    signin_only(&window)?;
    let directory = state.directory.clone();
    let signin = app.state::<Arc<signin_host::SignIn>>().inner().clone();
    let active = app.state::<Arc<ActiveRuns>>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        // Hold worker registration exclusion through the retirement transaction.
        let runs = active.0.lock().map_err(|_| Error::Busy {
            message: "Execution ownership unavailable.".into(),
        })?;
        if !runs.is_empty() {
            return Err(Error::Busy {
                message: "Wait for the active worker to stop before retirement.".into(),
            });
        }
        signin_host::retire(&directory, &signin)
    })
    .await
    .map_err(|_| Error::Busy {
        message: "Retirement interrupted; ownership remains withheld.".into(),
    })?
}
struct DataLock {
    _file: std::fs::File,
}
fn main() {
    tauri::Builder::default()
        .setup(|app| {
            app.manage(Arc::new(ActiveRuns::default()));
            app.manage(WorkerPermits(Arc::new(tokio::sync::Semaphore::new(8))));
            app.manage(Arc::new(signin_host::SignIn(std::sync::Mutex::new(None))));
            app.manage(RuntimeGate(Arc::new(tokio::sync::Semaphore::new(1))));
            let directory = app.path().app_data_dir()?;
            std::fs::create_dir_all(&directory)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
                if directory.symlink_metadata()?.file_type().is_symlink() {
                    return Err("Unsafe data directory".into());
                }
                std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
                let file = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(false)
                    .mode(0o600)
                    .custom_flags(libc::O_NOFOLLOW)
                    .open(directory.join("desktop.lock"))?;
                file.try_lock()
                    .map_err(|_| "Another process owns this desktop data directory")?;
                app.manage(DataLock { _file: file });
            }
            Store::open(&directory)
                .and_then(|s| s.withhold_unfinished_runs())
                .map_err(|_| "Startup storage reconciliation failed; execution withheld")?;
            app.manage(Database {
                directory,
                gate: Arc::new(tokio::sync::Semaphore::new(1)),
            });
            tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("Workflow Factory Desktop")
            .inner_size(1080.0, 720.0)
            .min_inner_size(640.0, 480.0)
            .on_navigation(|url| {
                url.host_str() == Some("localhost") && url.scheme() == "tauri"
                    || url.host_str() == Some("tauri.localhost") && url.scheme() == "http"
            })
            .build()?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                inspection_host::release(window.label());
            }
        })
        .invoke_handler(tauri::generate_handler![
            inspection_commands::inspect_workflow,
            inspection_commands::inspect_subscribe,
            inspection_commands::inspect_page,
            inspection_commands::inspect_close,
            inspection_commands::inspect_artifacts,
            inspection_commands::inspect_artifact,
            list_definitions,
            load_definition,
            save_definition,
            prepare_inputs,
            list_configurations,
            save_configuration,
            delete_configuration,
            auth_readiness,
            runtime_readiness,
            runtime_setup_plan,
            queue_status,
            queue_settings,
            save_queue_settings,
            authorize_run,
            start_run,
            run_status,
            retry_run_output,
            list_run_history,
            run_detail,
            stop_run,
            managed_session_status,
            open_session_sign_in,
            begin_session_sign_in,
            read_session_sign_in,
            cancel_session_sign_in,
            retire_session_sign_in
        ])
        .run(tauri::generate_context!())
        .expect("desktop host failed");
}
