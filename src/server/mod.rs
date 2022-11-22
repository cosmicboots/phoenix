mod db;

use base64ct::{Base64, Encoding};

use crossbeam_channel::{select, Receiver, Sender};
use db::Db;

use crate::{
    client::CHUNK_SIZE,
    messaging::{
        arguments::{Chunk, FileId, FileMetadata, QualifiedChunk, QualifiedChunkId},
        Directive,
    },
};

use super::{
    config::{Config, ServerConfig},
    messaging::MessageBuilder,
    net::{NetServer, NoiseConnection},
};
use std::{net::TcpListener, path::Path, sync::Arc, thread};

pub fn start_server(config_file: &Path) {
    let config = Arc::new(ServerConfig::read_config(config_file).expect("Bad config"));
    let db = Arc::new(Db::new(&config.storage_path).expect("Failed to open database"));

    // Construct TcpListener
    let listener = TcpListener::bind(&config.bind_address).unwrap();

    // Store channel senders for each client connection thread
    let (threads_tx, threads_rx): (Sender<Sender<Vec<u8>>>, Receiver<Sender<Vec<u8>>>) =
        crossbeam_channel::unbounded();
    let (broadcast_tx, broadcast_rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) =
        crossbeam_channel::unbounded();

    // Broadcast thread
    thread::spawn(move || {
        let mut threads: Vec<Sender<Vec<u8>>> = vec![];
        loop {
            select! {
                recv(threads_rx) -> t => {
                    threads.push(t.unwrap());
                    debug!("Added a client thread to the broadcast system.");
                },
                recv(broadcast_rx) -> raw_msg => {
                    if let Ok(msg) = raw_msg {
                        for thread in &threads {
                            thread.send(msg.clone()).unwrap();
                            debug!("Broadcasted a message through the system.");
                        };
                    }
                },
            };
        }
    });

    // Iterate through streams
    println!("Listening for connections on {}...", config.bind_address);
    for stream in listener.incoming() {
        println!("Spawning connection...");

        // Create channel to to recieve push events
        let (msg_tx, msg_rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = crossbeam_channel::unbounded();
        threads_tx.send(msg_tx).unwrap();

        // Spawn thread to handle each stream
        let config = config.clone();
        let db = db.clone();
        let broadcast = broadcast_tx.clone();
        thread::spawn(move || {
            // TODO: Handle broadcast messages
            let _bcast_rx = msg_rx;

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

            while let Ok(raw_msg) = &svc.recv() {
                let mut msg_builder = MessageBuilder::new(1);
                let msg = MessageBuilder::decode_message(raw_msg).unwrap();
                msg_builder.increment_counter();
                match msg.verb {
                    Directive::SendFile => {
                        let argument = &msg.argument.unwrap();
                        let metadata = argument.as_any().downcast_ref::<FileMetadata>().unwrap();
                        //let file_id = metadata.file_id.clone();
                        let chunks = db
                            .add_file(metadata)
                            .expect("Failed to add file to database");

                        for (i, chunk) in chunks.iter().enumerate() {
                            let qualified_chunk = QualifiedChunkId {
                                path: metadata.file_id.clone(),
                                offset: (i * CHUNK_SIZE) as u32,
                                id: chunk.clone(),
                            };
                            let msg = msg_builder
                                .encode_message(Directive::RequestChunk, Some(qualified_chunk));
                            let _ = &svc.send(&msg);
                        }
                    }
                    Directive::SendChunk => {
                        let complete = db
                            .add_chunk(
                                msg.argument
                                    .unwrap()
                                    .as_any()
                                    .downcast_ref::<Chunk>()
                                    .unwrap(),
                            )
                            .expect("Failed to add chunk to database");

                        // If the file is complete, broadcast a fake `SendFile` message for every
                        // thread to forward to the client
                        if let Some(id) = complete {
                            broadcast
                                .send(msg_builder.encode_message(Directive::SendFile, Some(id)))
                                .unwrap();
                        }
                    }
                    Directive::ListFiles => {
                        let files = db.get_files().unwrap();
                        debug!("Sending file list to client");
                        let msg = msg_builder.encode_message(Directive::SendFiles, Some(files));
                        let _ = &svc.send(&msg);
                    }
                    Directive::RequestFile => {
                        let argument = msg.argument.unwrap();
                        let file_id = argument.as_any().downcast_ref::<FileId>().unwrap();
                        let file = db.get_file(file_id.path.to_str().unwrap()).unwrap();
                        let msg = msg_builder.encode_message(Directive::SendFile, Some(file));
                        let _ = &svc.send(&msg);
                    }
                    Directive::RequestChunk => {
                        let argument = msg.argument.unwrap();
                        let chunk_id = argument
                            .as_any()
                            .downcast_ref::<QualifiedChunkId>()
                            .unwrap();
                        let mut buf = [0u8; 32];
                        buf.copy_from_slice(&chunk_id.id.0);
                        let chunk = db.get_chunk(buf).unwrap();
                        let q_chunk = QualifiedChunk {
                            id: chunk_id.clone(),
                            data: chunk.data,
                        };
                        let msg = msg_builder.encode_message::<QualifiedChunk>(
                            Directive::SendQualifiedChunk,
                            Some(q_chunk),
                        );
                        let _ = &svc.send(&msg);
                    }
                    _ => todo!(),
                }
            }
            info!("Client disconnected");
        });
    }
}

pub fn dump_data(config_file: &Path) {
    let config = Arc::new(ServerConfig::read_config(config_file).expect("Bad config"));
    let db = Db::new(&config.storage_path).expect("Failed to open database");
    db.dump_tree();
}
