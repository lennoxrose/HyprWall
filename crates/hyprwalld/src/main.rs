mod app;
mod config;
mod ipc;
mod monitor;
mod monitor_registry;
mod render;
mod wayland;
mod zone;
mod zone_manager;

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use app::AppState;
use monitor_registry::MonitorRegistry;
use wayland::connection::WaylandBackend;

fn main() -> anyhow::Result<()> {
    let (backend, mut data) = WaylandBackend::new()?;
    let shared_monitors = Arc::new(Mutex::new(MonitorRegistry::new()));

    {
        let shared_monitors = Arc::clone(&shared_monitors);
        let mut event_queue = backend.event_queue;
        std::thread::spawn(move || loop {
            event_queue.blocking_dispatch(&mut data).expect("wayland dispatch failed");
            *shared_monitors.lock().unwrap() = data.monitors.clone();
        });
    }

    let socket_path = ipc::socket::socket_path();
    let listener = ipc::socket::bind_listener(&socket_path)?;
    let config_path = config::store::default_config_path();

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
        let registry = shared_monitors.lock().unwrap().clone();
        let mut state = AppState::new(registry, config_path.clone());
        let response = match hyprwall_ipc::parse_command(&line) {
            Ok(cmd) => ipc::handler::handle_command(&mut state, cmd),
            Err(e) => hyprwall_ipc::Response::Error(e.to_string()),
        };
        let _ = conn.write_all(response.to_wire().as_bytes());
    }
    Ok(())
}
