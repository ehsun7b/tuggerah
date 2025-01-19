mod persist;

use persist::database;
use sqlite::Connection;
fn main() {
    env_logger::init();
    let con = Connection::open("..");

    match con {
        Ok(_) => print!("connection succeed"),
        Err(e) => print!("failed"),
    }
}
