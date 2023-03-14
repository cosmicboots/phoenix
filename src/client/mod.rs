use crate::messaging::arguments::FileMetadata;
use std::{collections::HashMap, path::PathBuf};

mod actor;
mod file_operations;
mod utils;

pub use file_operations::CHUNK_SIZE;

type Blacklist = HashMap<PathBuf, FileMetadata>;
