//! Phoenix Command Line Interface

use base64ct::{Base64, Encoding};
use clap::{ArgGroup, Parser, Subcommand};
use phoenix::{
    client::Client,
    config::{ClientConfig, Config, ServerConfig},
};
use std::path::PathBuf;

#[derive(Parser)]
#[clap(author, version, about)]
struct Cli {
    /// Specify custom config file
    #[clap(long, short, value_parser)]
    config: Option<PathBuf>,
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
        file_path: Option<PathBuf>,
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
    /// Dump the server database
    DumpDb,
    /// Generate Noise keypairs
    GenKey,
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_default_env()
        //.filter_level(LevelFilter::Debug)
        .init();
    let cli = Cli::parse();

    let config_file = phoenix::find_config(cli.config);

    match cli.command {
        Command::Run { server, file_path } => {
            if server {
                phoenix::start_server(&config_file).await;
            } else if let Some(arg) = file_path {
                let config = ClientConfig::read_config(&config_file).unwrap();
                let client = Client::new(config, &arg);
                client.start();
                loop {}
            }
        }
        Command::DumpDb => {
            phoenix::dump_data(&config_file);
        }
        Command::GenKey => {
            let keypair = phoenix::generate_noise_keypair();
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
                phoenix::config::handle_dump_config(config, file_path, write);
            } else {
                let config = ClientConfig::read_config(&config_file).unwrap();
                phoenix::config::handle_dump_config(config, file_path, write);
            }
        }
    }
}
