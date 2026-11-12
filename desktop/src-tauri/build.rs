fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "list_definitions",
            "load_definition",
            "save_definition",
            "prepare_inputs",
            "list_configurations",
            "save_configuration",
            "delete_configuration",
            "auth_readiness",
            "runtime_readiness",
            "runtime_setup_plan",
            "start_run",
            "run_status",
            "stop_run",
            "managed_session_status",
            "open_session_sign_in",
            "begin_session_sign_in",
            "read_session_sign_in",
            "cancel_session_sign_in",
            "retire_session_sign_in",
        ]),
    ))
    .expect("desktop build configuration");
}
