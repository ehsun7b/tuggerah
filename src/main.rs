mod data;
mod secret;

use data::{binary_file_entry_store::BinaryFileEntryStore, data_store::DataStore, model::Entry};
fn main() {
    let e = Entry {
        id: "1".to_string(),
        title: "title".to_string(),
        username: Some("username".to_string()),
        password: None,
        url: None,
        note: None,
    };

    let file = "db.txt".to_string();

    let store = BinaryFileEntryStore::new(file);

    //let _ = store.save(&e.id, &e);
}
