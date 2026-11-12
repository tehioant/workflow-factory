use super::{inspection_host as host, Database};
#[tauri::command]
pub fn inspect_workflow(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Database>,
    run: String,
) -> Result<String, workflow_engine::Error> {
    if window.label() != "main" {
        return Err(workflow_engine::Error::Validation {
            message: "Use the main window.".into(),
        });
    }
    workflow_engine::runs::history::HistoryReader::open(&state.directory)?.workflow(&run)
}
use workflow_engine::{runs::inspection::view::*, Error};
#[tauri::command]
pub fn inspect_subscribe(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Database>,
    definition_id: String,
    run: String,
    channel: tauri::ipc::Channel<Notice>,
) -> Result<InspectionPage, Error> {
    host::subscribe(
        &state.directory,
        window.label(),
        &definition_id,
        &run,
        channel,
    )
}
#[tauri::command]
pub fn inspect_page(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Database>,
    definition_id: String,
    run: String,
    subscription: String,
    cursor: Option<String>,
) -> Result<InspectionPage, Error> {
    host::read(
        &state.directory,
        window.label(),
        &definition_id,
        &run,
        &subscription,
        cursor.as_deref(),
    )
}
#[tauri::command]
pub fn inspect_close(
    window: tauri::WebviewWindow,
    definition_id: String,
    run: String,
    subscription: String,
) -> Result<(), Error> {
    host::close(window.label(), &definition_id, &run, &subscription)
}
#[tauri::command]
pub fn inspect_artifacts(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Database>,
    definition_id: String,
    run: String,
    subscription: String,
    cursor: Option<String>,
) -> Result<ArtifactPage, Error> {
    host::artifacts(
        &state.directory,
        window.label(),
        &definition_id,
        &run,
        &subscription,
        cursor.as_deref(),
    )
}
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn inspect_artifact(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Database>,
    definition_id: String,
    run: String,
    subscription: String,
    generation: String,
    id: String,
    cursor: Option<String>,
) -> Result<ArtifactText, Error> {
    host::artifact(
        &state.directory,
        window.label(),
        &definition_id,
        &run,
        &subscription,
        &generation,
        &id,
        cursor.as_deref(),
    )
}
