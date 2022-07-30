//! Phoenix main entry point
mod client;
mod common;
mod config;
mod messaging;
mod net;
mod server;

#[macro_use]
extern crate log;

use base64ct::{Base64, Encoding};
use clap::{ArgGroup, Parser, Subcommand};
use client::start_client;
use config::ServerConfig;
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
    /// Dump the current config.
    ///
    /// Default values are used if config doesn't exist
    DumpConfig {
        #[clap(long, action)]
        /// Generate server config
        server: bool,
        #[clap(long, action)]
        /// Write the config to a file rather than stdout
        write: bool,
        #[clap(requires = "write")]
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

    let config_file = find_config();

    match cli.command {
        Command::Run {
            server,
            test_client,
            file_path,
        } => {
            if server {
                server::start_server(&config_file);
            } else if test_client {
                println!("Connecting...");
                let mut client = net::Client::new(TcpStream::connect("127.0.0.1:8080").unwrap(), "".as_bytes());
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
                "Private: {}\nPublic: {}",
                Base64::encode_string(&keypair.private),
                Base64::encode_string(&keypair.public)
            );
        }
        Command::DumpConfig { server, write, file_path } => {
            if server {
                let config = ServerConfig::read_config(&config_file).unwrap();
                if write {
                    config.write_config(&file_path.unwrap()).unwrap();
                } else {
                    println!("{}", config.dump_config().unwrap());
                }
            } else {
                todo!()
            }
        }
    }
}

/// Fine the config file location
///
/// In order of preference
/// 1. XDG_CONFIG_HOME/phoenix/config.toml
/// 2. ~/.config/phoenix/config.toml
/// 3. ./config.toml
fn find_config() -> String {
    String::from("config.toml")
}
