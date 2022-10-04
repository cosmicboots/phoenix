use base64ct::{Base64, Encoding};
use notify::{watcher, DebouncedEvent, Watcher};
use std::{
    collections::HashSet,
    fs,
    net::TcpStream,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

mod file_operations;

use file_operations::Client;

use crate::{
    config::{ClientConfig, Config},
    messaging::{
        self,
        arguments::{ChunkId, FileId, FileList, QualifiedChunkId},
        Message, MessageBuilder,
    },
    net::{NetClient, NoiseConnection},
};

#[derive(Debug)]
pub enum QueueItem {
    ServerMsg(Vec<u8>),
    FileMsg(DebouncedEvent),
}

pub fn start_client(config_file: &Path, path: &Path) {
    let config = ClientConfig::read_config(config_file).unwrap();

    let net_client = NetClient::new(
        TcpStream::connect(config.server_address).unwrap(),
        &Base64::decode_vec(&config.privkey).unwrap(),
        &[Base64::decode_vec(&config.server_pubkey).unwrap()],
    )
    .unwrap();

    let (msg_queue, incoming_msg): (Sender<QueueItem>, Receiver<QueueItem>) = mpsc::channel();

    let builder = messaging::MessageBuilder::new(1);
    let mut client = Client::new(builder, net_client, msg_queue.clone());

    let watch_path = PathBuf::from(path);

    if !fs::metadata(&watch_path).unwrap().is_dir() {
        error!("Can only watch directories not files!");
        std::process::exit(1);
    }

    let (tx, rx) = mpsc::channel();
    let mut watcher = watcher(tx, Duration::from_secs(1)).unwrap();
    let tx = msg_queue;
    thread::spawn(move || {
        while let Ok(x) = rx.recv() {
            tx.send(QueueItem::FileMsg(x)).unwrap();
        }
    });

    info!("Watching files");
    watcher
        .watch(&watch_path, notify::RecursiveMode::Recursive)
        .unwrap();

    client.request_file_list().unwrap();

    loop {
        if let Ok(msg) = incoming_msg.recv() {
            match msg {
                QueueItem::ServerMsg(push) => {
                    // TODO: decrypt this message using noise
                    let msg = MessageBuilder::decode_message(&push).unwrap();
                    handle_server_event(&mut client, &watch_path, *msg);
                }
                QueueItem::FileMsg(event) => {
                    handle_fs_event(&mut client, &watch_path.canonicalize().unwrap(), event)
                }
            }
        }
    }
}

fn handle_server_event(_client: &mut Client, watch_path: &Path, event: Message) {
    debug!("Server message: {:?}", event);
    let verb = event.verb.clone();
    match verb {
        messaging::Directive::SendFiles => {
            let files = file_operations::generate_file_list(watch_path).unwrap();
            let mut local_files: HashSet<FileId> = HashSet::new();
            for file in files.0 {
                debug!("Found File: {:?}", file.path);
                local_files.insert(file);
            }

            let mut server_files: HashSet<FileId> = HashSet::new();

            if let Some(argument) = event.argument {
                let files = argument.as_any().downcast_ref::<FileList>().unwrap();

                for file in &files.0 {
                    server_files.insert(file.clone());
                }
            }

            for file in local_files.difference(&server_files) {
                debug!("File not on server: {:?}", file.path);
            }
        }
        messaging::Directive::RequestFile => todo!(),
        messaging::Directive::RequestChunk => {
            if let Some(argument) = event.argument {
                let _chunk_id = argument
                    .as_any()
                    .downcast_ref::<QualifiedChunkId>()
                    .unwrap();
            }
        }
        messaging::Directive::SendFile => todo!(),
        messaging::Directive::SendChunk => todo!(),
        messaging::Directive::DeleteFile => todo!(),
        _ => {}
    };
}

fn handle_fs_event(client: &mut Client, watch_path: &Path, event: DebouncedEvent) {
    match event {
        DebouncedEvent::Rename(_, p)
        | DebouncedEvent::Create(p)
        | DebouncedEvent::Write(p)
        | DebouncedEvent::Chmod(p) => {
            match client.send_file_info(watch_path, &p) {
                Ok(chunks) => {
                    info!("Successfully sent the file");
                    client.send_chunks(&p, chunks).unwrap();
                }
                Err(e) => error!("{:?}", e),
            };
        }
        _ => {}
    }
}
