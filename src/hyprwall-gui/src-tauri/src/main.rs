mod commands;

fn main() {
    // libmpv (used for thumbnail generation) refuses to initialize outside
    // the "C" locale for LC_NUMERIC; a normal desktop LANG/LC_* like
    // en_US.UTF-8 is enough to trip this. Only LC_NUMERIC is touched.
    unsafe { libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr()) };

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::config::get_library_folders,
            commands::config::set_library_folders,
            commands::library::scan_library,
            commands::monitors::list_monitors,
            commands::monitors::set_wallpaper,
            commands::monitors::unset_wallpaper,
            commands::monitors::pause_wallpaper,
            commands::monitors::play_wallpaper,
            commands::snapshot::capture_monitor_snapshot,
        ])
        .run(tauri::generate_context!())
        .expect("error while running hyprwall-gui");
}
