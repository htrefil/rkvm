use rkvm_input::Key;
use serde::Deserialize;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub listen_address: SocketAddr,
    pub switch_keys: HashSet<Key>,
    pub identity_path: PathBuf,
    #[serde(default)]
    pub identity_password: String,
}
