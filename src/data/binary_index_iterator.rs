use std::io::{self, Read};

use super::{binary_store_error::BinaryStoreError, indexed_binary_file_entry_store::IndexEntry};

pub struct BinaryIndexIterator<R: Read> {
    reader: R,
    record_size: usize,
}

impl<R: Read> BinaryIndexIterator<R> {
    pub fn new(reader: R, record_size: usize) -> Self {
        BinaryIndexIterator {
            reader,
            record_size,
        }
    }
}

impl<R: Read> Iterator for BinaryIndexIterator<R> {
    type Item = Result<IndexEntry, BinaryStoreError>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buffer = vec![0; self.record_size];
        match self.reader.read_exact(&mut buffer) {
            Ok(_) => {
                let record: Result<IndexEntry, _> = bincode::deserialize(&buffer);
                record.map_err(BinaryStoreError::SerializationError).into()
            }
            Err(ref e) if e.kind() == io::ErrorKind::UnexpectedEof => None,
            Err(e) => Some(Err(BinaryStoreError::IoError(e))),
        }
    }
}
