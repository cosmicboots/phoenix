#![allow(dead_code)]

use base64ct::{Base64, Encoding};
use notify::{watcher, DebouncedEvent, Watcher};
use std::{
    fs::{self, Metadata, Permissions},
    net::TcpStream,
    path::PathBuf,
    sync::mpsc,
    time::{Duration, SystemTime},
};

mod file_operations;

use file_operations::{chunk_file, get_file_info};

use crate::{
    client::file_operations::send_file,
    config::{ClientConfig, Config},
    net::{Client, NoiseConnection}, messaging::{self, Directive, arguments},
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

pub fn start_client(config_file: &str, path: &str) {
    let config = ClientConfig::read_config(config_file).unwrap();

    let mut client = Client::new(
        TcpStream::connect(config.server_address).unwrap(),
        &Base64::decode_vec(&config.privkey).unwrap(),
        &Base64::decode_vec(&config.server_pubkey).unwrap(),
    );

    let mut builder = messaging::MessageBuilder::new(1);
    let msg = builder.encode_message(Directive::AnnounceVersion, Some(arguments::Version(1)));

    client.send(&msg);

    let (tx, rx) = mpsc::channel();
    let mut watcher = watcher(tx, Duration::from_secs(1)).unwrap();

    let path = PathBuf::from(path);

    if !fs::metadata(&path).unwrap().is_dir() {
        error!("Can only watch directories not files!");
        std::process::exit(1);
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
                    debug!("{:?}", get_file_info(&p).unwrap());
                    let chk_file = chunk_file(&p).unwrap();
                    send_file(chk_file);
                }
                _ => {}
            },
            Err(e) => error!("File system watch error: {:?}", e),
        }
    }
}
