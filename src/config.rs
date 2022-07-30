use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Deserialize, Serialize)]
pub struct ServerConfig {
    pub bind_address: String,
    pub privkey: String,
}

impl ServerConfig {
    pub fn read_config(filename: &str) -> Result<ServerConfig, toml::de::Error> {
        if Path::new(filename).exists() {
            let raw = fs::read_to_string(filename).expect("Failed to read file");
            let toml: ServerConfig = toml::from_str(&raw)?;
            Ok(toml)
        } else {
            warn!("Config file doesn't exist. Using defaults.");
            let config = ServerConfig {
                bind_address: "127.0.0.1:8080".to_string(),
                privkey: String::new(),
            };
            Ok(config)
        }
    }

    #[allow(dead_code)]
    pub fn write_config(&self, filename: &str) -> Result<(), toml::ser::Error> {
        fs::write(filename, toml::to_string(&self)?).expect("Failed to write config");
        Ok(())
    }

    pub fn dump_config(&self) -> Result<String, toml::ser::Error> {
        toml::to_string(&self)
    }
}
