use std::collections::HashSet;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub struct Entry {
    pub device: u64,
    pub inode: u64,
}

#[derive(Clone)]
pub struct Registry {
    entries: Arc<Mutex<HashSet<Entry>>>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn register(&self, entry: Entry) -> Option<Handle> {
        if !self.entries.lock().unwrap().insert(entry) {
            return None;
        }

        Some(Handle {
            entries: self.entries.clone(),
            entry,
        })
    }
}

pub struct Handle {
    entries: Arc<Mutex<HashSet<Entry>>>,
    entry: Entry,
}

impl Drop for Handle {
    fn drop(&mut self) {
        assert!(self.entries.lock().unwrap().remove(&self.entry));
    }
}
