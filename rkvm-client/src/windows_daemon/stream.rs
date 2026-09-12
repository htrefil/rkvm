use crate::client::{Error, RkvmWriter};
use rkvm_net::Update;

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard};

pub struct LockWriter<W> {
    w: Arc<Mutex<W>>
}

impl<W> LockWriter<W>  {
    pub fn new(w: W) -> Self {
        LockWriter { w: Arc::new(Mutex::new(w)) }
    }

    pub async fn lock(self: &Self) -> MutexGuard<'_, W> {
        self.w.lock().await
    }
}

#[async_trait]
impl<W> RkvmWriter for LockWriter<W>
where
    W: RkvmWriter + Send {
    async fn send(self: &mut Self, update: Update) -> Result<(), Error> {
        self.w.lock().await.send(update).await
    }
}

impl<W> Clone for LockWriter<W>
where
    W: RkvmWriter + Send {
    fn clone(self: &Self) -> Self {
        LockWriter { w: self.w.clone() }
    }
}
