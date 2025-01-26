use std::{
    fmt::{self},
    fs::{remove_file, rename, File, OpenOptions},
    io::{self, Write},
    path::Path,
};

use bincode::Error as BincodeError;
use byteorder::{LittleEndian, WriteBytesExt};

use super::{
    binary_record_iterator::BinaryRecordIterator,
    data_store::{DataStore, Filter},
    model::Entry,
};
use log::{debug, error, info};

// ----- Binary store error

#[derive(Debug)]
pub enum BinaryStoreError {
    IoError(io::Error),
    SerializationError(BincodeError),
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
        }
    }
}

// ------------------------

pub struct BinaryFileEntryStore {
    file_path: String,
}

impl BinaryFileEntryStore {
    pub fn new(file_path: String) -> Self {
        if !Self::file_exists(&file_path) {
            debug!("File {} does not exist. Creating...", &file_path);

            match File::create(&file_path) {
                Ok(_) => info!("File {} has been created.", file_path),
                Err(e) => error!("File creation failed! {}: {}", file_path, e),
            }
        }

        BinaryFileEntryStore { file_path }
    }

    fn file_exists(file_path: &str) -> bool {
        let path = Path::new(file_path);

        if path.exists() {
            true
        } else {
            false
        }
    }

    fn move_to_new_file<P: AsRef<Path>>(
        &self,
        new_file_path: P,
        deleting_keys: &[String],
        appending_entries: Vec<&Entry>,
    ) -> Result<(), BinaryStoreError> {
        let mut new_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(new_file_path)?;

        let existing_file = File::open(&self.file_path)?;

        for result in BinaryRecordIterator::new(existing_file) {
            let (existing_id, existing_entry) = result?;
            if !deleting_keys.contains(&existing_id) {
                let _ = self.write_entry(&existing_entry, &mut new_file)?;
            }
        }

        for new_entry in appending_entries {
            let _ = self.write_entry(&new_entry, &mut new_file)?;
        }

        new_file.flush()?;
        Ok(())
    }

    fn write_entry<W: Write>(&self, entry: &Entry, writer: &mut W) -> Result<(), BinaryStoreError> {
        let serialized = &bincode::serialize(&(&entry.id, entry))?;
        writer.write_u64::<LittleEndian>(serialized.len() as u64)?;
        writer.write_all(&serialized)?;
        Ok(())
    }
}

impl DataStore<String, Entry, BinaryStoreError> for BinaryFileEntryStore {
    fn save(&self, id: &String, value: &Entry) -> Result<(), BinaryStoreError> {
        let to_delete: Vec<String> = vec![id.into()];
        let to_append = vec![value];
        let new_path_string = format!("{}-tmp", self.file_path);
        let new_path = &new_path_string;
        self.move_to_new_file(new_path, &to_delete, to_append)?;

        remove_file(&self.file_path)?;
        rename(new_path, &self.file_path)?;
        Ok(())
    }

    fn load(&self, id: &String) -> Result<Option<Entry>, BinaryStoreError> {
        // Use OpenOptions to open the file
        let file = OpenOptions::new().read(true).open(&self.file_path)?;

        for record in BinaryRecordIterator::new(file) {
            let (existing_id, existing_value) = record?;
            if existing_id == *id {
                return Ok(Some(existing_value));
            }
        }

        Ok(None)
    }

    fn delete(&self, id: &String) -> Result<(), BinaryStoreError> {
        let to_delete: Vec<String> = vec![id.into()];
        let to_append = vec![];
        let new_path_string = format!("{}-tmp", self.file_path);
        let new_path = &new_path_string;
        self.move_to_new_file(new_path, &to_delete, to_append)?;

        remove_file(&self.file_path)?;
        rename(new_path, &self.file_path)?;
        Ok(())
    }

    fn search(&self, filter: &dyn Filter<Entry>) -> Result<Vec<Entry>, BinaryStoreError> {
        // Use OpenOptions to open the file
        let file = OpenOptions::new().read(true).open(&self.file_path)?;
        let mut result: Vec<Entry> = vec![];

        for record in BinaryRecordIterator::new(file) {
            let (_, existing_value) = record?;
            if filter.pass(&existing_value) {
                result.push(existing_value);
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self};
    use std::path::Path;
    use uuid::Uuid;

    struct TitleFilter {
        keyword: String,
    }

    impl Filter<Entry> for TitleFilter {
        fn pass(&self, entry: &Entry) -> bool {
            entry.title.contains(&self.keyword)
        }
    }

    fn setup_test_file() -> String {
        let test_id = Uuid::new_v4();
        print!("{}", test_id);
        let test_file_path = format!("test_store_{}.bin", test_id);
        if Path::new(&test_file_path).exists() {
            fs::remove_file(&test_file_path).unwrap();
        }
        test_file_path.to_string()
    }

    #[test]
    fn test_save_and_load() {
        let test_file_path = setup_test_file();
        let store = BinaryFileEntryStore::new(test_file_path.clone());

        let entry = Entry {
            id: "1".to_string(),
            title: "Test Entry".to_string(),
            username: Some("user1".to_string()),
            password: Some("pass1".to_string()),
            url: Some("http://example.com".to_string()),
            note: Some("This is a note".to_string()),
        };

        // Save the entry
        let _ = store.save(&entry.id, &entry);

        // Load the entry
        let loaded_entry = store.load(&entry.id).unwrap();

        assert_eq!(loaded_entry, Some(entry));

        // Clean up
        fs::remove_file(test_file_path).unwrap();
    }

    #[test]
    fn test_delete() {
        let test_file_path = setup_test_file();
        let store = BinaryFileEntryStore::new(test_file_path.clone());

        let entry = Entry {
            id: "1".to_string(),
            title: "Entry to delete".to_string(),
            username: Some("user1".to_string()),
            password: None,
            url: None,
            note: None,
        };

        // Save the entry
        store.save(&entry.id, &entry).unwrap();

        // Delete the entry
        store.delete(&entry.id).unwrap();

        // Ensure the entry is gone
        let loaded_entry = store.load(&entry.id).unwrap();
        assert!(loaded_entry.is_none());

        // Clean up
        fs::remove_file(test_file_path).unwrap();
    }

    #[test]
    fn test_search() {
        let test_file_path = setup_test_file();
        let store = BinaryFileEntryStore::new(test_file_path.clone());

        let entry1 = Entry {
            id: "1".to_string(),
            title: "Searchable Entry 1".to_string(),
            username: Some("user1".to_string()),
            password: Some("pass1".to_string()),
            url: None,
            note: None,
        };

        let entry2 = Entry {
            id: "2".to_string(),
            title: "Another Searchable Entry".to_string(),
            username: Some("user2".to_string()),
            password: Some("pass2".to_string()),
            url: None,
            note: None,
        };

        let entry3 = Entry {
            id: "3".to_string(),
            title: "Non-Matching Entry".to_string(),
            username: None,
            password: None,
            url: None,
            note: None,
        };

        //Save entries
        store.save(&entry1.id, &entry1).unwrap();
        store.save(&entry2.id, &entry2).unwrap();
        store.save(&entry3.id, &entry3).unwrap();

        //Search with a filter
        let filter = TitleFilter {
            keyword: "Searchable".to_string(),
        };

        let results = store.search(&filter).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.contains(&entry1));
        assert!(results.contains(&entry2));

        // Clean up
        fs::remove_file(test_file_path).unwrap();
    }

    #[test]
    fn test_load_nonexistent_entry() {
        let test_file_path = setup_test_file();
        let store = BinaryFileEntryStore::new(test_file_path.clone());

        // Attempt to load a nonexistent entry
        let loaded_entry = store.load(&"nonexistent".to_string()).unwrap();
        assert!(loaded_entry.is_none());

        // Clean up
        fs::remove_file(test_file_path).unwrap();
    }
}
