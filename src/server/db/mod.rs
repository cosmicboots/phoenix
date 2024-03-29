//! Database module to handle backend storage and transactions

#![allow(dead_code)]

pub mod error;

use base64ct::{Base64, Encoding};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use sled::{
    transaction::{ConflictableTransactionResult, TransactionalTree, ConflictableTransactionError, TransactionError},
    IVec, Transactional, Tree,
};
use std::{collections::HashSet, fmt::Write, path::Path, vec};
use crate::messaging::arguments::{Chunk, ChunkId, FileId, FileList, FileMetadata, FilePath};

use self::error::DbError;

/// Static name of the file_table
static FILE_TABLE: &str = "file_table";
/// Static name of the pending_table
static PENDING_TABLE: &str = "pending table";
/// Static name of the chunk_table
static CHUNK_TABLE: &str = "chunk_table";
/// Static name of the chunk_count table
static CHUNK_COUNT: &str = "chunk_count";
/// Static name of the missing_chunks table
static MISSING_CHUNKS: &str = "missing_chunks";

#[derive(Debug)]
/// The main database stucture to store back-end data.
pub struct Db {
    /// Database table to store file metadata and associated chunk hashes
    file_table: Tree,
    /// Database table to store the actual data for each chunk
    chunk_table: Tree,
    /// Backpointer table storing the count of references to any given chunk
    ///
    /// This will be used to determine when it's safe to remove a chunk from the database (in the
    /// case where multiple files reference the same chunk)
    chunk_count: Tree,
    /// Table to store partial file transfers while they're still in progress
    pending_table: Tree,
    /// Table to store chunks that the database doesn't have yet
    missing_chunks: Tree,
}

impl Db {
    /// Create a new instance of the database.
    ///
    /// This also opens the database tables using the statics:
    /// - [`FILE_TABLE`](static.FILE_TABLE.html)
    /// - [`CHUNK_TABLE`](static.CHUNK_TABLE.html)
    /// - [`CHUNK_COUNT`](static.CHUNK_COUNT.html)
    pub fn new(path: &Path) -> sled::Result<Db> {
        let db = sled::open(path)?;
        let file_table = db.open_tree(FILE_TABLE)?;
        let chunk_table = db.open_tree(CHUNK_TABLE)?;
        let chunk_count = db.open_tree(CHUNK_COUNT)?;
        let pending_table = db.open_tree(PENDING_TABLE)?;
        let missing_chunks = db.open_tree(MISSING_CHUNKS)?;
        Ok(Db {
            file_table,
            chunk_table,
            chunk_count,
            pending_table,
            missing_chunks,
        })
    }

    pub fn new_temporary() -> sled::Result<Db> {
        let db = sled::Config::new().temporary(true).open()?;
        Ok(Db {
            file_table: db.open_tree(FILE_TABLE)?,
            chunk_table: db.open_tree(CHUNK_TABLE)?,
            chunk_count: db.open_tree(CHUNK_COUNT)?,
            pending_table: db.open_tree(PENDING_TABLE)?,
            missing_chunks: db.open_tree(MISSING_CHUNKS)?,
        })
    }

    /// Adds a [File](struct.File.html) struct into the file_table database.
    ///
    /// This also increments the referenced values in the [`chunk_count`](#structfield.chunk_count)
    /// table; however, it doesn't actually insert any data into the
    /// [`chunk_table`](#structfield.chunk_table) table.
    ///
    /// This function also doubles as an update file function. If the fily being added is already
    /// in the database, there will be a check to see if it's identical. If the file has changed, a
    /// difference operation runst to see which chunks changed.
    ///
    /// The new chunks are then inserted into the database, and the chunks no longer used are
    /// removed.
    pub fn add_file(&self, file: &FileMetadata) -> Result<Vec<ChunkId>, DbError> {
        let value = match bincode::serialize(&file) {
            Ok(x) => x,
            Err(_) => panic!("Couldn't serialize file to store in database"),
        };
        // TODO: Improve error handling
        let chunks: Vec<ChunkId> = match (
            &self.file_table,
            &self.pending_table,
            &self.chunk_count,
            &self.chunk_table,
            &self.missing_chunks,
        )
            .transaction(
                |(ft, pt, cc, ct, mc): &(
                    TransactionalTree,
                    TransactionalTree,
                    TransactionalTree,
                    TransactionalTree,
                    TransactionalTree,
                )|
                 -> ConflictableTransactionResult<Vec<ChunkId>, DbError> {
                    let mut insert_chunks = file.chunks.clone();
                    let mut new_chunks = vec![];

                    // Prevent duplicate entries with the same data
                    if let Some(x) = ft.get(&file.file_id.path.to_str().unwrap().as_bytes())? {
                        let old_file = bincode::deserialize::<FileMetadata>(&x).unwrap();
                        if old_file == *file {
                            // The file is the same as the old
                            warn!("Duplicate file attempted to add to the file store");
                            return Err(ConflictableTransactionError::Abort(DbError::DuplicateFile));
                        } else {
                            debug!("Updating file: {:?}", file.file_id.path);
                            let mut old_chunks = HashSet::new();
                            old_file.chunks.iter().for_each(|x| {
                                old_chunks.insert(x);
                            });

                            let mut new_chunks = HashSet::new();
                            file.chunks.iter().for_each(|x| {
                                new_chunks.insert(x);
                            });

                            let chunks_to_remove = old_chunks.difference(&new_chunks);
                            let chunks_to_add = new_chunks.difference(&old_chunks);

                            for chunk in chunks_to_remove {
                                let count = rc_merge(cc.get(&*chunk.0)?, -1);

                                if let Some(x) = count {
                                    cc.insert(&*chunk.0, &*x)?;
                                    let mut buf = [0u8; 4];
                                    buf.copy_from_slice(&x);
                                    if u32::from_le_bytes(buf) == 0 {
                                        ct.remove(&*chunk.0)?;
                                        cc.remove(&*chunk.0)?;
                                    }
                                }
                            }

                            insert_chunks = vec![];
                            for chunk in chunks_to_add {
                                insert_chunks.push((*chunk).clone());
                            }
                        }
                    }

                    // Add all the chunks into the chunk count table
                    for chunk in insert_chunks {
                        // TODO: this probably should be done with a merge operation
                        if let Some(x) = rc_merge(cc.get(&chunk.0)?, 1) {
                            cc.insert(&*chunk.0, x)?;
                        };
                        if (ct.get(&chunk.0)?).is_none() {
                            new_chunks.push(chunk.clone());
                            let mut ref_files: Vec<String> = match mc.get(&*chunk.0)? {
                                Some(x) => bincode::deserialize::<Vec<String>>(&x).unwrap(),
                                None => vec![],
                            };
                            ref_files.push(file.file_id.path.display().to_string());
                            mc.insert(&*chunk.0, bincode::serialize(&ref_files).unwrap())?;
                        }
                    }

                    // Add the file metadata to the file table
                    if new_chunks.is_empty() {
                        ft.insert(file.file_id.path.to_str().unwrap().as_bytes(), &*value)
                            .unwrap();
                    } else {
                        pt.insert(file.file_id.path.to_str().unwrap().as_bytes(), &*value)
                            .unwrap();
                    }
                    Ok(new_chunks)
                },
            ) {
                Ok(x) => x,
                Err(TransactionError::Abort(e)) => {
                    return Err(e);
                }
                // TODO: Fix this error handling
                _ => panic!("Database operation failed"),
            };
        Ok(chunks)
    }

    /// Returns a [File](struct.File.html) from the database when given a file_hash.
    pub fn get_file(&self, file: &str) -> sled::Result<Option<FileMetadata>> {
        match self.file_table.get(file) {
            Ok(x) => match x {
                Some(value) => Ok(
                    Some(bincode::deserialize::<FileMetadata>(&value).expect("Failed to deserialize"))
                ),
                None => Ok(None),
            },
            Err(e) => Err(e),
        }
    }

    /// Adds a chunk into the [`chunk_table`](#structfield.chunk_table) table.
    ///
    /// NOTE: This should be run after [`add_file()`](#method.add_file).
    /// This function checks the chunk count table to ensure references to the chunk exist. If this
    /// check wasn't preformed, it would be possible to add orphaned chunks into the database,
    /// which would be expensive to clean up.
    ///
    /// An optional `FileId` is returned if the file transfer was completed.
    pub fn add_chunk(&self, chunk: &Chunk) -> sled::Result<Option<FileId>> {
        let ret = (
            &self.chunk_table,
            &self.missing_chunks,
            &self.pending_table,
            &self.file_table,
        )
            .transaction(
                |(ct, mc, pt, ft): &(
                    TransactionalTree,
                    TransactionalTree,
                    TransactionalTree,
                    TransactionalTree,
                )|
                 -> ConflictableTransactionResult<Option<FileId>, sled::Error> {
                    // Check to see if the chunk is missing (via the missing_chunks table) to make
                    // sure orphaned chunks are never added into the database. This should prevent
                    // the need of expensive database clean up operations
                    if let Some(x) = mc.get(&chunk.id.0)? {
                        ct.insert(chunk.id.0.to_vec(), chunk.data.to_owned())?;
                        mc.remove(chunk.id.0.to_vec())?;
                        // TODO: Cleanup Partially transferred files
                        let files = bincode::deserialize::<Vec<String>>(&x).unwrap();
                        for file in files {
                            if let Some(raw_file) = pt.get(&file)? {
                                let file_md: FileMetadata =
                                    bincode::deserialize::<FileMetadata>(&raw_file).unwrap();
                                let mut file_complete = true;
                                for chunk in file_md.chunks {
                                    if (mc.get(&chunk.0)?).is_some() {
                                        file_complete = false;
                                        break;
                                    }
                                }
                                if file_complete {
                                    debug!("File completed transfer: {:?}", file);
                                    ft.insert(file.as_bytes(), &pt.remove(&*file)?.unwrap())?;
                                    return Ok(Some(file_md.file_id));
                                }
                            }
                        }
                    }
                    Ok(None)
                },
            )
            .unwrap();
        Ok(ret)
    }

    /// Gets a chunk out of the database given it's ID (hash).
    pub fn get_chunk(&self, chunk_hash: [u8; 32]) -> sled::Result<Chunk> {
        // TODO: Improve error handling
        match self.chunk_table.get(&chunk_hash) {
            Ok(x) => match x {
                Some(value) => Ok(Chunk {
                    id: ChunkId(chunk_hash.to_vec()),
                    data: value.to_vec(),
                }),
                None => panic!("Chunk not found"),
            },
            Err(e) => Err(e),
        }
    }

    pub fn rm_file(&self, file_path: &FilePath) {
        (&self.file_table, &self.chunk_table, &self.chunk_count)
            .transaction(
                |(ft, ct, cc)| -> ConflictableTransactionResult<(), sled::Error> {
                    // 1. Get the file and desearialize it
                    // 2. Iterate through the chunks and decrement the refcounter
                    // 3.   if 0 refs, delete the chunk from the chunk table
                    if let Ok(Some(bin_file)) = ft.get(&file_path.0.as_bytes()) {
                        // Deserialize bin into the File struct
                        if let Ok(file) = bincode::deserialize::<FileMetadata>(&bin_file) {
                            for chunk in file.chunks {
                                if let Ok(Some(x)) = cc.get(&chunk.0) {
                                    let mut rdr = std::io::Cursor::new(x);
                                    match rdr.read_u32::<LittleEndian>() {
                                        // If there are no more references to the given chunk,
                                        // remove it from the chunk table and the chunk count table
                                        Ok(0) | Ok(1) => {
                                            ct.remove(&*chunk.0)?;
                                            cc.remove(&*chunk.0)?;
                                        }
                                        Ok(x) => {
                                            let mut wtr = vec![];
                                            wtr.write_u32::<LittleEndian>(x - 1).unwrap();
                                            cc.insert(chunk.0, wtr)?;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            ft.remove(file_path.0.as_bytes()).unwrap();
                        }
                    }
                    Ok(())
                },
            )
            .unwrap();
    }

    pub fn get_files(&self) -> Result<FileList, sled::Error> {
        let mut files: Vec<FileId> = vec![];
        for file in self.file_table.iter() {
            let file_struct = bincode::deserialize::<FileMetadata>(&file?.1)
                .expect("Failed to create FileMetadata struct from the database.");
            files.push(file_struct.file_id);
        }
        Ok(FileList(files))
    }

    /// Dump the current database to stdout
    pub fn dump_tree(&self) {
        let mut table = self.pending_table.iter();
        println!("\n=== Printing pending_table ===");
        while let Some(Ok((key, value))) = table.next() {
            println!(
                "Key: {:?}\n{}",
                String::from_utf8(key.to_vec()).unwrap(),
                bincode::deserialize::<FileMetadata>(&value).unwrap()
            );
        }
        let mut table = self.missing_chunks.iter();
        println!("\n=== Printing missing_chunks ===");
        while let Some(Ok((key, value))) = table.next() {
            println!(
                "ChunkId: {}\n - File: {:?}",
                Base64::encode_string(&key),
                bincode::deserialize::<Vec<String>>(&value).unwrap()
            );
        }
        let mut table = self.file_table.iter();
        println!("\n=== Printing file_table ===");
        while let Some(Ok((key, value))) = table.next() {
            println!(
                "Key: {:?}\n{}",
                String::from_utf8(key.to_vec()).unwrap(),
                bincode::deserialize::<FileMetadata>(&value).unwrap()
            );
        }
        let mut table = self.chunk_table.iter();
        println!("\n=== Printing chunk_table ===");
        while let Some(Ok((key, value))) = table.next() {
            let mut chunk_data = String::new();
            for byte in value.iter() {
                let _ = write!(chunk_data, "{:02x} ", byte);
            }
            println!(
                "Chunk ID: {}\nData: {}",
                Base64::encode_string(&key),
                chunk_data
            );
        }
        let mut table = self.chunk_count.iter();
        println!("\n=== Printing chunk_count ===");
        while let Some(Ok((key, value))) = table.next() {
            let mut buf = [0u8; 4];
            buf.copy_from_slice(&value);
            println!(
                "Chunk ID: {:?}\nChunk count: {:?}",
                Base64::encode_string(&key),
                u32::from_le_bytes(buf),
            );
        }
    }
}

/// This is a poor mans merge operator for TransactionalTrees because they don't support proper
/// merge operations.
fn rc_merge(old_value: Option<IVec>, increment: i32) -> Option<Vec<u8>> {
    let mut buf = [0u8; 4];
    let mut x = 0;
    if let Some(v) = old_value {
        buf.copy_from_slice(&v);
        x = u32::from_le_bytes(buf);
    }

    Some((x as i32 + increment).to_le_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    #![allow(unreachable_code, unused)]
    use std::{
        panic,
        path::PathBuf,
        str::FromStr,
        sync::{Arc, Mutex},
        time,
    };

    use crate::messaging::arguments::FileId;

    use super::*;

    // The tests need to be able to use their own temperary database rather than using the global
    // static

    fn run_test<T>(test: T) -> ()
    where
        T: FnOnce(Arc<Mutex<Db>>) -> () + panic::UnwindSafe,
    {
        let db = Arc::new(Mutex::new(Db::new_temporary().unwrap()));
        create_test_data(db.clone());
        let result = panic::catch_unwind(|| test(db));
        assert!(result.is_ok())
    }

    fn create_test_data(db: Arc<Mutex<Db>>) {
        let db = db.lock().unwrap();
        // Empty file
        let file = FileMetadata {
            file_id: FileId {
                path: PathBuf::from("TestFile"),
                hash: [0u8; 32],
            },
            file_name: "TestFile".to_owned(),
            permissions: 0b110110000,
            modified: time::SystemTime::now()
                .duration_since(time::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
            created: time::SystemTime::now()
                .duration_since(time::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
            chunks: vec![],
        };
        db.add_file(&file);
    }

    #[test]
    fn test_get_file() {
        run_test(|db| {
            let db = db.lock().unwrap();
            let file = FileMetadata {
                file_id: FileId {
                    path: PathBuf::from("TestFile"),
                    hash: [0u8; 32],
                },
                file_name: "TestFile".to_owned(),
                permissions: 0b110110000,
                modified: time::SystemTime::now()
                    .duration_since(time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis(),
                created: time::SystemTime::now()
                    .duration_since(time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis(),
                chunks: vec![],
            };
            assert_eq!(Some(file), db.get_file("TestFile").unwrap())
        })
    }

    #[test]
    fn test_file_rm() {
        run_test(|db| {
            let db = db.lock().unwrap();
            db.rm_file(&FilePath("TestFile".to_owned()));
            assert_eq!(None, db.get_file("TestFile").unwrap())
        })
    }
}
