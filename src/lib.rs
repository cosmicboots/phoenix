//! Phoenix is a chunk-based file synchronization platform using a custom binary
//! protocol that sits on-top of the [Noise Protocol](https://noiseprotocol.org/).
//!
//! This crate is a library which can be used to create custom client and server interfaces.
//!
//! The basic flow that should be followed when creating a Phoenix client is:
//!
//! 1. **Load in the configuration.**  
//!    The [`find_config`](fn.find_config.html) should be helpful.
//! 2. **Used the loaded configuration to start the client or server.**  
//!    This can be done with the [`start_client`](client/fn.start_client.html) or
//!    [`start_server`](server/fn.start_server.html) functions.

pub mod client;
pub mod config;
mod messaging;
mod net;
pub mod server;

use std::{env, path::PathBuf};

pub use client::start_client;
use log::info;
pub use net::generate_noise_keypair;
pub use server::dump_data;
pub use server::start_server;

/// Find the config file location
///
/// In order of preference
/// 1. File specified with `--config` cli argument
/// 2. XDG_CONFIG_HOME/phoenix/config.toml
/// 3. ~/.config/phoenix/config.toml
/// 4. ./config.toml
pub fn find_config(config: Option<PathBuf>) -> PathBuf {
    // File spcified with --config flag
    if let Some(cfg) = config {
        return cfg;
    }

    // Fall back paths
    let mut base_path = PathBuf::from("config.toml");
    if let Ok(path_str) = env::var("XDG_CONFIG_HOME") {
        let path = PathBuf::from(path_str).join("phoenix");
        if path.is_dir() {
            base_path = path.join("config.toml");
        }
    } else if let Ok(home) = env::var("HOME") {
        let path = PathBuf::from(home).join(".config/phoenix");
        if path.is_dir() {
            base_path = path.join("config.toml");
        }
    }
    info!("Using {:?} as config path", base_path);
    base_path
}
