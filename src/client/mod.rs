use base64ct::{Base64, Encoding};
use notify::{watcher, DebouncedEvent, Watcher};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::Write,
    net::TcpStream,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

mod file_operations;
mod utils;

use file_operations::Client;

use crate::{
    config::{ClientConfig, Config},
    messaging::{
        self,
        arguments::{FileId, FileList, FileMetadata, QualifiedChunkId},
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

fn handle_server_event(client: &mut Client, watch_path: &Path, event: Message) {
    let verb = event.verb.clone();
    match verb {
        messaging::Directive::SendFiles => {
            let files = utils::generate_file_list(watch_path).unwrap();
            let mut local_files: HashSet<FileId> = HashSet::new();
            for file in files.0 {
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
                debug!("File not found on server: {:?}", file.path);
            }
            for file in server_files.difference(&local_files) {
                debug!("File not found locally: {:?}", file.path);
                let _ = client.request_file(file.clone());
            }
        }
        messaging::Directive::RequestFile => todo!(),
        messaging::Directive::RequestChunk => {
            if let Some(argument) = event.argument {
                let chunk: &QualifiedChunkId = argument
                    .as_any()
                    .downcast_ref::<QualifiedChunkId>()
                    .unwrap();
                let path = watch_path.join(chunk.path.path.clone());
                client
                    .send_chunk(&chunk.id, &path)
                    .expect("Failed to queue chunk");
            }
        }
        messaging::Directive::SendFile => {
            if let Some(argument) = event.argument {
                let file_md = argument.as_any().downcast_ref::<FileMetadata>().unwrap();
                // TODO: this will cause the file metadata to be resent to the server as the file
                // is written to
                let mut file = File::create(watch_path.join(&file_md.file_id.path)).unwrap();
                let _ = file.write_all(format!("{:?}", file_md.chunks).as_bytes());
                info!("Wrote file to {:?}", &file_md.file_id.path);
            }
        }
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
                Ok(_) => {
                    info!("Successfully sent the file");
                }
                Err(e) => error!("{:?}", e),
            };
        }
        _ => {}
    }
}
