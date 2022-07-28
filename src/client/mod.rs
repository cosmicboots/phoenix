#![allow(dead_code)]

use std::{
    fs::{self, Metadata, Permissions},
    path::Path,
    time::SystemTime,
};

#[derive(Debug)]
pub struct FileMetadata {
    permissions: Permissions,
    modified: SystemTime,
    created: SystemTime,
}

impl From<Metadata> for FileMetadata {
    fn from(metadata: Metadata) -> Self {
        Self {
            permissions: metadata.permissions(),
            modified: metadata.modified().unwrap(),
            created: metadata.created().unwrap(),
        }
    }
}

pub fn get_file_info(path: &str) -> Result<FileMetadata, std::io::Error> {
    let md = fs::metadata(Path::new(path))?;
    Ok(FileMetadata::from(md))
}
