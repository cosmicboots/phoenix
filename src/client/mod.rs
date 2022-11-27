use crate::{
    config::{ClientConfig, Config},
    messaging::{
        self,
        arguments::{FileId, FileList, FileMetadata, FilePath, QualifiedChunk, QualifiedChunkId},
        Message, MessageBuilder,
    },
    net::{NetClient, NoiseConnection},
};
use base64ct::{Base64, Encoding};
use file_operations::Client;
use notify::{watcher, DebouncedEvent, Watcher};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{
    net::TcpStream,
    select,
    sync::mpsc::{self, Receiver, Sender},
};

mod file_operations;
mod utils;

pub use file_operations::CHUNK_SIZE;

pub type Blacklist = HashMap<PathBuf, FileMetadata>;

pub async fn start_client(config_file: &Path, path: &Path) {
    let config = ClientConfig::read_config(config_file).unwrap();

    let net_client = NetClient::new(
        TcpStream::connect(config.server_address).await.unwrap(),
        &Base64::decode_vec(&config.privkey).unwrap(),
        &[Base64::decode_vec(&config.server_pubkey).unwrap()],
    )
    .await
    .unwrap();

    let builder = messaging::MessageBuilder::new(1);
    let mut client = Client::new(builder, net_client);

    let watch_path = PathBuf::from(path);
    if !fs::metadata(&watch_path).unwrap().is_dir() {
        error!("Can only watch directories not files!");
        std::process::exit(1);
    }

    let (tx, _rx) = std::sync::mpsc::channel();
    let mut watcher = watcher(tx, Duration::from_secs(1)).unwrap();
    // TODO: figure out async filesystem watching
    let (_tx, mut fs_event): (Sender<DebouncedEvent>, Receiver<DebouncedEvent>) =
        mpsc::channel(100);

    info!("Watching files");
    watcher
        .watch(&watch_path, notify::RecursiveMode::Recursive)
        .unwrap();

    // Get startup file list to compare against local file tree
    client.request_file_list().await.unwrap();

    let mut blacklist: Blacklist = HashMap::new();
    loop {
        select! {
            // Server messages
            push = (&mut client).recv() => {
                match MessageBuilder::decode_message(&push.unwrap()) {
                    Ok(msg) => handle_server_event(&mut client, &watch_path, *msg, &mut blacklist).await,
                    Err(e) => error!("msg decode error: {:?}", e),
                } 
            }
            // Filesystem messages
            event = fs_event.recv() => {
                if event.is_some() {
                    handle_fs_event(
                        &mut client,
                        &watch_path.canonicalize().unwrap(),
                        event.unwrap(),
                        &mut blacklist).await;
                } else {
                    debug!("Failing fs_event checking");
                }
            }
        }
    }
}

async fn handle_server_event(
    client: &mut Client,
    watch_path: &Path,
    event: Message,
    blacklist: &mut Blacklist,
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
                    .await
                    .unwrap();
            }
            for file in server_files.difference(&local_files) {
                debug!("File not found locally: {:?}", file.path);
                let _ = client.request_file(file.clone()).await;
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
                    .await
                    .expect("Failed to queue chunk");
            }
        }
        messaging::Directive::SendFile => {
            if let Some(argument) = event.argument {
                let file_md = argument.as_any().downcast_ref::<FileMetadata>().unwrap();
                let path = file_md.file_id.path.clone();
                // The blacklist needs to be updated to make sure we dont send file information for
                // a in progress transfer
                blacklist.insert(path, file_md.clone());
                let mut _file = File::create(watch_path.join(&file_md.file_id.path)).unwrap();
                info!("Started file download: {:?}", &file_md.file_id.path);
                for (i, chunk) in file_md.chunks.iter().enumerate() {
                    let q_chunk = QualifiedChunkId {
                        path: file_md.file_id.clone(),
                        offset: (i * CHUNK_SIZE) as u32,
                        id: chunk.clone(),
                    };
                    client.request_chunk(q_chunk).await.unwrap();
                }
            }
        }
        messaging::Directive::SendQualifiedChunk => {
            if let Some(argument) = event.argument {
                if let Err(e) = utils::write_chunk(
                    blacklist,
                    &watch_path.canonicalize().unwrap(),
                    argument.as_any().downcast_ref::<QualifiedChunk>().unwrap(),
                ) {
                    error!("{}", e);
                }
            }
        }
        messaging::Directive::DeleteFile => todo!(),
        _ => {}
    };
}

async fn handle_fs_event(
    client: &mut Client,
    watch_path: &Path,
    event: DebouncedEvent,
    blacklist: &mut Blacklist,
) {
    match event {
        DebouncedEvent::Rename(_, p)
        | DebouncedEvent::Create(p)
        | DebouncedEvent::Write(p)
        | DebouncedEvent::Chmod(p) => {
            // Check the blacklist to make sure the event isn't from a partial file transfer
            if !blacklist.contains_key(p.strip_prefix(watch_path).unwrap()) {
                match client.send_file_info(watch_path, &p).await {
                    Ok(_) => {
                        info!("Successfully sent the file");
                    }
                    Err(e) => error!("{:?}", e),
                };
            }
        }
        DebouncedEvent::Remove(p) => {
            match client
                .delete_file(FilePath::new(p.strip_prefix(watch_path).unwrap()))
                .await
            {
                Ok(_) => info!("Successfully deleted the file"),
                Err(e) => error!("{:?}", e),
            }
        }
        _ => {}
    }
}
