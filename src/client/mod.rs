use base64ct::{Base64, Encoding};
use notify::{watcher, DebouncedEvent, Watcher};
use std::{
    fs,
    net::TcpStream,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

mod file_operations;

use file_operations::Client;

use crate::{
    config::{ClientConfig, Config},
    messaging,
    net::{self, NetClient, NoiseConnection},
};

#[derive(Debug)]
enum QueueItem {
    ServerMsg(Vec<u8>),
    FileMsg(DebouncedEvent),
}

pub fn start_client(config_file: &Path, path: &Path) {
    let config = ClientConfig::read_config(config_file).unwrap();

    let net_client = NetClient::new(
        TcpStream::connect(config.server_address).unwrap(),
        &Base64::decode_vec(&config.privkey).unwrap(),
        &[Base64::decode_vec(&config.server_pubkey).unwrap()],
    )
    .unwrap();

    let listen_stream = net_client.clone_stream().unwrap();
    let builder = messaging::MessageBuilder::new(1);
    let mut client = Client::new(builder, net_client);

    let watch_path = PathBuf::from(path);

    if !fs::metadata(&watch_path).unwrap().is_dir() {
        error!("Can only watch directories not files!");
        std::process::exit(1);
    }

    let (msg_queue, incoming_msg): (Sender<QueueItem>, Receiver<QueueItem>) = mpsc::channel();

    let (tx, rx) = mpsc::channel();
    let mut watcher = watcher(tx, Duration::from_secs(1)).unwrap();
    let tx = msg_queue.clone();
    thread::spawn(move || {
        while let Ok(x) = rx.recv() {
            tx.send(QueueItem::FileMsg(x)).unwrap();
        }
    });

    info!("Watching files");
    watcher
        .watch(watch_path, notify::RecursiveMode::Recursive)
        .unwrap();

    let tx = msg_queue.clone();
    thread::spawn(move || {
        let mut stream = listen_stream;
        debug!("Listening for messages on tcp stram");
        while let Ok(msg) = net::recv(&mut stream) {
            debug!("TCP MESSAGE: {:?}", msg);
            tx.send(QueueItem::ServerMsg(msg)).unwrap();
        }
    });

    loop {
        match incoming_msg.recv() {
            Ok(msg) => debug!("Message: {:?}", msg),
            Err(_) => todo!(),
        }
    }

    //loop {
    //    match rx.recv() {
    //        Ok(event) => match event {
    //            DebouncedEvent::Rename(_, p)
    //            | DebouncedEvent::Create(p)
    //            | DebouncedEvent::Write(p)
    //            | DebouncedEvent::Chmod(p) => {
    //                match client.send_file_info(&p) {
    //                    Ok(chunks) => {
    //                        info!("Successfully sent the file");
    //                        client.send_chunks(&p, chunks).unwrap();
    //                    }
    //                    Err(e) => error!("{:?}", e),
    //                };
    //            }
    //            _ => {}
    //        },
    //        Err(e) => error!("File system watch error: {:?}", e),
    //    }
    //}
}
