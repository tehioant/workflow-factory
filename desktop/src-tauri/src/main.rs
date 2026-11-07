#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::{path::PathBuf, sync::Arc};
use tauri::Manager;
use workflow_engine::{
    resolve_inputs, Definition, Error, PreparedInputs, SaveDefinition, SavedDefinition, Store,
};

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
fn main() {
    tauri::Builder::default()
        .setup(|app| {
            app.manage(Database {
                directory: app.path().app_data_dir()?,
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
            prepare_inputs
        ])
        .run(tauri::generate_context!())
        .expect("desktop host failed");
}
