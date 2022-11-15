use std::{error::Error, fmt::Display, io};

#[derive(Debug)]
pub enum NetError {
    NoiseError(Box<dyn Error>),
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
        NetError::NoiseError(Box::new(e))
    }
}

impl From<io::Error> for NetError {
    fn from(e: io::Error) -> Self {
        NetError::IOError(format!("{}", e))
    }
}
