use std::{error::Error, fmt::Display, io};

#[derive(Debug)]
pub enum NetError {
    /// Generic Noise Error
    NoiseError(String),
    /// Network message too long to send using Noise
    MsgLengthError(usize),
    /// Generic IO Error
    IOError(String),
}

impl Display for NetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self))
    }
}

impl Error for NetError {}

impl From<snow::Error> for NetError {
    fn from(e: snow::Error) -> Self {
        NetError::NoiseError(format!("{}", e))
    }
}

impl From<io::Error> for NetError {
    fn from(e: io::Error) -> Self {
        NetError::IOError(format!("{}", e))
    }
}
