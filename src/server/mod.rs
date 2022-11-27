mod db;

use super::{
    config::{Config, ServerConfig},
    messaging::MessageBuilder,
    net::{NetServer, NoiseConnection},
};
use crate::{
    client::CHUNK_SIZE,
    messaging::{
        arguments::{Chunk, FileId, FileMetadata, FilePath, QualifiedChunk, QualifiedChunkId},
        Directive,
    },
};
use base64ct::{Base64, Encoding};
use db::error::DbError;
use db::Db;
use std::{path::Path, sync::Arc, time::Duration};
use tokio::{
    net::TcpListener,
    select,
    sync::mpsc::{self, Receiver, Sender},
};

type TxRxHandles = (Sender<Sender<Vec<u8>>>, Receiver<Sender<Vec<u8>>>);

pub async fn start_server(config_file: &Path) {
    let config = Arc::new(ServerConfig::read_config(config_file).expect("Bad config"));
    let db = Arc::new(Db::new(&config.storage_path).expect("Failed to open database"));

    // Construct TcpListener
    let listener = TcpListener::bind(&config.bind_address).await.unwrap();

    // Store channel senders for each client connection thread
    let (threads_tx, mut threads_rx): TxRxHandles = mpsc::channel(100);
    let (broadcast_tx, mut broadcast_rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = mpsc::channel(100);

    // Broadcast thread
    tokio::spawn(async move {
        let mut threads: Vec<Sender<Vec<u8>>> = vec![];
        let mut remove_queue: Vec<usize> = vec![];
        loop {
            select! {
                t = (&mut threads_rx).recv() => {
                    match t {
                        None => error!("threads_rx channel dropped"),
                        Some(x) => {
                            threads.push(x);
                            debug!("Added a client thread to the broadcast system.");
                        }
                    }
                },
                raw_msg = (&mut broadcast_rx).recv() => {
                    if let Some(msg) = raw_msg {
                        for (i, thread) in threads.iter().enumerate() {
                            if thread.send(msg.clone()).await.is_err() {
                                // Assume the recieving thread died
                                remove_queue.push(i);
                            }
                        };
                        debug!("Broadcasted a message through the system.");
                    }
                },
            };
            while let Some(i) = remove_queue.pop() {
                debug!("Removed an old broadcast channel handel");
                threads.remove(i);
            }
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
    });

    // Iterate through streams
    println!("Listening for connections on {}...", config.bind_address);
    loop {
        let (stream, _) = listener.accept().await.unwrap();
        println!("Spawning connection...");

        // Create channel to to recieve push events
        let (msg_tx, mut msg_rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = mpsc::channel(100);
        debug!("threads_tx still alive: {:?}", threads_tx);
        threads_tx.send(msg_tx).await.unwrap();

        // Spawn thread to handle each stream
        let config = config.clone();
        let db = db.clone();
        let broadcast = broadcast_tx.clone();
        tokio::spawn(async move {
            // Create new Server for use with noise layer
            let mut svc = NetServer::new(
                stream,
                &Base64::decode_vec(&config.privkey).expect("Couldn't decode private key"),
                &config
                    .clients
                    .iter()
                    .map(|x| Base64::decode_vec(x).unwrap())
                    .collect::<Vec<Vec<u8>>>(),
            )
            .await
            .unwrap();
            info!("Connection established!");

            //while let Ok(raw_msg) = &svc.recv().await {}
            let mut msg_builder = MessageBuilder::new(1);
            loop {
                select! {
                    // Messages from the client
                    raw_msg = svc.recv() => {
                        match raw_msg {
                            Ok(msg) => {
                                handle_client_msg(&mut svc,
                                    &db,
                                    &mut msg_builder,
                                    &broadcast,
                                    &msg).await;
                            },
                            Err(_) => break,
                        }
                    }
                    // Messages from the broadcast system
                    msg = msg_rx.recv() => {
                        svc.send(&msg.unwrap()).await.unwrap();
                    }
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

async fn handle_client_msg(
    svc: &mut NetServer,
    db: &Db,
    msg_builder: &mut MessageBuilder,
    broadcast: &Sender<Vec<u8>>,
    raw_msg: &[u8],
) {
    let msg = MessageBuilder::decode_message(raw_msg).unwrap();
    msg_builder.increment_counter();
    match msg.verb {
        Directive::SendFile => {
            let argument = msg.argument.unwrap();
            let metadata = argument.as_any().downcast_ref::<FileMetadata>().unwrap();
            //let file_id = metadata.file_id.clone();

            let chunks = match db.add_file(metadata) {
                Ok(x) => {
                    if x.len() == 0 {
                        // File is already completed
                        let rmsg =
                            msg_builder.encode_message(Directive::SendFile, Some(metadata.clone()));
                        broadcast.send(rmsg).await.unwrap();
                    }
                    x
                }
                Err(DbError::DuplicateFile) => vec![],
                Err(_) => panic!("Failed to add file to database"),
            };

            for (i, chunk) in chunks.iter().enumerate() {
                let qualified_chunk = QualifiedChunkId {
                    path: metadata.file_id.clone(),
                    offset: (i * CHUNK_SIZE) as u32,
                    id: chunk.clone(),
                };
                let msg =
                    msg_builder.encode_message(Directive::RequestChunk, Some(qualified_chunk));
                let _ = &svc.send(&msg).await;
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
                let file_md = db.get_file(id.path.to_str().unwrap()).unwrap().unwrap();
                let rmsg = msg_builder.encode_message(Directive::SendFile, Some(file_md));
                broadcast.send(rmsg).await.unwrap();
            }
        }
        Directive::ListFiles => {
            let files = db.get_files().unwrap();
            debug!("Sending file list to client");
            let msg = msg_builder.encode_message(Directive::SendFiles, Some(files));
            let _ = &svc.send(&msg).await;
        }
        Directive::RequestFile => {
            let argument = msg.argument.unwrap();
            let file_id = argument.as_any().downcast_ref::<FileId>().unwrap();
            let file = db
                .get_file(file_id.path.to_str().unwrap())
                .unwrap()
                .unwrap();
            let msg = msg_builder.encode_message(Directive::SendFile, Some(file));
            let _ = &svc.send(&msg).await;
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
            let msg = msg_builder
                .encode_message::<QualifiedChunk>(Directive::SendQualifiedChunk, Some(q_chunk));
            let _ = &svc.send(&msg).await;
        }
        Directive::DeleteFile => {
            let argument = msg.argument.unwrap();
            let file_path = argument.as_any().downcast_ref::<FilePath>().unwrap();
            db.rm_file(file_path);
            debug!("Removed {:?} from the database", file_path);
        }
        _ => todo!(),
    }
}
