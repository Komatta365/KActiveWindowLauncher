mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
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
    .invoke_handler(tauri::generate_handler![
      commands::launcher_init,
      commands::launcher_start_tracking,
      commands::launcher_stop_tracking,
      commands::settings_load,
      commands::settings_save,
      commands::slot_swap,
      commands::slot_update,
      commands::slot_clear,
      commands::slot_execute,
      commands::path_detect_data_type,
      commands::path_exists,
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
