use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};
use sled::{transaction::ConflictableTransactionResult, Transactional, Tree};

static FILE_TABLE: &str = "file_table";
static CHUNK_TABLE: &str = "chunk_table";
static CHUNK_COUNT: &str = "chunk_count";
static DB_NAME: &str = "data";

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct File {
    pub filename: String,
    pub chunks: Vec<String>,
    pub hash: String,
}

#[derive(Debug)]
pub struct Chunk {
    pub hash: String,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub struct Db {
    file_table: Tree,
    chunk_table: Tree,
    chunk_count: Tree,
}

fn merge_inc(old: Option<&[u8]>) -> Vec<u8> {
    let mut wtr = vec![];
    dbg!(&old);
    let ret = match old {
        Some(x) => {
            let mut rdr = std::io::Cursor::new(x);
            wtr.write_u32::<LittleEndian>(rdr.read_u32::<LittleEndian>().unwrap() + 1)
                .unwrap();
            wtr
        }
        None => {
            wtr.write_u32::<LittleEndian>(1).unwrap();
            wtr
        }
    };
    dbg!(&ret);
    ret.to_vec()
}

impl Db {
    pub fn new() -> sled::Result<Db> {
        let db = sled::open(DB_NAME)?;
        Ok(Db {
            file_table: db.open_tree(FILE_TABLE)?,
            chunk_table: db.open_tree(CHUNK_TABLE)?,
            chunk_count: db.open_tree(CHUNK_COUNT)?,
        })
    }

    pub fn add_file(&self, file: &File) -> sled::Result<()> {
        let value = match bincode::serialize(&file) {
            Ok(x) => x,
            Err(_) => panic!("Couldn't serialize file to store in database"),
        };
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

    pub fn get_file(&self, file_hash: String) -> sled::Result<File> {
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

    pub fn add_chunk(&self, chunk: &Chunk) -> sled::Result<()> {
        self.chunk_table.insert(&*chunk.hash, &*chunk.data)?;
        Ok(())
    }

    pub fn get_chunk(&self, chunk_hash: String) -> sled::Result<Chunk> {
        match self.chunk_table.get(&chunk_hash) {
            Ok(x) => match x {
                Some(value) => Ok(Chunk {
                    hash: chunk_hash,
                    data: value.to_vec(),
                }),
                None => panic!("Chunk now found"),
            },
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_db() {
        let db = Db::new().unwrap();
        let f = File {
            filename: String::from("filename.txt"),
            chunks: vec![String::from("chunk1")],
            hash: String::from("ABCDEF1234567890"),
        };
        db.add_file(&f).unwrap();
        assert_eq!(f, db.get_file("ABCDEF1234567890".to_string()).unwrap())
    }

    #[test]
    fn test_merge_inc() {
        assert_eq!(
            merge_inc(&[0u8], None, None),
            Some([1u8, 0u8, 0u8, 0u8].to_vec())
        );
        assert_eq!(
            merge_inc(&[0u8], Some(&[100u8, 0u8, 0u8, 0u8]), None),
            Some([101u8, 0u8, 0u8, 0u8].to_vec())
        );
        assert_eq!(
            merge_inc(&[0u8], Some(&[255u8, 0u8, 0u8, 0u8]), None),
            Some([0u8, 1u8, 0u8, 0u8].to_vec())
        );
    }
}
