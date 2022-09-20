use std::{
    error::Error,
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom},
    path::Path,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
};

use sha2::{Digest, Sha256};

use crate::{
    messaging::{
        arguments::{self, Argument, ChunkId, FileId, FileList, FileMetadata},
        Directive, MessageBuilder,
    },
    net::{self, NetClient, NoiseConnection},
};

use super::QueueItem;

const CHUNK_SIZE: usize = 8; // 8 byte chunk size. TODO: automatically determine this. Probably
                             // using file size ranges

/// This struct is the main entry point for any operations that come from the client.
///
/// Any message that is transmitted through the network should be generated by this struct at a
/// high level.
pub struct Client {
    builder: MessageBuilder,
    msg_queue_tx: Sender<Vec<u8>>,
}

impl Client {
    pub fn new(
        builder: MessageBuilder,
        net_client: NetClient,
        msg_queue: Sender<QueueItem>,
    ) -> Self {
        let net_client = Arc::new(Mutex::new(net_client));

        let (tx, rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = mpsc::channel();
        let client = net_client.clone();
        thread::spawn(move || {
            while let Ok(data) = rx.recv() {
                if let Err(e) = client.lock().unwrap().send(&data) {
                    // TODO: handle errors. Possibly requeue them
                    error!("{:?}", e);
                };
            }
        });

        let mut raw_stream = net_client.lock().unwrap().clone_stream().unwrap();
        let client = net_client;
        thread::spawn(move || {
            while let Ok(raw_msg) = net::recv(&mut raw_stream) {
                if let Ok(msg) = client.lock().unwrap().decrypt(&raw_msg) {
                    msg_queue.send(QueueItem::ServerMsg(msg)).unwrap();
                }
            }
        });

        Client {
            builder,
            msg_queue_tx: tx,
        }
    }

    /// Send file metadata to the server
    pub fn send_file_info(
        &mut self,
        base: &Path,
        path: &Path,
    ) -> Result<Vec<ChunkId>, Box<dyn Error>> {
        let mut file_info = get_file_info(path)?;
        file_info.file_id.path = path.strip_prefix(base).unwrap().to_owned();
        let chunks = file_info.chunks.clone();
        let msg = self
            .builder
            .encode_message(Directive::SendFile, Some(file_info));
        self.msg_queue_tx.send(msg)?;
        Ok(chunks)
    }

    pub fn send_chunks(&mut self, path: &Path, chunks: Vec<ChunkId>) -> Result<(), Box<dyn Error>> {
        let mut file = File::open(path)?;
        let mut hasher = Sha256::new();

        file.seek(SeekFrom::Start(0))?;

        for id in chunks.iter() {
            let mut buf = vec![0; CHUNK_SIZE];
            let len = file.read(&mut buf)?;
            hasher.update(&buf[..len]);
            let hash: Vec<u8> = hasher.finalize_reset().to_vec();
            if id.to_bin() == hash {
                let chunk = arguments::Chunk {
                    id: arguments::ChunkId(hash),
                    data: buf[..len].to_vec(),
                };
                let msg = self
                    .builder
                    .encode_message(Directive::SendChunk, Some(chunk));
                self.msg_queue_tx.send(msg)?;
            } else {
                panic!("Chunks don't match up. File must have changed. This error will be handled in the future")
            }
        }
        Ok(())
    }

    pub fn request_file_list(&mut self) -> Result<(), Box<dyn Error>> {
        let msg = self
            .builder
            .encode_message::<arguments::Dummy>(Directive::ListFiles, None);
        self.msg_queue_tx.send(msg)?;
        Ok(())
    }
}

/// Calculate chunk boundries and file hash
fn chunk_file(path: &Path) -> Result<Vec<[u8; 32]>, io::Error> {
    let mut file = File::open(path)?;
    let size = file.metadata().unwrap().len();

    let mut hasher = Sha256::new();

    let mut chunks: Vec<[u8; 32]> = vec![];

    file.seek(SeekFrom::Start(0))?;

    for _ in 0..(size as f32 / CHUNK_SIZE as f32).ceil() as usize {
        let mut buf = vec![0; CHUNK_SIZE];
        let len = file.read(&mut buf)?;
        hasher.update(&buf[..len]);
        chunks.push(hasher.finalize_reset().into());
    }

    Ok(chunks)
}

/// Get the file metadata from a file at a given path.
pub fn get_file_info(path: &Path) -> Result<FileMetadata, std::io::Error> {
    let md = fs::metadata(path)?;
    let file_id = FileId::new(path.to_owned())?;
    let chunks = chunk_file(path)?;
    Ok(FileMetadata::new(file_id, md, &chunks).unwrap())
}

/// Generate a file listing of the watched directory.
///
/// This will be used to preform an initial synchronization when the clients connect.
pub fn generate_file_list(path: &Path) -> Result<FileList, std::io::Error> {
    Ok(FileList(recursive_file_list(path, path)?))
}

fn recursive_file_list(base: &Path, path: &Path) -> Result<Vec<FileId>, std::io::Error> {
    let mut files: Vec<FileId> = vec![];

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.append(&mut recursive_file_list(base, &path)?);
        } else {
            let mut file_info = get_file_info(&path)?.file_id;
            file_info.path = file_info.path.strip_prefix(base).unwrap().to_owned();
            files.push(file_info);
        }
    }
    Ok(files)
}
