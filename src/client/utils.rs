use super::{file_operations::CHUNK_SIZE, Blacklist};
use crate::messaging::{arguments::{FileId, FileList, FileMetadata, QualifiedChunk}, error::MessageError};
use std::{
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
};

/// Calculate chunk boundries and file hash
fn chunk_file(path: &Path) -> Result<Vec<[u8; 32]>, io::Error> {
    let mut file = File::open(path)?;
    let size = file.metadata().unwrap().len();

    let mut hasher = blake3::Hasher::new();

    let mut chunks: Vec<[u8; 32]> = vec![];

    file.seek(SeekFrom::Start(0))?;

    for _ in 0..(size as f32 / CHUNK_SIZE as f32).ceil() as usize {
        let mut buf = vec![0; CHUNK_SIZE];
        let len = file.read(&mut buf)?;
        hasher.update(&buf[..len]);
        chunks.push(hasher.finalize().into());
        hasher.reset();
    }

    Ok(chunks)
}

/// Write a `QualifiedChunk` to it's specified file
pub fn write_chunk(
    blacklist: &mut Blacklist,
    base_path: &Path,
    chunk: &QualifiedChunk,
) -> Result<(), std::io::Error> {
    let mut file = File::options()
        .write(true)
        .open(base_path.join(&chunk.id.path.path))?;
    file.seek(SeekFrom::Start(chunk.id.offset as u64))?;
    file.write_all(&chunk.data)?;
    if let Some(x) = blacklist.get(&chunk.id.path.path) {
        let hash = x.file_id.hash;
        if FileId::new(base_path.join(&chunk.id.path.path))
            .unwrap()
            .hash
            .to_vec()
            == hash
        {
            debug!("File download completed for {:?}", chunk.id.path.path);
            blacklist.remove(&chunk.id.path.path);
        }
    }
    Ok(())
}

/// Get the file metadata from a file at a given path.
pub fn get_file_info(path: &Path) -> Result<FileMetadata, MessageError> {
    let md = fs::metadata(path)?;
    let file_id = FileId::new(path.to_owned())?;
    let chunks = chunk_file(path)?;
    Ok(FileMetadata::new(file_id, md, &chunks).unwrap())
}

/// Generate a file listing of the watched directory.
///
/// This will be used to preform an initial synchronization when the clients connect.
pub fn generate_file_list(path: &Path) -> Result<FileList, MessageError> {
    Ok(FileList(recursive_file_list(path, path)?))
}

fn recursive_file_list(base: &Path, path: &Path) -> Result<Vec<FileId>, MessageError> {
    let mut files: Vec<FileId> = vec![];

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.append(&mut recursive_file_list(base, &path)?);
        } else {
            let mut file_info = FileId::new(path)?;
            file_info.path = file_info.path.strip_prefix(base).unwrap().to_owned();
            files.push(file_info);
        }
    }
    Ok(files)
}
