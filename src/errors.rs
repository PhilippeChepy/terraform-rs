use crate::process;

#[derive(Debug)]
pub enum Error {
    PathError,
    IOError(String),
    RegexError(String),
    ProcessError(process::Error),
}

impl From<regex::Error> for Error {
    fn from(e: regex::Error) -> Error {
        Error::RegexError(e.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::IOError(e.to_string())
    }
}

impl From<process::Error> for Error {
    fn from(e: process::Error) -> Error {
        Error::ProcessError(e)
    }
}
