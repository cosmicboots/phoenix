use serde::{Deserialize, Serialize};

mod db;

use db::{Db, File, Chunk};

#[derive(Serialize, Deserialize, Debug)]
struct File2 {
    file: String,
    chunks: Vec<String>,
}

fn main() -> sled::Result<()> {
    println!("Hello world");

    let f = File {
        filename: String::from("filename.txt"),
        chunks: vec![String::from("chunk1")],
        hash: String::from("ABCDEF1234567890"),
    };

    let c = Chunk {
        hash: String::from("chunk1"),
        data: "Hello world".as_bytes().to_vec(),
    };

    let db = Db::new()?;

    db.add_file(&f).unwrap();
    db.add_chunk(&c).unwrap();

    dbg!(db.get_file(f.hash).unwrap());
    dbg!(db.get_chunk(c.hash).unwrap());

    Ok(())
}
