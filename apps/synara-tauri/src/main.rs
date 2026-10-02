fn main() {
    tauri::Builder::default()
        .setup(|_app| {
            tauri::async_runtime::spawn(async {
                if let Err(error) = synara_server::run().await {
                    eprintln!("Synara server stopped with error: {error:#}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![health])
        .run(tauri::generate_context!())
        .expect("error while running Synara");
}

#[tauri::command]
fn health() -> &'static str {
    "ok"
}
