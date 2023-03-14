pub mod actor;
mod file_operations;
mod utils;

use crate::{config::ClientConfig, messaging::arguments::FileMetadata};
use std::{
    collections::HashMap,
    marker::PhantomData,
    path::{Path, PathBuf},
};

use self::actor::EventActorHandle;
pub use file_operations::CHUNK_SIZE;

type Blacklist = HashMap<PathBuf, FileMetadata>;

#[derive(Debug)]
pub struct Stopped;

#[derive(Debug)]
pub struct Running;

#[derive(Debug)]
pub struct Client<'a, State = Stopped> {
    config: ClientConfig,
    watch_path: &'a Path,
    event_handle: Option<EventActorHandle>,
    state: PhantomData<State>,
}

impl<'a> Client<'a> {
    pub fn new(config: ClientConfig, path: &'a Path) -> Self {
        Self {
            config,
            watch_path: path,
            event_handle: None,
            state: Default::default(),
        }
    }
}

impl<'a> Client<'a, Stopped> {
    pub fn start(self) -> Client<'a, Running> {
        let event_handle = Some(EventActorHandle::new(&self.config, self.watch_path));

        Client {
            config: self.config,
            watch_path: self.watch_path,
            event_handle,
            state: PhantomData::<Running>,
        }
    }
}

impl<'a> Client<'a, Running> {
    pub async fn stop(self) -> Client<'a, Stopped> {
        self.event_handle.unwrap().stop().await;
        Client {
            config: self.config,
            watch_path: self.watch_path,
            event_handle: None,
            state: PhantomData::<Stopped>,
        }
    }
}
