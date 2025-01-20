mod data;

use data::database::{self, database_exists};
use sqlite::Connection;
fn main() {
    env_logger::init();
    if !database_exists("db_file") {
        match database::create_database("db_file") {
            Ok(con) => database::create_tables(&con).unwrap(),
            Err(e) => panic!("Error {}", e),
        }
    }
}
