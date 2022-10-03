#![cfg(test)]

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

#[test]
fn test_argument_filelist() {
    todo!()
}
