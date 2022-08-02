// Directive specific abstractions for parsing the byte array argument data
use core::fmt::Debug;
use sha2::{Digest, Sha256};
use std::{
    fs::{File, Metadata, Permissions},
    io,
    os::unix::prelude::PermissionsExt,
    path::PathBuf,
    time, vec,
};

#[derive(Debug, PartialEq)]
pub struct Error(String);

pub trait Argument: Debug {
    fn to_bin(self: &Self) -> Vec<u8>;
    fn from_bin(data: &[u8]) -> Result<Self, Error>
    where
        Self: Sized;
}

#[derive(Debug, PartialEq, Clone)]
pub struct Version(pub u8);

impl Argument for Version {
    fn to_bin(self: &Self) -> Vec<u8> {
        self.0.to_be_bytes().to_vec()
    }

    fn from_bin(ver: &[u8]) -> Result<Self, Error> {
        Ok(Version(ver[0]))
    }
}

#[derive(Debug, PartialEq)]
pub struct FileId {
    path: PathBuf,
    hash: [u8; 32],
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
    fn to_bin(self: &Self) -> Vec<u8> {
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
}

#[derive(Debug, PartialEq)]
pub struct ChunkId(String);

impl Argument for ChunkId {
    fn to_bin(self: &Self) -> Vec<u8> {
        self.0.as_bytes().to_vec()
    }

    fn from_bin(data: &[u8]) -> Result<Self, Error> {
        match String::from_utf8(data.to_vec()) {
            Ok(x) => Ok(Self(x)),
            Err(e) => Err(Error("Failed to parse ChunkId".to_owned())),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct FileMetadata {
    file_id: FileId,
    file_name: String,
    pub permissions: Permissions,
    pub modified: u128,
    pub created: u128,
}

impl FileMetadata {
    pub fn new(file_id: FileId, metadata: Metadata) -> Result<Self, io::Error> {
        Ok(FileMetadata {
            file_name: file_id
                .path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned(),
            file_id,
            permissions: metadata.permissions(),
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
        })
    }
}

impl Argument for FileMetadata {
    fn to_bin(self: &Self) -> Vec<u8> {
        let mut buf: Vec<u8> = vec![];

        let path = self.file_id.path.to_str().unwrap().as_bytes();
        buf.extend_from_slice(&(path.len() as u64).to_be_bytes());
        buf.extend_from_slice(path);

        buf.extend_from_slice(&self.permissions.mode().to_be_bytes());
        buf.extend_from_slice(&self.modified.to_be_bytes());
        buf.extend_from_slice(&self.created.to_be_bytes());
        buf.extend_from_slice(&self.file_id.hash);
        buf
    }

    fn from_bin(data: &[u8]) -> Result<Self, Error> {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&data[0..8]);
        let end = 8 + u64::from_be_bytes(buf) as usize;
        let path = PathBuf::from(String::from_utf8(data[8..(end)].to_vec()).unwrap());

        let mut buf = [0u8; 4];
        buf.copy_from_slice(&data[end..end + 4]);
        let permissions: Permissions = PermissionsExt::from_mode(u32::from_be_bytes(buf));

        let end = end + 4;
        let mut buf = [0u8; 16];
        buf.copy_from_slice(&data[end..end + 16]);
        let modified = u128::from_be_bytes(buf);
        buf.copy_from_slice(&data[end + 16..end + 32]);
        let created = u128::from_be_bytes(buf);

        let end = end + 32;
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&data[end..end + 32]);

        Ok(FileMetadata {
            file_name: path.file_name().unwrap().to_str().unwrap().to_owned(),
            file_id: FileId { path, hash },
            permissions,
            modified,
            created,
        })
    }
}

#[derive(Debug, PartialEq)]
pub struct Chunk(Vec<u8>);

impl Argument for Chunk {
    fn to_bin(self: &Self) -> Vec<u8> {
        Vec::clone(&self.0)
    }

    fn from_bin(data: &[u8]) -> Result<Self, Error> {
        Ok(Chunk(data.to_vec()))
    }
}

#[derive(Debug, PartialEq)]
pub struct ResponseCode(u16);

impl Argument for ResponseCode {
    fn to_bin(self: &Self) -> Vec<u8> {
        self.0.to_be_bytes().to_vec()
    }

    fn from_bin(data: &[u8]) -> Result<Self, Error> {
        let mut buf = [0u8; 2];
        buf.copy_from_slice(data);
        Ok(ResponseCode(u16::from_be_bytes(buf)))
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
            ChunkId("Hello world".to_owned()).to_bin(),
            vec![72, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100]
        );
        assert_eq!(
            ChunkId::from_bin(&[72, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100]).unwrap(),
            ChunkId("Hello world".to_owned())
        );
    }
}
