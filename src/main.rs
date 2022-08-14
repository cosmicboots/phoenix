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
use config::{ClientConfig, Config, ServerConfig};
use log::LevelFilter;

#[derive(Parser)]
#[clap(author, version, about)]
struct Cli {
    /// Specify custom config file
    #[clap(long, short, value_parser)]
    config: Option<String>,
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the client
    #[clap(group(
            ArgGroup::new("")
                .required(true)
                .args(&["server", "file-path"]),
            ))]
    Run {
        #[clap(long, action)]
        server: bool,
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

    let config_file = find_config(cli.config);

    match cli.command {
        Command::Run { server, file_path } => {
            if server {
                server::start_server(&config_file);
            } else if let Some(arg) = file_path {
                start_client(&config_file, &arg);
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
        Command::DumpConfig {
            server,
            write,
            file_path,
        } => {
            if server {
                let config = ServerConfig::read_config(&config_file).unwrap();
                handle_dump_config(config, file_path, write);
            } else {
                let config = ClientConfig::read_config(&config_file).unwrap();
                handle_dump_config(config, file_path, write);
            }
        }
    }
}

fn handle_dump_config<T>(config: T, file_path: Option<String>, write: bool)
where
    T: Config,
{
    if write {
        config.write_config(&file_path.unwrap()).unwrap();
    } else {
        println!("{}", config.dump_config().unwrap());
    }
}

/// Find the config file location
///
/// In order of preference
/// 1. File specified with `--config` cli argument
/// 2. XDG_CONFIG_HOME/phoenix/config.toml
/// 3. ~/.config/phoenix/config.toml
/// 4. ./config.toml
fn find_config(config: Option<String>) -> String {
    if let Some(cfg) = config {
        return cfg;
    }
    String::from("config.toml")
}
