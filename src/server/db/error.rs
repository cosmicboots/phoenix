//! Database module errors

use std::{error::Error, fmt::Display};

#[derive(Debug)]
/// Error type used by the `Db` module.
pub enum DbError {
    /// Error from the underlying db engine
    EngineError(sled::Error),
    /// Error indicating duplicate file was added to database
    DuplicateFile,
}

impl Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self))
    }
}

impl Error for DbError {}

impl From<sled::Error> for DbError {
    fn from(e: sled::Error) -> Self {
        DbError::EngineError(e)
    }
}
