//! Database module to handle backend storage and transactions

#![allow(dead_code)]

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};
use sled::{transaction::ConflictableTransactionResult, Transactional, Tree};

/// Static name of the file_table
static FILE_TABLE: &str = "file_table";
/// Static name of the chunk_table
static CHUNK_TABLE: &str = "chunk_table";
/// Static name of the chunk_count table
static CHUNK_COUNT: &str = "chunk_count";
/// Static name of the entire database
static DB_NAME: &str = "data";

#[derive(Serialize, Deserialize, Debug, PartialEq)]
/// Structure to store needed metadata for a file
///
/// This includes:
/// - filename
/// - A vector of chunk IDs (their hashes)
/// - The overall hash of the file being represented
pub struct File {
    pub filename: String,
    pub chunks: Vec<String>,
    pub hash: String,
}

#[derive(Debug)]
/// Structure for a single file chunk. Composed of the hash and the data.
pub struct Chunk {
    pub hash: String,
    pub data: Vec<u8>,
}

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
}

impl Db {
    /// Create a new instance of the database.
    ///
    /// This also opens the database tables using the statics:
    /// - [`FILE_TABLE`](static.FILE_TABLE.html)
    /// - [`CHUNK_TABLE`](static.CHUNK_TABLE.html)
    /// - [`CHUNK_COUNT`](static.CHUNK_COUNT.html)
    pub fn new() -> sled::Result<Db> {
        let db = sled::open(DB_NAME)?;
        Ok(Db {
            file_table: db.open_tree(FILE_TABLE)?,
            chunk_table: db.open_tree(CHUNK_TABLE)?,
            chunk_count: db.open_tree(CHUNK_COUNT)?,
        })
    }

    /// Adds a [File](struct.File.html) struct into the file_table database.
    ///
    /// This also increments the referenced values in the [`chunk_count`](#structfield.chunk_count)
    /// table; however, it doesn't actually insert any data into the
    /// [`chunk_table`](#structfield.chunk_table) table.
    pub fn add_file(&self, file: &File) -> sled::Result<()> {
        let value = match bincode::serialize(&file) {
            Ok(x) => x,
            Err(_) => panic!("Couldn't serialize file to store in database"),
        };
        // TODO: Improve error handling
        (&self.file_table, &self.chunk_count)
            .transaction(
                |(ft, cc)| -> ConflictableTransactionResult<(), sled::Error> {
                    // Add the file metadata to the file table
                    ft.insert(file.hash.as_bytes(), &*value).unwrap();
                    // Add all the chunks into the chunk count table
                    for chunk in &file.chunks {
                        let mut wtr = vec![];
                        cc.insert(
                            chunk.as_bytes(),
                            match cc.get(chunk).unwrap() {
                                Some(x) => {
                                    // Increment value already stored
                                    let mut rdr = std::io::Cursor::new(x);
                                    wtr.write_u32::<LittleEndian>(
                                        rdr.read_u32::<LittleEndian>().unwrap() + 1,
                                    )
                                    .unwrap();
                                    wtr
                                }
                                None => {
                                    // If no value, make it 1
                                    wtr.write_u32::<LittleEndian>(1).unwrap();
                                    wtr
                                }
                            },
                        )
                        .unwrap();
                    }
                    Ok(())
                },
            )
            .unwrap();
        Ok(())
    }

    /// Returns a [File](struct.File.html) from the database when given a file_hash.
    pub fn get_file(&self, file_hash: &str) -> sled::Result<File> {
        match self.file_table.get(file_hash) {
            Ok(x) => match x {
                Some(value) => {
                    Ok(bincode::deserialize::<File>(&value).expect("Failed to deserialize"))
                }
                None => panic!("Not found in the database"),
            },
            Err(e) => Err(e),
        }
    }

    /// Adds a chunk into the [`chunk_table`](#structfield.chunk_table) table.
    ///
    /// NOTE: This should be run after [`add_file()`](#method.add_file) so orphaned chunks into the
    /// database. This function checks the chunk count table to ensure references to the chunk
    /// exist. If this check wasn't preformed, it would be possible to add orphaned chunks into the
    /// database, which would be expensive to clean up.
    pub fn add_chunk(&self, chunk: &Chunk) -> sled::Result<()> {
        //self.chunk_table.insert(&*chunk.hash, &*chunk.data)?;
        (&self.chunk_table, &self.chunk_count)
            .transaction(
                |(ct, cc)| -> ConflictableTransactionResult<(), sled::Error> {
                    // Check to see if the chunk is referenced (via the chunk_count table) to make
                    // sure orphaned chunks are never added into the database. This should prevent
                    // the need of expensive database clean up operations
                    if let Ok(Some(_)) = cc.get(&chunk.hash) {
                        ct.insert(chunk.hash.as_str(), chunk.data.to_owned())?;
                    }
                    Ok(())
                },
            )
            .unwrap();
        Ok(())
    }

    /// Gets a chunk out of the database given it's ID (hash).
    pub fn get_chunk(&self, chunk_hash: &str) -> sled::Result<Chunk> {
        // TODO: Improve error handling
        match self.chunk_table.get(&chunk_hash) {
            Ok(x) => match x {
                Some(value) => Ok(Chunk {
                    hash: chunk_hash.to_owned(),
                    data: value.to_vec(),
                }),
                None => panic!("Chunk not found"),
            },
            Err(e) => Err(e),
        }
    }

    pub fn rm_file(&self, file_hash: &str) {
        (&self.file_table, &self.chunk_table, &self.chunk_count)
            .transaction(
                |(ft, ct, cc)| -> ConflictableTransactionResult<(), sled::Error> {
                    // 1. Get the file and desearialize it
                    // 2. Iterate through the chunks and decrement the refcounter
                    // 3.   if 0 refs, delete the chunk from the chunk table
                    if let Ok(Some(bin_file)) = ft.get(&file_hash) {
                        // Deserialize bin into the File struct
                        if let Ok(file) = bincode::deserialize::<File>(&bin_file) {
                            for chunk in file.chunks {
                                if let Ok(Some(x)) = cc.get(&chunk) {
                                    let mut rdr = std::io::Cursor::new(x);
                                    match rdr.read_u32::<LittleEndian>() {
                                        // If there are no more references to the given chunk,
                                        // remove it from the chunk table and the chunk count table
                                        Ok(0) | Ok(1) => {
                                            ct.remove(&*chunk)?;
                                            cc.remove(&*chunk)?;
                                        }
                                        Ok(x) => {
                                            let mut wtr = vec![];
                                            wtr.write_u32::<LittleEndian>(x - 1).unwrap();
                                            cc.insert(chunk.as_bytes(), wtr)?;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            ft.remove(file.hash.as_bytes()).unwrap();
                        }
                    }
                    Ok(())
                },
            )
            .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The tests need to be able to use their own temperary database rather than using the global
    // static

    #[test]
    fn test_file_db() {
        let db = Db::new().unwrap();
        let f = File {
            filename: String::from("filename.txt"),
            chunks: vec![String::from("chunk1")],
            hash: String::from("ABCDEF1234567890"),
        };
        db.add_file(&f).unwrap();
        assert_eq!(f, db.get_file("ABCDEF1234567890").unwrap())
    }

    #[test]
    fn test_file_rm() {
        let db = Db::new().unwrap();
        let f = File {
            filename: String::from("filename.txt"),
            chunks: vec![String::from("chunk1")],
            hash: String::from("ABCDEF1234567890"),
        };
        db.add_file(&f).unwrap();
        db.rm_file(&f.hash);
        assert_eq!(f, db.get_file("ABCDEF1234567890").unwrap())
    }
}
