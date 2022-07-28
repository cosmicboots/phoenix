//! Phoenix main entry point
mod messaging;
mod net;
mod server;
mod config;

use clap::{Parser, Subcommand};
use net::NoiseConnection;
use std::net::TcpStream;

#[derive(Parser)]
#[clap(author, version, about)]
struct Cli {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the client
    Run {
        #[clap(short, long, action)]
        server: bool,
    },
    /// Generate Noise keypairs
    GenKey,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Run { server } => {
            if server {
                server::start_server();
            } else {
                println!("Connecting...");
                let mut client = net::Client::new(TcpStream::connect("127.0.0.1:8080").unwrap());
                println!("Connection established!");
                println!("Client completed handshake");
                let mut builder = messaging::MessageBuilder::new(1);
                for _ in 0..10 {
                    let msg = builder.encode_message(
                        messaging::Directive::AnnounceVersion,
                        Some(messaging::arguments::Version(1)),
                    );
                    println!("Sending message... {msg:?}");
                    client.send(&msg);
                    println!("Message sent");
                    std::thread::sleep(std::time::Duration::from_secs(5));
                }
            }
        }
        Command::GenKey => {
            let keypair = net::generate_noise_keypair();
            println!("Private: {:?}\nPublic: {:?}", base64::encode(keypair.private), base64::encode(keypair.public));
        },
    }
}
