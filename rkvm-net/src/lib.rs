// This is not really public API.
#![allow(async_fn_in_trait)]

pub mod auth;
pub mod message;
pub mod version;

use rkvm_input::abs::{AbsAxis, AbsInfo};
use rkvm_input::event::Event;
use rkvm_input::key::Key;
use rkvm_input::rel::RelAxis;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::future::Future;
use std::io::{Error, ErrorKind};
use std::time::Duration;
use tokio::time;

pub const PING_INTERVAL: Duration = Duration::from_secs(1);

// Message read timeout (does not apply to updates, only auth negotiation and replies).
pub const READ_TIMEOUT: Duration = Duration::from_millis(500);

// Message write timeout (applies to all messages).
pub const WRITE_TIMEOUT: Duration = Duration::from_millis(500);

// TLS negotiation timeout.
pub const TLS_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Deserialize, Serialize, Debug)]
pub enum Update {
    CreateDevice {
        id: usize,
        name: CString,
        vendor: u16,
        product: u16,
        version: u16,
        rel: HashSet<RelAxis>,
        abs: HashMap<AbsAxis, AbsInfo>,
        keys: HashSet<Key>,
        delay: Option<i32>,
        period: Option<i32>,
    },
    DestroyDevice {
        id: usize,
    },
    Event {
        id: usize,
        event: Event,
    },
    Ping,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Pong;

pub async fn timeout<T: Future<Output = Result<U, Error>>, U>(
    duration: Duration,
    future: T,
) -> Result<U, Error> {
    time::timeout(duration, future)
        .await
        .map_err(|_| Error::new(ErrorKind::TimedOut, "Message timeout"))?
}

#[cfg(test)]
mod test {
    use super::message::Message;
    use super::*;

    #[tokio::test]
    async fn pong_is_not_empty() {
        let mut data = Vec::new();
        Pong.encode(&mut data).await.unwrap();

        assert!(!data.is_empty());
    }
}
