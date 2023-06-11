use rkvm_input::key::Keyboard;
use serde::Deserialize;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub listen: SocketAddr,
    pub certificate: PathBuf,
    pub key: PathBuf,
    pub password: String,
    pub switch_keys: HashSet<Keyboard>,
}
