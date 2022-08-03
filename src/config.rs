use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}};

pub trait Config: Serialize {
    fn read_config(filename: &str) -> Result<Self, toml::de::Error>
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
    pub storage_path: PathBuf,
    pub clients: Vec<String>,
}

impl Config for ServerConfig {
    fn read_config(filename: &str) -> Result<ServerConfig, toml::de::Error> {
        if Path::new(filename).exists() {
            let raw = fs::read_to_string(filename).expect("Failed to read file");
            let toml: ServerConfig = toml::from_str(&raw)?;
            Ok(toml)
        } else {
            warn!("Config file doesn't exist. Using defaults.");
            let config = ServerConfig {
                bind_address: "127.0.0.1:8080".to_string(),
                privkey: String::new(),
                storage_path: PathBuf::new(),
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

#[derive(Debug, Deserialize, Serialize)]
pub struct ClientConfig {
    pub privkey: String,
    pub server_address: String,
    pub server_pubkey: String,
}

impl Config for ClientConfig {
    fn read_config(filename: &str) -> Result<Self, toml::de::Error> {
        if Path::new(filename).exists() {
            let raw = fs::read_to_string(filename).expect("Failed to read file");
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
