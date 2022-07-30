#![allow(dead_code)]

use super::{
    messaging::MessageBuilder,
    net::{NoiseConnection, Server},
    config::ServerConfig,
};
use std::{net::TcpListener, thread, sync::Arc};

pub fn start_server(config_file: &str) {
    let config = ServerConfig::read_config(config_file).unwrap();

    let private_key = Arc::new(config.privkey);

    // Construct TcpListener
    let listener = TcpListener::bind(&config.bind_address).unwrap();

    // Iterate through streams
    println!("Listening for connections on {}...", config.bind_address);
    for stream in listener.incoming() {
        println!("Spawning connection...");
        // Spawn thread to handle each stream
        let pk = private_key.clone();
        thread::spawn(move || {
            // Create new Server for use with noise layer
            let mut svc = Server::new(stream.unwrap(), pk.as_bytes());
            println!("Connection established!");
            // Complete noise handshake
            println!("Server completed handshake");
            for _ in 0..10 {
                let msg = MessageBuilder::decode_message(&svc.recv().unwrap());
                println!("{msg:?}");
            }
        });
    }
}
