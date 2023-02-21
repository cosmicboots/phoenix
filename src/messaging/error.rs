//! Message module errors

use std::{error::Error, fmt::Display, io, string::FromUtf8Error};

#[derive(Debug)]
/// Error used when encoding and decoding messages.
pub enum MessageError {
    /// Invalid message conversion from binary
    InvalidBin,
    EmptyPath,
    UtfError,
    /// Generic IO error
    IO(String),
}

impl Display for MessageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self))
    }
}

impl Error for MessageError {}

impl From<io::Error> for MessageError {
    fn from(e: io::Error) -> Self {
        MessageError::IO(format!("{}", e))
    }
}

impl From<FromUtf8Error> for MessageError {
    fn from(e: FromUtf8Error) -> Self {
        MessageError::UtfError
    }
}
