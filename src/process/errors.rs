#[derive(Debug)]
pub enum Error {
    TimeoutError,
    IOError(String),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::IOError(e.to_string())
    }
}
