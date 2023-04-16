use rkvm_input::Key;
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub listen: SocketAddr,
    pub switch_key: Key,
    pub certificate: PathBuf,
    pub key: PathBuf,
}
