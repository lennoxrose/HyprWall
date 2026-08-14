mod client;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use hyprwall_ipc::{Command, parse_response, Response};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Subcommand)]
enum CliCommand {
    /// List known monitor names
    MonitorList,
    /// Assign a wallpaper to one monitor, or a comma-separated list to span them as one zone
    Set { monitors: String, path: String },
    Pause { monitor: String },
    Play { monitor: String },
    Get { monitor: String },
}

fn socket_path() -> PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").expect("XDG_RUNTIME_DIR is not set");
    PathBuf::from(runtime_dir).join("hyprwall.sock")
}

fn main() {
    let cli = Cli::parse();
    let command = match cli.command {
        CliCommand::MonitorList => Command::MonitorList,
        CliCommand::Set { monitors, path } => Command::Set {
            monitors: monitors.split(',').map(str::to_string).collect(),
            path,
        },
        CliCommand::Pause { monitor } => Command::Pause { monitor },
        CliCommand::Play { monitor } => Command::Play { monitor },
        CliCommand::Get { monitor } => Command::Get { monitor },
    };

    match client::send(&socket_path(), &command) {
        Ok(response) => {
            match parse_response(&response) {
                Response::Error(msg) => {
                    eprintln!("hyprwallctl: {msg}");
                    std::process::exit(1);
                }
                _ => println!("{response}"),
            }
        }
        Err(e) => {
            eprintln!("hyprwallctl: {e}");
            std::process::exit(1);
        }
    }
}
