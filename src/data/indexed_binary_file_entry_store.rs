use super::{
    binary_index_iterator::BinaryIndexIterator, binary_store_error::BinaryStoreError,
    data_store::DataStore, model::Entry,
};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{remove_file, rename, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

// 36 (id: string representation of uuid v4) + 8 (offset) + 8 (length) = 52 bytes
const INDEX_RECORD_SIZE: usize = 52;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Position {
    offset: u64,
    length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexEntry {
    id: String,
    position: Position,
}

pub struct IndexedBinaryFileEntryStore {
    data_file_path: String,
    index_file_path: String,
    index: HashMap<String, Position>,
    needs_index_rewrite: bool,
    needs_data_rewrite: bool,
}

impl IndexedBinaryFileEntryStore {
    pub fn new(data_file_path: String, index_file_path: String) -> Self {
        let check_files = (
            Self::file_exists(&data_file_path),
            Self::file_exists(&index_file_path),
        );

        match check_files {
            // None of the files exist!
            (false, false) => {
                debug!(
                    "Files {} and {} do not exist. Creating...",
                    data_file_path, index_file_path
                );
                match File::create(&data_file_path) {
                    Ok(_) => info!("File {} has been created.", data_file_path),
                    Err(e) => error!("File creation failed! {}: {}", data_file_path, e),
                }
                match File::create(&index_file_path) {
                    Ok(_) => info!("File {} has been created.", index_file_path),
                    Err(e) => error!("File creation failed! {}: {}", index_file_path, e),
                }
            }
            // Both files exist
            (true, true) => debug!("Files {} and {} do exist.", data_file_path, index_file_path),
            // Index file does not exist!
            (true, false) => {
                debug!("File {} does not exist. Creating...", index_file_path);
                match File::create(&index_file_path) {
                    Ok(_) => info!("File {} has been created.", index_file_path),
                    Err(e) => error!("File creation failed! {}: {}", index_file_path, e),
                }
            }
            // Data file does not exist!
            (false, true) => {
                debug!("File {} does not exist. Creating...", data_file_path);
                match File::create(&data_file_path) {
                    Ok(_) => info!("File {} has been created.", data_file_path),
                    Err(e) => error!("File creation failed! {}: {}", data_file_path, e),
                }
            }
        }

        Self {
            data_file_path,
            index_file_path,
            index: HashMap::new(),
            needs_index_rewrite: false,
            needs_data_rewrite: false,
        }
    }

    fn file_exists(file_path: &str) -> bool {
        let path = Path::new(file_path);

        if path.exists() {
            true
        } else {
            false
        }
    }

    pub fn reload_index(&mut self) {
        match Self::load_index(&self.index_file_path) {
            Ok(map) => self.index = map,
            Err(e) => error!(
                "Reloading index failed. Index file: {} - error: {}",
                self.index_file_path, e
            ),
        }
    }

    pub fn rewrite_index(&mut self) -> Result<(), BinaryStoreError> {
        let temp_index_file = format!("temp_{}", self.index_file_path);

        match Self::write_index(&temp_index_file, &self.index) {
            Ok(_) => {
                remove_file(&self.index_file_path)?;
                rename(&temp_index_file, &self.index_file_path)?;
                self.needs_index_rewrite = false;
                Ok(())
            }
            Err(e) => {
                error!("Writing index file failed!, {}", e);
                Err(e)
            }
        }
    }

    pub fn needs_index_rewrite(&self) -> bool {
        self.needs_index_rewrite
    }

    pub fn needs_data_rewrite(&self) -> bool {
        self.needs_data_rewrite
    }

    fn write_index<P: AsRef<Path>>(
        index_file: P,
        index: &HashMap<String, Position>,
    ) -> Result<(), BinaryStoreError> {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(index_file)?;

        for (id, position) in index {
            let serialized: &Vec<u8> = &bincode::serialize(&(id, position))?;

            // Ensure the serialized data is exactly INDEX_RECORD_SIZE bytes
            if serialized.len() > INDEX_RECORD_SIZE {
                return Err(BinaryStoreError::IndexRecordTooLarge);
            }

            let mut record = vec![0; INDEX_RECORD_SIZE];
            record[..serialized.len()].copy_from_slice(&serialized);

            file.write_all(&record)?;
        }

        Ok(())
    }

    fn load_index<P: AsRef<Path>>(
        index_file: P,
    ) -> Result<HashMap<String, Position>, BinaryStoreError> {
        let file = OpenOptions::new().read(true).open(index_file)?;

        let mut result = HashMap::new();

        for record in BinaryIndexIterator::new(file, INDEX_RECORD_SIZE) {
            let index = record?;
            result.insert(index.id, index.position);
        }

        Ok(result)
    }

    fn update_index_entry(&mut self, id: &String, position: Position) {
        self.index.insert(id.to_string(), position);
        self.needs_index_rewrite = true;
    }

    fn get(&self, position: &Position) -> Result<Entry, BinaryStoreError> {
        let mut file = OpenOptions::new().read(true).open(&self.data_file_path)?;

        file.seek(SeekFrom::Start(position.offset))?;

        let mut buf = vec![0; position.length];
        file.read_exact(&mut buf)?;
        bincode::deserialize(&buf).map_err(|e| BinaryStoreError::from(e))
    }

    fn write_data(&mut self) -> Result<(), BinaryStoreError> {
        let temp_file = format!("temp_{}", self.data_file_path);

        let mut new_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp_file)?;

        let mut new_index: HashMap<String, Position> = HashMap::new();

        for (key, pos) in &self.index {
            let entry = self.get(pos)?;
            let new_pos = Self::write_entry(&entry, &mut new_file)?;
            new_index.insert(key.to_string(), new_pos);
        }

        self.index = new_index;

        remove_file(&self.data_file_path)?;
        rename(&temp_file, &self.data_file_path)?;

        self.needs_data_rewrite = false;

        Ok(())
    }

    fn write_entry<W: Write + Seek>(
        value: &Entry,
        file: &mut W,
    ) -> Result<Position, BinaryStoreError> {
        // Serialize data
        let serialized: &Vec<u8> = &bincode::serialize(value)?;

        // Position
        let offset = file.seek(SeekFrom::End(0))?;
        let length = serialized.len();
        let pos = Position { length, offset };

        // Write data
        file.write_all(&serialized)?;

        Ok(pos)
    }
}

impl DataStore<String, Entry, BinaryStoreError> for IndexedBinaryFileEntryStore {
    fn save(&mut self, id: &String, value: &Entry) -> Result<(), BinaryStoreError> {
        // Open file
        let mut file = OpenOptions::new()
            .write(true)
            .append(true)
            .open(&self.data_file_path)?;

        let pos = Self::write_entry(value, &mut file)?;

        // Update index (not index file)
        self.update_index_entry(id, pos);

        Ok(())
    }

    fn load(&self, key: &String) -> Result<Option<Entry>, BinaryStoreError> {
        match self.index.get(key) {
            Some(pos) => self.get(pos).map(Some),
            None => Ok(None),
        }
    }

    fn delete(&mut self, id: &String) -> Result<(), BinaryStoreError> {
        self.index.remove(id);
        self.needs_data_rewrite = true;

        Ok(())
    }

    fn search(
        &self,
        filter: &dyn super::data_store::Filter<Entry>,
    ) -> Result<Vec<Entry>, BinaryStoreError> {
        let mut file = OpenOptions::new().read(true).open(&self.data_file_path)?;

        // sort index entries
        let mut sorted_index_entries: Vec<_> = self.index.iter().collect();
        sorted_index_entries.sort_by_key(|(_, position)| position.offset);

        // result to return
        let mut result: Vec<Entry> = vec![];

        for (_, pos) in sorted_index_entries {
            // Seek to the correct offset
            file.seek(SeekFrom::Start(pos.offset))?;

            let mut buf = vec![0; pos.length];
            file.read_exact(&mut buf)?;
            let entry: Entry = bincode::deserialize(&buf)?;

            if filter.pass(&entry) {
                result.push(entry);
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use crate::data::data_store::Filter;

    use super::*;
    use std::fs::{self, File};
    use std::io::{self, Read, Write};
    use std::path::Path;

    // Helper function to create a temporary file and return its path
    fn create_temp_file(file_path: &str) -> io::Result<()> {
        let mut file = File::create(file_path)?;
        file.write_all(b"")?; // Create an empty file
        Ok(())
    }

    // Helper function to clean up temporary files after tests
    fn cleanup_temp_file(file_path: &str) {
        if Path::new(file_path).exists() {
            fs::remove_file(file_path).unwrap();
        }
    }

    #[test]
    fn test_save_new_entry() {
        // Create temporary files for data and index
        let data_file_path = "test_data1.bin";
        let index_file_path = "test_index1.bin";

        create_temp_file(data_file_path).unwrap();
        create_temp_file(index_file_path).unwrap();

        // Initialize the store
        let mut store = IndexedBinaryFileEntryStore::new(
            data_file_path.to_string(),
            index_file_path.to_string(),
        );

        // Create a test entry
        let entry = Entry {
            id: "test_id".to_string(),
            title: "Test Title".to_string(),
            username: Some("test_user".to_string()),
            password: Some("test_password".to_string()),
            url: Some("https://example.com".to_string()),
            note: Some("This is a test entry".to_string()),
        };

        // Save the entry
        let id = entry.id.clone();
        store.save(&id, &entry).unwrap();

        // Verify that the index was updated
        assert!(store.index.contains_key(&id));
        let position = store.index.get(&id).unwrap();
        assert_eq!(position.length, bincode::serialize(&entry).unwrap().len());

        // Verify that the data file contains the serialized entry
        let data_file_content = fs::read(&data_file_path).unwrap();
        let serialized_entry = bincode::serialize(&entry).unwrap();
        assert_eq!(data_file_content, serialized_entry);

        // Clean up temporary files
        cleanup_temp_file(&data_file_path);
        cleanup_temp_file(&index_file_path);
    }

    #[test]
    fn test_save_multiple_entries() {
        // Create temporary files for data and index
        let data_file_path = "test_data2.bin";
        let index_file_path = "test_index2.bin";

        create_temp_file(data_file_path).unwrap();
        create_temp_file(index_file_path).unwrap();

        // Initialize the store
        let mut store = IndexedBinaryFileEntryStore::new(
            data_file_path.to_string(),
            index_file_path.to_string(),
        );

        // Create and save multiple entries
        let entry1 = Entry {
            id: "id1".to_string(),
            title: "First Entry".to_string(),
            username: Some("user1".to_string()),
            password: Some("password1".to_string()),
            url: Some("https://example.com/1".to_string()),
            note: Some("First test entry".to_string()),
        };
        let id1 = entry1.id.clone();
        store.save(&id1, &entry1).unwrap();

        let entry2 = Entry {
            id: "id2".to_string(),
            title: "Second Entry".to_string(),
            username: Some("user2".to_string()),
            password: Some("password2".to_string()),
            url: Some("https://example.com/2".to_string()),
            note: Some("Second test entry".to_string()),
        };
        let id2 = entry2.id.clone();
        store.save(&id2, &entry2).unwrap();

        // Verify that the index contains both entries
        assert!(store.index.contains_key(&id1));
        assert!(store.index.contains_key(&id2));

        // Verify that the data file contains both serialized entries
        let data_file_content = fs::read(&data_file_path).unwrap();
        let serialized_entry1 = bincode::serialize(&entry1).unwrap();
        let serialized_entry2 = bincode::serialize(&entry2).unwrap();

        assert!(data_file_content.starts_with(&serialized_entry1));
        assert!(data_file_content.ends_with(&serialized_entry2));

        // Clean up temporary files
        cleanup_temp_file(&data_file_path);
        cleanup_temp_file(&index_file_path);
    }

    #[test]
    fn test_save_and_persist_index() {
        // Create temporary files for data and index
        let data_file_path = "test_data3.bin";
        let index_file_path = "test_index3.bin";

        create_temp_file(data_file_path).unwrap();
        create_temp_file(index_file_path).unwrap();

        // Initialize the store
        let mut store = IndexedBinaryFileEntryStore::new(
            data_file_path.to_string(),
            index_file_path.to_string(),
        );

        // Create and save an entry
        let entry = Entry {
            id: "test_id".to_string(),
            title: "Test Title".to_string(),
            username: Some("test_user".to_string()),
            password: Some("test_password".to_string()),
            url: Some("https://example.com".to_string()),
            note: Some("This is a test entry".to_string()),
        };
        let id = entry.id.clone();
        store.save(&id, &entry).unwrap();

        // Persist the index to the index file
        store.rewrite_index().unwrap();

        // Verify that the index file contains the updated index
        let index_content = fs::read(&index_file_path).unwrap();
        assert!(!index_content.is_empty()); // Ensure the index file is not empty

        // Clean up temporary files
        cleanup_temp_file(&data_file_path);
        cleanup_temp_file(&index_file_path);
    }

    #[test]
    fn test_save_existing_entry_updates_index() {
        // Create temporary files for data and index
        let data_file_path = "test_data4.bin";
        let index_file_path = "test_index4.bin";

        create_temp_file(data_file_path).unwrap();
        create_temp_file(index_file_path).unwrap();

        // Initialize the store
        let mut store = IndexedBinaryFileEntryStore::new(
            data_file_path.to_string(),
            index_file_path.to_string(),
        );

        // Create and save an initial entry
        let entry1 = Entry {
            id: "test_id".to_string(),
            title: "Initial Title".to_string(),
            username: Some("initial_user".to_string()),
            password: Some("initial_password".to_string()),
            url: Some("https://example.com/initial".to_string()),
            note: Some("Initial test entry".to_string()),
        };
        let id = entry1.id.clone();
        store.save(&id, &entry1).unwrap();

        // Save a new entry with the same ID (should overwrite the index entry)
        let entry2 = Entry {
            id: "test_id".to_string(),
            title: "Updated Title".to_string(),
            username: Some("updated_user".to_string()),
            password: Some("updated_password".to_string()),
            url: Some("https://example.com/updated".to_string()),
            note: Some("Updated test entry".to_string()),
        };
        store.save(&id, &entry2).unwrap();

        // Verify that the index was updated with the new position
        let position = store.index.get(&id).unwrap();
        assert_eq!(position.length, bincode::serialize(&entry2).unwrap().len());

        // Verify that the data file contains the new serialized entry
        let mut file = OpenOptions::new().read(true).open(data_file_path).unwrap();
        file.seek(SeekFrom::Start(position.offset)).unwrap();
        let mut data_file_content = vec![0; position.length as usize];
        file.read_exact(&mut data_file_content).unwrap();
        let serialized_entry2 = bincode::serialize(&entry2).unwrap();
        assert_eq!(data_file_content, serialized_entry2);

        // Clean up temporary files
        cleanup_temp_file(&data_file_path);
        cleanup_temp_file(&index_file_path);
    }

    // --- new tests

    #[test]
    fn test_load_non_existent_entry() {
        let data_file_path = "test_data5.bin";
        let index_file_path = "test_index5.bin";

        create_temp_file(data_file_path).unwrap();
        create_temp_file(index_file_path).unwrap();

        let mut store = IndexedBinaryFileEntryStore::new(
            data_file_path.to_string(),
            index_file_path.to_string(),
        );

        let non_existent_id = "non_existent_id".to_string();
        let result = store.load(&non_existent_id).unwrap();

        assert!(result.is_none());

        cleanup_temp_file(&data_file_path);
        cleanup_temp_file(&index_file_path);
    }

    #[test]
    fn test_delete_non_existent_entry() {
        let data_file_path = "test_data6.bin";
        let index_file_path = "test_index6.bin";

        create_temp_file(data_file_path).unwrap();
        create_temp_file(index_file_path).unwrap();

        let mut store = IndexedBinaryFileEntryStore::new(
            data_file_path.to_string(),
            index_file_path.to_string(),
        );

        let non_existent_id = "non_existent_id".to_string();
        store.delete(&non_existent_id).unwrap();

        assert!(!store.index.contains_key(&non_existent_id));

        cleanup_temp_file(&data_file_path);
        cleanup_temp_file(&index_file_path);
    }

    #[test]
    fn test_rewrite_data_file_after_deletion() {
        let data_file_path = "test_data7.bin";
        let index_file_path = "test_index7.bin";

        create_temp_file(data_file_path).unwrap();
        create_temp_file(index_file_path).unwrap();

        let mut store = IndexedBinaryFileEntryStore::new(
            data_file_path.to_string(),
            index_file_path.to_string(),
        );

        let entry = Entry {
            id: "test_id".to_string(),
            title: "Test Title".to_string(),
            username: Some("test_user".to_string()),
            password: Some("test_password".to_string()),
            url: Some("https://example.com".to_string()),
            note: Some("This is a test entry".to_string()),
        };
        let id = entry.id.clone();
        store.save(&id, &entry).unwrap();

        store.delete(&id).unwrap();
        store.write_data().unwrap();

        let data_file_content = fs::read(&data_file_path).unwrap();
        assert!(data_file_content.is_empty());

        cleanup_temp_file(&data_file_path);
        cleanup_temp_file(&index_file_path);
    }

    #[test]
    fn test_reload_index() {
        let data_file_path = "test_data8.bin";
        let index_file_path = "test_index8.bin";

        create_temp_file(data_file_path).unwrap();
        create_temp_file(index_file_path).unwrap();

        let mut store = IndexedBinaryFileEntryStore::new(
            data_file_path.to_string(),
            index_file_path.to_string(),
        );

        let entry = Entry {
            id: "test_id".to_string(),
            title: "Test Title".to_string(),
            username: Some("test_user".to_string()),
            password: Some("test_password".to_string()),
            url: Some("https://example.com".to_string()),
            note: Some("This is a test entry".to_string()),
        };
        let id = &entry.id;
        store.save(id, &entry).unwrap();

        store.rewrite_index().unwrap();
        store.reload_index();

        assert!(store.index.contains_key(id));

        cleanup_temp_file(&data_file_path);
        cleanup_temp_file(&index_file_path);
    }

    #[test]
    fn test_error_handling_for_file_operations() {
        let data_file_path = "test_data9.bin";
        let index_file_path = "test_index9.bin";

        create_temp_file(data_file_path).unwrap();
        create_temp_file(index_file_path).unwrap();

        let mut store = IndexedBinaryFileEntryStore::new(
            data_file_path.to_string(),
            index_file_path.to_string(),
        );

        // Simulate a file operation error by deleting the data file
        fs::remove_file(&data_file_path).unwrap();

        let entry = Entry {
            id: "test_id".to_string(),
            title: "Test Title".to_string(),
            username: Some("test_user".to_string()),
            password: Some("test_password".to_string()),
            url: Some("https://example.com".to_string()),
            note: Some("This is a test entry".to_string()),
        };
        let id = entry.id.clone();

        let result = store.save(&id, &entry);
        assert!(result.is_err());

        cleanup_temp_file(&data_file_path);
        cleanup_temp_file(&index_file_path);
    }

    #[test]
    fn test_save_entry_with_existing_id() {
        let data_file_path = "test_data10.bin";
        let index_file_path = "test_index10.bin";

        create_temp_file(data_file_path).unwrap();
        create_temp_file(index_file_path).unwrap();

        let mut store = IndexedBinaryFileEntryStore::new(
            data_file_path.to_string(),
            index_file_path.to_string(),
        );

        let entry1 = Entry {
            id: "test_id".to_string(),
            title: "Initial Title".to_string(),
            username: Some("initial_user".to_string()),
            password: Some("initial_password".to_string()),
            url: Some("https://example.com/initial".to_string()),
            note: Some("Initial test entry".to_string()),
        };
        let id = entry1.id.clone();
        store.save(&id, &entry1).unwrap();

        let entry2 = Entry {
            id: "test_id".to_string(),
            title: "Updated Title".to_string(),
            username: Some("updated_user".to_string()),
            password: Some("updated_password".to_string()),
            url: Some("https://example.com/updated".to_string()),
            note: Some("Updated test entry".to_string()),
        };
        store.save(&id, &entry2).unwrap();

        let loaded_entry = store.load(&id).unwrap().unwrap();
        assert_eq!(loaded_entry.title, "Updated Title");

        cleanup_temp_file(&data_file_path);
        cleanup_temp_file(&index_file_path);
    }

    // Search tests

    // Define a filter that matches all entries
    struct MatchAllFilter;
    impl Filter<Entry> for MatchAllFilter {
        fn pass(&self, _: &Entry) -> bool {
            true
        }
    }

    // Define a filter that matches no entries
    struct MatchNoneFilter;
    impl Filter<Entry> for MatchNoneFilter {
        fn pass(&self, _: &Entry) -> bool {
            false
        }
    }

    // Define a filter that matches entries with a specific title
    struct TitleFilter {
        title: String,
    }
    impl Filter<Entry> for TitleFilter {
        fn pass(&self, entry: &Entry) -> bool {
            entry.title == self.title
        }
    }

    #[test]
    fn test_search_match_all() {
        let data_file_path = "test_search_match_all_data.bin";
        let index_file_path = "test_search_match_all_index.bin";

        create_temp_file(data_file_path).unwrap();
        create_temp_file(index_file_path).unwrap();

        let mut store = IndexedBinaryFileEntryStore::new(
            data_file_path.to_string(),
            index_file_path.to_string(),
        );

        // Add some entries
        let entry1 = Entry {
            id: "id1".to_string(),
            title: "First Entry".to_string(),
            username: Some("user1".to_string()),
            password: Some("password1".to_string()),
            url: Some("https://example.com/1".to_string()),
            note: Some("First test entry".to_string()),
        };
        let entry2 = Entry {
            id: "id2".to_string(),
            title: "Second Entry".to_string(),
            username: Some("user2".to_string()),
            password: Some("password2".to_string()),
            url: Some("https://example.com/2".to_string()),
            note: Some("Second test entry".to_string()),
        };

        store.save(&entry1.id, &entry1).unwrap();
        store.save(&entry2.id, &entry2).unwrap();

        // Search for entries with a filter that matches all
        let filter = MatchAllFilter;
        let results = store.search(&filter).unwrap();

        // Verify the results
        assert_eq!(results.len(), 2);
        assert!(results.contains(&entry1));
        assert!(results.contains(&entry2));

        cleanup_temp_file(&data_file_path);
        cleanup_temp_file(&index_file_path);
    }

    #[test]
    fn test_search_match_none() {
        let data_file_path = "test_search_match_none_data.bin";
        let index_file_path = "test_search_match_none_index.bin";

        create_temp_file(data_file_path).unwrap();
        create_temp_file(index_file_path).unwrap();

        let mut store = IndexedBinaryFileEntryStore::new(
            data_file_path.to_string(),
            index_file_path.to_string(),
        );

        // Add some entries
        let entry1 = Entry {
            id: "id1".to_string(),
            title: "First Entry".to_string(),
            username: Some("user1".to_string()),
            password: Some("password1".to_string()),
            url: Some("https://example.com/1".to_string()),
            note: Some("First test entry".to_string()),
        };
        let entry2 = Entry {
            id: "id2".to_string(),
            title: "Second Entry".to_string(),
            username: Some("user2".to_string()),
            password: Some("password2".to_string()),
            url: Some("https://example.com/2".to_string()),
            note: Some("Second test entry".to_string()),
        };

        store.save(&entry1.id, &entry1).unwrap();
        store.save(&entry2.id, &entry2).unwrap();

        // Search for entries with a filter that matches none
        let filter = MatchNoneFilter;
        let results = store.search(&filter).unwrap();

        // Verify the results
        assert!(results.is_empty());

        cleanup_temp_file(&data_file_path);
        cleanup_temp_file(&index_file_path);
    }

    #[test]
    fn test_search_match_specific_title() {
        let data_file_path = "test_search_match_specific_title_data.bin";
        let index_file_path = "test_search_match_specific_title_index.bin";

        create_temp_file(data_file_path).unwrap();
        create_temp_file(index_file_path).unwrap();

        let mut store = IndexedBinaryFileEntryStore::new(
            data_file_path.to_string(),
            index_file_path.to_string(),
        );

        // Add some entries
        let entry1 = Entry {
            id: "id1".to_string(),
            title: "First Entry".to_string(),
            username: Some("user1".to_string()),
            password: Some("password1".to_string()),
            url: Some("https://example.com/1".to_string()),
            note: Some("First test entry".to_string()),
        };
        let entry2 = Entry {
            id: "id2".to_string(),
            title: "Second Entry".to_string(),
            username: Some("user2".to_string()),
            password: Some("password2".to_string()),
            url: Some("https://example.com/2".to_string()),
            note: Some("Second test entry".to_string()),
        };

        store.save(&entry1.id, &entry1).unwrap();
        store.save(&entry2.id, &entry2).unwrap();

        // Search for entries with a specific title
        let filter = TitleFilter {
            title: "First Entry".to_string(),
        };
        let results = store.search(&filter).unwrap();

        // Verify the results
        assert_eq!(results.len(), 1);
        assert!(results.contains(&entry1));

        cleanup_temp_file(&data_file_path);
        cleanup_temp_file(&index_file_path);
    }

    #[test]
    fn test_search_empty_store() {
        let data_file_path = "test_search_empty_store_data.bin";
        let index_file_path = "test_search_empty_store_index.bin";

        create_temp_file(data_file_path).unwrap();
        create_temp_file(index_file_path).unwrap();

        let store = IndexedBinaryFileEntryStore::new(
            data_file_path.to_string(),
            index_file_path.to_string(),
        );

        // Search for entries in an empty store
        let filter = MatchAllFilter;
        let results = store.search(&filter).unwrap();

        // Verify the results
        assert!(results.is_empty());

        cleanup_temp_file(&data_file_path);
        cleanup_temp_file(&index_file_path);
    }

    #[test]
    fn test_needs_index_rewrite_after_save() {
        let data_file_path = "test_needs_index_rewrite_after_save_data.bin";
        let index_file_path = "test_needs_index_rewrite_after_save_index.bin";

        create_temp_file(data_file_path).unwrap();
        create_temp_file(index_file_path).unwrap();

        let mut store = IndexedBinaryFileEntryStore::new(
            data_file_path.to_string(),
            index_file_path.to_string(),
        );

        let entry = Entry {
            id: "test_id".to_string(),
            title: "Test Title".to_string(),
            username: Some("test_user".to_string()),
            password: Some("test_password".to_string()),
            url: Some("https://example.com".to_string()),
            note: Some("This is a test entry".to_string()),
        };

        // Save the entry
        store.save(&entry.id, &entry).unwrap();

        // Verify that the index rewrite flag is set
        assert!(store.needs_index_rewrite());

        cleanup_temp_file(&data_file_path);
        cleanup_temp_file(&index_file_path);
    }

    #[test]
    fn test_needs_index_rewrite_cleared_after_rewrite() {
        let data_file_path = "test_needs_index_rewrite_cleared_after_rewrite_data.bin";
        let index_file_path = "test_needs_index_rewrite_cleared_after_rewrite_index.bin";

        create_temp_file(data_file_path).unwrap();
        create_temp_file(index_file_path).unwrap();

        let mut store = IndexedBinaryFileEntryStore::new(
            data_file_path.to_string(),
            index_file_path.to_string(),
        );

        let entry = Entry {
            id: "test_id".to_string(),
            title: "Test Title".to_string(),
            username: Some("test_user".to_string()),
            password: Some("test_password".to_string()),
            url: Some("https://example.com".to_string()),
            note: Some("This is a test entry".to_string()),
        };

        // Save the entry (sets needs_index_rewrite to true)
        store.save(&entry.id, &entry).unwrap();

        // Rewrite the index (clears needs_index_rewrite)
        store.rewrite_index().unwrap();

        // Verify that the index rewrite flag is cleared
        assert!(!store.needs_index_rewrite());

        cleanup_temp_file(&data_file_path);
        cleanup_temp_file(&index_file_path);
    }

    #[test]
    fn test_needs_data_rewrite_after_delete() {
        let data_file_path = "test_needs_data_rewrite_after_delete_data.bin";
        let index_file_path = "test_needs_data_rewrite_after_delete_index.bin";

        create_temp_file(data_file_path).unwrap();
        create_temp_file(index_file_path).unwrap();

        let mut store = IndexedBinaryFileEntryStore::new(
            data_file_path.to_string(),
            index_file_path.to_string(),
        );

        let entry = Entry {
            id: "test_id".to_string(),
            title: "Test Title".to_string(),
            username: Some("test_user".to_string()),
            password: Some("test_password".to_string()),
            url: Some("https://example.com".to_string()),
            note: Some("This is a test entry".to_string()),
        };

        // Save the entry
        store.save(&entry.id, &entry).unwrap();

        // Delete the entry
        store.delete(&entry.id).unwrap();

        // Verify that the data rewrite flag is set
        assert!(store.needs_data_rewrite());

        cleanup_temp_file(&data_file_path);
        cleanup_temp_file(&index_file_path);
    }

    #[test]
    fn test_needs_data_rewrite_cleared_after_rewrite() {
        let data_file_path = "test_needs_data_rewrite_cleared_after_rewrite_data.bin";
        let index_file_path = "test_needs_data_rewrite_cleared_after_rewrite_index.bin";

        create_temp_file(data_file_path).unwrap();
        create_temp_file(index_file_path).unwrap();

        let mut store = IndexedBinaryFileEntryStore::new(
            data_file_path.to_string(),
            index_file_path.to_string(),
        );

        let entry = Entry {
            id: "test_id".to_string(),
            title: "Test Title".to_string(),
            username: Some("test_user".to_string()),
            password: Some("test_password".to_string()),
            url: Some("https://example.com".to_string()),
            note: Some("This is a test entry".to_string()),
        };

        // Save the entry
        store.save(&entry.id, &entry).unwrap();

        // Delete the entry (sets needs_data_rewrite to true)
        store.delete(&entry.id).unwrap();

        // Rewrite the data file (clears needs_data_rewrite)
        store.write_data().unwrap();

        // Verify that the data rewrite flag is cleared
        assert!(!store.needs_data_rewrite());

        cleanup_temp_file(&data_file_path);
        cleanup_temp_file(&index_file_path);
    }

    #[test]
    fn test_needs_index_rewrite_unchanged_without_modifications() {
        let data_file_path = "test_needs_index_rewrite_unchanged_data.bin";
        let index_file_path = "test_needs_index_rewrite_unchanged_index.bin";

        create_temp_file(data_file_path).unwrap();
        create_temp_file(index_file_path).unwrap();

        let mut store = IndexedBinaryFileEntryStore::new(
            data_file_path.to_string(),
            index_file_path.to_string(),
        );

        // Verify that the index rewrite flag is initially false
        assert!(!store.needs_index_rewrite());

        cleanup_temp_file(&data_file_path);
        cleanup_temp_file(&index_file_path);
    }

    #[test]
    fn test_needs_data_rewrite_unchanged_without_deletions() {
        let data_file_path = "test_needs_data_rewrite_unchanged_data.bin";
        let index_file_path = "test_needs_data_rewrite_unchanged_index.bin";

        create_temp_file(data_file_path).unwrap();
        create_temp_file(index_file_path).unwrap();

        let mut store = IndexedBinaryFileEntryStore::new(
            data_file_path.to_string(),
            index_file_path.to_string(),
        );

        let entry = Entry {
            id: "test_id".to_string(),
            title: "Test Title".to_string(),
            username: Some("test_user".to_string()),
            password: Some("test_password".to_string()),
            url: Some("https://example.com".to_string()),
            note: Some("This is a test entry".to_string()),
        };

        // Save the entry
        store.save(&entry.id, &entry).unwrap();

        // Verify that the data rewrite flag is still false (no deletions)
        assert!(!store.needs_data_rewrite());

        cleanup_temp_file(&data_file_path);
        cleanup_temp_file(&index_file_path);
    }
}
