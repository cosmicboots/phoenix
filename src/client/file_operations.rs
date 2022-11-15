use crate::{
    client::utils::get_file_info,
    client::QueueItem,
    messaging::{
        arguments::{self, Argument, ChunkId, FileId, QualifiedChunkId},
        Directive, MessageBuilder,
    },
    net::{self, NetClient, NoiseConnection},
};
use std::{
    error::Error,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
    sync::{
        mpsc::{self, Receiver, SendError, Sender},
        Arc, Mutex,
    },
    thread,
};

pub const CHUNK_SIZE: usize = 1024; // 8 byte chunk size. TODO: automatically determine this.
                                    // Probably using file size ranges

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
                    error!("Failed to process msg_queue: {:?}", e);
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
    pub fn send_file_info(&mut self, base: &Path, path: &Path) -> Result<(), Box<dyn Error>> {
        let mut file_info = get_file_info(path)?;
        file_info.file_id.path = path.strip_prefix(base).unwrap().to_owned();
        let msg = self
            .builder
            .encode_message(Directive::SendFile, Some(file_info));
        self.msg_queue_tx.send(msg)?;
        Ok(())
    }

    /// Send a specific chunk from a given file
    pub fn send_chunk(
        &mut self,
        chunk_id: &ChunkId,
        file_path: &Path,
    ) -> Result<(), Box<dyn Error>> {
        let file_info = get_file_info(file_path)?;
        let mut file = File::open(&file_path)?;
        let mut hasher = blake3::Hasher::new();

        let chunk_index = file_info
            .chunks
            .iter()
            .position(|i| *i == *chunk_id)
            .expect("Attempted to get a chunk from a file that's changed");

        file.seek(SeekFrom::Start((chunk_index * CHUNK_SIZE) as u64))?;

        let mut buf = vec![0; CHUNK_SIZE];
        let len = file.read(&mut buf)?;

        hasher.update(&buf[..len]);
        let hash = hasher.finalize().as_bytes().to_vec();

        if chunk_id.to_bin() == hash {
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

        Ok(())
    }

    pub fn request_chunk(&mut self, chunk: QualifiedChunkId) -> Result<(), SendError<Vec<u8>>> {
        let msg = self
            .builder
            .encode_message::<arguments::QualifiedChunkId>(Directive::RequestChunk, Some(chunk));
        self.msg_queue_tx.send(msg)
    }

    pub fn request_file_list(&mut self) -> Result<(), Box<dyn Error>> {
        let msg = self
            .builder
            .encode_message::<arguments::Dummy>(Directive::ListFiles, None);
        self.msg_queue_tx.send(msg)?;
        Ok(())
    }

    pub fn request_file(&mut self, file: FileId) -> Result<(), SendError<Vec<u8>>> {
        let msg = self
            .builder
            .encode_message(Directive::RequestFile, Some(file));
        self.msg_queue_tx.send(msg)
    }
}
