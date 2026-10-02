fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![health])
        .run(tauri::generate_context!())
        .expect("error while running Synara");
}

#[tauri::command]
fn health() -> &'static str {
    "ok"
}
