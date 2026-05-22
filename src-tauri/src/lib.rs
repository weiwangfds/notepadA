pub mod app_state;
pub mod buffer;
pub mod commands;
pub mod file;
pub mod search;
pub mod viewport;

use app_state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::file_cmds::open_file,
            commands::file_cmds::close_file,
            commands::file_cmds::get_tabs,
            commands::file_cmds::save_file,
            commands::file_cmds::save_file_as,
            commands::viewport_cmds::get_viewport,
            commands::viewport_cmds::goto_line,
            commands::viewport_cmds::get_line_count,
            commands::edit_cmds::insert_text,
            commands::edit_cmds::delete_range,
            commands::edit_cmds::replace_range,
            commands::edit_cmds::undo,
            commands::edit_cmds::redo,
            commands::search_cmds::search,
            commands::search_cmds::search_next,
            commands::search_cmds::replace_all,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
