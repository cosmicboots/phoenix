// Directive specific abstractions for parsing the byte array argument data
use base64ct::{Base64, Encoding};
use core::fmt::Debug;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    any::Any,
    fmt::{Display, Write},
    fs::{File, Metadata},
    io,
    os::unix::prelude::PermissionsExt,
    path::PathBuf,
    time, vec,
};

#[derive(Debug, PartialEq, Eq)]
pub struct Error(String);

pub trait Argument: Debug {
    fn to_bin(&self) -> Vec<u8>;
    fn from_bin(data: &[u8]) -> Result<Self, Error>
    where
        Self: Sized;
    fn as_any(&self) -> &dyn Any;
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Version(pub u8);

impl Argument for Version {
    fn to_bin(&self) -> Vec<u8> {
        self.0.to_be_bytes().to_vec()
    }

    fn from_bin(ver: &[u8]) -> Result<Self, Error> {
        Ok(Version(ver[0]))
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct FileId {
    pub path: PathBuf,
    pub hash: [u8; 32],
}

impl FileId {
    pub fn new(path: PathBuf) -> Result<Self, io::Error> {
        let mut file = File::open(&path)?;
        let mut hasher = Sha256::new();
        io::copy(&mut file, &mut hasher).unwrap();
        let hash = hasher.finalize();
        Ok(FileId {
            path,
            hash: hash.into(),
        })
    }
}

impl Argument for FileId {
    fn to_bin(&self) -> Vec<u8> {
        let mut x = self.path.to_str().unwrap().as_bytes().to_vec();
        x.extend_from_slice(&self.hash);
        x
    }

    fn from_bin(data: &[u8]) -> Result<Self, Error> {
        if data.len() < 32 {
            return Err(Error("FileId bin to short to convert".to_owned()));
        }
        let path = match String::from_utf8(data[..data.len() - 32].to_vec()) {
            Ok(x) => Ok(PathBuf::from(x)),
            Err(_) => Err(Error("Failed to parse FileId path".to_owned())),
        }?;

        let mut xdata = [0u8; 32];

        xdata.copy_from_slice(&data[data.len() - 32..]);

        Ok(FileId { path, hash: xdata })
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct ChunkId(pub Vec<u8>);

impl Argument for ChunkId {
    fn to_bin(&self) -> Vec<u8> {
        self.0.to_owned()
    }

    fn from_bin(data: &[u8]) -> Result<Self, Error> {
        Ok(Self(data.to_vec()))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FileMetadata {
    pub file_id: FileId,
    pub file_name: String,
    pub permissions: u32,
    pub modified: u128,
    pub created: u128,
    pub chunks: Vec<ChunkId>,
}

impl PartialEq for FileMetadata {
    fn eq(&self, other: &Self) -> bool {
        self.file_id == other.file_id
            && self.file_name == other.file_name
            && self.permissions == other.permissions
            && self.chunks == other.chunks
    }
}

impl Eq for FileMetadata {}

impl Display for FileMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut chunks = String::new();
        for chunk in &self.chunks {
            let _ = write!(chunks, "\n - {}", Base64::encode_string(&chunk.0));
        }
        write!(
            f,
            r#"Path: {:?}
File hash: {}
Permissions: {}
Created: {} Modified: {}
Chunks: {}"#,
            self.file_id.path,
            Base64::encode_string(&self.file_id.hash),
            self.permissions,
            self.created,
            self.modified,
            chunks,
        )
    }
}

impl FileMetadata {
    pub fn new(
        file_id: FileId,
        metadata: Metadata,
        chunks: &[[u8; 32]],
    ) -> Result<Self, io::Error> {
        Ok(FileMetadata {
            file_name: file_id
                .path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned(),
            file_id,
            permissions: metadata.permissions().mode(),
            modified: metadata
                .modified()?
                .duration_since(time::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
            created: metadata
                .created()?
                .duration_since(time::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
            chunks: chunks
                .iter()
                .map(|x| ChunkId(x.to_vec()))
                .collect::<Vec<ChunkId>>(),
        })
    }
}

impl Argument for FileMetadata {
    fn to_bin(&self) -> Vec<u8> {
        let mut buf: Vec<u8> = vec![];

        let path = self.file_id.path.to_str().unwrap().as_bytes();
        buf.extend_from_slice(&(path.len() as u64).to_be_bytes());
        buf.extend_from_slice(path);

        buf.extend_from_slice(&self.permissions.to_be_bytes());
        buf.extend_from_slice(&self.modified.to_be_bytes());
        buf.extend_from_slice(&self.created.to_be_bytes());
        buf.extend_from_slice(&self.file_id.hash);
        for chunk in &self.chunks {
            buf.extend_from_slice(&chunk.0);
        }
        buf
    }

    fn from_bin(data: &[u8]) -> Result<Self, Error> {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&data[0..8]);
        let end = 8 + u64::from_be_bytes(buf) as usize;
        let path = PathBuf::from(String::from_utf8(data[8..(end)].to_vec()).unwrap());

        let mut buf = [0u8; 4];
        buf.copy_from_slice(&data[end..end + 4]);
        let permissions = u32::from_be_bytes(buf);

        let end = end + 4;
        let mut buf = [0u8; 16];
        buf.copy_from_slice(&data[end..end + 16]);
        let modified = u128::from_be_bytes(buf);
        buf.copy_from_slice(&data[end + 16..end + 32]);
        let created = u128::from_be_bytes(buf);

        let end = end + 32;
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&data[end..end + 32]);

        let end = end + 32;
        let mut chunks: Vec<ChunkId> = vec![];
        for cur in (end..data.len()).step_by(32) {
            if cur + 32 <= data.len() {
                chunks.push(ChunkId(data[cur..cur + 32].to_vec()));
            } else {
                chunks.push(ChunkId(data[cur..].to_vec()));
            }
        }

        Ok(FileMetadata {
            file_name: path.file_name().unwrap().to_str().unwrap().to_owned(),
            file_id: FileId { path, hash },
            permissions,
            modified,
            created,
            chunks,
        })
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Chunk {
    pub id: ChunkId,
    pub data: Vec<u8>,
}

impl Argument for Chunk {
    fn to_bin(&self) -> Vec<u8> {
        let mut buf: Vec<u8> = self.id.to_bin();
        buf.extend_from_slice(&self.data);
        buf
    }

    fn from_bin(data: &[u8]) -> Result<Self, Error> {
        let chunk_id = ChunkId::from_bin(&data[..32]).unwrap();
        let chunk_data = data[32..].to_vec();
        Ok(Chunk {
            id: chunk_id,
            data: chunk_data,
        })
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ResponseCode(u16);

impl Argument for ResponseCode {
    fn to_bin(&self) -> Vec<u8> {
        self.0.to_be_bytes().to_vec()
    }

    fn from_bin(data: &[u8]) -> Result<Self, Error> {
        let mut buf = [0u8; 2];
        buf.copy_from_slice(data);
        Ok(ResponseCode(u16::from_be_bytes(buf)))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::*;

    #[test]
    fn test_argument_version() {
        assert_eq!(Version(1).to_bin(), vec![1u8]);
        assert_eq!(Version::from_bin(&[1u8]).unwrap(), Version(1));
    }

    #[test]
    fn test_argument_fileid() {
        let mut h = Sha256::new();
        let mut a = vec![112, 97, 116, 104, 47, 116, 111, 47, 102, 105, 108, 101];
        h.update(b"Hello world");
        let mut b = [0u8; 32];
        b.copy_from_slice(&h.finalize());
        a.extend_from_slice(&b);
        assert_eq!(
            FileId {
                path: PathBuf::from("path/to/file"),
                hash: b
            }
            .to_bin(),
            a
        );
        assert_eq!(
            FileId::from_bin(&a).unwrap(),
            FileId {
                path: PathBuf::from("path/to/file"),
                hash: b
            }
        );
    }

    #[test]
    fn test_argument_chunkid() {
        assert_eq!(
            ChunkId(b"Hello world".to_vec()).to_bin(),
            vec![72, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100]
        );
        assert_eq!(
            ChunkId::from_bin(&[72, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100]).unwrap(),
            ChunkId(b"Hello world".to_vec())
        );
    }
}
