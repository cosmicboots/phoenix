use std::{
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};

use sha2::{Digest, Sha256};

use crate::{messaging::{arguments::{FileId, FileMetadata}, MessageBuilder, Directive}, net::{Client, NoiseConnection}};

pub fn send_file_info(builder: &mut MessageBuilder, client: &mut Client, file: FileMetadata) -> Result<(), snow::Error> {
    let msg = builder.encode_message(Directive::SendFile, Some(file));
    client.send(&msg)?;
    Ok(())
}

/// Calculate chunk boundries and file hash
pub fn chunk_file(path: &Path) -> Result<Vec<[u8; 32]>, io::Error> {
    const CHUNK_SIZE: usize = 8; // 8 byte chunk size. TODO: automatically determine this. Probably
                                 // using file size ranges

    let mut file = File::open(path)?;
    let size = file.metadata().unwrap().len();

    let mut hasher = Sha256::new();

    let mut chunks: Vec<[u8; 32]> = vec![];

    file.seek(SeekFrom::Start(0))?;

    for i in 0..(size as f32 / CHUNK_SIZE as f32).ceil() as usize {
        debug!("Chunk: {}", i);
        let mut buf = vec![0; CHUNK_SIZE];
        let len = file.read(&mut buf)?;
        hasher.update(&buf[..len]);
        chunks.push(hasher.finalize_reset().into());
        debug!("{:?}", chunks);
    }

    Ok(chunks)
}

pub fn get_file_info(path: &Path) -> Result<FileMetadata, std::io::Error> {
    let md = fs::metadata(path)?;
    let file_id = FileId::new(path.to_owned())?;
    let chunks = chunk_file(path)?;
    Ok(FileMetadata::new(file_id, md, &chunks).unwrap())
}
