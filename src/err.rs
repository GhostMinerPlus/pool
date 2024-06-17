use std::fmt::Display;

#[derive(Debug)]
pub enum Error {
    Other(String),
    NotLogin(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Other(msg) => write!(f, "{msg}"),
            Error::NotLogin(msg) => write!(f, "{msg}"),
        }
    }
}
