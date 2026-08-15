mod commands;

use clap::Parser;

/// Takes no runtime arguments today -- this exists so `--help`/`--version`
/// are handled properly (and an unrecognized flag is a clear error) instead
/// of silently being ignored and the GUI launching anyway.
#[derive(Parser)]
#[command(version, about = "HyprWall: browse and assign local video/image wallpapers")]
struct Cli;

fn main() {
    Cli::parse();

    // libmpv (used for thumbnail generation) refuses to initialize outside
    // the "C" locale for LC_NUMERIC; a normal desktop LANG/LC_* like
    // en_US.UTF-8 is enough to trip this. Only LC_NUMERIC is touched.
    unsafe { libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr()) };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(commands::library_watch::WatcherState::default())
        .invoke_handler(tauri::generate_handler![
            commands::audio_probe::has_audio_track,
            commands::config::get_library_folders,
            commands::config::set_library_folders,
            commands::config::get_default_fit_mode,
            commands::config::set_default_fit_mode,
            commands::library::scan_library,
            commands::library_watch::watch_library_folders,
            commands::monitors::list_monitors,
            commands::monitors::set_wallpaper,
            commands::monitors::unset_wallpaper,
            commands::monitors::pause_wallpaper,
            commands::monitors::play_wallpaper,
            commands::service::get_background_service_enabled,
            commands::service::set_background_service_enabled,
            commands::service::get_start_on_login_enabled,
            commands::service::set_start_on_login_enabled,
            commands::snapshot::capture_monitor_snapshot,
            commands::wallpaper_settings::get_wallpaper_settings,
            commands::wallpaper_settings::set_wallpaper_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running hyprwall-gui");
}
