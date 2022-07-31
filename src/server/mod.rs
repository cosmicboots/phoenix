#![allow(dead_code)]

use base64ct::{Base64, Encoding};

use super::{
    config::{Config, ServerConfig},
    messaging::MessageBuilder,
    net::{NoiseConnection, Server},
};
use std::{net::TcpListener, sync::Arc, thread};

pub fn start_server(config_file: &str) {
    let config = Arc::new(ServerConfig::read_config(config_file).unwrap());

    // Construct TcpListener
    let listener = TcpListener::bind(&config.bind_address).unwrap();

    // Iterate through streams
    println!("Listening for connections on {}...", config.bind_address);
    for stream in listener.incoming() {
        println!("Spawning connection...");
        // Spawn thread to handle each stream
        let config = config.clone();
        thread::spawn(move || {
            // Create new Server for use with noise layer
            let mut svc = Server::new(
                stream.unwrap(),
                &Base64::decode_vec(&config.privkey).unwrap(),
                &config
                    .clients
                    .iter()
                    .map(|x| Base64::decode_vec(x).unwrap())
                    .collect::<Vec<Vec<u8>>>()[..],
            ).unwrap();
            info!("Connection established!");
            // Complete noise handshake
            info!("Server completed handshake");
            while let Ok(msg) = &svc.recv() {
                println!("{:?}", MessageBuilder::decode_message(msg));
            }
            info!("Client disconnected");
        });
    }
}
