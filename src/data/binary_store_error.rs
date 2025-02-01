use std::{fmt, io};

use bincode::Error as BincodeError;

#[derive(Debug)]
pub enum BinaryStoreError {
    IoError(io::Error),
    SerializationError(BincodeError),
    IndexRecordTooLarge,
}

impl From<io::Error> for BinaryStoreError {
    fn from(error: io::Error) -> Self {
        BinaryStoreError::IoError(error)
    }
}

impl From<BincodeError> for BinaryStoreError {
    fn from(error: BincodeError) -> Self {
        BinaryStoreError::SerializationError(error)
    }
}

impl fmt::Display for BinaryStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            BinaryStoreError::IoError(ref err) => {
                write!(f, "I/O error: {}", err)
            }
            BinaryStoreError::SerializationError(ref err) => {
                write!(f, "Serialization error: {}", err)
            }
            BinaryStoreError::IndexRecordTooLarge => {
                write!(f, "Index record is too large: ")
            }
        }
    }
}
