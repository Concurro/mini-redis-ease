use mini_redis_ease::db::*;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use mini_redis::{Command, Connection, Frame};
use tokio::net::{TcpListener, TcpStream};
// -----------------

// -----------------

#[tokio::main]
async fn main() {
    let tcp_listener = TcpListener::bind("127.0.0.1:8081").await.unwrap();
    let db = new_sharded_db(10);

    loop {
        let (socket, _) = tcp_listener.accept().await.unwrap();
        let db = Arc::clone(&db);

        tokio::spawn(async move {
            process(socket, db).await;
        });
    }
}
async fn process(socket: TcpStream, db: ShardedDb) {
    let mut connection = Connection::new(socket);
    while let Some(frame) = connection.read_frame().await.unwrap() {
        let response = match Command::from_frame(frame).unwrap() {
            Command::Set(cmd) => {
                let mut db = db[hash(cmd.key()) % db.len()].lock().unwrap();
                db.insert(cmd.key().to_string(), cmd.value().clone());
                Frame::Simple("OK".to_string())
            }
            Command::Get(cmd) => {
                let db = db[hash(cmd.key()) % db.len()].lock().unwrap();
                if let Some(value) = db.get(cmd.key()) {
                    Frame::Bulk(value.clone().into())
                } else {
                    Frame::Null
                }
            }

            cmd => panic!("unimplemented: {:?}", cmd),
        };
        connection.write_frame(&response).await.unwrap();
    }
}
fn hash(key: &str) -> usize {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish() as usize
}
