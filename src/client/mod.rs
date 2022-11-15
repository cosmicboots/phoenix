use crate::{
    config::{ClientConfig, Config},
    messaging::{
        self,
        arguments::{FileId, FileList, FileMetadata, QualifiedChunk, QualifiedChunkId},
        Message, MessageBuilder,
    },
    net::{NetClient, NoiseConnection},
};
use base64ct::{Base64, Encoding};
use file_operations::Client;
use notify::{watcher, DebouncedEvent, Watcher};
use std::{
    collections::HashSet,
    fs::{self, File},
    net::TcpStream,
    path::{Path, PathBuf},
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

mod file_operations;
mod utils;

pub use file_operations::CHUNK_SIZE;

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

    let blacklist: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));
    loop {
        if let Ok(msg) = incoming_msg.recv() {
            match msg {
                QueueItem::ServerMsg(push) => {
                    // TODO: decrypt this message using noise
                    let msg = MessageBuilder::decode_message(&push).unwrap();
                    handle_server_event(&mut client, &watch_path, *msg, blacklist.clone());
                }
                QueueItem::FileMsg(event) => {
                    handle_fs_event(
                        &mut client,
                        &watch_path.canonicalize().unwrap(),
                        event,
                        blacklist.clone(),
                    );
                }
            }
        }
    }
}

fn handle_server_event(
    client: &mut Client,
    watch_path: &Path,
    event: Message,
    blacklist: Arc<Mutex<HashSet<PathBuf>>>,
) {
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
                client
                    .send_file_info(watch_path, &watch_path.join(&file.path))
                    .unwrap();
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
                let path = file_md.file_id.path.clone();
                {
                    // The blacklist needs to be updated to make sure we dont send file information
                    // for a in progress transfer
                    let mut bl = blacklist.lock().unwrap();
                    bl.insert(path);
                    debug!("Added file to watcher blacklist. Current list: {:?}", bl);
                }
                let mut _file = File::create(watch_path.join(&file_md.file_id.path)).unwrap();
                info!("Started file download: {:?}", &file_md.file_id.path);
                for (i, chunk) in file_md.chunks.iter().enumerate() {
                    let q_chunk = QualifiedChunkId {
                        path: file_md.file_id.clone(),
                        offset: (i * CHUNK_SIZE) as u32,
                        id: chunk.clone(),
                    };
                    client.request_chunk(q_chunk).unwrap();
                }
            }
        }
        messaging::Directive::SendQualifiedChunk => {
            if let Some(argument) = event.argument {
                utils::write_chunk(
                    &watch_path.canonicalize().unwrap(),
                    argument.as_any().downcast_ref::<QualifiedChunk>().unwrap(),
                )
                .unwrap();
            }
        }
        messaging::Directive::DeleteFile => todo!(),
        _ => {}
    };
}

fn handle_fs_event(
    client: &mut Client,
    watch_path: &Path,
    event: DebouncedEvent,
    blacklist: Arc<Mutex<HashSet<PathBuf>>>,
) {
    match event {
        DebouncedEvent::Rename(_, p)
        | DebouncedEvent::Create(p)
        | DebouncedEvent::Write(p)
        | DebouncedEvent::Chmod(p) => {
            // Check the blacklist to make sure the event isn't from a partial file transfer
            let bl = blacklist.lock().unwrap();
            if !bl.contains(p.strip_prefix(watch_path).unwrap()) {
                match client.send_file_info(watch_path, &p) {
                    Ok(_) => {
                        info!("Successfully sent the file");
                    }
                    Err(e) => error!("{:?}", e),
                };
            }
        }
        _ => {}
    }
}
