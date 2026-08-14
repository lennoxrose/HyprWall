mod app;
mod config;
mod ipc;
mod monitor;
mod monitor_registry;
mod zone;
mod zone_manager;

use std::io::{Read, Write};

use app::AppState;
use monitor_registry::MonitorRegistry;

fn main() {
    let socket_path = ipc::socket::socket_path();
    let listener = ipc::socket::bind_listener(&socket_path).unwrap_or_else(|e| {
        eprintln!("hyprwalld: {e}");
        std::process::exit(1);
    });

    let config_path = config::store::default_config_path();
    // Task 7 replaces this empty registry with one populated from real
    // Wayland outputs; until then every command that needs a monitor name
    // will correctly report it as unknown.
    let mut state = AppState::new(MonitorRegistry::new(), config_path);

    println!("hyprwalld listening on {}", socket_path.display());
    for conn in listener.incoming() {
        let mut conn = match conn {
            Ok(c) => c,
            Err(e) => {
                eprintln!("hyprwalld: connection error: {e}");
                continue;
            }
        };
        let mut line = String::new();
        if conn.read_to_string(&mut line).is_err() {
            continue;
        }
        let response = match hyprwall_ipc::parse_command(&line) {
            Ok(cmd) => ipc::handler::handle_command(&mut state, cmd),
            Err(e) => hyprwall_ipc::Response::Error(e.to_string()),
        };
        let _ = conn.write_all(response.to_wire().as_bytes());
    }
}
