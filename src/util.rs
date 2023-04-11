use std::{error, io};

pub fn report_data_err(e: impl Into<Box<dyn error::Error + Send + Sync>>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e)
}

pub fn other_error(details: String) -> io::Error {
    io::Error::new(io::ErrorKind::Other, details)
}
