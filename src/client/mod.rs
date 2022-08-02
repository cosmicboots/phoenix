#![allow(dead_code)]

use base64ct::{Base64, Encoding};
use notify::{watcher, DebouncedEvent, Watcher};
use std::{fs, net::TcpStream, path::PathBuf, sync::mpsc, time::Duration};

mod file_operations;

use file_operations::get_file_info;

use crate::{
    client::file_operations::send_file_info,
    config::{ClientConfig, Config},
    messaging::{self, arguments, Directive},
    net::{Client, NoiseConnection},
};

pub fn start_client(config_file: &str, path: &str) {
    let config = ClientConfig::read_config(config_file).unwrap();

    let mut client = Client::new(
        TcpStream::connect(config.server_address).unwrap(),
        &Base64::decode_vec(&config.privkey).unwrap(),
        &[Base64::decode_vec(&config.server_pubkey).unwrap()],
    )
    .unwrap();

    let mut builder = messaging::MessageBuilder::new(1);
    let msg = builder.encode_message(Directive::AnnounceVersion, Some(arguments::Version(1)));

    client.send(&msg).unwrap();

    let path = PathBuf::from(path);

    if !fs::metadata(&path).unwrap().is_dir() {
        error!("Can only watch directories not files!");
        std::process::exit(1);
    }

    let (tx, rx) = mpsc::channel();
    let mut watcher = watcher(tx, Duration::from_secs(1)).unwrap();

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
                    let file_info = get_file_info(&p).unwrap();
                    debug!("{:?}", file_info);
                    match send_file_info(&mut builder, &mut client, file_info) {
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
