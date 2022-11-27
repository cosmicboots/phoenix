use crate::{
    client::utils::get_file_info,
    messaging::{
        arguments::{self, Argument, ChunkId, FileId, FilePath, QualifiedChunkId},
        Directive, MessageBuilder,
    },
    net::{error::NetError, NetClient, NoiseConnection},
};
use std::{
    error::Error,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

pub const CHUNK_SIZE: usize = 1024; // 8 byte chunk size. TODO: automatically determine this.
                                    // Probably using file size ranges

/// This struct is the main entry point for any operations that come from the client.
///
/// Any message that is transmitted through the network should be generated by this struct at a
/// high level.
pub struct Client {
    builder: MessageBuilder,
    net_client: NetClient,
}

impl Client {
    pub fn new(builder: MessageBuilder, net_client: NetClient) -> Self {
        //let (tx, mut rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = mpsc::channel(100);
        //let client = net_client.clone();
        //tokio::spawn(async move {
        //    while let Some(data) = rx.recv().await {
        //        if let Err(e) = client.lock().await.send(&data).await {
        //            // TODO: handle errors. Possibly requeue them
        //            error!("Failed to process msg_queue: {:?}", e);
        //        };
        //    }
        //});

        //let mut raw_stream = net_client.lock().await.clone_stream().unwrap();
        //let client = net_client;
        //tokio::spawn(async move {
        //    while let Ok(raw_msg) = net::recv(&mut raw_stream).await {
        //        if let Ok(msg) = client.lock().unwrap().decrypt(&raw_msg) {
        //            msg_queue.send(QueueItem::ServerMsg(msg)).unwrap();
        //        }
        //    }
        //});

        Client {
            builder,
            net_client,
        }
    }

    /// Send file metadata to the server
    pub async fn send_file_info(&mut self, base: &Path, path: &Path) -> Result<(), Box<dyn Error>> {
        let mut file_info = get_file_info(path)?;
        file_info.file_id.path = path.strip_prefix(base).unwrap().to_owned();
        let msg = self
            .builder
            .encode_message(Directive::SendFile, Some(file_info));
        self.net_client.send(&msg).await?;
        Ok(())
    }

    /// Send a specific chunk from a given file
    pub async fn send_chunk(
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
            self.net_client.send(&msg).await?;
        } else {
            panic!("Chunks don't match up. File must have changed. This error will be handled in the future")
        }

        Ok(())
    }

    pub async fn request_chunk(&mut self, chunk: QualifiedChunkId) -> Result<(), NetError> {
        let msg = self
            .builder
            .encode_message::<arguments::QualifiedChunkId>(Directive::RequestChunk, Some(chunk));
        self.net_client.send(&msg).await
    }

    pub async fn request_file_list(&mut self) -> Result<(), NetError> {
        let msg = self
            .builder
            .encode_message::<arguments::Dummy>(Directive::ListFiles, None);
        self.net_client.send(&msg).await
    }

    pub async fn request_file(&mut self, file: FileId) -> Result<(), NetError> {
        let msg = self
            .builder
            .encode_message(Directive::RequestFile, Some(file));
        self.net_client.send(&msg).await
    }

    pub async fn delete_file(&mut self, file_path: FilePath) -> Result<(), NetError> {
        let msg = self
            .builder
            .encode_message(Directive::DeleteFile, Some(file_path));
        self.net_client.send(&msg).await
    }

    pub async fn recv(&mut self) -> Result<Vec<u8>, NetError> {
        let ret = self.net_client.recv().await;
        ret
    }
}
