mod db;

use db::{Database, LibrarySummary};
use tauri::Manager;

#[tauri::command]
fn get_library_summary(database: tauri::State<'_, Database>) -> Result<LibrarySummary, String> {
    database.summary().map_err(|error| error.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("unable to resolve the application data directory");
            let database = Database::open(&app_data_dir).expect("unable to initialize SQLite");
            app.manage(database);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_library_summary])
        .run(tauri::generate_context!())
        .expect("error while running Open Reader Desktop");
}
