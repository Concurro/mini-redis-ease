use bytes::Bytes;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub type ShardedDb = Arc<Vec<Mutex<HashMap<String, Bytes>>>>;

pub fn new_sharded_db(num_shards: usize) -> ShardedDb {
    let mut db = Vec::with_capacity(num_shards);
    for _ in 0..num_shards {
        db.push(Mutex::new(HashMap::new()));
    }
    Arc::new(db)
}
