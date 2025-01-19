mod data;

use data::database;
use sqlite::Connection;
fn main() {
    env_logger::init();
    match database::create_database("db_file") {
        Ok(con) => database::create_tables(&con).unwrap(),
        Err(e) => panic!("Error {}", e),
    }
}
