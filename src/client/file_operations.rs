use std::{
    fs::{self, File},
    io,
    path::Path,
};

use sha2::{Digest, Sha256};

use crate::messaging::arguments::{FileId, FileMetadata};

#[derive(Debug)]
pub struct ChunkedFile {
    hash: [u8; 32],
    chunk_size: u64,
}

pub fn send_file_info(file: FileMetadata) {
    todo!()
}

/// Calculate chunk boundries and file hash
pub fn chunk_file(path: &Path) -> Result<ChunkedFile, io::Error> {
    const CHUNK_SIZE: u64 = 8; // 8 byte chunk size. TODO: automatically determine this. Probably using
                               // file size ranges

    let mut file = File::open(path)?;

    let mut hasher = Sha256::new();
    let size = io::copy(&mut file, &mut hasher).unwrap();
    let hash = hasher.finalize();

    Ok(ChunkedFile {
        hash: hash.into(),
        chunk_size: (size as f32 / 2f32).ceil() as u64,
    })
}

pub fn get_file_info(path: &Path) -> Result<FileMetadata, std::io::Error> {
    let md = fs::metadata(path)?;
    let file_id = FileId::new(path.to_owned()).unwrap();
    Ok(FileMetadata::new(file_id, md).unwrap())
}
