#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
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
    let scope = format!(
        "Data: {}\nRequest: {}\nDefinition: {} revision {}\nLegacy configuration revision: {}\nCaptured configuration revisions: {:?}",
        state.directory.display(),
        request.request_id,
        request.definition_id,
        request.expected_revision,
        request.expected_configuration_revision,
        request.expected_configuration_revisions
    );
    let status = database(&state, move |s| {
        if let Some(original) = s.recorded_run(&request)? {
            return Ok(original);
        }
        let admission = run_host::admission();
        if admission.blocker.is_none() {
            run_host::confirm(&format!("{scope}\n{}", s.available_session_scope()?))?;
        }
        s.admit_run(request, admission)
    })
    .await?;
    if status.state == workflow_engine::runs::RunState::Admitted {
        let directory = state.directory.clone();
        let id = status.id.clone();
        let active = app.state::<Arc<ActiveRuns>>().inner().clone();
        let cancel = {
            let mut runs = active.0.lock().map_err(|_| Error::Busy {
                message: "Execution ownership unavailable.".into(),
            })?;
            if runs.contains_key(&id) {
                return Ok(status);
            }
            let cancel = workflow_engine::sandbox::Cancel::default();
            runs.insert(id.clone(), cancel.clone());
            cancel
        };
        tauri::async_runtime::spawn_blocking(move || {
            run_host::execute(directory, id.clone(), cancel);
            if let Ok(mut runs) = active.0.lock() {
                runs.remove(&id);
            }
        });
    }
    Ok(status)
}
#[tauri::command]
async fn run_status(
    state: tauri::State<'_, Database>,
    id: Option<String>,
) -> Result<Option<workflow_engine::runs::RunStatus>, Error> {
    database(&state, move |s| {
        run_host::retry_outputs(&s)?;
        match id {
            Some(id) => s.run_status(&id).map(Some),
            None => s.latest_run(),
        }
    })
    .await
}
#[tauri::command]
async fn stop_run(
    app: tauri::AppHandle,
    state: tauri::State<'_, Database>,
    id: String,
) -> Result<workflow_engine::runs::RunStatus, Error> {
    let active = app.state::<Arc<ActiveRuns>>();
    active.signal_stop(&id)?;
    database(&state, move |s| s.stop_run(&id)).await.map_err(|_| Error::Storage {
        message: "Selected worker cancellation requested, but stop storage failed. Ownership and cleanup remain held; retry Stop/status. No new work authorized.".into(),
    })
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
    tauri::async_runtime::spawn_blocking(move || signin_host::read(&directory, &signin))
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
        .invoke_handler(tauri::generate_handler![
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
            start_run,
            run_status,
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
