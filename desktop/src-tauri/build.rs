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
        ]),
    ))
    .expect("desktop build configuration");
}
