use notify::DebouncedEvent;
use tokio::{
    select,
    sync::{
        mpsc::{self, error::SendError, Receiver, Sender},
        oneshot,
    },
};

#[derive(Debug)]
pub enum ApiRequest {
    GetStatus(oneshot::Sender<usize>),
}

#[derive(Clone)]
pub struct EventActorHandle {
    api_tx: Sender<ApiRequest>,
    fs_tx: Sender<DebouncedEvent>,
    serv_tx: Sender<Vec<u8>>,
}

impl EventActorHandle {
    pub fn new() -> Self {
        let (api_tx, api_rx) = mpsc::channel(8);
        let (fs_tx, fs_rx) = mpsc::channel(8);
        let (serv_tx, serv_rx) = mpsc::channel(8);
        let actor = EventActor::new(api_rx, fs_rx, serv_rx);
        tokio::spawn(async move { actor.run().await });

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
    pub async fn run(mut self) {
        loop {
            select! {
                // Client API requests
                req = self.api_rx.recv() => {
                    todo!()
                }
                // Filesystem messages
                event = self.fs_rx.recv() => {
                    todo!()
                }
                // TODO: Server messages
            }
        }
    }
}
