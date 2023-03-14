mod handlers;

use crate::{
    client::{actor::handlers::handle_server_event, file_operations::Client, Blacklist},
    config::ClientConfig,
    messaging::{self, MessageBuilder},
    net::{NetClient, NoiseConnection},
};
use base64ct::{Base64, Encoding};
use handlers::handle_fs_event;
use log::{debug, error, info};
use notify::{watcher, DebouncedEvent, Watcher};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};
use tokio::{
    net::TcpStream,
    select,
    sync::{
        mpsc::{self, error::SendError, Receiver, Sender},
        oneshot,
    },
};

#[derive(Debug)]
pub enum ApiRequest {
    GetStatus(oneshot::Sender<usize>),
    Stop,
}

#[derive(Clone, Debug)]
pub struct EventActorHandle {
    api_tx: Sender<ApiRequest>,
    fs_tx: Sender<DebouncedEvent>,
    serv_tx: Sender<Vec<u8>>,
}

impl EventActorHandle {
    pub fn new(config: ClientConfig, path: &Path) -> Self {
        let (api_tx, api_rx) = mpsc::channel(8);
        let (fs_tx, fs_rx) = mpsc::channel(8);
        let (serv_tx, serv_rx) = mpsc::channel(8);
        let actor = EventActor::new(api_rx, fs_rx, serv_rx);
        tokio::spawn(async move { actor.run(config, path).await });

        Self {
            api_tx,
            fs_tx,
            serv_tx,
        }
    }

    pub async fn send_api_request(&self, req: ApiRequest) -> Result<(), SendError<ApiRequest>> {
        self.api_tx.send(req).await
    }

    pub async fn send_fs_event(
        &self,
        event: DebouncedEvent,
    ) -> Result<(), SendError<DebouncedEvent>> {
        self.fs_tx.send(event).await
    }

    pub async fn stop(self) {
        let _ = self.api_tx.send(ApiRequest::Stop).await;
    }
}

/// This Actor will process all client events, which includes:
/// 1. File System events
/// 2. Server messages
/// 3. Public client API (library)
pub struct EventActor {
    api_rx: Receiver<ApiRequest>,
    fs_rx: Receiver<DebouncedEvent>,
    serv_rx: Receiver<Vec<u8>>,
}

impl EventActor {
    pub fn new(
        api_rx: Receiver<ApiRequest>,
        fs_rx: Receiver<DebouncedEvent>,
        serv_rx: Receiver<Vec<u8>>,
    ) -> Self {
        Self {
            api_rx,
            fs_rx,
            serv_rx,
        }
    }

    /// The main entrypoint for the actor.
    ///
    /// It should be called from it's own thread:
    /// ```rust
    /// let actor = EventActor::new(api_rx, fs_rx);
    /// tokio::spawn(async move { actor.run().await });
    /// ```
    pub async fn run(mut self, config: ClientConfig, path: &Path) {
        let net_client = NetClient::new(
            TcpStream::connect(config.server_address).await.unwrap(),
            &Base64::decode_vec(&config.privkey).unwrap(),
            &[Base64::decode_vec(&config.server_pubkey).unwrap()],
        )
        .await
        .unwrap();

        let builder = messaging::MessageBuilder::new(1);
        let mut client = Client::new(builder, net_client);

        let watch_path = PathBuf::from(path);
        if !fs::metadata(&watch_path).unwrap().is_dir() {
            error!("Can only watch directories not files!");
            std::process::exit(1);
        }

        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = watcher(tx, Duration::from_secs(1)).unwrap();
        let (tx, mut fs_event): (Sender<DebouncedEvent>, Receiver<DebouncedEvent>) =
            mpsc::channel(100);
        thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    while let Ok(x) = rx.recv() {
                        tx.send(x).await.unwrap();
                    }
                });
        });

        info!("Watching files");
        watcher
            .watch(&watch_path, notify::RecursiveMode::Recursive)
            .unwrap();

        // Get startup file list to compare against local file tree
        client.request_file_list().await.unwrap();

        let mut blacklist: Blacklist = HashMap::new();

        loop {
            select! {
                // Client API requests
                req = self.api_rx.recv() => {
                    if let Some(req) = req {
                        match req {
                            ApiRequest::GetStatus(_) => todo!(),
                            ApiRequest::Stop => break,
                        }
                    }
                }
                // Server messages
                push = (&mut client).recv() => {
                    match MessageBuilder::decode_message(&push.unwrap()) {
                        Ok(msg) => handle_server_event(&mut client, &watch_path, *msg, &mut blacklist).await,
                        Err(e) => error!("msg decode error: {:?}", e),
                    }
                }
                // Filesystem messages
                event = fs_event.recv() => {
                    if event.is_some() {
                        handle_fs_event(
                            &mut client,
                            &watch_path.canonicalize().unwrap(),
                            event.unwrap(),
                            &mut blacklist).await;
                    } else {
                        debug!("Failing fs_event checking");
                    }
                }
                // TODO: Server messages
            }
        }
        debug!("Client event loop stopped");
    }
}
