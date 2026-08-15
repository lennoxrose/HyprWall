mod commands;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::monitors::list_monitors,
            commands::monitors::set_wallpaper,
            commands::monitors::pause_wallpaper,
            commands::monitors::play_wallpaper,
        ])
        .run(tauri::generate_context!())
        .expect("error while running hyprwall-gui");
}
