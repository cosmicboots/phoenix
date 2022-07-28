#![allow(dead_code)]

use super::{
    messaging::MessageBuilder,
    net::{NoiseConnection, Server},
    config::ServerConfig,
};
use std::{net::TcpListener, thread};

pub fn start_server() {
    let config = ServerConfig::read_config("./config.toml").unwrap();

    // Construct TcpListener
    let listener = TcpListener::bind(&config.bind_address).unwrap();

    // Iterate through streams
    println!("Listening for connections on {}...", config.bind_address);
    for stream in listener.incoming() {
        println!("Spawning connection...");
        // Spawn thread to handle each stream
        thread::spawn(|| {
            // Create new Server for use with noise layer
            let mut svc = Server::new(stream.unwrap());
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
