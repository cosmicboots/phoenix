#![cfg(test)]

use super::*;
use sha2::{Digest, Sha256};
use pretty_assertions::assert_eq;

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
fn test_qualfied_chunk() {
    let chunk = QualifiedChunk {
        id: QualifiedChunkId {
            path: FileId {
                path: PathBuf::from("dir/file"),
                hash: [0u8; 32],
            },
            offset: 0x02020202,
            id: ChunkId([1u8; 32].to_vec()),
        },
        data: vec![9,8,7,6,5,4,3,2,1,0],
    };
    assert_eq!(chunk, QualifiedChunk::from_bin(&chunk.to_bin()).unwrap())
}

#[test]
fn test_qualified_chunkid() {
    let chunk_id = QualifiedChunkId {
        path: FileId {
            path: PathBuf::from("dir/file"),
            hash: [0u8; 32],
        },
        offset: 0x02020202,
        id: ChunkId([1u8; 32].to_vec()),
    };

    assert_eq!(
        chunk_id.to_bin(),
        vec![
            0u8, 0u8, 0u8, 40u8, 100u8, 105u8, 114u8, 47u8, 102u8, 105u8, 108u8, 101u8, 0u8, 0u8,
            0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8,
            0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 2u8, 2u8, 2u8, 2u8,
            1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8,
            1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8,
        ]
    );

    let raw_chunk_id = vec![
        0u8, 0u8, 0u8, 40u8, 100u8, 105u8, 114u8, 47u8, 102u8, 105u8, 108u8, 101u8, 0u8, 0u8, 0u8,
        0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8,
        0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 2u8, 2u8, 2u8, 2u8, 1u8, 1u8, 1u8,
        1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8,
        1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8, 1u8,
    ];

    assert_eq!(QualifiedChunkId::from_bin(&raw_chunk_id).unwrap(), chunk_id);
}
