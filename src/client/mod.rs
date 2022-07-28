#![allow(dead_code)]

use notify::{watcher, DebouncedEvent, Watcher};
use std::{
    fs::{self, Metadata, Permissions},
    path::PathBuf,
    sync::mpsc,
    time::{Duration, SystemTime},
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

pub fn start_client(path: &str) {
    let (tx, rx) = mpsc::channel();
    let mut watcher = watcher(tx, Duration::from_secs(1)).unwrap();

    let path = PathBuf::from(path);

    if !fs::metadata(&path).unwrap().is_dir() {
        error!("Can only watch directories not files!");
        return;
    }

    info!("Watching files");
    watcher
        .watch(path, notify::RecursiveMode::Recursive)
        .unwrap();

    loop {
        match rx.recv() {
            Ok(event) => match event {
                DebouncedEvent::Rename(_, p)
                | DebouncedEvent::Create(p)
                | DebouncedEvent::Write(p)
                | DebouncedEvent::Chmod(p) => {
                    debug!("{:?}", get_file_info(p).unwrap());
                }
                _ => {}
            },
            Err(e) => error!("File system watch error: {:?}", e),
        }
    }
}

fn get_file_info(path: PathBuf) -> Result<FileMetadata, std::io::Error> {
    let md = fs::metadata(path)?;
    Ok(FileMetadata::from(md))
}
