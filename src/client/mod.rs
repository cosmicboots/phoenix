#![allow(dead_code)]

use base64ct::{Base64, Encoding};
use notify::{watcher, DebouncedEvent, Watcher};
use std::{fs, net::TcpStream, path::PathBuf, sync::mpsc, time::Duration};

mod file_operations;

use file_operations::{get_file_info, Client};

use crate::{
    config::{ClientConfig, Config},
    messaging,
    net::{NetClient, NoiseConnection},
};

pub fn start_client(config_file: &str, path: &str) {
    let config = ClientConfig::read_config(config_file).unwrap();

    let net_client = NetClient::new(
        TcpStream::connect(config.server_address).unwrap(),
        &Base64::decode_vec(&config.privkey).unwrap(),
        &[Base64::decode_vec(&config.server_pubkey).unwrap()],
    )
    .unwrap();
    let builder = messaging::MessageBuilder::new(1);
    let mut client = Client::new(builder, net_client);

    let watch_path = PathBuf::from(path);

    if !fs::metadata(&watch_path).unwrap().is_dir() {
        error!("Can only watch directories not files!");
        std::process::exit(1);
    }

    let (tx, rx) = mpsc::channel();
    let mut watcher = watcher(tx, Duration::from_secs(1)).unwrap();

    info!("Watching files");
    watcher
        .watch(watch_path, notify::RecursiveMode::Recursive)
        .unwrap();

    loop {
        match rx.recv() {
            Ok(event) => match event {
                DebouncedEvent::Rename(_, p)
                | DebouncedEvent::Create(p)
                | DebouncedEvent::Write(p)
                | DebouncedEvent::Chmod(p) => {
                    match client.send_file_info(&p) {
                        Ok(_) => info!("Successfully sent the file"),
                        Err(e) => error!("{:?}", e),
                    };
                }
                _ => {}
            },
            Err(e) => error!("File system watch error: {:?}", e),
        }
    }
}
