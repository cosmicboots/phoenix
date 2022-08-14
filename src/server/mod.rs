mod db;

use base64ct::{Base64, Encoding};

use db::Db;

use crate::messaging::{arguments::FileMetadata, Directive};

use super::{
    config::{Config, ServerConfig},
    messaging::MessageBuilder,
    net::{NoiseConnection, NetServer},
};
use std::{net::TcpListener, sync::Arc, thread};

pub fn start_server(config_file: &str) {
    let config = Arc::new(ServerConfig::read_config(config_file).expect("Bad config"));

    let db = Arc::new(Db::new(&config.storage_path).expect("Failed to open database"));

    // Construct TcpListener
    let listener = TcpListener::bind(&config.bind_address).unwrap();

    // Iterate through streams
    println!("Listening for connections on {}...", config.bind_address);
    for stream in listener.incoming() {
        println!("Spawning connection...");
        // Spawn thread to handle each stream
        let config = config.clone();
        let db = db.clone();
        thread::spawn(move || {
            // Create new Server for use with noise layer
            let mut svc = NetServer::new(
                stream.unwrap(),
                &Base64::decode_vec(&config.privkey).expect("Couldn't decode private key"),
                &config
                    .clients
                    .iter()
                    .map(|x| Base64::decode_vec(x).unwrap())
                    .collect::<Vec<Vec<u8>>>(),
            )
            .unwrap();
            info!("Connection established!");
            // Complete noise handshake
            info!("Server completed handshake");
            while let Ok(raw_msg) = &svc.recv() {
                let msg = MessageBuilder::decode_message(raw_msg).unwrap();
                println!("{:?}", msg);
                match msg.verb {
                    Directive::SendFile => {
                        db.add_file(
                            msg.argument
                                .unwrap()
                                .as_any()
                                .downcast_ref::<FileMetadata>()
                                .unwrap(),
                        )
                        .expect("Failed to add file to database");
                    }
                    Directive::SendChunk => todo!(),
                    _ => todo!(),
                }
            }
            info!("Client disconnected");
        });
    }
}
