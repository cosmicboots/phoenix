//! Network module errors

use std::{error::Error, fmt::Display, io};

#[derive(Debug)]
/// Error type used by the `Net` module.
pub enum NetError {
    /// Generic Noise Error
    Noise(String),
    /// Network message too long to send using Noise
    MsgLength(usize),
    /// Generic IO Error
    IO(String),
}

impl Display for NetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self))
    }
}

impl Error for NetError {}

impl From<snow::Error> for NetError {
    fn from(e: snow::Error) -> Self {
        NetError::Noise(format!("{}", e))
    }
}

impl From<io::Error> for NetError {
    fn from(e: io::Error) -> Self {
        NetError::IO(format!("{}", e))
    }
}
