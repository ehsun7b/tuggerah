use std::fmt;

use log::{debug, warn};
use sqlite::{Connection, OpenFlags};

#[derive(Debug)]
pub enum DatabaseError {
    ConnectionError(sqlite::Error),
    InitializationError(sqlite::Error),
    Unknown(String),
}

// Implement Display for user-friendly error messages
impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DatabaseError::ConnectionError(err) => write!(f, "Failed to open database: {}", err),
            DatabaseError::InitializationError(err) => {
                write!(f, "Failed to initialize database: {}", err)
            }
            DatabaseError::Unknown(msg) => write!(f, "An unknown database error occurred, {}", msg),
        }
    }
}

// Implement the Error trait for compatibility with `?` operator
impl std::error::Error for DatabaseError {}

// Implement From to convert sqlite::Error into DatabaseError
impl From<sqlite::Error> for DatabaseError {
    fn from(err: sqlite::Error) -> Self {
        DatabaseError::ConnectionError(err)
    }
}

pub fn database_exists(db_file: &str) -> bool {
    debug!("Checking if database exists: {}", db_file);
    match Connection::open_with_flags(db_file, OpenFlags::new().with_read_write()) {
        Ok(_) => {
            debug!("Successfully connected to database: {}", db_file);
            true
        }
        Err(e) => {
            warn!("Failed to connect to database {}: {}", db_file, e);
            false
        }
    }
}

pub fn create_database(db_file: &str) -> Result<Connection, DatabaseError> {
    debug!("Creating database: {}", db_file);
    match Connection::open(db_file) {
        Ok(conn) => Ok(conn),
        Err(e) => Err(DatabaseError::ConnectionError(e)),
    }
}

pub fn create_tables(con: &Connection) -> Result<(), DatabaseError> {
    let result = con.execute(
        "
                CREATE TABLE entry (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    username TEXT,
                    password TEXT,
                    url TEXT,
                    note TEXT
                );
                -- Indexes for frequently queried columns
                CREATE INDEX idx_username ON entry (username);
                CREATE INDEX idx_url ON entry (url);
                CREATE INDEX idx_title ON entry (title);
                ",
    );

    match result {
        Ok(()) => Ok(()),
        Err(e) => Err(DatabaseError::InitializationError(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    #[test]
    fn test_database_exists() {
        // Create a temporary database file
        let db_file = "temp_test.db";
        {
            // Open a connection to create the database file
            let _conn = Connection::open(db_file).unwrap();
        }

        // True case: Check if the database file exists
        assert!(
            database_exists(db_file),
            "Database should exist but it doesn't!"
        );

        // False case: Check for a non-existent file
        let non_existent_db = "non_existent_test.db";
        assert!(
            !database_exists(non_existent_db),
            "Database should not exist but it does!"
        );

        // Clean up: Delete the temporary database file
        if Path::new(db_file).exists() {
            fs::remove_file(db_file).unwrap();
        }
    }

    #[test]
    fn test_create_database_success() {
        let db_file = "test_db_success.sqlite";

        // Ensure the file does not already exist
        if Path::new(db_file).exists() {
            fs::remove_file(db_file).unwrap();
        }

        // Test creating the database
        let result = create_database(db_file);
        assert!(
            result.is_ok(),
            "Expected Ok(Connection), got {}",
            result.err().unwrap().to_string()
        );

        // Ensure the database file was created
        assert!(Path::new(db_file).exists(), "Database file was not created");

        // Clean up
        fs::remove_file(db_file).unwrap();
    }

    #[test]
    fn test_create_database_failure() {
        // Simulate a failure by providing an invalid file path
        let invalid_path = ".."; // .. is invalid
        let result = create_database(invalid_path);

        // Match against the specific error type if desired
        if let Err(DatabaseError::ConnectionError(_)) = result {
            // Test passed
        } else {
            panic!("Expected DatabaseError::ConnectionError");
        }
    }
}
