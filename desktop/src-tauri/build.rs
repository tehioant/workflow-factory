fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "list_definitions",
            "load_definition",
            "save_definition",
            "prepare_inputs",
        ]),
    ))
    .expect("desktop build configuration");
}
