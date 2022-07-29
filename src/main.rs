//! Phoenix main entry point
mod client;
mod config;
mod messaging;
mod net;
mod server;

#[macro_use]
extern crate log;

use base64ct::{Base64, Encoding};
use clap::{ArgGroup, Parser, Subcommand};
use client::start_client;
use log::LevelFilter;
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
    #[clap(group(
            ArgGroup::new("")
                .required(true)
                .args(&["server", "test-client", "file-path"]),
            ))]
    Run {
        #[clap(long, action)]
        server: bool,
        #[clap(long, action)]
        test_client: bool,
        #[clap(value_parser)]
        file_path: Option<String>,
    },
    /// Generate Noise keypairs
    GenKey,
}

fn main() {
    env_logger::Builder::from_default_env()
        .filter_level(LevelFilter::Debug)
        .init();
    let cli = Cli::parse();

    match cli.command {
        Command::Run {
            server,
            test_client,
            file_path,
        } => {
            if server {
                server::start_server();
            } else if test_client {
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
            } else {
                if let Some(x) = file_path {
                    start_client(&x);
                }
            }
        }
        Command::GenKey => {
            let keypair = net::generate_noise_keypair();
            println!(
                "Private: {:?}\nPublic: {:?}",
                Base64::encode_string(&keypair.private),
                Base64::encode_string(&keypair.public)
            );
        }
    }
}
