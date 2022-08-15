//! This module provides the configuration file structure for both the client and the server.

use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

pub trait Config: Serialize {
    fn read_config(path: &Path) -> Result<Self, toml::de::Error>
    where
        Self: Sized;

    fn write_config(&self, filename: &str) -> Result<(), toml::ser::Error> {
        fs::write(filename, toml::to_string(&self)?).expect("Failed to write config");
        Ok(())
    }

    fn dump_config(&self) -> Result<String, toml::ser::Error> {
        toml::to_string(&self)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ServerConfig {
    pub bind_address: String,
    pub privkey: String,
    #[serde(default = "get_server_storage_path")]
    pub storage_path: PathBuf,
    pub clients: Vec<String>,
}

impl Config for ServerConfig {
    fn read_config(path: &Path) -> Result<ServerConfig, toml::de::Error> {
        if path.exists() {
            let raw = fs::read_to_string(path).expect("Failed to read file");
            let toml: ServerConfig = toml::from_str(&raw)?;
            Ok(toml)
        } else {
            warn!("Config file doesn't exist. Using defaults.");
            let config = ServerConfig {
                bind_address: "127.0.0.1:8080".to_string(),
                privkey: String::new(),
                storage_path: get_server_storage_path(),
                clients: vec![],
            };
            Ok(config)
        }
    }

    #[allow(dead_code)]
    fn write_config(&self, filename: &str) -> Result<(), toml::ser::Error> {
        fs::write(filename, toml::to_string(&self)?).expect("Failed to write config");
        Ok(())
    }

    fn dump_config(&self) -> Result<String, toml::ser::Error> {
        toml::to_string(&self)
    }
}

fn get_server_storage_path() -> PathBuf {
    let mut base_path = PathBuf::new();
    if let Ok(var) = env::var("XDG_DATA_HOME") {
        base_path = PathBuf::from(var);
    } else if let Ok(home) = env::var("HOME") {
        let default_path = PathBuf::from(home).join(".local/share");
        debug!("Default path: {:?}", default_path);
        if default_path.exists() {
            base_path = default_path;
            debug!("Base path: {:?}", base_path);
        }
    }
    debug!("Base path: {:?}", base_path);
    base_path.join("phoenix")
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ClientConfig {
    pub privkey: String,
    pub server_address: String,
    pub server_pubkey: String,
}

impl Config for ClientConfig {
    fn read_config(path: &Path) -> Result<Self, toml::de::Error> {
        if path.exists() {
            let raw = fs::read_to_string(path).expect("Failed to read file");
            let toml: ClientConfig = toml::from_str(&raw)?;
            Ok(toml)
        } else {
            warn!("Config file doesn't exist. Using defaults.");
            let config = ClientConfig {
                privkey: String::new(),
                server_address: "127.0.0.1:8080".to_string(),
                server_pubkey: String::new(),
            };
            Ok(config)
        }
    }
}
