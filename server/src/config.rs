use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::SocketAddr;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub listen_address: SocketAddr,
    pub switch_keys: HashSet<u16>,
}
