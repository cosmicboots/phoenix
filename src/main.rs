//! Phoenix main entry point
mod db;
mod messaging;
mod net;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[clap(author, version, about)]
struct Cli {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Run {
        #[clap(short, long, action)]
        server: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Run { server } => {
            if server {
                println!("Waiting for connection...");
                let mut svc = net::Server::new();
                println!("Connection established!");
                svc.handshake();
                println!("Server completed handshake");
            } else {
                println!("Connecting...");
                let mut client = net::Client::new();
                println!("Connection established!");
                client.handshake();
                println!("Client completed handshake");
            }
        }
    }
}
