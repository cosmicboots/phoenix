// Directive specific abstractions for parsing the byte array argument data
use core::fmt::Debug;

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
pub struct FileId(String);

impl Argument for FileId {
    fn to_bin(self: &Self) -> Vec<u8> {
        self.0.as_bytes().to_vec()
    }

    fn from_bin(data: &[u8]) -> Result<Self, Error> {
        match String::from_utf8(data.to_vec()) {
            Ok(x) => Ok(Self(x)),
            Err(e) => Err(Error("Failed to parse FileId".to_owned())),
        }
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
    file_size: usize,
}

impl Argument for FileMetadata {
    fn to_bin(self: &Self) -> Vec<u8> {
        todo!()
    }

    fn from_bin(data: &[u8]) -> Result<Self, Error> {
        todo!()
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

    fn from_bin(data: &[u8]) -> Result<Self, Error>
    {
        let mut buf = [0u8; 2];
        buf.copy_from_slice(data);
        Ok(ResponseCode(u16::from_be_bytes(buf)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_argument_version() {
        assert_eq!(Version(1).to_bin(), vec![1u8]);
        assert_eq!(Version::from_bin(&[1u8]).unwrap(), Version(1));
    }

    #[test]
    fn test_argument_fileid() {
        assert_eq!(
            FileId("Hello world".to_owned()).to_bin(),
            vec![72, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100]
        );
        assert_eq!(
            FileId::from_bin(&[72, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100]).unwrap(),
            FileId("Hello world".to_owned())
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
