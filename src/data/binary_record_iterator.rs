use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{self, Read};

use super::{binary_file_entry_store::BinaryStoreError, model::Entry};

pub struct BinaryRecordIterator<R: Read> {
    reader: R,
}

impl<R: Read> BinaryRecordIterator<R> {
    pub fn new(reader: R) -> Self {
        BinaryRecordIterator { reader }
    }
}

impl<R: Read> Iterator for BinaryRecordIterator<R> {
    type Item = Result<(String, Entry), BinaryStoreError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.reader.read_u64::<LittleEndian>() {
            Ok(len) => {
                let mut buffer = vec![0; len as usize];
                match self.reader.read_exact(&mut buffer) {
                    Ok(()) => {
                        let record: Result<(String, Entry), _> = bincode::deserialize(&buffer);
                        record.map_err(BinaryStoreError::SerializationError).into()
                    }
                    Err(e) => Some(Err(BinaryStoreError::IoError(e))),
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::UnexpectedEof => None,
            Err(e) => Some(Err(BinaryStoreError::IoError(e))),
        }
    }
}
